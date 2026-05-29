use crate::utils::{config, horizon, optimizer, print as p, info, soroban};
use crate::commands::info;
use crate::utils::{config, horizon, optimizer, print as p, soroban};
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
        "âœ“".green().bold(),
        "Ready! Run this to complete the deployment:".bright_white()
    );
    println!();
    let deploy_cmd = build_stellar_deploy_command(&wasm_path, &wallet.public_key, &args.network);
    for line in deploy_cmd.lines() {
        println!("  {}", line.cyan());
    }
    println!();
    if args.execute {
        let stellar_path = info::detect_stellar_cli().ok_or_else(|| {
            anyhow::anyhow!(
                "Cannot execute deploy: Stellar CLI not found on PATH.\nInstall it from https://developers.stellar.org/docs/tools/stellar-cli"
            )
        })?;

        p::info(&format!(
            "Executing with Stellar CLI at {}",
            stellar_path.display()
        ));
        let cmd_args = build_stellar_deploy_args(&wasm_path, &wallet.public_key, &args.network);
        let output = Command::new(stellar_path).args(&cmd_args).output()?;
        if output.status.success() {
            p::success("Deployment command executed successfully.");
            let stdout = String::from_utf8_lossy(&output.stdout);
            if !stdout.trim().is_empty() {
                println!("{}", stdout.trim());
            }
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "Stellar CLI deployment failed (exit: {}). {}",
                output.status,
                stderr.trim()
            );
        }
    } else {
        p::info("Dry-run mode (default): command not executed. Use --execute to run it.");
    }
    p::info("Install the Stellar CLI: https://developers.stellar.org/docs/tools/stellar-cli");
    p::separator();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    // ---------------------------------------------------------------------------
    // SHA-256 hash tests
    // ---------------------------------------------------------------------------

    /// The output must always be a 64-character lowercase hex string (256 bits).
    #[test]
    fn sha256_output_is_64_hex_chars() {
        let hash = compute_local_wasm_hash(b"hello-starforge");
        assert_eq!(hash.len(), 64, "SHA-256 hex digest must be 64 characters");
        assert!(
            hash.chars().all(|c| c.is_ascii_hexdigit()),
            "digest must be lowercase hex"
        );
    }

    /// Same bytes → same digest (deterministic).
    #[test]
    fn sha256_is_deterministic() {
        let bytes = b"hello-starforge";
        assert_eq!(
            compute_local_wasm_hash(bytes),
            compute_local_wasm_hash(bytes)
        );
    }

    /// Different bytes → different digest (collision-resistance sanity check).
    #[test]
    fn sha256_differs_for_different_inputs() {
        assert_ne!(
            compute_local_wasm_hash(b"abc"),
            compute_local_wasm_hash(b"abd")
        );
    }

    /// Known-answer test: SHA-256("abc") == the FIPS 180-4 test vector.
    ///
    /// Expected value verified against `echo -n abc | sha256sum` and the
    /// NIST FIPS 180-4 published test vector.
    #[test]
    fn sha256_known_answer_abc() {
        let hash = compute_local_wasm_hash(b"abc");
        assert_eq!(
            hash,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad" // SHA-256("abc") as computed by sha2 0.10 / FIPS 180-4.
        );
    }

    /// Known-answer test against `tests/fixtures/minimal.wasm`.
    ///
    /// Expected value: `sha256sum tests/fixtures/minimal.wasm`
    ///   → 93a44bbb96c751218e4c00d479e4c14358122a389acca16205b1e4d0dc5f9476
    #[test]
    fn sha256_minimal_wasm_fixture() {
        let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("minimal.wasm");
        let wasm_bytes = fs::read(&fixture_path).expect("failed to read minimal.wasm fixture");
        let hash = compute_local_wasm_hash(&wasm_bytes);
        assert_eq!(
            hash,
            "93a44bbb96c751218e4c00d479e4c14358122a389acca16205b1e4d0dc5f9476"
        );
        assert_eq!(hash.len(), 64);
    }

    /// Hashing an empty slice must not panic and must equal the well-known
    /// SHA-256 digest of the empty string.
    #[test]
    fn sha256_empty_input() {
        let hash = compute_local_wasm_hash(b"");
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    /// Round-trip via a real (temporary) file to confirm fs::read → SHA-256
    /// produces a 64-char hex string.
    #[test]
    fn sha256_real_file_round_trip() {
        let dir = tempdir().expect("failed to create temp dir");
        let wasm_path = dir.path().join("token.wasm");
        let wasm_magic: &[u8] = &[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
        fs::write(&wasm_path, wasm_magic).expect("failed to write wasm");
        let bytes = fs::read(&wasm_path).expect("failed to read wasm");

        let hash = compute_local_wasm_hash(&bytes);
        assert_eq!(hash.len(), 64);
        assert_eq!(
            hash,
            "93a44bbb96c751218e4c00d479e4c14358122a389acca16205b1e4d0dc5f9476"
        );
    }

    // ---------------------------------------------------------------------------
    // Unchanged helper tests
    // ---------------------------------------------------------------------------

    #[test]
    fn builds_expected_deploy_command() {
        let command = build_stellar_deploy_command(
            std::path::Path::new("target/release/token.wasm"),
            "GABCDEF1234567890",
            "testnet",
        );

        assert!(command.contains("stellar contract deploy"));
        assert!(command.contains("--wasm target/release/token.wasm"));
        assert!(command.contains("--source GABCDEF1234567890"));
        assert!(command.contains("--network testnet"));
    }

    #[test]
    fn builds_expected_deploy_args() {
        let args = build_stellar_deploy_args(
            std::path::Path::new("target/release/token.wasm"),
            "GABCDEF1234567890",
            "testnet",
        );
        assert_eq!(
            args,
            vec![
                "contract",
                "deploy",
                "--wasm",
                "target/release/token.wasm",
                "--source",
                "GABCDEF1234567890",
                "--network",
                "testnet"
            ]
        );
    }

    #[test]
    fn flags_large_wasm_sizes() {
        assert!(!is_wasm_above_size_limit(127.9));
        assert!(!is_wasm_above_size_limit(128.0));
        assert!(is_wasm_above_size_limit(128.1));
    }

    #[test]
    fn deploy_args_support_simulate_flag() {
        let args = DeployArgs {
            wasm: PathBuf::from("contract.wasm"),
            network: "testnet".to_string(),
            wallet: None,
            optimize: false,
            yes: false,
            execute: false,
            simulate: true,
        };
        assert!(args.simulate);
    }
}
