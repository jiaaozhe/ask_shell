use std::io::{self, IsTerminal, Write};

use anyhow::{bail, Result};
use clap::Subcommand;
use rustyline::DefaultEditor;

use crate::config::{self, Config, ProviderKind};

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Create ~/.config/ask/config.toml interactively.
    Init,
    /// Show the current config with the API key masked.
    Show,
    /// Print the config file path.
    Path,
}

pub fn run(command: ConfigCommand) -> Result<()> {
    match command {
        ConfigCommand::Init => init(),
        ConfigCommand::Show => show(),
        ConfigCommand::Path => path(),
    }
}

pub fn ensure_initialized() -> Result<()> {
    let path = config::config_path()?;
    if path.exists() {
        return Ok(());
    }

    eprintln!("No ask config found.");
    eprintln!("Config path:");
    eprintln!("  {}", path.display());
    eprintln!();

    if !confirm("Set up model configuration now? [Y/n]: ", true)? {
        bail!("config is required; run `ask config init` before using ask");
    }

    eprintln!();
    init()?;
    if !path.exists() {
        bail!("config setup was canceled; run `ask config init` before using ask");
    }

    Ok(())
}

fn init() -> Result<()> {
    let path = config::config_path()?;
    eprintln!("Config path:");
    eprintln!("  {}", path.display());
    eprintln!();

    let existing = if path.exists() {
        eprintln!("Config already exists.");
        eprintln!();
        if !confirm("Overwrite? [y/N]: ", false)? {
            eprintln!("Canceled.");
            return Ok(());
        }
        eprintln!();

        match Config::load() {
            Ok(config) => Some(config),
            Err(error) => {
                eprintln!("Existing config could not be loaded: {error}");
                eprintln!("Using default values.");
                eprintln!();
                None
            }
        }
    } else {
        None
    };

    let default_provider = existing
        .as_ref()
        .map(|config| config.provider.clone())
        .unwrap_or(ProviderKind::Openai);
    let provider = prompt_provider(&default_provider)?;
    let default_base_url = existing
        .as_ref()
        .map(|config| config.base_url.as_str())
        .unwrap_or_else(|| provider.default_base_url());
    let default_model = existing
        .as_ref()
        .map(|config| config.model.as_str())
        .unwrap_or_else(|| provider.default_model());
    let default_temperature = existing
        .as_ref()
        .and_then(|config| config.temperature)
        .unwrap_or(0.2);

    let base_url = prompt_with_default("Base URL", default_base_url)?;
    let model = prompt_with_default("Model", default_model)?;
    let api_key = prompt_api_key(existing.as_ref().map(|config| config.api_key.as_str()))?;
    if api_key.trim().is_empty() {
        bail!("API key must not be empty");
    }
    let temperature = prompt_temperature(default_temperature)?;

    let config = Config {
        provider,
        model,
        base_url,
        api_key,
        temperature: Some(temperature),
    };
    let path = config.save()?;
    eprintln!();
    eprintln!("Config written:");
    eprintln!("  {}", path.display());
    Ok(())
}

fn show() -> Result<()> {
    let config = Config::load()?;
    println!("provider: {}", config.provider.as_str());
    println!("model: {}", config.model);
    println!("base_url: {}", config.base_url);
    println!("api_key: {}", config.masked_api_key());
    println!("temperature: {}", config.temperature.unwrap_or(0.2));
    Ok(())
}

fn path() -> Result<()> {
    println!("{}", config::config_path()?.display());
    Ok(())
}

fn prompt_provider(default: &ProviderKind) -> Result<ProviderKind> {
    eprintln!("Provider format:");
    eprintln!("  openai");
    eprintln!("  anthropic");
    let value = prompt(&format!("Provider format [{}]: ", default.as_str()))?;
    if value.trim().is_empty() {
        Ok(default.clone())
    } else {
        ProviderKind::parse(&value)
    }
}

fn prompt_temperature(default: f64) -> Result<f64> {
    let value = prompt(&format!("Temperature [{default}]: "))?;
    if value.trim().is_empty() {
        return Ok(default);
    }
    Ok(value.trim().parse()?)
}

fn prompt_api_key(default: Option<&str>) -> Result<String> {
    let label = if default.is_some() {
        "API key [keep existing]: "
    } else {
        "API key: "
    };

    let value = if io::stdin().is_terminal() && io::stderr().is_terminal() {
        rpassword::prompt_password(label)?
    } else {
        prompt(label)?
    };

    if value.trim().is_empty() {
        if let Some(default) = default {
            Ok(default.to_string())
        } else {
            Ok(value)
        }
    } else {
        Ok(value)
    }
}

fn prompt_with_default(label: &str, default: &str) -> Result<String> {
    let value = prompt(&format!("{label} [{default}]: "))?;
    let value = value.trim();
    if value.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(value.to_string())
    }
}

fn confirm(label: &str, default: bool) -> Result<bool> {
    let value = prompt(label)?;
    match value.trim().to_lowercase().as_str() {
        "" => Ok(default),
        "y" | "yes" => Ok(true),
        "n" | "no" => Ok(false),
        _ => bail!("please answer y or n"),
    }
}

fn prompt(label: &str) -> Result<String> {
    if io::stdin().is_terminal() && io::stderr().is_terminal() {
        let mut editor = DefaultEditor::new()?;
        Ok(editor.readline(label)?)
    } else {
        eprint!("{label}");
        io::stderr().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        Ok(input.trim_end_matches(['\r', '\n']).to_string())
    }
}
