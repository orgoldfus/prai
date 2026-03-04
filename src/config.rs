use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Top-level application configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub agent: AgentConfig,
    pub ui: UiConfig,
}

/// Which AI agent to use and its defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentConfig {
    /// The agent provider name (e.g. `"cursor"`).
    pub provider: String,
    /// Default model to use when sending to the agent.
    pub default_model: String,
}

/// UI-related preferences.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    /// Color theme name.
    pub theme: String,
    /// How long to show the splash screen, in milliseconds.
    pub splash_duration_ms: u64,
}

// ── Defaults ──────────────────────────────────────────────────────────────

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            provider: "cursor".to_owned(),
            default_model: "claude-4-sonnet".to_owned(),
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "catppuccin-mocha".to_owned(),
            splash_duration_ms: 1500,
        }
    }
}

// ── Loading / Saving ──────────────────────────────────────────────────────

impl Config {
    /// Canonical config file path: `~/.config/prai/config.toml`.
    pub fn path() -> Result<PathBuf> {
        let dir = dirs::config_dir()
            .context("could not determine config directory")?
            .join("prai");
        Ok(dir.join("config.toml"))
    }

    /// Load config from disk, creating a default file if it doesn't exist.
    pub fn load() -> Result<Self> {
        let path = Self::path()?;

        if !path.exists() {
            let config = Self::default();
            config.save()?;
            return Ok(config);
        }

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;

        let config: Self = toml::from_str(&contents)
            .with_context(|| format!("failed to parse {}", path.display()))?;

        Ok(config)
    }

    /// Write the current config to disk.
    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }

        let contents = toml::to_string_pretty(self).context("failed to serialize config")?;

        fs::write(&path, contents)
            .with_context(|| format!("failed to write {}", path.display()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_values() {
        let config = Config::default();
        assert_eq!(config.agent.provider, "cursor");
        assert_eq!(config.agent.default_model, "claude-4-sonnet");
        assert_eq!(config.ui.theme, "catppuccin-mocha");
        assert_eq!(config.ui.splash_duration_ms, 1500);
    }

    #[test]
    fn round_trip_serialize_deserialize() {
        let config = Config {
            agent: AgentConfig {
                provider: "test-agent".to_owned(),
                default_model: "test-model".to_owned(),
            },
            ui: UiConfig {
                theme: "dark".to_owned(),
                splash_duration_ms: 500,
            },
        };

        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: Config = toml::from_str(&serialized).unwrap();

        assert_eq!(deserialized.agent.provider, "test-agent");
        assert_eq!(deserialized.agent.default_model, "test-model");
        assert_eq!(deserialized.ui.theme, "dark");
        assert_eq!(deserialized.ui.splash_duration_ms, 500);
    }

    #[test]
    fn deserialize_partial_config_uses_defaults() {
        let partial = r#"
[agent]
provider = "custom"
"#;
        let config: Config = toml::from_str(partial).unwrap();
        assert_eq!(config.agent.provider, "custom");
        assert_eq!(config.agent.default_model, "claude-4-sonnet");
        assert_eq!(config.ui.splash_duration_ms, 1500);
    }

    #[test]
    fn deserialize_empty_config_uses_all_defaults() {
        let config: Config = toml::from_str("").unwrap();
        assert_eq!(config.agent.provider, "cursor");
        assert_eq!(config.ui.theme, "catppuccin-mocha");
    }
}
