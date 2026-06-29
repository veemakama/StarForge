use crate::utils::{config, print as p, repl, sandbox::LocalSorobanSandbox};
use anyhow::Result;
use clap::Args;
use std::collections::HashSet;
use std::fs;

#[derive(Args)]
pub struct ShellArgs {
    /// Path to the compiled contract .wasm (local sandbox execution)
    #[arg(long)]
    pub contract: String,
    /// Network to use (docker-testnet runs against local Docker Soroban sandbox)
    #[arg(long, default_value = "testnet")]
    pub network: String,
    /// Disable persistent command history
    #[arg(long, default_value = "false")]
    pub no_history: bool,
    /// Maximum number of commands stored in history
    #[arg(long, default_value_t = 1000)]
    pub history_max_lines: usize,
}

pub async fn handle(args: ShellArgs) -> Result<()> {
    p::header("Interactive Contract Shell");
    p::separator();
    p::kv("Contract WASM", &args.contract);
    p::kv("Network", &args.network);
    p::separator();
    println!();

    let sandbox = LocalSorobanSandbox::new(&args.contract, &args.network).await?;
    let runner = ShellRunner { sandbox };
    let repl_options = repl::ReplOptions {
        history_enabled: !args.no_history,
        max_history_lines: args.history_max_lines,
        completion_candidates: completion_candidates(),
        ..Default::default()
    };
    repl::Repl::with_options(runner, repl_options).run()
}

struct ShellRunner {
    sandbox: LocalSorobanSandbox,
}

impl repl::ReplRunner for ShellRunner {
    fn run_invocation(&mut self, function: &str, args: &[String]) -> Result<String> {
        self.sandbox.invoke(function, args)
    }
}

fn completion_candidates() -> Vec<String> {
    let mut candidates = HashSet::new();

    if let Ok(cfg) = config::load() {
        for wallet in cfg.wallets {
            candidates.insert(wallet.name);
        }
    }

    if let Ok(entries) = fs::read_dir(config::config_dir()) {
        for entry in entries.flatten() {
            if entry.file_type().map(|ty| ty.is_file()).unwrap_or(false) {
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    collect_contract_ids(&content, &mut candidates);
                }
            }
        }
    }

    let mut candidates = candidates.into_iter().collect::<Vec<_>>();
    candidates.sort();
    candidates
}

fn collect_contract_ids(content: &str, candidates: &mut HashSet<String>) {
    for token in content.split(|ch: char| !ch.is_ascii_alphanumeric()) {
        if token.len() == 56
            && token.starts_with('C')
            && token.chars().all(|ch| matches!(ch, 'A'..='Z' | '2'..='7'))
        {
            candidates.insert(token.to_string());
        }
    }
}
