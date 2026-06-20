use std::{env, fs, io::ErrorKind, path::PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    Openai,
    Anthropic,
}

impl ProviderKind {
    pub fn default_base_url(&self) -> &'static str {
        match self {
            Self::Openai => "https://api.openai.com/v1",
            Self::Anthropic => "https://api.anthropic.com",
        }
    }

    pub fn default_model(&self) -> &'static str {
        match self {
            Self::Openai => "gpt-4.1-mini",
            Self::Anthropic => "claude-3-5-sonnet-latest",
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Openai => "openai",
            Self::Anthropic => "anthropic",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value.trim().to_lowercase().as_str() {
            "" | "openai" => Ok(Self::Openai),
            "anthropic" => Ok(Self::Anthropic),
            other => bail!("unsupported provider format '{other}'; use openai or anthropic"),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub provider: ProviderKind,
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    pub temperature: Option<f64>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = config_path()?;

        let raw = match fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                bail!(
                    "config file not found at {}; run `ask config init` first",
                    path.display()
                );
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to read config file at {}", path.display()));
            }
        };
        let config: Self = toml::from_str(&raw)
            .with_context(|| format!("failed to parse config file at {}", path.display()))?;
        config.validate()?;
        Ok(config)
    }

    pub fn api_key(&self) -> Result<String> {
        Ok(self.api_key.clone())
    }

    pub fn save(&self) -> Result<PathBuf> {
        self.validate()?;
        let path = config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create config directory {}", parent.display())
            })?;
        }
        let raw = toml::to_string_pretty(self).context("failed to serialize config")?;
        fs::write(&path, raw)
            .with_context(|| format!("failed to write config file at {}", path.display()))?;
        Ok(path)
    }

    pub fn masked_api_key(&self) -> String {
        let suffix: String = self
            .api_key
            .chars()
            .rev()
            .take(4)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        if suffix.is_empty() {
            "not set".to_string()
        } else {
            format!("set (****{suffix})")
        }
    }

    fn validate(&self) -> Result<()> {
        if self.model.trim().is_empty() {
            bail!("config field model must not be empty");
        }
        if self.base_url.trim().is_empty() {
            bail!("config field base_url must not be empty");
        }
        if self.api_key.trim().is_empty() {
            bail!("config field api_key must not be empty");
        }
        Ok(())
    }
}

pub fn config_path() -> Result<PathBuf> {
    home_dir()
        .map(|home| home.join(".config").join("ask").join("config.toml"))
        .ok_or_else(|| anyhow!("HOME is not set; expected config at ~/.config/ask/config.toml"))
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_literal_api_key() {
        let config = Config {
            provider: ProviderKind::Openai,
            model: "m".to_string(),
            base_url: "http://localhost".to_string(),
            api_key: "literal".to_string(),
            temperature: None,
        };

        assert_eq!(config.api_key().unwrap(), "literal");
    }

    #[test]
    fn config_path_uses_dot_config_under_home() {
        temp_env::with_var("HOME", Some("/tmp/ask-home"), || {
            assert_eq!(
                config_path().unwrap(),
                PathBuf::from("/tmp/ask-home/.config/ask/config.toml")
            );
        });
    }

    #[test]
    fn masks_api_key_suffix() {
        let config = Config {
            provider: ProviderKind::Openai,
            model: "m".to_string(),
            base_url: "http://localhost".to_string(),
            api_key: "sk-test-1234".to_string(),
            temperature: None,
        };

        assert_eq!(config.masked_api_key(), "set (****1234)");
    }
}
