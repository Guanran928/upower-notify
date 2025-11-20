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

#[derive(Debug, OwnedValue)]
#[repr(u32)]
pub enum State {
    Unknown = 0,
    Charging = 1,
    Discharging = 2,
    Empty = 3,
    FullyCharged = 4,
    PendingCharge = 5,
    PendingDischarge = 6,
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
    #[zbus(property)]
    fn state(&self) -> zbus::Result<State>;
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
    let mut warning_stream = upower.receive_warning_level_changed().await;
    let mut state_stream = upower.receive_state_changed().await;
    let mut warning_notification: Option<NotificationHandle> = None;
    let mut state_notification: Option<NotificationHandle> = None;

    let parse_timeout = |t: u32| match t {
        0 => Timeout::Never,
        ms => Timeout::Milliseconds(ms),
    };

    loop {
        let (active_handle, selected_config) = tokio::select! {
            Some(msg) = warning_stream.next() => {
                let event = msg.get().await?;
                info!("Received event: WarningLevel::{:?}", event);
                let cfg = match event {
                    WarningLevel::Unknown => &config.warning_level.unknown,
                    WarningLevel::None => &config.warning_level.none,
                    WarningLevel::Discharging => &config.warning_level.discharging,
                    WarningLevel::Low => &config.warning_level.low,
                    WarningLevel::Critical => &config.warning_level.critical,
                    WarningLevel::Action => &config.warning_level.action,
                };
                (&mut warning_notification, cfg)
            }

            Some(msg) = state_stream.next() => {
                let event = msg.get().await?;
                info!("Received event: State::{:?}", event);
                let cfg = match event {
                    State::Unknown => &config.state.unknown,
                    State::Charging => &config.state.charging,
                    State::Discharging =>&config.state.discharging,
                    State::Empty => &config.state.empty,
                    State::FullyCharged => &config.state.fully_charged,
                    State::PendingCharge => &config.state.pending_charge,
                    State::PendingDischarge => &config.state.pending_discharge,
                };
                (&mut state_notification, cfg)
            }

            _ = tokio::signal::ctrl_c() => {
                info!("Exiting...");
                break;
            }
        };

        for cmd in &selected_config.exec.commands {
            info!("Executing: {cmd}");
            match Command::new("sh").arg("-c").arg(cmd).spawn() {
                Ok(_) => {}
                Err(e) => error!("Failed to spawn command '{cmd}': {e}"),
            };
        }

        if let Some(handle) = active_handle.take() {
            handle.close();
        };

        let n_cfg = &selected_config.notification;
        if n_cfg.enable {
            info!("Sending notification: {:#?}", n_cfg);

            *active_handle = Some(
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
