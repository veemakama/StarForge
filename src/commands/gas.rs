use crate::utils::{
    config, cost_estimation as ce, optimizer, print as p, profiler,
};
use anyhow::Result;
use clap::Subcommand;
use comfy_table::{modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL, Attribute, Cell, Color, Table};
use std::path::PathBuf;

// ── Subcommand tree ───────────────────────────────────────────────────────────

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
    /// Estimate the full deployment cost (gas + storage fees) for a wasm
    Estimate {
        /// Path to the compiled wasm
        wasm: PathBuf,
        /// Target network for fee heuristics
        #[arg(long, default_value = "testnet")]
        network: String,
        /// Alert threshold in stroops — prints a warning when the estimate
        /// exceeds this value and saves the alert for future runs
        #[arg(long)]
        alert_threshold: Option<u64>,
        /// Save this estimate to cost history
        #[arg(long, default_value = "true")]
        save: bool,
    },
    /// Show cost estimation history
    History {
        /// Filter by network (omit for all networks)
        #[arg(long)]
        network: Option<String>,
        /// Maximum number of entries to display (most recent first)
        #[arg(long, default_value = "10")]
        limit: usize,
    },
    /// Manage cost alert thresholds
    Alerts {
        #[command(subcommand)]
        action: AlertsAction,
    },
}

#[derive(Subcommand)]
pub enum AlertsAction {
    /// List all configured alert rules
    List,
    /// Set a new alert threshold for a network
    Set {
        /// Network to alert on (`testnet`, `mainnet`, or `*` for all)
        #[arg(long, default_value = "testnet")]
        network: String,
        /// Maximum acceptable fee in stroops
        #[arg(long)]
        threshold: u64,
        /// Optional human-readable label for this rule
        #[arg(long)]
        label: Option<String>,
    },
    /// Remove alert rules for a network (use `*` to clear all)
    Clear {
        /// Network whose alerts to clear, or `*` for all
        #[arg(long, default_value = "*")]
        network: String,
    },
}

#[derive(Args)]
pub struct HistoryArgs {
    /// Filter by contract label
    #[arg(long)]
    pub label: Option<String>,
    /// Maximum number of reports to show
    #[arg(long, default_value = "20")]
    pub limit: usize,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct ShowArgs {
    /// Report ID (prefix is fine)
    pub id: String,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

// ── Legacy diff output (kept for backward compat) ─────────────────────────────
#[derive(Debug, Serialize)]
struct LegacyDiffOutput {
    old_size_bytes: usize,
    new_size_bytes: usize,
    old_est_sim_cost: u64,
    new_est_sim_cost: u64,
    delta: i64,
    delta_pct: f64,
    result: &'static str,
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

pub async fn handle(cmd: GasCommands) -> Result<()> {
    match cmd {
        GasCommands::Analyze { wasm, network } => analyze(wasm, network),
        GasCommands::Optimize { target, output } => optimize(target, output),
        GasCommands::Diff { old_wasm, new_wasm } => diff(old_wasm, new_wasm),
        GasCommands::Estimate {
            wasm,
            network,
            alert_threshold,
            save,
        } => estimate(wasm, network, alert_threshold, save),
        GasCommands::History { network, limit } => history(network, limit),
        GasCommands::Alerts { action } => alerts(action),
    }
}

// ── helpers ────────────────────────────────────────────────────────────────

fn base_table() -> Table {
    let mut t = Table::new();
    t.load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS);
    t
}

fn header_cell(text: &str) -> Cell {
    Cell::new(text)
        .add_attribute(Attribute::Bold)
        .fg(Color::Cyan)
}

fn value_cell(text: &str) -> Cell {
    Cell::new(text)
}

fn good_cell(text: &str) -> Cell {
    Cell::new(text).fg(Color::Green)
}

fn warn_cell(text: &str) -> Cell {
    Cell::new(text).fg(Color::Yellow)
}

fn bad_cell(text: &str) -> Cell {
    Cell::new(text).fg(Color::Red)
}

fn estimate_simulation_cost(size_bytes: usize) -> u64 {
    2_000 + (size_bytes as u64 / 8)
}

// ── subcommands ────────────────────────────────────────────────────────────

fn analyze(wasm: PathBuf, network: Option<String>) -> Result<()> {
    config::validate_file_path(&wasm, Some("wasm"))?;

    let cfg = config::load()?;
    let network = args.network.unwrap_or(cfg.network);
    config::validate_network(&network)?;

    p::header("Gas & Compute Visualizer — Analyze");
    p::kv("Network", &network);
    p::kv("WASM", &args.wasm.display().to_string());

    let timer = profiler::Timer::start();
    let report = ga::analyze_wasm_file(&args.wasm, args.label.as_deref())?;
    let elapsed = timer.elapsed();

    let est_cost = estimate_simulation_cost(report.size_bytes);

    // ── Cost breakdown table ──────────────────────────────────────────────
    println!();
    let mut table = base_table();
    table.set_header(vec![
        header_cell("Metric"),
        header_cell("Value"),
    ]);
    table.add_row(vec![
        value_cell("WASM size (bytes)"),
        value_cell(&report.size_bytes.to_string()),
    ]);
    table.add_row(vec![
        value_cell("WASM size (KB)"),
        value_cell(&format!("{:.2} KB", report.size_bytes as f64 / 1024.0)),
    ]);
    table.add_row(vec![
        value_cell("SHA-256"),
        value_cell(&report.sha256),
    ]);
    table.add_row(vec![
        value_cell("Heuristic score"),
        if report.score >= 80 {
            good_cell(&report.score.to_string())
        } else if report.score >= 50 {
            warn_cell(&report.score.to_string())
        } else {
            bad_cell(&report.score.to_string())
        },
    ]);
    table.add_row(vec![
        value_cell("Est. simulation cost (stroops)"),
        value_cell(&est_cost.to_string()),
    ]);
    table.add_row(vec![
        value_cell("Est. ledger footprint reads"),
        value_cell(&format!("{}", report.size_bytes / 4096 + 1)),
    ]);
    table.add_row(vec![
        value_cell("Est. auth cost (stroops)"),
        value_cell(&format!("{}", est_cost / 10)),
    ]);
    table.add_row(vec![
        value_cell("Analysis duration"),
        value_cell(&format!("{:?}", elapsed)),
    ]);
    println!("{table}");

    // ── Suggestions ───────────────────────────────────────────────────────
    p::separator();

    // Header metrics
    p::kv_accent("Contract", &report.contract_label);
    p::kv("SHA-256", &format!("{}…", &report.wasm_sha256[..16]));
    p::kv(
        "Size",
        &format!(
            "{:.1} KB  ({:.1}% of 128 KB limit)",
            report.size_bytes as f64 / 1024.0,
            report.size_limit_pct
        ),
    );

    // Score
    let score_str = format!("{}/100", report.optimization_score);
    let score_colored = if report.optimization_score >= 80 {
        score_str.green().bold().to_string()
    } else if report.optimization_score >= 50 {
        score_str.yellow().bold().to_string()
    } else {
        score_str.red().bold().to_string()
    };
    p::kv_accent("Optimization score", &score_colored);

    // Section profile
    println!();
    p::info("Section Profile");
    let sp = &report.section_profile;
    p::kv("Functions (local)", &sp.function_count.to_string());
    p::kv("Imports", &sp.import_count.to_string());
    p::kv("Exports", &sp.export_count.to_string());
    p::kv("Globals", &sp.global_count.to_string());
    p::kv("Data segments", &sp.data_segment_count.to_string());
    p::kv("Code section", &format!("{:.1} KB", sp.code_section_bytes as f64 / 1024.0));
    p::kv("Custom sections", &format!("{:.1} KB", sp.custom_section_bytes as f64 / 1024.0));
    p::kv("Est. instructions", &report.section_profile.estimated_instruction_count.to_string());
    p::kv("Debug symbols", if sp.has_debug_section || sp.has_name_section { "yes (strip recommended)" } else { "no" });

    // Gas cost breakdown
    println!();
    p::info("Estimated Gas Cost Breakdown");
    let gc = &report.gas_cost;
    p::kv("Upload cost", &format!("{:>10} gas", gc.upload_cost));
    p::kv("CPU (execution)", &format!("{:>10} gas", gc.cpu_cost));
    p::kv("Imports overhead", &format!("{:>10} gas", gc.import_cost));
    p::kv("Exports overhead", &format!("{:>10} gas", gc.export_cost));
    p::kv("Globals overhead", &format!("{:>10} gas", gc.global_cost));
    p::kv("Data segments", &format!("{:>10} gas", gc.data_cost));
    p::kv_accent("Total estimated", &format!("{:>10} gas", gc.total));
    p::kv("Cost / KB", &format!("{:.0} gas/KB", gc.cost_per_kb));

    // Findings
    if report.findings.is_empty() {
        println!();
        p::success("No gas issues found — WASM is well-optimized.");
    } else {
        println!();
        p::info(&format!(
            "Findings ({} critical, {} high, {} medium)",
            report.critical_count(),
            report.high_count(),
            report.medium_count()
        ));
        println!();
        p::info("Optimization suggestions:");
        let mut stbl = base_table();
        stbl.set_header(vec![header_cell("#"), header_cell("Suggestion")]);
        for (i, s) in report.suggestions.iter().enumerate() {
            stbl.add_row(vec![
                warn_cell(&(i + 1).to_string()),
                value_cell(s),
            ]);
        }
        println!("{stbl}");
    } else {
        println!();
        p::success("No optimization suggestions — contract looks lean.");
    }

    Ok(())
}

// ── optimize ──────────────────────────────────────────────────────────────────

fn optimize(args: OptimizeArgs) -> Result<()> {
    config::validate_file_path(&args.target, Some("wasm"))?;

    p::header("Gas & Compute Visualizer — Optimize");
    p::kv("Input", &target.display().to_string());
    p::kv("Output", &output.display().to_string());

    let timer = profiler::Timer::start();
    let result = optimizer::optimize_wasm(&args.target, &args.output)?;
    let elapsed = timer.elapsed();

    let old_cost = estimate_simulation_cost(result.input_size_bytes);
    let new_cost = estimate_simulation_cost(result.output_size_bytes);
    let cost_delta = new_cost as i64 - old_cost as i64;

    println!();
    let mut table = base_table();
    table.set_header(vec![
        header_cell("Metric"),
        header_cell("Before"),
        header_cell("After"),
        header_cell("Delta"),
    ]);
    table.add_row(vec![
        value_cell("Size (bytes)"),
        value_cell(&result.input_size_bytes.to_string()),
        value_cell(&result.output_size_bytes.to_string()),
        if result.reduction_bytes() > 0 {
            good_cell(&format!("-{} bytes", result.reduction_bytes()))
        } else {
            warn_cell("0 bytes")
        },
    ]);
    table.add_row(vec![
        value_cell("Size (KB)"),
        value_cell(&format!("{:.2}", result.input_size_bytes as f64 / 1024.0)),
        value_cell(&format!("{:.2}", result.output_size_bytes as f64 / 1024.0)),
        good_cell(&format!("{:+.2}%", result.reduction_percent())),
    ]);
    table.add_row(vec![
        value_cell("Est. sim cost (stroops)"),
        value_cell(&old_cost.to_string()),
        value_cell(&new_cost.to_string()),
        if cost_delta < 0 {
            good_cell(&format!("{:+}", cost_delta))
        } else {
            warn_cell(&format!("{:+}", cost_delta))
        },
    ]);
    table.add_row(vec![
        value_cell("Optimizer"),
        value_cell(&result.tool),
        value_cell("—"),
        value_cell("—"),
    ]);
    table.add_row(vec![
        value_cell("Duration"),
        value_cell(&format!("{:?}", elapsed)),
        value_cell("—"),
        value_cell("—"),
    ]);
    println!("{table}");

    println!();
    p::success("Optimization complete — output written successfully.");

    Ok(())
}

// ── New subcommand handlers ───────────────────────────────────────────────────

    p::header("Gas & Compute Visualizer — Diff");
    p::kv("Baseline", &old_wasm.display().to_string());
    p::kv("Candidate", &new_wasm.display().to_string());

    let mut profile = profiler::Profiler::start();
    let old_report = optimizer::analyze_wasm(&old_wasm)?;
    profile.mark("analyze_old");
    let new_report = optimizer::analyze_wasm(&new_wasm)?;
    profile.mark("analyze_new");

    let old_cost = estimate_simulation_cost(old_report.size_bytes);
    let new_cost = estimate_simulation_cost(new_report.size_bytes);
    let cost_delta = new_cost as i64 - old_cost as i64;
    let cost_pct = if old_cost == 0 {
        0.0
    } else {
        (cost_delta as f64 / old_cost as f64) * 100.0
    };

    let size_delta = new_report.size_bytes as i64 - old_report.size_bytes as i64;
    let size_pct = if old_report.size_bytes == 0 {
        0.0
    } else {
        (size_delta as f64 / old_report.size_bytes as f64) * 100.0
    };
    let comparison = optimizer::compare_gas_reports(&old_report, &new_report);

    let old_auth = old_cost / 10;
    let new_auth = new_cost / 10;
    let auth_delta = new_auth as i64 - old_auth as i64;

    let old_reads = old_report.size_bytes / 4096 + 1;
    let new_reads = new_report.size_bytes / 4096 + 1;
    let reads_delta = new_reads as i64 - old_reads as i64;

    println!();
    let mut table = base_table();
    table.set_header(vec![
        header_cell("Metric"),
        header_cell("Baseline"),
        header_cell("Candidate"),
        header_cell("Delta"),
        header_cell("Change %"),
    ]);

    // Size row
    table.add_row(vec![
        value_cell("WASM size (bytes)"),
        value_cell(&old_report.size_bytes.to_string()),
        value_cell(&new_report.size_bytes.to_string()),
        if size_delta <= 0 {
            good_cell(&format!("{:+}", size_delta))
        } else {
            bad_cell(&format!("{:+}", size_delta))
        },
        if size_pct <= 0.0 {
            good_cell(&format!("{:+.2}%", size_pct))
        } else {
            bad_cell(&format!("{:+.2}%", size_pct))
        },
    ]);

    // Sim cost row
    table.add_row(vec![
        value_cell("Est. sim cost (stroops)"),
        value_cell(&old_cost.to_string()),
        value_cell(&new_cost.to_string()),
        if cost_delta <= 0 {
            good_cell(&format!("{:+}", cost_delta))
        } else {
            bad_cell(&format!("{:+}", cost_delta))
        },
        if cost_pct <= 0.0 {
            good_cell(&format!("{:+.2}%", cost_pct))
        } else {
            bad_cell(&format!("{:+.2}%", cost_pct))
        },
    ]);

    // Auth cost row
    table.add_row(vec![
        value_cell("Est. auth cost (stroops)"),
        value_cell(&old_auth.to_string()),
        value_cell(&new_auth.to_string()),
        if auth_delta <= 0 {
            good_cell(&format!("{:+}", auth_delta))
        } else {
            bad_cell(&format!("{:+}", auth_delta))
        },
        value_cell("—"),
    ]);

    // Ledger reads row
    table.add_row(vec![
        value_cell("Est. ledger footprint reads"),
        value_cell(&old_reads.to_string()),
        value_cell(&new_reads.to_string()),
        if reads_delta <= 0 {
            good_cell(&format!("{:+}", reads_delta))
        } else {
            bad_cell(&format!("{:+}", reads_delta))
        },
        value_cell("—"),
    ]);

    // Heuristic score row
    table.add_row(vec![
        value_cell("Heuristic score"),
        value_cell(&old_report.score.to_string()),
        value_cell(&new_report.score.to_string()),
        if new_report.score >= old_report.score {
            good_cell(&format!("{:+}", new_report.score as i32 - old_report.score as i32))
    p::separator();

    // Gas breakdown
    p::header("Gas Breakdown");
    p::kv(
        "CPU instructions",
        &format!("{}", est.gas.cpu_instructions),
    );
    p::kv(
        "Memory bytes",
        &format!("{}", est.gas.memory_bytes),
    );
    p::kv(
        "CPU fee",
        &format!("{} stroops", est.gas.cpu_fee_stroops),
    );
    p::kv(
        "Memory fee",
        &format!("{} stroops", est.gas.memory_fee_stroops),
    );
    p::kv_accent(
        "Total gas fee",
        &format!("{} stroops", est.gas.total_gas_stroops),
    );

    println!();

    // Storage breakdown
    p::header("Storage Fees");
    p::kv(
        "WASM upload",
        &format!(
            "{} stroops  ({} bytes)",
            est.storage.wasm_upload_fee_stroops, est.storage.wasm_upload_bytes
        ),
    );
    p::kv(
        "Result",
        if comparison.delta_stroops < 0 {
            "Improved (lower estimated cost)"
        } else if comparison.regression {
            "Regressed (estimated fee increased by more than 5%)"
        } else if comparison.delta_stroops > 0 {
            "Regressed (higher estimated cost)"
        } else {
            bad_cell(&format!("{:+}", new_report.score as i32 - old_report.score as i32))
        },
        value_cell("—"),
    ]);

    println!("{table}");

    // ── Verdict ───────────────────────────────────────────────────────────
    println!();
    if cost_delta < 0 {
        p::success(&format!(
            "Candidate is BETTER — saves {} stroops ({:+.2}%)",
            cost_delta.abs(),
            cost_pct
        ));
    } else if cost_delta > 0 {
        p::warn(&format!(
            "Candidate REGRESSED — costs {} more stroops ({:+.2}%)",
            cost_delta,
            cost_pct
        ));
    } else {
        p::info("No change in estimated compute cost.");
    }

    // ── Profile table ─────────────────────────────────────────────────────
    println!();
    let mut ptbl = base_table();
    ptbl.set_header(vec![header_cell("Step"), header_cell("Elapsed")]);
    for point in profile.points() {
        ptbl.add_row(vec![
            value_cell(&point.label),
            value_cell(&format!("{:?}", point.elapsed)),
        ]);
    }
    ptbl.add_row(vec![
        value_cell("Total"),
        value_cell(&format!("{:?}", profile.total_elapsed())),
    ]);
    println!("{ptbl}");

    let headers = &["ID", "Network", "WASM", "Total Fee (stroops)", "XLM", "Recorded At"];
    let rows: Vec<Vec<String>> = filtered
        .iter()
        .map(|e| {
            vec![
                e.id[..8].to_string(),
                e.estimate.network.clone(),
                shorten_path(&e.estimate.wasm_path, 30),
                e.estimate.total_fee_stroops.to_string(),
                format!("{:.7}", e.estimate.total_fee_xlm),
                e.estimate.estimated_at[..10].to_string(),
            ]
        })
        .collect();

    p::table(headers, &rows);
    p::separator();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_simulation_cost_zero() {
        assert_eq!(estimate_simulation_cost(0), 2_000);
    }

    #[test]
    fn estimate_simulation_cost_nonzero() {
        // 8 bytes → 2000 + 1 = 2001
        assert_eq!(estimate_simulation_cost(8), 2_001);
    }

    #[test]
    fn estimate_simulation_cost_large() {
        // 80_000 bytes → 2000 + 10000 = 12000
        assert_eq!(estimate_simulation_cost(80_000), 12_000);
    }

    #[test]
    fn base_table_has_utf8_preset() {
        let table = base_table();
        // Just ensure it constructs without panic
        let _ = table.to_string();
    }
}
