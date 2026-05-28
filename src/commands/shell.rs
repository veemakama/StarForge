use crate::utils::{print as p, repl, sandbox::LocalSorobanSandbox};
use anyhow::Result;
use clap::Args;

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

pub fn handle(args: ShellArgs) -> Result<()> {
    p::header("Interactive Contract Shell");
    p::separator();
    p::kv("Contract WASM", &args.contract);
    p::kv("Network", &args.network);
    p::separator();
    println!();

    let sandbox = LocalSorobanSandbox::new(&args.contract, &args.network)?;
    let runner = ShellRunner { sandbox };
    let mut repl_options = repl::ReplOptions::default();
    repl_options.history_enabled = !args.no_history;
    repl_options.max_history_lines = args.history_max_lines;
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
