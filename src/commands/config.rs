use anyhow::{bail, Result};
use clap::Subcommand;
use colored::*;

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Get a configuration value
    Get {
        /// Configuration key to retrieve (e.g., telemetry, network, wallet)
        key: String,
    },
    /// Set a configuration value
    Set {
        /// Configuration key to set (e.g., telemetry)
        key: String,
        /// Configuration value (e.g., true, false)
        value: String,
    },
    /// Show all configuration settings
    Show,
}

pub fn handle_config(cmd: ConfigCommands) -> Result<()> {
    match cmd {
        ConfigCommands::Get { key } => handle_get(&key),
        ConfigCommands::Set { key, value } => handle_set(&key, &value),
        ConfigCommands::Show => handle_show(),
    }
}

fn handle_get(key: &str) -> Result<()> {
    let cfg = crate::utils::config::load()?;

    match key.to_lowercase().as_str() {
        "telemetry" => {
            let enabled = cfg.telemetry_enabled.unwrap_or(true);
            println!("{}: {}", key.cyan(), if enabled { "enabled" } else { "disabled" });
            Ok(())
        }
        "network" => {
            println!("{}: {}", key.cyan(), cfg.network);
            Ok(())
        }
        _ => {
            bail!(
                "Unknown configuration key: '{}'\n\nAvailable keys:\n  - telemetry\n  - network",
                key
            );
        }
    }
}

fn handle_set(key: &str, value: &str) -> Result<()> {
    let mut cfg = crate::utils::config::load()?;

    match key.to_lowercase().as_str() {
        "telemetry" => {
            let enabled = match value.to_lowercase().as_str() {
                "true" | "on" | "enabled" | "yes" => true,
                "false" | "off" | "disabled" | "no" => false,
                _ => {
                    bail!(
                        "Invalid value for telemetry: '{}'\n\nUse 'true'/'enabled'/'on'/'yes' or 'false'/'disabled'/'off'/'no'.",
                        value
                    );
                }
            };
            cfg.telemetry_enabled = Some(enabled);
            crate::utils::config::save(&cfg)?;
            println!(
                "✓ {} set to {}",
                "telemetry".green(),
                if enabled { "enabled" } else { "disabled" }
            );
            Ok(())
        }
        "network" => {
            crate::utils::config::validate_network(value)?;
            cfg.network = value.to_string();
            crate::utils::config::save(&cfg)?;
            println!("✓ {} set to {}", "network".green(), value);
            Ok(())
        }
        _ => {
            bail!(
                "Unknown configuration key: '{}'\n\nAvailable keys:\n  - telemetry\n  - network",
                key
            );
        }
    }
}

fn handle_show() -> Result<()> {
    let cfg = crate::utils::config::load()?;

    println!("\n{}", "=== StarForge Configuration ===".bold());
    println!();
    println!("  {}: {}", "Network".cyan(), cfg.network);

    let telemetry_status = cfg.telemetry_enabled.unwrap_or(true);
    println!(
        "  {}: {}",
        "Telemetry".cyan(),
        if telemetry_status { "enabled" } else { "disabled" }
    );

    println!("\n{}", "Configuration file:".cyan());
    if let Ok(cfg_path) = crate::utils::config::get_config_path() {
        println!("  {}", cfg_path.display());
    }

    println!("\n{}", "Data directory:".cyan());
    if let Ok(data_dir) = crate::utils::config::get_data_dir() {
        println!("  {}", data_dir.display());
    }

    println!();

    Ok(())
}
