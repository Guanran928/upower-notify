use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub struct Config {
    pub device: String,
    pub on_battery_none: LevelConfig,
    pub on_battery_low: LevelConfig,
    pub on_battery_critical: LevelConfig,
    pub on_battery_action: LevelConfig,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct LevelConfig {
    pub notification: NotificationConfig,
    pub exec: ExecConfig,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct NotificationConfig {
    pub enable: bool,
    pub summary: String,
    pub body: String,
    pub icon: String,
    pub timeout: u32,
    pub urgency: UrgencyConfig,
}

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct ExecConfig {
    pub commands: Vec<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum UrgencyConfig {
    Low,
    Normal,
    Critical,
}

impl From<&UrgencyConfig> for notify_rust::Urgency {
    fn from(val: &UrgencyConfig) -> Self {
        match val {
            UrgencyConfig::Low => Self::Low,
            UrgencyConfig::Normal => Self::Normal,
            UrgencyConfig::Critical => Self::Critical,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            device: "/org/freedesktop/UPower/devices/battery_BAT0".to_string(),
            on_battery_none: LevelConfig {
                exec: ExecConfig::default(),
                notification: NotificationConfig {
                    enable: false,
                    summary: "Battery Discharging".into(),
                    body: "Power levels normal. <b>{time}</b> remaining ({percentage}%)".into(),
                    icon: "battery-good-symbolic".into(),
                    timeout: 5000,
                    urgency: UrgencyConfig::Low,
                },
            },
            on_battery_low: LevelConfig {
                exec: ExecConfig::default(),
                notification: NotificationConfig {
                    enable: true,
                    summary: "Battery low".into(),
                    body: "Approximately <b>{time}</b> remaining ({percentage}%)".into(),
                    icon: "battery-low-symbolic".into(),
                    timeout: 30000,
                    urgency: UrgencyConfig::Normal,
                },
            },
            on_battery_critical: LevelConfig {
                exec: ExecConfig::default(),
                notification: NotificationConfig {
                    enable: true,
                    summary: "Battery critically low".into(),
                    body: "Shutting down soon unless plugged in.".into(),
                    icon: "battery-caution-symbolic".into(),
                    timeout: 0,
                    urgency: UrgencyConfig::Critical,
                },
            },
            on_battery_action: LevelConfig {
                exec: ExecConfig::default(),
                notification: NotificationConfig {
                    enable: true,
                    summary: "Battery critically low".into(),
                    body: "The battery is below the critical level and this computer is about to shutdown.".into(),
                    icon: "battery-action-symbolic".into(),
                    timeout: 0,
                    urgency: UrgencyConfig::Critical,
                },
            },
        }
    }
}
