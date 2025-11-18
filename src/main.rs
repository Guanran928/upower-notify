pub mod config;

use {
    crate::config::Config,
    anyhow::{Context, Result},
    clap::Parser,
    env_logger::Env,
    figment::{
        Figment,
        providers::{Format, Serialized, Toml},
    },
    futures::stream::StreamExt,
    log::{debug, error, info},
    notify_rust::{Notification, NotificationHandle, Timeout},
    std::{path::PathBuf, process::Command, time::Duration},
    zbus::{Connection, proxy, zvariant::OwnedValue},
};

/// Simple program to send notifications on battery status changes
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the configuration file
    #[arg(short, long)]
    config: Option<String>,
}

#[derive(Debug, OwnedValue)]
#[repr(u32)]
pub enum WarningLevel {
    Unknown = 0,
    None = 1,
    Discharging = 2,
    Low = 3,
    Critical = 4,
    Action = 5,
}

#[proxy(
    interface = "org.freedesktop.UPower.Device",
    default_service = "org.freedesktop.UPower",
    assume_defaults = false
)]
pub trait Device {
    #[zbus(property)]
    fn percentage(&self) -> zbus::Result<f64>;
    #[zbus(property)]
    fn time_to_empty(&self) -> zbus::Result<i64>;
    #[zbus(property)]
    fn warning_level(&self) -> zbus::Result<WarningLevel>;
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    let args = Args::parse();
    let config_path = if let Some(cfg) = args.config {
        PathBuf::from(cfg)
    } else {
        let xdg_dirs = xdg::BaseDirectories::with_prefix("upower-notify");
        xdg_dirs
            .get_config_file("config.toml")
            .context("failed to load XDG base directories")?
    };

    debug!("Looking for config at: {:?}", config_path);
    let config: Config = Figment::from(Serialized::defaults(Config::default()))
        .merge(Toml::file(config_path))
        .extract()?;

    debug!("Config loaded: {config:#?}");
    info!("Using device {}", config.device);

    let connection = Connection::system().await?;
    let upower = DeviceProxy::new(&connection, config.device).await?;
    let mut stream = upower.receive_warning_level_changed().await;
    let mut active_notification: Option<NotificationHandle> = None;

    let parse_timeout = |t: u32| match t {
        0 => Timeout::Never,
        ms => Timeout::Milliseconds(ms),
    };

    while let Some(event) = stream.next().await {
        let event = event.get().await?;
        info!("WarningLevel changed: {event:?}");

        if let Some(handle) = active_notification.take() {
            handle.close();
        }

        let selected_config = match event {
            WarningLevel::None => &config.on_battery_none,
            WarningLevel::Low => &config.on_battery_low,
            WarningLevel::Critical => &config.on_battery_critical,
            WarningLevel::Action => &config.on_battery_action,
            _ => continue,
        };

        let e_cfg = &selected_config.exec;
        for cmd in &e_cfg.commands {
            info!("Executing: {cmd}");
            match Command::new("sh").arg("-c").arg(cmd).spawn() {
                Ok(_) => {}
                Err(e) => error!("Failed to spawn command '{cmd}': {e}"),
            }
        }

        let n_cfg = &selected_config.notification;
        if n_cfg.enable {
            active_notification = Some(
                Notification::new()
                    .summary(&n_cfg.summary)
                    .body(&generate_body(&upower, &n_cfg.body).await?)
                    .icon(&n_cfg.icon)
                    .timeout(parse_timeout(n_cfg.timeout))
                    .urgency((&n_cfg.urgency).into())
                    .show_async()
                    .await?,
            );
        };
    }

    Ok(())
}

async fn generate_body(device: &DeviceProxy<'_>, template: &str) -> Result<String> {
    let time_val = device.time_to_empty().await?;
    let percentage = device.percentage().await?;

    let time = Duration::from_secs(time_val as u64);
    Ok(template
        .replace("{time}", &format_duration(time))
        .replace("{percentage}", &percentage.to_string()))
}

fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;

    let mut parts = Vec::new();

    if hours > 0 {
        let hour_str = if hours == 1 { "hour" } else { "hours" };
        parts.push(format!("{hours} {hour_str}"));
    }

    if minutes > 0 {
        let minute_str = if minutes == 1 { "minute" } else { "minutes" };
        parts.push(format!("{minutes} {minute_str}"));
    }

    if parts.is_empty() {
        parts.push("0 minutes".to_string());
    }

    parts.join(", ")
}
