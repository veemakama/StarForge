use crate::utils::{config, print as p, test_runner};
use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

#[derive(Args)]
pub struct TestArgs {
    /// Path to the compiled wasm
    #[arg(long)]
    pub wasm: PathBuf,

    /// Collect a lightweight coverage report (heuristic)
    #[arg(long, default_value = "false")]
    pub coverage: bool,

    /// Output report format (e.g. html, json)
    #[arg(long)]
    pub report: Option<String>,
}

pub fn handle(args: TestArgs) -> Result<()> {
    config::validate_file_path(&args.wasm, Some("wasm"))?;

    p::header("Contract Test Runner");
    p::kv("Wasm", &args.wasm.display().to_string());
    p::kv("Coverage", if args.coverage { "yes" } else { "no" });
    if let Some(r) = &args.report {
        p::kv("Report", r);
    }

    let result = test_runner::run_contract_tests(
        &args.wasm,
        test_runner::TestOptions {
            coverage: args.coverage,
            report_format: args.report.clone(),
        },
    )?;

    println!();
    p::separator();
    p::kv_accent("SHA256", &result.sha256);
    p::kv("Wasm bytes", &result.size_bytes.to_string());
    p::kv("Cases executed", &result.cases_executed.to_string());
    p::kv("Failures", &result.failures.to_string());
    if let Some(path) = &result.report_path {
        p::kv("Report path", &path.display().to_string());
    }
    p::separator();

    if result.failures > 0 {
        anyhow::bail!("Some contract tests failed");
    }

    p::success("All contract tests passed");
    Ok(())
}
