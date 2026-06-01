use crate::utils::{config, horizon, optimizer, print as p, soroban};
use crate::commands::info;
use anyhow::Result;
use clap::Args;
use colored::*;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const SOROBAN_WASM_LIMIT_KB: f64 = 128.0;

/// Deploy a compiled Soroban WASM artifact to testnet or mainnet.
///
/// By default StarForge performs a dry-run: it validates the WASM, checks the
/// wallet on Horizon, prints the Stellar CLI command, and optionally simulates
/// fees with `--simulate`. Pass `--execute` to run `stellar contract deploy`.
#[derive(Args)]
pub struct DeployArgs {
    /// Path to the compiled .wasm file
    #[arg(long)]
    pub wasm: PathBuf,
    /// Network to deploy to
    #[arg(long, default_value = "testnet", value_parser = ["testnet", "mainnet"])]
    pub network: String,
    /// Wallet name to use for deployment
    #[arg(long)]
    pub wallet: Option<String>,
    /// Optimize the WASM before deployment using the built-in optimizer
    #[arg(long, default_value = "false")]
    pub optimize: bool,
    /// Skip confirmation prompt
    #[arg(long, default_value = "false")]
    pub yes: bool,
    /// Execute deployment immediately if Stellar CLI is installed
    #[arg(long, default_value = "false")]
    pub execute: bool,
    /// Simulate the deploy transaction using Soroban RPC
    /// Simulate deploy transaction via Soroban RPC before confirmation
    #[arg(long, default_value = "false")]
    pub simulate: bool,
}

fn is_wasm_above_size_limit(wasm_size_kb: f64) -> bool {
    wasm_size_kb > SOROBAN_WASM_LIMIT_KB
}

/// Compute the Soroban WASM hash (SHA-256 over raw `.wasm` file bytes)
/// and return it as a 64-character lowercase hex string.
///
/// This matches the hash that `stellar contract inspect --wasm <file>` reports
/// and that Soroban uses to identify uploaded contract bytecode on-chain.
fn compute_local_wasm_hash(wasm_bytes: &[u8]) -> String {
    let digest = Sha256::digest(wasm_bytes);
    hex::encode(digest)
}

fn build_stellar_deploy_command(wasm: &std::path::Path, source: &str, network: &str) -> String {
    format!(
        "stellar contract deploy \\\n  --wasm {} \\\n  --source {} \\\n  --network {}",
        wasm.display(),
        source,
        network
    )
}

fn build_stellar_deploy_args(wasm: &std::path::Path, source: &str, network: &str) -> Vec<String> {
    vec![
        "contract".to_string(),
        "deploy".to_string(),
        "--wasm".to_string(),
        wasm.display().to_string(),
        "--source".to_string(),
        source.to_string(),
        "--network".to_string(),
        network.to_string(),
    ]
}

pub fn handle(args: DeployArgs) -> Result<()> {
    p::header("Deploy Soroban Contract");

    if !args.wasm.exists() {
        anyhow::bail!(
            "WASM file not found: {:?}\nRun `stellar contract build` first.",
            args.wasm
        );
    }

    let mut wasm_path = args.wasm.clone();
    let mut wasm_bytes = fs::read(&wasm_path)?;
    let mut wasm_size_kb = wasm_bytes.len() as f64 / 1024.0;

    if args.optimize {
        let optimized_path = args.wasm.with_file_name(format!(
            "{}-optimized.wasm",
            args.wasm.file_stem().unwrap_or_default().to_string_lossy()
        ));
        p::header("WASM Optimization");
        p::kv("Input WASM", &args.wasm.display().to_string());
        p::kv("Output WASM", &optimized_path.display().to_string());
        let result = optimizer::optimize_wasm(&args.wasm, &optimized_path)?;
        wasm_path = optimized_path;
        wasm_bytes = fs::read(&wasm_path)?;
        wasm_size_kb = wasm_bytes.len() as f64 / 1024.0;
        println!();
        p::success("Optimization pass completed");
        p::kv("Input size", &format!("{} bytes", result.input_size_bytes));
        p::kv(
            "Output size",
            &format!("{} bytes", result.output_size_bytes),
        );
        p::separator();
    }

    p::separator();
    p::kv("WASM file", &wasm_path.display().to_string());
    p::kv("WASM size", &format!("{:.1} KB", wasm_size_kb));
    p::kv("Network", &args.network);

    if is_wasm_above_size_limit(wasm_size_kb) {
        p::warn(&format!(
            "WASM is {:.1} KB - Soroban limit is 128 KB. Optimize with --release.",
            wasm_size_kb
        ));
        p::info("If this contract is still too large, use `starforge gas optimize --target <input>.wasm --output <output>.wasm` or external tools such as `wasm-opt -Oz`.");
    }

    let cfg = config::load()?;
    let wasm_hash = compute_local_wasm_hash(&wasm_bytes);
    let wallet = if let Some(ref wallet_name) = args.wallet {
        cfg.wallets
            .iter()
            .find(|w| &w.name == wallet_name)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Wallet '{}' not found. Run `starforge wallet list`",
                    wallet_name
                )
            })?
    } else if !cfg.wallets.is_empty() {
        p::info(&format!(
            "No --wallet specified. Using: {}",
            cfg.wallets[0].name.cyan()
        ));
        &cfg.wallets[0]
    } else {
        anyhow::bail!(
            "No wallets found. Create one first:\n  starforge wallet create deployer --fund"
        );
    };

    p::kv("Wallet", &wallet.name);
    p::kv_accent("Public Key", &wallet.public_key);
    p::separator();

    let wasm_hash = compute_local_wasm_hash(&wasm_bytes);

    if args.simulate {
        p::info("Simulating deploy transaction via Soroban RPC...");
        match soroban::simulate_deploy_transaction(&wasm_hash, &args.network, wallet) {
            Ok(simulation) => {
                p::kv("Estimated Fee", &format!("{} stroops", simulation.fee));
                if !simulation.errors.is_empty() {
                    for error in &simulation.errors {
                        p::warn(&format!("Simulation error: {}", error));
                    }
                } else {
                    p::success("Simulation completed without reported RPC errors");
                }
            }
            Err(error) => {
                p::warn(&format!("Simulation failed: {}", error));
            }
        }
        p::separator();
    }

    if args.network == "mainnet" {
        p::warn("You are deploying to MAINNET. This costs real XLM.");
    }

    if !args.yes {
        println!();
        print!("  Proceed? [y/N] ");
        use std::io::BufRead;
        let line = std::io::stdin()
            .lock()
            .lines()
            .next()
            .unwrap_or(Ok(String::new()))?;
        if !matches!(line.trim().to_lowercase().as_str(), "y" | "yes") {
            p::info("Deployment cancelled.");
            return Ok(());
        }
    }

    println!();
    println!();
    let pb = p::progress_bar(3, "Starting deployment steps...");

    pb.set_message("Verifying account on-chain...");
    let account = horizon::fetch_account(&wallet.public_key, &args.network).map_err(|e| {
        pb.abandon();
        anyhow::anyhow!(
            "Account not active on {}: {}\nFund it with: starforge wallet fund {}",
            args.network,
            e,
            wallet.name
        )
    })?;

    let xlm = account
        .balances
        .iter()
        .find(|b| b.asset_type == "native")
        .map(|b| b.balance.as_str())
        .unwrap_or("0");

    pb.inc(1);
    pb.set_message("Calculating WASM SHA-256 hash...");
    pb.set_message("Recording WASM SHA-256 hash...");

    pb.inc(1);
    pb.set_message("Generating stellar CLI command...");
    pb.finish_with_message("Deployment preparation complete!");

    println!();
    p::kv_accent("XLM Balance", &format!("{} XLM", xlm));
    p::kv("WASM Hash (local SHA-256)", &wasm_hash);

    println!();
    p::separator();
    println!(
        "  {} {}",
        "✓".green().bold(),
        "Ready! Run this to complete the deployment:".bright_white()
    );
    println!();
    let deploy_cmd = build_stellar_deploy_command(&wasm_path, &wallet.public_key, &args.network);
    for line in deploy_cmd.lines() {
        println!("  {}", line.cyan());
    }
    println!();

    if args.execute {
        p::info("Executing deployment with Stellar CLI...");
        let deploy_args = build_stellar_deploy_args(&wasm_path, &wallet.public_key, &args.network);
        let output = Command::new("stellar")
            .args(&deploy_args)
            .output()
            .map_err(|e| anyhow!("Failed to execute stellar CLI: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Stellar CLI deployment failed: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        p::success("Deployment executed successfully!");
        println!("{}", stdout);
    } else {
        p::info("Dry-run complete. Use --execute to deploy for real.");
    }

    Ok(())
}
