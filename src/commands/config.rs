use crate::utils::{config, print as p};
use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Show current global configuration
    Show,
    /// Set a scalar configuration value
    Set {
        /// Configuration key, e.g. telemetry.enabled or network
        key: String,
        /// New value
        value: String,
    },
    /// Manage trusted plugin source allowlist
    #[command(subcommand)]
    PluginTrust(PluginTrustCommands),
    /// Set global wallet encryption parameters (Argon2id)
    SetEncryption {
        /// Argon2 memory cost in KiB (e.g. 65536)
        #[arg(long)]
        mem: Option<u32>,
        /// Argon2 iteration count (e.g. 3)
        #[arg(long)]
        iterations: Option<u32>,
        /// Argon2 parallelism factor (e.g. 4)
        #[arg(long)]
        parallelism: Option<u32>,
        /// Reset to library defaults
        #[arg(long, default_value = "false")]
        reset: bool,
    },
}

#[derive(Subcommand)]
pub enum PluginTrustCommands {
    /// List trusted plugin sources
    List,
    /// Add a trusted plugin domain or URL prefix
    Add {
        /// Domain or URL prefix to trust
        source: String,
    },
    /// Remove a trusted plugin source
    Remove {
        /// Domain or URL prefix to remove
        source: String,
    },
    /// Reset trusted plugin sources to StarForge defaults
    Reset,
}

pub fn handle(cmd: ConfigCommands) -> Result<()> {
    match cmd {
        ConfigCommands::Show => show(),
        ConfigCommands::Set { key, value } => set_value(&key, &value),
        ConfigCommands::PluginTrust(cmd) => plugin_trust(cmd),
        ConfigCommands::SetEncryption {
            mem,
            iterations,
            parallelism,
            reset,
        } => set_encryption(mem, iterations, parallelism, reset),
    }
}

fn show() -> Result<()> {
    let cfg = config::load()?;
    p::header("StarForge Configuration");
    p::separator();

    p::kv("Config file", &config::config_path().display().to_string());
    p::kv("Active network", &cfg.network);
    p::kv(
        "telemetry.enabled",
        &cfg.telemetry_enabled.unwrap_or(false).to_string(),
    );

    println!();
    p::header("Plugin Trust");
    if cfg.plugin_trust.trusted_sources.is_empty() {
        p::warn("No trusted remote plugin sources configured.");
    } else {
        for source in &cfg.plugin_trust.trusted_sources {
            p::kv("trusted source", source);
        }
    }

    println!();
    p::header("Wallet Encryption (Argon2id)");
    if let Some(kdf) = &cfg.wallet_encryption {
        p::kv("Memory cost", &format!("{} KiB", kdf.mem.unwrap_or(32768)));
        p::kv("Iterations", &kdf.iterations.unwrap_or(3).to_string());
        p::kv("Parallelism", &kdf.parallelism.unwrap_or(1).to_string());
    } else {
        p::info("Using default Argon2id parameters:");
        p::kv("Memory cost", "32768 KiB (default)");
        p::kv("Iterations", "3 (default)");
        p::kv("Parallelism", "1 (default)");
    }

    p::separator();
    Ok(())
}

fn set_value(key: &str, value: &str) -> Result<()> {
    let mut cfg = config::load()?;
    match key {
        "telemetry" | "telemetry.enabled" => {
            cfg.telemetry_enabled = Some(parse_bool(value)?);
        }
        "network" => {
            config::validate_network_exists(&cfg, value)?;
            cfg.network = value.to_string();
        }
        _ => {
            anyhow::bail!(
                "Unsupported config key '{}'. Supported keys: telemetry.enabled, network",
                key
            );
        }
    }
    config::save(&cfg)?;
    p::success(&format!("{} set to '{}'", key, value));
    Ok(())
}

fn parse_bool(value: &str) -> Result<bool> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" | "enabled" => Ok(true),
        "false" | "0" | "no" | "off" | "disabled" => Ok(false),
        _ => anyhow::bail!(
            "Expected boolean value for telemetry.enabled, got '{}'",
            value
        ),
    }
}

fn plugin_trust(cmd: PluginTrustCommands) -> Result<()> {
    match cmd {
        PluginTrustCommands::List => {
            let cfg = config::load()?;
            print_plugin_trust_sources(&cfg);
        }
        PluginTrustCommands::Add { source } => {
            let mut cfg = config::load()?;
            let added = config::add_trusted_plugin_source(&mut cfg, source.clone())?;
            config::save(&cfg)?;
            if added {
                p::success(&format!("Trusted plugin source added: {}", source.trim()));
            } else {
                p::info(&format!(
                    "Trusted plugin source already exists: {}",
                    source.trim()
                ));
            }
            print_plugin_trust_sources(&cfg);
        }
        PluginTrustCommands::Remove { source } => {
            let mut cfg = config::load()?;
            if !config::remove_trusted_plugin_source(&mut cfg, &source) {
                anyhow::bail!("Trusted plugin source not found: {}", source.trim());
            }
            config::save(&cfg)?;
            p::success(&format!("Trusted plugin source removed: {}", source.trim()));
            print_plugin_trust_sources(&cfg);
        }
        PluginTrustCommands::Reset => {
            let mut cfg = config::load()?;
            config::reset_trusted_plugin_sources(&mut cfg);
            config::save(&cfg)?;
            p::success("Trusted plugin sources reset to defaults.");
            print_plugin_trust_sources(&cfg);
        }
    }
    Ok(())
}

fn print_plugin_trust_sources(cfg: &config::Config) {
    p::header("Trusted Plugin Sources");
    if cfg.plugin_trust.trusted_sources.is_empty() {
        p::warn("No trusted remote plugin sources configured.");
        return;
    }
    for source in &cfg.plugin_trust.trusted_sources {
        p::info(&format!("- {}", source));
    }
}

fn set_encryption(
    mem: Option<u32>,
    iterations: Option<u32>,
    parallelism: Option<u32>,
    reset: bool,
) -> Result<()> {
    let mut cfg = config::load()?;

    if reset {
        cfg.wallet_encryption = None;
        config::save(&cfg)?;
        p::success("Wallet encryption parameters reset to defaults.");
        return Ok(());
    }

    if mem.is_none() && iterations.is_none() && parallelism.is_none() {
        anyhow::bail!("Provide at least one parameter to set (e.g. --mem 65536) or use --reset");
    }

    let mut kdf = cfg.wallet_encryption.unwrap_or_default();
    if let Some(m) = mem {
        kdf.mem = Some(m);
    }
    if let Some(i) = iterations {
        kdf.iterations = Some(i);
    }
    if let Some(p) = parallelism {
        kdf.parallelism = Some(p);
    }

    cfg.wallet_encryption = Some(kdf);
    config::save(&cfg)?;

    p::success("Global wallet encryption parameters updated.");
    show()?;
    Ok(())
}
