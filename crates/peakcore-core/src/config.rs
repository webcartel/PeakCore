//! User configuration stored in `%APPDATA%\PeakCore\config.toml`.
//!
//! The daemon owns the file: it loads it at startup, applies changes live and
//! writes it back when the dashboard saves new settings over IPC.

use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};

pub const MIN_TRAY_INTERVAL_MS: u32 = 500;
pub const MAX_TRAY_INTERVAL_MS: u32 = 60_000;
pub const MIN_WINDOW_INTERVAL_MS: u32 = 250;
pub const MAX_WINDOW_INTERVAL_MS: u32 = 60_000;
pub const MAX_ALERT_RULES: usize = 32;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrayMode {
    #[default]
    CpuRam,
    Cpu,
    Ram,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Dark,
    Light,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    Ru,
    En,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AlertMetric {
    Cpu,
    Ram,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SamplingConfig {
    pub tray_interval_ms: u32,
    pub window_interval_ms: u32,
}

impl Default for SamplingConfig {
    fn default() -> Self {
        Self {
            tray_interval_ms: 2_000,
            window_interval_ms: 1_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TrayConfig {
    pub mode: TrayMode,
    pub warn_pct: u8,
    pub crit_pct: u8,
}

impl Default for TrayConfig {
    fn default() -> Self {
        Self {
            mode: TrayMode::CpuRam,
            warn_pct: 60,
            crit_pct: 85,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HistoryConfig {
    pub minutes_detail: u32,
    pub hours_aggregate: u32,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            minutes_detail: 60,
            hours_aggregate: 24,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    pub theme: Theme,
    pub language: Language,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            language: Language::Ru,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlertRule {
    pub metric: AlertMetric,
    pub threshold_pct: u8,
    pub duration_s: u32,
    pub cooldown_min: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub sampling: SamplingConfig,
    pub tray: TrayConfig,
    pub history: HistoryConfig,
    pub ui: UiConfig,
    pub alerts: Vec<AlertRule>,
}

impl Config {
    /// Absolute path of the per-user configuration file.
    pub fn path() -> PathBuf {
        match std::env::var_os("APPDATA") {
            Some(directory) => PathBuf::from(directory)
                .join("PeakCore")
                .join("config.toml"),
            None => PathBuf::from("config.toml"),
        }
    }

    pub fn from_toml(text: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(text)
    }

    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }

    /// Reads and validates the configuration file.
    pub fn load(path: &Path) -> io::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        let config = Self::from_toml(&text)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
        config
            .validate()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        Ok(config)
    }

    /// Reads the configuration, writing the defaults first when it is missing.
    pub fn load_or_create(path: &Path) -> io::Result<Self> {
        match Self::load(path) {
            Ok(config) => Ok(config),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let config = Self::default();
                config.save(path)?;
                Ok(config)
            }
            Err(error) => Err(error),
        }
    }

    /// Writes the configuration, creating its directory. An empty alert list is
    /// left out of the file so the default config stays readable.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        self.validate()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
        let text = self
            .to_toml()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
        let text = if self.alerts.is_empty() {
            text.strip_prefix("alerts = []\n\n")
                .unwrap_or(&text)
                .to_owned()
        } else {
            text
        };
        if let Some(directory) = path.parent() {
            std::fs::create_dir_all(directory)?;
        }
        std::fs::write(path, text)
    }

    /// Rejects values the daemon cannot apply, with a message naming the field.
    pub fn validate(&self) -> Result<(), String> {
        let sampling = &self.sampling;
        if !(MIN_TRAY_INTERVAL_MS..=MAX_TRAY_INTERVAL_MS).contains(&sampling.tray_interval_ms) {
            return Err(format!(
                "sampling.tray_interval_ms must be between {MIN_TRAY_INTERVAL_MS} and {MAX_TRAY_INTERVAL_MS}"
            ));
        }
        if !(MIN_WINDOW_INTERVAL_MS..=MAX_WINDOW_INTERVAL_MS).contains(&sampling.window_interval_ms)
        {
            return Err(format!(
                "sampling.window_interval_ms must be between {MIN_WINDOW_INTERVAL_MS} and {MAX_WINDOW_INTERVAL_MS}"
            ));
        }

        let tray = &self.tray;
        if tray.warn_pct == 0 || tray.warn_pct >= tray.crit_pct || tray.crit_pct > 100 {
            return Err(
                "tray.warn_pct must be at least 1 and below tray.crit_pct, which cannot exceed 100"
                    .into(),
            );
        }

        if !(5..=60).contains(&self.history.minutes_detail) {
            return Err("history.minutes_detail must be between 5 and 60".into());
        }
        if !(1..=24).contains(&self.history.hours_aggregate) {
            return Err("history.hours_aggregate must be between 1 and 24".into());
        }
        if self.history.hours_aggregate * 60 < self.history.minutes_detail {
            return Err("history.minutes_detail cannot exceed history.hours_aggregate".into());
        }

        if self.alerts.len() > MAX_ALERT_RULES {
            return Err(format!(
                "at most {MAX_ALERT_RULES} alert rules are supported"
            ));
        }
        for (index, rule) in self.alerts.iter().enumerate() {
            if !(1..=100).contains(&rule.threshold_pct) {
                return Err(format!(
                    "alerts[{index}].threshold_pct must be between 1 and 100"
                ));
            }
            if !(1..=3_600).contains(&rule.duration_s) {
                return Err(format!(
                    "alerts[{index}].duration_s must be between 1 and 3600"
                ));
            }
            if rule.cooldown_min > 1_440 {
                return Err(format!("alerts[{index}].cooldown_min must be at most 1440"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("peakcore-config-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("config.toml")
    }

    #[test]
    fn default_config_matches_the_documented_file() {
        let config = Config::default();
        assert_eq!(config.sampling.tray_interval_ms, 2_000);
        assert_eq!(config.sampling.window_interval_ms, 1_000);
        assert_eq!(config.tray.mode, TrayMode::CpuRam);
        assert_eq!(config.tray.warn_pct, 60);
        assert_eq!(config.tray.crit_pct, 85);
        assert_eq!(config.history.minutes_detail, 60);
        assert_eq!(config.history.hours_aggregate, 24);
        assert_eq!(config.ui.theme, Theme::System);
        assert_eq!(config.ui.language, Language::Ru);
        assert!(config.alerts.is_empty());
        assert_eq!(config.validate(), Ok(()));
    }

    #[test]
    fn config_round_trips_through_toml() {
        let config = Config {
            sampling: SamplingConfig {
                tray_interval_ms: 1_000,
                window_interval_ms: 500,
            },
            tray: TrayConfig {
                mode: TrayMode::Ram,
                warn_pct: 55,
                crit_pct: 80,
            },
            history: HistoryConfig {
                minutes_detail: 30,
                hours_aggregate: 12,
            },
            ui: UiConfig {
                theme: Theme::Light,
                language: Language::En,
            },
            alerts: vec![AlertRule {
                metric: AlertMetric::Ram,
                threshold_pct: 90,
                duration_s: 30,
                cooldown_min: 10,
            }],
        };

        let text = config.to_toml().expect("serialize config");
        let parsed = Config::from_toml(&text).expect("parse config");
        assert_eq!(parsed, config);
        assert!(text.contains("mode = \"ram\""), "unexpected toml: {text}");
        assert!(
            text.contains("threshold_pct = 90"),
            "unexpected toml: {text}"
        );
    }

    #[test]
    fn json_payload_always_carries_the_alert_list() {
        let value = serde_json::to_value(Config::default()).expect("serialize config json");
        assert_eq!(
            value["alerts"],
            serde_json::json!([]),
            "the dashboard reads alerts without a fallback, so the key must always be present"
        );
    }

    #[test]
    fn partial_config_keeps_defaults_for_missing_sections() {
        let config = Config::from_toml("[tray]\nmode = \"cpu\"\n").expect("parse partial config");
        assert_eq!(config.tray.mode, TrayMode::Cpu);
        assert_eq!(config.tray.crit_pct, 85);
        assert_eq!(config.sampling.tray_interval_ms, 2_000);
        assert_eq!(config.ui.theme, Theme::System);
    }

    #[test]
    fn config_rejects_out_of_range_and_crossed_thresholds() {
        let mut config = Config::default();
        config.tray.warn_pct = 90;
        config.tray.crit_pct = 85;
        assert!(
            config
                .validate()
                .is_err_and(|error| error.contains("warn_pct"))
        );

        let mut config = Config::default();
        config.sampling.tray_interval_ms = 10;
        assert!(
            config
                .validate()
                .is_err_and(|error| error.contains("tray_interval_ms"))
        );

        let mut config = Config::default();
        config.alerts.push(AlertRule {
            metric: AlertMetric::Cpu,
            threshold_pct: 0,
            duration_s: 30,
            cooldown_min: 10,
        });
        assert!(
            config
                .validate()
                .is_err_and(|error| error.contains("threshold_pct"))
        );

        let mut config = Config::default();
        config.alerts.push(AlertRule {
            metric: AlertMetric::Cpu,
            threshold_pct: 90,
            duration_s: 0,
            cooldown_min: 10,
        });
        assert!(
            config
                .validate()
                .is_err_and(|error| error.contains("duration_s"))
        );
    }

    #[test]
    fn config_bounds_the_history_to_one_hour_of_detail_and_a_day_of_aggregates() {
        let mut config = Config::default();
        config.history.minutes_detail = 61;
        assert!(
            config
                .validate()
                .is_err_and(|error| error.contains("minutes_detail"))
        );

        let mut config = Config::default();
        config.history.hours_aggregate = 25;
        assert!(
            config
                .validate()
                .is_err_and(|error| error.contains("hours_aggregate"))
        );

        let mut config = Config::default();
        config.history.minutes_detail = 60;
        config.history.hours_aggregate = 1;
        assert_eq!(config.validate(), Ok(()));

        config.history.hours_aggregate = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn toml_with_unknown_values_is_reported_not_silently_defaulted() {
        let error = Config::from_toml("[tray]\nmode = \"gpu\"\n").unwrap_err();
        assert!(
            error.to_string().contains("unknown variant") || error.to_string().contains("gpu"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn saving_creates_the_directory_and_reloads_the_same_values() {
        let path = temp_path("save");
        let mut config = Config::default();
        config.tray.mode = TrayMode::Cpu;
        config.alerts.push(AlertRule {
            metric: AlertMetric::Cpu,
            threshold_pct: 75,
            duration_s: 15,
            cooldown_min: 5,
        });

        config.save(&path).expect("save config");
        assert!(path.exists());

        let loaded = Config::load(&path).expect("load config");
        assert_eq!(loaded, config);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn an_empty_alert_list_is_left_out_of_the_config_file() {
        let path = temp_path("no-alerts");
        Config::default().save(&path).expect("save config");
        let text = std::fs::read_to_string(&path).expect("read config");
        assert!(!text.contains("alerts"), "unexpected config: {text}");
        assert_eq!(Config::load(&path).expect("load config"), Config::default());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn loading_a_missing_file_reports_not_found() {
        let path = temp_path("missing");
        let error = Config::load(&path).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    }
}
