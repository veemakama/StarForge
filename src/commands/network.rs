use crate::utils::{config, print as p};
use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum NetworkCommands {
    /// Show the current active network and available networks
    Show,
    /// Switch the active network (testnet, mainnet, or custom)
    Switch {
        /// Target network to switch to
        network: String,
    },
    /// Add a custom network endpoint
    Add {
        /// Name for the custom network
        name: String,
        /// Horizon API URL
        #[arg(long)]
        horizon_url: String,
        /// Optional Soroban RPC URL
        #[arg(long)]
        soroban_rpc_url: Option<String>,
        /// Optional network faucet / Friendbot URL
        #[arg(long)]
        friendbot_url: Option<String>,
        /// Optional network passphrase for transaction signing (defaults to testnet passphrase)
        #[arg(long)]
        passphrase: Option<String>,
    },
    /// Test connectivity to a network
    Test {
        /// Network to test (defaults to current active network)
        #[arg(default_value = None)]
        network: Option<String>,
    },
}

pub fn handle(cmd: NetworkCommands) -> Result<()> {
    match cmd {
        NetworkCommands::Show => show(),
        NetworkCommands::Switch { network } => switch(network),
        NetworkCommands::Add {
            name,
            horizon_url,
            soroban_rpc_url,
            friendbot_url,
            passphrase,
        } => add_network(name, horizon_url, soroban_rpc_url, friendbot_url, passphrase),
        NetworkCommands::Test { network } => test_network(network),
    }
}

fn show() -> Result<()> {
    let cfg = config::load()?;
    p::header("Networks");
    p::separator();

    for (name, net_cfg) in &cfg.networks {
        let active = if cfg.network == *name { " ✓" } else { "" };
        println!("  {} {}", name.to_uppercase(), active);
        p::kv("Horizon", &net_cfg.horizon_url);
        if let Some(soroban_url) = &net_cfg.soroban_rpc_url {
            p::kv("Soroban RPC", soroban_url);
        }
        if let Some(friendbot_url) = &net_cfg.friendbot_url {
            p::kv("Friendbot", friendbot_url);
        }
        println!();
    }

    p::separator();
    p::info(&format!("Active network: {}", cfg.network));
    Ok(())
}

fn switch(target: String) -> Result<()> {
    let mut cfg = config::load()?;

    // Validate network exists (accepts built-ins + custom networks)
    config::validate_network_exists(&cfg, &target)?;

    // Check if already on the target network
    if cfg.network == target {
        p::info(&format!("Already on {}. No changes made.", target));
        return Ok(());
    }

    let previous = cfg.network.clone();
    cfg.network = target.clone();
    config::save(&cfg)?;

    // Print mainnet warning
    if target == "mainnet" {
        p::warn("You are now on MAINNET. Transactions use real funds!");
        p::warn("Double-check all addresses and amounts before sending.");
    }

    p::success(&format!(
        "Network switched from {} to {}.",
        previous, target
    ));

    Ok(())
}

fn validate_url(label: &str, url: &str) -> Result<()> {
    if url.is_empty() {
        anyhow::bail!("{} URL cannot be empty", label);
    }
    if !url.starts_with("http://") && !url.starts_with("https://") {
        anyhow::bail!("{} URL must start with http:// or https://", label);
    }
    Ok(())
}

fn add_network(
    name: String,
    horizon_url: String,
    soroban_rpc_url: Option<String>,
    friendbot_url: Option<String>,
    passphrase: Option<String>,
) -> Result<()> {
    let mut cfg = config::load()?;

    validate_url("Horizon", &horizon_url)?;

    if let Some(ref url) = soroban_rpc_url {
        validate_url("Soroban RPC", url)?;
    }

    if let Some(ref url) = friendbot_url {
        validate_url("Friendbot", url)?;
    }

    // Normalize trailing slashes so URL construction is consistent downstream
    let horizon_url = horizon_url.trim_end_matches('/').to_string();
    let soroban_rpc_url = soroban_rpc_url.map(|u| u.trim_end_matches('/').to_string());
    let friendbot_url = friendbot_url.map(|u| u.trim_end_matches('/').to_string());

    config::add_custom_network(
        &mut cfg,
        name.clone(),
        horizon_url.clone(),
        soroban_rpc_url.clone(),
        friendbot_url.clone(),
        passphrase,
    )?;
    config::save(&cfg)?;

    p::success(&format!("Network '{}' added successfully", name));
    p::kv("Horizon", &horizon_url);
    if let Some(url) = soroban_rpc_url {
        p::kv("Soroban RPC", &url);
    }
    if let Some(url) = friendbot_url {
        p::kv("Friendbot", &url);
    }
    Ok(())
}

fn test_network(network_name: Option<String>) -> Result<()> {
    let cfg = config::load()?;
    let test_network = network_name.unwrap_or_else(|| cfg.network.clone());

    let net_cfg = config::get_network_config(&cfg, &test_network)?;

    p::info(&format!("Testing connectivity to '{}'…", test_network));
    p::info(&format!("Horizon: {}", net_cfg.horizon_url));

    // Test Horizon endpoint
    match ureq::get(&format!("{}/health", net_cfg.horizon_url)).call() {
        Ok(_) => {
            p::success("✓ Horizon endpoint is reachable");
        }
        Err(e) => {
            p::warn(&format!("✗ Horizon endpoint failed: {}", e));
        }
    }

    // Test Soroban RPC if available
    if let Some(soroban_url) = &net_cfg.soroban_rpc_url {
        p::info(&format!("Soroban RPC: {}", soroban_url));
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getLatestLedger",
            "params": []
        });

        match ureq::post(soroban_url)
            .set("Content-Type", "application/json")
            .send_json(&req)
        {
            Ok(_) => {
                p::success("✓ Soroban RPC endpoint is reachable");
            }
            Err(e) => {
                p::warn(&format!("✗ Soroban RPC endpoint failed: {}", e));
            }
        }
    }

    p::info("Network test complete");
    Ok(())
}
