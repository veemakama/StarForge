use crate::utils::{print as p, profiler::Profiler};
use anyhow::Result;
use clap::Args;
use colored::*;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Args)]
pub struct BenchmarkArgs {
    /// Benchmark WASM processing by reading a .wasm file and simulating operations
    #[arg(long)]
    pub wasm: Option<PathBuf>,
    /// Number of operations to simulate
    #[arg(long, default_value_t = 10_000)]
    pub operations: u64,
    /// Benchmark common CLI command paths (simulated)
    #[arg(long, default_value = "false")]
    pub cli_commands: bool,
    /// Output report format
    #[arg(long, value_parser = ["text", "json"], default_value = "text")]
    pub report: String,
}

#[derive(Debug, Serialize)]
struct BenchmarkReport {
    wasm: Option<String>,
    operations: u64,
    cli_commands: bool,
    wasm_bytes: Option<usize>,
    accumulator: Option<String>,
    phase_ms: Vec<PhaseMetric>,
    elapsed_ms: u128,
}

#[derive(Debug, Serialize)]
struct PhaseMetric {
    name: String,
    ms: u128,
}

pub fn handle(args: BenchmarkArgs) -> Result<()> {
    let mut profiler = Profiler::start();
    p::header("Benchmark");

    let mut wasm_bytes = None;
    let mut accumulator: Option<u64> = None;

    if let Some(wasm) = &args.wasm {
        if !wasm.exists() {
            anyhow::bail!("WASM file not found: {}", wasm.display());
        }
        let bytes = std::fs::read(wasm)?;
        p::kv("WASM", &wasm.display().to_string());
        p::kv("WASM bytes", &bytes.len().to_string());
        wasm_bytes = Some(bytes);
    }
    profiler.mark("wasm_load");

    if args.cli_commands {
        p::info("Benchmarking CLI hot paths (parse, env, formatting)...");
        let _ = std::env::args().collect::<Vec<_>>();
        let _ = std::env::var("HOME").ok();
        let _ = format!("starforge benchmark --operations {}", args.operations);
    }
    profiler.mark("cli_hot_paths");

    if let Some(ref bytes) = wasm_bytes {
        p::info(&format!(
            "Simulating {} operations over WASM bytes…",
            args.operations.to_string().cyan()
        ));
        let mut acc: u64 = 0;
        for i in 0..args.operations {
            let idx = (i as usize) % bytes.len().max(1);
            acc = acc.wrapping_add(bytes.get(idx).copied().unwrap_or(0) as u64);
        }
        accumulator = Some(acc);
        p::kv("Accumulator", &format!("0x{:x}", acc));
        profiler.mark("wasm_ops_loop");
    } else {
        profiler.mark("wasm_ops_loop");
    }

    let phase_points = profiler.points();
    let phase_ms: Vec<PhaseMetric> = phase_points
        .iter()
        .map(|p| PhaseMetric {
            name: p.label.clone(),
            ms: p.elapsed.as_millis(),
        })
        .collect();
    let elapsed_ms = profiler.total_elapsed().as_millis();

    if args.report == "json" {
        let report = BenchmarkReport {
            wasm: args.wasm.as_ref().map(|p| p.display().to_string()),
            operations: args.operations,
            cli_commands: args.cli_commands,
            wasm_bytes: wasm_bytes.as_ref().map(|b| b.len()),
            accumulator: accumulator.map(|v| format!("0x{:x}", v)),
            phase_ms,
            elapsed_ms,
        };
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        p::separator();
        p::kv_accent("Elapsed", &format!("{} ms", elapsed_ms));
        for phase in &phase_ms {
            p::kv(
                &format!("Phase {}", phase.name),
                &format!("{} ms", phase.ms),
            );
        }
        p::info(&format!(
            "Run Criterion benchmarks with: {}",
            "cargo bench".cyan()
        ));
        p::separator();
    }

    Ok(())
}
