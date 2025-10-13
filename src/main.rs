use {
    anyhow::Result,
    clap::Parser,
    env_logger::Env,
    futures::stream::StreamExt,
    log::info,
    notify_rust::{Notification, Timeout, Urgency},
    std::time::Duration,
    zbus::{Connection, proxy, zvariant::OwnedValue},
};

/// Simple program to send notifications on battery status changes
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// D-Bus address of the battery device
    #[arg(
        short,
        long,
        default_value = "/org/freedesktop/UPower/devices/battery_BAT0"
    )]
    device: String,
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
    info!("Using device {}", args.device);

    let connection = Connection::system().await?;
    let upower = DeviceProxy::new(&connection, args.device).await?;
    let mut stream = upower.receive_warning_level_changed().await;

    while let Some(event) = stream.next().await {
        let event = event.get().await?;
        info!("WarningLevel changed: {event:?}");

        match event {
            WarningLevel::Low => {
                let time_to_empty = Duration::new(upower.time_to_empty().await? as u64, 0);
                let percentage = upower.percentage().await?;

                Notification::new()
                    .summary("Battery low")
                    .body(&format!(
                        "Approximately <b>{}</b> remaining ({}%)",
                        format_duration(time_to_empty),
                        percentage
                    ))
                    .timeout(Timeout::Milliseconds(30 * 1000))
                    .urgency(Urgency::Normal)
                    .show()?;
            }
            WarningLevel::Critical => {
                Notification::new()
                    .summary("Battery critically low")
                    .body("Shutting down soon unless plugged in.")
                    .timeout(Timeout::Never)
                    .urgency(Urgency::Critical)
                    .show()?;
            }
            WarningLevel::Action => {
                Notification::new()
                    .summary("Battery critically low")
                    .body("The battery is below the critical level and this computer is about to shutdown.")
                    .timeout(Timeout::Never)
                    .urgency(Urgency::Critical)
                    .show()?;
            }
            _ => {}
        }
    }

    Ok(())
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
