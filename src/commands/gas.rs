use crate::utils::{config, optimizer, print as p, profiler};
use anyhow::Result;
use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum GasCommands {
    /// Analyze a compiled Soroban contract for gas/cpu opportunities
    Analyze {
        /// Path to the compiled wasm
        wasm: PathBuf,
        /// Network context (used for fee heuristics)
        #[arg(long)]
        network: Option<String>,
    },
    /// Emit an "optimized" wasm (lightweight, heuristic-based)
    Optimize {
        /// Path to the input wasm
        #[arg(long)]
        target: PathBuf,
        /// Output path for optimized wasm
        #[arg(long)]
        output: PathBuf,
    },
    /// Compare two wasm builds and diff estimated simulation costs
    Diff {
        /// Path to the baseline wasm
        old_wasm: PathBuf,
        /// Path to the candidate wasm
        new_wasm: PathBuf,
    },
}

pub fn handle(cmd: GasCommands) -> Result<()> {
    match cmd {
        GasCommands::Analyze { wasm, network } => analyze(wasm, network),
        GasCommands::Optimize { target, output } => optimize(target, output),
        GasCommands::Diff { old_wasm, new_wasm } => diff(old_wasm, new_wasm),
    }
}

fn analyze(wasm: PathBuf, network: Option<String>) -> Result<()> {
    config::validate_file_path(&wasm, Some("wasm"))?;

    let cfg = config::load()?;
    let network = network.unwrap_or(cfg.network);
    config::validate_network(&network)?;

    p::header("Gas Analyzer");
    p::kv("Network", &network);
    p::kv("Wasm", &wasm.display().to_string());

    let t = profiler::Timer::start();
    let report = optimizer::analyze_wasm(&wasm)?;
    let elapsed = t.elapsed();

    println!();
    p::separator();
    p::kv_accent("Size (bytes)", &report.size_bytes.to_string());
    p::kv("SHA256", &report.sha256);
    p::kv("Heuristic score", &report.score.to_string());
    if !report.suggestions.is_empty() {
        println!();
        p::info("Suggestions:");
        for s in &report.suggestions {
            println!("  - {}", s);
        }
    }
    p::separator();
    p::kv("Duration", &format!("{:?}", elapsed));
    Ok(())
}

fn optimize(target: PathBuf, output: PathBuf) -> Result<()> {
    config::validate_file_path(&target, Some("wasm"))?;

    p::header("Gas Optimizer");
    p::kv("Input", &target.display().to_string());
    p::kv("Output", &output.display().to_string());

    let t = profiler::Timer::start();
    let result = optimizer::optimize_wasm(&target, &output)?;
    let elapsed = t.elapsed();

    println!();
    p::success("Optimization output written");
    p::kv("Bytes in", &result.input_size_bytes.to_string());
    p::kv("Bytes out", &result.output_size_bytes.to_string());
    p::kv("Duration", &format!("{:?}", elapsed));
    Ok(())
}

fn diff(old_wasm: PathBuf, new_wasm: PathBuf) -> Result<()> {
    config::validate_file_path(&old_wasm, Some("wasm"))?;
    config::validate_file_path(&new_wasm, Some("wasm"))?;

    p::header("Gas Diff");
    p::kv("Old wasm", &old_wasm.display().to_string());
    p::kv("New wasm", &new_wasm.display().to_string());

    let mut profile = profiler::Profiler::start();
    let old_report = optimizer::analyze_wasm(&old_wasm)?;
    profile.mark("analyze_old");
    let new_report = optimizer::analyze_wasm(&new_wasm)?;
    profile.mark("analyze_new");

    let old_est = estimate_simulation_cost(old_report.size_bytes);
    let new_est = estimate_simulation_cost(new_report.size_bytes);
    let delta = new_est as i64 - old_est as i64;
    let pct = if old_est == 0 {
        0.0
    } else {
        (delta as f64 / old_est as f64) * 100.0
    };

    println!();
    p::separator();
    p::kv("Old size (bytes)", &old_report.size_bytes.to_string());
    p::kv("New size (bytes)", &new_report.size_bytes.to_string());
    p::kv("Old est. sim cost", &old_est.to_string());
    p::kv("New est. sim cost", &new_est.to_string());
    p::kv(
        "Estimated delta",
        &format!(
            "{} ({:+.2}%)",
            if delta >= 0 {
                format!("+{}", delta)
            } else {
                delta.to_string()
            },
            pct
        ),
    );
    p::kv(
        "Result",
        if delta < 0 {
            "Improved (lower estimated cost)"
        } else if delta > 0 {
            "Regressed (higher estimated cost)"
        } else {
            "No change"
        },
    );
    for point in profile.points() {
        p::kv(
            &format!("Step {}", point.label),
            &format!("{:?}", point.elapsed),
        );
    }
    p::kv("Total profile", &format!("{:?}", profile.total_elapsed()));
    p::separator();

    Ok(())
}

fn estimate_simulation_cost(size_bytes: usize) -> u64 {
    2_000 + (size_bytes as u64 / 8)
}
