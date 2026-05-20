//! Layered configuration: bundled defaults → file → env → programmatic override.

use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

/// The full configuration object serialised at the root of TOML files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub analysis: AnalysisConfig,
    pub output: OutputConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisConfig {
    pub low_percentile: f64,
    pub min_days_for_activity: u32,
    pub activity_threshold: f64,
    pub zero_war_kick_threshold: u32,
    pub poor_war_threshold: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputConfig {
    pub formats: Vec<String>,
}

/// Optional/partial config used for overlays (file, env, CLI).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ConfigOverlay {
    #[serde(default)]
    pub analysis: AnalysisOverlay,
    #[serde(default)]
    pub output: OutputOverlay,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct AnalysisOverlay {
    pub low_percentile: Option<f64>,
    pub min_days_for_activity: Option<u32>,
    pub activity_threshold: Option<f64>,
    pub zero_war_kick_threshold: Option<u32>,
    pub poor_war_threshold: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct OutputOverlay {
    pub formats: Option<Vec<String>>,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("io error reading config {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("could not parse TOML at {path}: {source}")]
    Toml {
        path: String,
        #[source]
        source: toml::de::Error,
    },
    #[error("bundled default config is corrupt: {0}")]
    BundledCorrupt(#[source] toml::de::Error),
    #[error("invalid env override {var}={value}: {detail}")]
    Env {
        var: String,
        value: String,
        detail: String,
    },
}

/// The bundled default configuration, baked into the binary at build time.
pub const DEFAULT_CONFIG_TOML: &str = include_str!("../../../default-config.toml");

impl Default for Config {
    fn default() -> Self {
        Self::from_default().expect("bundled default-config.toml must parse")
    }
}

impl Config {
    /// Parse the bundled default.
    pub fn from_default() -> Result<Self, ConfigError> {
        toml::from_str::<Config>(DEFAULT_CONFIG_TOML).map_err(ConfigError::BundledCorrupt)
    }

    /// Build a config by layering: bundled → optional file → env → optional explicit overlay.
    pub fn layered(file: Option<&Path>, cli: Option<&ConfigOverlay>) -> Result<Self, ConfigError> {
        let mut cfg = Self::from_default()?;

        if let Some(path) = file {
            let text = std::fs::read_to_string(path).map_err(|e| ConfigError::Io {
                path: path.display().to_string(),
                source: e,
            })?;
            let overlay: ConfigOverlay = toml::from_str(&text).map_err(|e| ConfigError::Toml {
                path: path.display().to_string(),
                source: e,
            })?;
            cfg.apply_overlay(&overlay);
        }

        let env_overlay = Self::overlay_from_env()?;
        cfg.apply_overlay(&env_overlay);

        if let Some(o) = cli {
            cfg.apply_overlay(o);
        }

        Ok(cfg)
    }

    pub fn apply_overlay(&mut self, o: &ConfigOverlay) {
        if let Some(v) = o.analysis.low_percentile {
            self.analysis.low_percentile = v;
        }
        if let Some(v) = o.analysis.min_days_for_activity {
            self.analysis.min_days_for_activity = v;
        }
        if let Some(v) = o.analysis.activity_threshold {
            self.analysis.activity_threshold = v;
        }
        if let Some(v) = o.analysis.zero_war_kick_threshold {
            self.analysis.zero_war_kick_threshold = v;
        }
        if let Some(v) = o.analysis.poor_war_threshold {
            self.analysis.poor_war_threshold = v;
        }
        if let Some(v) = &o.output.formats {
            self.output.formats = v.clone();
        }
    }

    /// Build a ConfigOverlay from `TWR_*` environment variables.
    pub fn overlay_from_env() -> Result<ConfigOverlay, ConfigError> {
        let mut o = ConfigOverlay::default();

        fn parse_env<T: std::str::FromStr>(var: &str) -> Result<Option<T>, ConfigError>
        where
            T::Err: std::fmt::Display,
        {
            match std::env::var(var) {
                Ok(v) => v.parse::<T>().map(Some).map_err(|e| ConfigError::Env {
                    var: var.to_string(),
                    value: v,
                    detail: e.to_string(),
                }),
                Err(_) => Ok(None),
            }
        }

        o.analysis.low_percentile = parse_env::<f64>("TWR_LOW_PERCENTILE")?;
        o.analysis.activity_threshold = parse_env::<f64>("TWR_ACTIVITY_THRESHOLD")?;
        o.analysis.min_days_for_activity = parse_env::<u32>("TWR_MIN_DAYS")?;
        o.analysis.zero_war_kick_threshold = parse_env::<u32>("TWR_ZERO_WAR_KICK_THRESHOLD")?;
        o.analysis.poor_war_threshold = parse_env::<u32>("TWR_POOR_WAR_THRESHOLD")?;

        if let Ok(v) = std::env::var("TWR_FORMATS") {
            o.output.formats = Some(
                v.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect(),
            );
        }

        Ok(o)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_parses() {
        let c = Config::default();
        assert!((c.analysis.low_percentile - 0.20).abs() < 1e-9);
        assert_eq!(c.analysis.min_days_for_activity, 7);
        assert!((c.analysis.activity_threshold - 750.0).abs() < 1e-9);
        assert_eq!(c.analysis.zero_war_kick_threshold, 2);
        assert_eq!(c.analysis.poor_war_threshold, 2);
        assert_eq!(c.output.formats, vec!["xlsx", "csv", "markdown"]);
    }

    #[test]
    fn overlay_replaces_only_set_fields() {
        let mut c = Config::default();
        let o = ConfigOverlay {
            analysis: AnalysisOverlay {
                low_percentile: Some(0.10),
                ..Default::default()
            },
            output: OutputOverlay { formats: None },
        };
        c.apply_overlay(&o);
        assert!((c.analysis.low_percentile - 0.10).abs() < 1e-9);
        assert_eq!(c.analysis.min_days_for_activity, 7);
    }

    #[test]
    fn layered_with_file_overrides_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "[analysis]\nlow_percentile = 0.30\nactivity_threshold = 500.0\n",
        )
        .unwrap();
        let cfg = Config::layered(Some(&path), None).unwrap();
        assert!((cfg.analysis.low_percentile - 0.30).abs() < 1e-9);
        assert!((cfg.analysis.activity_threshold - 500.0).abs() < 1e-9);
        // Untouched fields keep their default.
        assert_eq!(cfg.analysis.min_days_for_activity, 7);
    }
}
