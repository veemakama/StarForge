#![allow(
    dead_code,
    clippy::needless_borrows_for_generic_args,
    clippy::needless_range_loop,
    clippy::redundant_closure,
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::unnecessary_lazy_evaluations,
    clippy::needless_borrow
)]

mod commands;
pub use starforge::plugins;
pub use starforge::utils;

use clap::{Parser, Subcommand};
use colored::*;

#[derive(Parser)]
#[command(
    name = "starforge",
    about = "⚡ Stellar & Soroban developer productivity CLI",
    long_about = "starforge is an open-source CLI toolkit for developers building on the Stellar network.\nManage wallets, deploy Soroban contracts, and scaffold new projects — all from your terminal.",
    version = "0.1.0"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Suppress the ASCII banner and decorative output
    #[arg(long, short = 'q', global = true)]
    quiet: bool,

    /// Log output format: human (default) or json
    #[arg(long, global = true, default_value = "human", value_parser = ["human", "json"])]
    log_format: String,

    /// Directory to write rotating log files into (optional)
    #[arg(long, global = true)]
    log_dir: Option<std::path::PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage test wallets (create, list, fund, show, remove)
    #[command(subcommand)]
    Wallet(commands::wallet::WalletCommands),
    /// Generate Soroban project boilerplate
    #[command(subcommand)]
    New(commands::new::NewCommands),
    /// Contract operations (invoke, inspect, etc.)
    #[command(subcommand)]
    Contract(commands::contract::ContractCommands),
    /// Debug Soroban contracts with breakpoints, stepping, and inspection
    #[command(subcommand)]
    Debug(commands::debug::DebugCommands),
    /// Deep contract storage inspection (state, key, storage)
    #[command(subcommand)]
    Inspect(commands::inspect::InspectCommands),
    /// Deploy a compiled Soroban contract (.wasm)
    Deploy(commands::deploy::DeployArgs),
    /// Deployment history, rollback, verification, and dashboard
    #[command(subcommand)]
    Deployments(commands::deployments::DeploymentsCommands),
    /// Show starforge config and environment info
    Info,
    /// Manage starforge configuration (telemetry, network)
    #[command(subcommand)]
    Config(commands::config::ConfigCommands),

    /// Manage telemetry collection
    #[command(subcommand)]
    Telemetry(commands::telemetry::TelemetryCommands),

    Tx(commands::tx::TxArgs), // fetch transaction for the account

    /// View or switch the active network (testnet/mainnet)
    #[command(subcommand)]
    Network(commands::network::NetworkCommands),
    /// Local Soroban devnet (Docker quickstart)
    #[command(subcommand)]
    Node(commands::node::NodeCommands),
    /// Generate shell completions for bash, zsh, and fish
    #[command(subcommand)]
    Completions(commands::completions::CompletionShell),

    /// Interactive REPL for local Soroban contract testing
    Shell(commands::shell::ShellArgs),

    /// Live monitoring (contract events or wallet threshold)
    Monitor(commands::monitor::MonitorArgs),

    /// Interactive CLI tutorials
    #[command(subcommand)]
    Tutorial(commands::tutorial::TutorialCommands),

    /// Performance benchmarking utilities and industry-standard comparisons
    #[command(subcommand)]
    Benchmark(commands::benchmark::BenchmarkCommands),

    /// Contract testing utilities for Soroban wasm
    Test(commands::test::TestArgs),

    /// Gas analysis and optimization helpers
    #[command(subcommand)]
    Gas(commands::gas::GasCommands),

    /// Manage third-party plugins
    #[command(subcommand)]
    Plugin(commands::plugin::PluginCommands),
    /// Manage community contract templates from the marketplace
    #[command(subcommand)]
    Template(commands::template::TemplateCommands),

    /// Interact with the remote template registry
    #[command(subcommand)]
    Registry(commands::registry::RegistryCommands),

    /// Manage multi-signature transactions
    #[command(subcommand)]
    Multisig(commands::multisig_builder::MultisigCommands),

    /// Contract upgrade management (propose, approve, execute, rollback)
    #[command(subcommand)]
    Upgrade(commands::upgrade::UpgradeCommands),

    /// Contract upgrade governance (proposals, voting, timelock, audit)
    #[command(subcommand)]
    Governance(commands::governance::GovernanceCommands),

    /// Multi-contract deployment orchestration
    #[command(subcommand)]
    Orchestrate(commands::orchestrate::OrchestrateCommands),

    /// Visual pipeline builder for contract deployment workflows
    #[command(subcommand)]
    Pipeline(commands::pipeline_builder::PipelineCommands),

    /// Security hardening, validation, and monitoring
    #[command(subcommand)]
    Security(commands::security::SecurityCommands),

    /// Run a comprehensive security audit on a Soroban contract
    Audit(commands::audit::AuditArgs),

    /// Schedule deployments for future execution with approval workflows
    #[command(subcommand)]
    Schedule(commands::schedule::ScheduleCommands),

    /// Local network simulation and testing environment
    #[command(subcommand)]
    Simulate(commands::simulate::SimulateCommands),

    /// Backup and disaster recovery for contract state and code
    #[command(subcommand)]
    Backup(commands::backup::BackupCommands),

    /// Static analysis and linting for Soroban contracts
    Lint(commands::lint::LintArgs),

    /// Run connectivity diagnostics for attached Ledger/Trezor devices
    Diagnostics(commands::diagnostics::DiagnosticsArgs),

    /// Template version control (versioning, branching, changelog)
    #[command(subcommand)]
    TemplateVcs(commands::template_vcs::TemplateVcsCommands),

    /// Contract performance monitoring and metrics dashboard
    #[command(subcommand)]
    Perf(commands::perf::PerfCommands),

    /// Contract documentation portal (generate, view, search)
    #[command(subcommand)]
    Docs(commands::docs::DocsCommands),

    /// Contract deployment analytics, dashboards, and reporting
    #[command(subcommand)]
    Analytics(commands::analytics::AnalyticsCommands),

    /// Approval workflow for contract deployments (multi-level approvals, audit, compliance)
    #[command(subcommand)]
    Approval(commands::approval::ApprovalCommands),

    /// Execute an installed plugin command (e.g. `starforge defi ...`)
    #[command(external_subcommand)]
    External(Vec<String>),
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Initialise structured logging before anything else runs.
    let log_cfg =
        utils::logging::config_from_env(Some(cli.log_format.as_str()), cli.log_dir.clone());
    if let Err(e) = utils::logging::init(log_cfg) {
        eprintln!("Warning: failed to initialise logger: {}", e);
    }

    if !cli.quiet {
        print_banner();
    }

    let command_name = match &cli.command {
        Commands::Wallet(_) => "wallet",
        Commands::New(_) => "new",
        Commands::Contract(_) => "contract",
        Commands::Debug(_) => "debug",
        Commands::Inspect(_) => "inspect",
        Commands::Deploy(_) => "deploy",
        Commands::Deployments(_) => "deployments",
        Commands::Info => "info",
        Commands::Config(_) => "config",
        Commands::Telemetry(_) => "telemetry",
        Commands::Tx(_) => "tx",
        Commands::Network(_) => "network",
        Commands::Node(_) => "node",
        Commands::Completions(_) => "completions",
        Commands::Shell(_) => "shell",
        Commands::Monitor(_) => "monitor",
        Commands::Multisig(_) => "multisig",
        Commands::Tutorial(_) => "tutorial",
        Commands::Benchmark(_) => "benchmark",
        Commands::Test(_) => "test",
        Commands::Gas(_) => "gas",
        Commands::Plugin(_) => "plugin",
        Commands::Template(_) => "template",
        Commands::Registry(_) => "registry",
        Commands::Upgrade(_) => "upgrade",
        Commands::Governance(_) => "governance",
        Commands::Orchestrate(_) => "orchestrate",
        Commands::Pipeline(_) => "pipeline",
        Commands::Security(_) => "security",
        Commands::Audit(_) => "audit",
        Commands::Schedule(_) => "schedule",
        Commands::Simulate(_) => "simulate",
        Commands::Backup(_) => "backup",
        Commands::Lint(_) => "lint",
        Commands::Diagnostics(_) => "diagnostics",
        Commands::TemplateVcs(_) => "template-vcs",
        Commands::Perf(_) => "perf",
        Commands::Docs(_) => "docs",
        Commands::Analytics(_) => "analytics",
        Commands::Approval(_) => "approval",
        Commands::External(_) => "external",
    }
    .to_string();

    let start = std::time::Instant::now();
    let result = match cli.command {
        Commands::Wallet(cmd) => commands::wallet::handle(cmd).await,
        Commands::New(cmd) => commands::new::handle(cmd).await,
        Commands::Contract(cmd) => commands::contract::handle(cmd).await,
        Commands::Inspect(cmd) => commands::inspect::handle(cmd).await,
        Commands::Debug(cmd) => commands::debug::handle(cmd).await,
        Commands::Deploy(args) => commands::deploy::handle(args).await,
        Commands::Deployments(cmd) => commands::deployments::handle(cmd).await,
        Commands::Info => commands::info::handle().await,
        Commands::Config(cmd) => commands::config::handle(cmd).await,
        Commands::Telemetry(cmd) => commands::telemetry::handle(cmd).await,
        Commands::Tx(args) => commands::tx::handle(args).await,
        Commands::Network(cmd) => commands::network::handle(cmd).await,
        Commands::Node(cmd) => commands::node::handle(cmd).await,
        Commands::Completions(shell) => commands::completions::handle(shell).await,
        Commands::Shell(args) => commands::shell::handle(args).await,
        Commands::Monitor(args) => commands::monitor::handle(args).await,
        Commands::Multisig(cmd) => commands::multisig_builder::handle(cmd).await,
        Commands::Tutorial(cmd) => commands::tutorial::handle(cmd).await,
        Commands::Benchmark(args) => commands::benchmark::handle(args).await,
        Commands::Test(args) => commands::test::handle(args).await,
        Commands::Gas(args) => commands::gas::handle(args).await,
        Commands::Plugin(args) => commands::plugin::handle(args).await,
        Commands::Template(args) => commands::template::handle(args).await,
        Commands::Registry(cmd) => commands::registry::handle(cmd).await,
        Commands::Upgrade(cmd) => commands::upgrade::handle(cmd).await,
        Commands::Governance(cmd) => commands::governance::handle(cmd).await,
        Commands::Orchestrate(cmd) => commands::orchestrate::handle(cmd).await,
        Commands::Pipeline(cmd) => commands::pipeline_builder::handle(cmd).await,
        Commands::Security(cmd) => commands::security::handle(cmd).await,
        Commands::Audit(args) => commands::audit::handle(args).await,
        Commands::Schedule(cmd) => commands::schedule::handle(cmd).await,
        Commands::Simulate(cmd) => commands::simulate::handle(cmd).await,
        Commands::Backup(cmd) => commands::backup::handle(cmd).await,
        Commands::Lint(args) => commands::lint::handle(args).await,
        Commands::Diagnostics(args) => commands::diagnostics::handle(args).await,
        Commands::TemplateVcs(cmd) => commands::template_vcs::handle(cmd).await,
        Commands::Perf(cmd) => commands::perf::handle(cmd).await,
        Commands::Docs(cmd) => commands::docs::handle(cmd).await,
        Commands::Analytics(cmd) => commands::analytics::handle(cmd).await,
        Commands::Approval(cmd) => commands::approval::handle(cmd).await,
        Commands::External(args) => handle_external_plugin(args),
    };
    let duration = start.elapsed();

    let _ = utils::telemetry::track_event(
        &command_name,
        serde_json::json!({
            "success": result.is_ok(),
            "duration_ms": duration.as_millis(),
        }),
    );

    if let Err(e) = result {
        let hints = recovery_hints(&command_name, &e);
        utils::print::cli_error(&e, &hints.iter().map(String::as_str).collect::<Vec<_>>());
        std::process::exit(1);
    }
}

/// Returns command-specific recovery hints for the error sink.
///
/// Hints are chosen based on the command that failed and the error message text
/// so users get actionable next steps instead of a raw error dump.
fn recovery_hints(command: &str, err: &anyhow::Error) -> Vec<String> {
    let msg = err.to_string().to_lowercase();
    let mut hints: Vec<String> = Vec::new();

    match command {
        "wallet" => {
            if msg.contains("not found") || msg.contains("no wallet") {
                hints.push("Create a wallet first: starforge wallet create <name>".into());
                hints.push("List existing wallets: starforge wallet list".into());
            } else if msg.contains("password") || msg.contains("decrypt") {
                hints.push("Re-enter the password you used when creating the wallet.".into());
                hints.push("If you forgot it, remove the wallet and create a new one: starforge wallet remove <name>".into());
            } else if msg.contains("fund") || msg.contains("friendbot") {
                hints.push("Fund a testnet wallet: starforge wallet fund <name>".into());
                hints.push("Friendbot is only available on testnet — switch networks: starforge network switch testnet".into());
            } else if msg.contains("already exists") {
                hints.push("Use a different wallet name, or remove the existing one first.".into());
                hints.push("List wallets: starforge wallet list".into());
            }
        }
        "deploy" => {
            if msg.contains("wasm") || msg.contains("not found") || msg.contains("no such file") {
                hints.push("Build your contract first: stellar contract build".into());
                hints.push("Make sure you pass the correct --wasm path to deploy.".into());
            } else if msg.contains("account") || msg.contains("not found on") {
                hints.push("Fund your account before deploying: starforge wallet fund <name>".into());
                hints.push("Check the active network: starforge network show".into());
            } else if msg.contains("network") {
                hints.push("Check available networks: starforge network show".into());
                hints.push("Switch to testnet for free deployments: starforge network switch testnet".into());
            }
        }
        "contract" => {
            if msg.contains("no wallet") || msg.contains("wallet not found") {
                hints.push("Create a wallet first: starforge wallet create deployer --fund".into());
            } else if msg.contains("contract id") || msg.contains("invalid contract") {
                hints.push("Contract IDs start with 'C' and are exactly 56 characters long.".into());
                hints.push("Find your contract ID in the deploy output or: starforge contract list".into());
            } else if msg.contains("invoke") || msg.contains("simulate") {
                hints.push("Run `stellar contract build` to ensure the contract is up to date.".into());
                hints.push("Check function name and argument types match the contract ABI.".into());
            }
        }
        "tx" => {
            if msg.contains("account not found") || msg.contains("not active") {
                hints.push("Fund your account first: starforge wallet fund <name>".into());
                hints.push("Verify you are on the right network: starforge network show".into());
            } else if msg.contains("insufficient") {
                hints.push("Check your XLM balance: starforge wallet show <name>".into());
                hints.push("Fund the account: starforge wallet fund <name>".into());
            } else if msg.contains("asset") {
                hints.push("Asset format is CODE:ISSUER (e.g. USDC:GA5ZS...) or XLM for native.".into());
            }
        }
        "network" => {
            if msg.contains("unsupported") || msg.contains("not found") {
                hints.push("List configured networks: starforge network show".into());
                hints.push("Add a custom network: starforge network add <name> --horizon <url>".into());
                hints.push("Valid built-in networks: testnet, mainnet, docker-testnet".into());
            }
        }
        "node" => {
            if msg.contains("docker") || msg.contains("not found") || msg.contains("command") {
                hints.push("Install Docker Desktop from https://www.docker.com/products/docker-desktop".into());
                hints.push("Ensure the Docker daemon is running before retrying.".into());
            }
        }
        "config" => {
            if msg.contains("parse") || msg.contains("toml") || msg.contains("json") {
                hints.push("Your config file may be corrupted. Inspect it at: ~/.config/starforge/config.toml".into());
                hints.push("Run `starforge config doctor` to diagnose configuration issues.".into());
            }
        }
        "plugin" => {
            if msg.contains("not found") || msg.contains("load") {
                hints.push("Re-install the plugin: starforge plugin install <name> --path <lib>".into());
                hints.push("List installed plugins: starforge plugin list".into());
            } else if msg.contains("untrusted") || msg.contains("trust") {
                hints.push("Review the plugin source and mark it trusted: starforge plugin trust <name>".into());
            }
        }
        "template" => {
            if msg.contains("not found") || msg.contains("fetch") {
                hints.push("List available templates: starforge template search".into());
                hints.push("Check your internet connection and retry.".into());
            }
        }
        "benchmark" | "test" => {
            if msg.contains("wasm") || msg.contains("not found") {
                hints.push("Build your contract first: stellar contract build".into());
                hints.push("Pass the correct --wasm path to the command.".into());
            }
        }
        _ => {}
    }

    // Generic fallbacks always appended when nothing command-specific matched
    if hints.is_empty() {
        if msg.contains("permission denied") || msg.contains("access denied") {
            hints.push("Check file and directory permissions.".into());
        } else if msg.contains("connection") || msg.contains("network") || msg.contains("timeout") {
            hints.push("Check your internet connection and try again.".into());
            hints.push("If behind a proxy, set the HTTPS_PROXY environment variable.".into());
        } else if msg.contains("config") {
            hints.push("Run `starforge config doctor` to diagnose configuration issues.".into());
        }
        // If still nothing, the cli_error fn will print the generic fallback.
    }

    hints
}

fn handle_external_plugin(args: Vec<String>) -> anyhow::Result<()> {
    use anyhow::Context;
    use plugins::registry::TrustLevel;

    if args.is_empty() {
        anyhow::bail!("No plugin command provided");
    }

    let plugin_name = &args[0];
    let plugin_args = &args[1..];

    let cfg = starforge::utils::config::load()?;
    let reg = plugins::registry::load_registry().unwrap_or_default();
    if reg.plugins.is_empty() {
        anyhow::bail!(
            "Unknown command '{}'. No plugins installed.\n\nTry: starforge plugin install <name> --path <lib>",
            plugin_name
        );
    }

    // Check if the command matches any registered plugin command before loading .so files.
    let all_commands = plugins::registry::load_all_registered_commands();
    let known = all_commands.iter().any(|c| c.name == *plugin_name);
    if !known {
        let available: Vec<String> = all_commands
            .iter()
            .map(|c| format!("  • {}", c.name))
            .collect();
        let hint = if available.is_empty() {
            "No plugin commands registered. Re-install plugins to discover their commands."
                .to_string()
        } else {
            format!("Available plugin commands:\n{}", available.join("\n"))
        };
        anyhow::bail!("Unknown command '{}'.\n\n{}", plugin_name, hint);
    }

    // Warn about unknown-trust plugins before loading.
    for pl in reg.plugins.iter().filter(|p| {
        plugins::registry::classify_source(&p.source) == TrustLevel::Unknown && !p.source.is_empty()
    }) {
        eprintln!(
            "  ⚠  Warning: plugin '{}' is from an untrusted source: {}",
            pl.name, pl.source
        );
    }

    let mut pm = plugins::PluginManager::new();
    for pl in &reg.plugins {
        unsafe {
            pm.load_plugin(&pl.path)
                .with_context(|| format!("Failed to load plugin '{}' from {}", pl.name, pl.path))?;
        }
    }

    pm.execute(plugin_name, plugin_args)
        .map_err(|e| anyhow::anyhow!(e))
}

fn print_banner() {
    println!(
        "{}",
        "\n  ███████╗████████╗ █████╗ ██████╗ ███████╗ ██████╗ ██████╗  ██████╗ ███████╗\n  ██╔════╝╚══██╔══╝██╔══██╗██╔══██╗██╔════╝██╔═══██╗██╔══██╗██╔════╝ ██╔════╝\n  ███████╗   ██║   ███████║██████╔╝█████╗  ██║   ██║██████╔╝██║  ███╗█████╗  \n  ╚════██║   ██║   ██╔══██║██╔══██╗██╔══╝  ██║   ██║██╔══██╗██║   ██║██╔══╝  \n  ███████║   ██║   ██║  ██║██║  ██║██║     ╚██████╔╝██║  ██║╚██████╔╝███████╗\n  ╚══════╝   ╚═╝   ╚═╝  ╚═╝╚═╝  ╚═╝╚═╝      ╚═════╝ ╚═╝  ╚═╝ ╚═════╝ ╚══════╝\n"
        .cyan().bold()
    );
    println!(
        "  {} {}\n",
        "⚡ Stellar & Soroban Developer CLI".bright_white(),
        "v0.1.0".dimmed()
    );
}
