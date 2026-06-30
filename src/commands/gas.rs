use crate::utils::{
    config, gas_analyzer as ga, optimizer, print as p, profiler,
};
use anyhow::Result;
use clap::{Args, Subcommand};
use colored::*;
use serde::Serialize;
use std::path::PathBuf;

// ── Subcommand tree ───────────────────────────────────────────────────────────

#[derive(Subcommand)]
pub enum GasCommands {
    /// Deep gas profiling for a compiled Soroban contract
    Profile(ProfileArgs),
    /// Emit an optimized WASM (runs external optimizer if available)
    Optimize(OptimizeArgs),
    /// Compare gas costs between two WASM builds
    Compare(CompareArgs),
    /// List previously saved gas analysis reports
    History(HistoryArgs),
    /// Show a specific gas report by ID
    Show(ShowArgs),
    /// Print the optimization best-practices guide
    Guide,
}

// ── Argument structs ──────────────────────────────────────────────────────────

#[derive(Args)]
pub struct ProfileArgs {
    /// Path to the compiled WASM file
    pub wasm: PathBuf,
    /// Optional contract label (defaults to filename stem)
    #[arg(long)]
    pub label: Option<String>,
    /// Network context (informational only)
    #[arg(long)]
    pub network: Option<String>,
    /// Output format: text (default) or json
    #[arg(long, value_parser = ["text", "json"], default_value = "text")]
    pub output: String,
    /// Save the report to disk
    #[arg(long, default_value = "true")]
    pub save: bool,
    /// Exit with code 1 if critical issues are found
    #[arg(long)]
    pub fail_on_critical: bool,
}

#[derive(Args)]
pub struct OptimizeArgs {
    /// Path to the input WASM
    #[arg(long)]
    pub target: PathBuf,
    /// Output path for the optimized WASM
    #[arg(long)]
    pub output: PathBuf,
}

#[derive(Args)]
pub struct CompareArgs {
    /// Path to the baseline WASM
    pub baseline: PathBuf,
    /// Path to the candidate WASM
    pub candidate: PathBuf,
    /// Output format: text (default) or json
    #[arg(long, value_parser = ["text", "json"], default_value = "text")]
    pub output: String,
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
        GasCommands::Profile(args) => profile(args),
        GasCommands::Optimize(args) => optimize(args),
        GasCommands::Compare(args) => compare(args),
        GasCommands::History(args) => history(args),
        GasCommands::Show(args) => show(args),
        GasCommands::Guide => guide(),
    }
}

// ── profile ───────────────────────────────────────────────────────────────────

fn profile(args: ProfileArgs) -> Result<()> {
    config::validate_file_path(&args.wasm, Some("wasm"))?;

    let cfg = config::load()?;
    let network = args.network.unwrap_or(cfg.network);
    config::validate_network(&network)?;

    p::header("Gas Profiler");
    p::kv("Network", &network);
    p::kv("WASM", &args.wasm.display().to_string());

    let timer = profiler::Timer::start();
    let report = ga::analyze_wasm_file(&args.wasm, args.label.as_deref())?;
    let elapsed = timer.elapsed();

    if args.output == "json" {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_profile_report(&report);
    }

    if args.save {
        let path = ga::save_report(&report)?;
        p::kv("Report saved", path.file_name().unwrap_or_default().to_str().unwrap_or(""));
    }

    p::kv("Analysis time", &format!("{:.1}ms", elapsed.as_secs_f64() * 1000.0));

    if args.fail_on_critical && report.critical_count() > 0 {
        anyhow::bail!(
            "{} critical gas issue(s) found. Resolve before deploying.",
            report.critical_count()
        );
    }

    Ok(())
}

fn print_profile_report(report: &ga::GasAnalysisReport) {
    println!();
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
        for finding in &report.findings {
            let sev_str = match finding.severity {
                ga::FindingSeverity::Critical => finding.severity.to_string().red().bold().to_string(),
                ga::FindingSeverity::High => finding.severity.to_string().red().to_string(),
                ga::FindingSeverity::Medium => finding.severity.to_string().yellow().to_string(),
                ga::FindingSeverity::Low => finding.severity.to_string().cyan().to_string(),
                ga::FindingSeverity::Info => finding.severity.to_string().dimmed().to_string(),
            };
            println!(
                "  {} [{}]  {}",
                finding.id.white().bold(),
                sev_str,
                finding.description.white()
            );
            println!("    {} {}", "→".dimmed(), finding.recommendation.dimmed());
            if finding.estimated_gas_saving > 0 {
                println!(
                    "    {} Saves ~{} gas ({:.0}%)",
                    "~".dimmed(),
                    finding.estimated_gas_saving,
                    finding.estimated_saving_pct
                );
            }
            println!();
        }
    }

    // Top recommendations
    if !report.top_recommendations.is_empty() {
        p::info("Priority Actions");
        for (i, rec) in report.top_recommendations.iter().enumerate() {
            println!("  {}. {}", i + 1, rec);
        }
        println!();
    }

    let potential_saving = report.total_estimated_gas_saving();
    if potential_saving > 0 {
        p::kv_accent(
            "Potential saving",
            &format!("~{} gas across all findings", potential_saving),
        );
    }

    p::separator();
}

// ── optimize ──────────────────────────────────────────────────────────────────

fn optimize(args: OptimizeArgs) -> Result<()> {
    config::validate_file_path(&args.target, Some("wasm"))?;

    p::header("Gas Optimizer");
    p::kv("Input", &args.target.display().to_string());
    p::kv("Output", &args.output.display().to_string());

    let timer = profiler::Timer::start();
    let result = optimizer::optimize_wasm(&args.target, &args.output)?;
    let elapsed = timer.elapsed();

    println!();
    p::success("Optimization complete");
    p::kv("Tool", &result.tool);
    p::kv("Bytes in", &result.input_size_bytes.to_string());
    p::kv("Bytes out", &result.output_size_bytes.to_string());
    p::kv(
        "Size reduction",
        &format!(
            "{} bytes ({:+.2}%)",
            result.reduction_bytes(),
            result.reduction_percent()
        ),
    );
    p::kv("Duration", &format!("{:.1}ms", elapsed.as_secs_f64() * 1000.0));

    if result.output_size_bytes < result.input_size_bytes {
        println!();
        p::info("Run `starforge gas profile` on the output to verify improvements.");
    }

    Ok(())
}

// ── compare ───────────────────────────────────────────────────────────────────

fn compare(args: CompareArgs) -> Result<()> {
    config::validate_file_path(&args.baseline, Some("wasm"))?;
    config::validate_file_path(&args.candidate, Some("wasm"))?;

    p::header("Gas Version Comparison");
    p::kv("Baseline", &args.baseline.display().to_string());
    p::kv("Candidate", &args.candidate.display().to_string());

    let mut prof = profiler::Profiler::start();
    let cmp = ga::compare_versions(&args.baseline, &args.candidate)?;
    prof.mark("compare");

    if args.output == "json" {
        println!("{}", serde_json::to_string_pretty(&cmp)?);
        return Ok(());
    }

    println!();
    p::separator();

    // Hashes
    p::kv("Baseline SHA", &format!("{}…", &cmp.baseline_sha256[..16]));
    p::kv("Candidate SHA", &format!("{}…", &cmp.candidate_sha256[..16]));
    println!();

    // Size comparison
    let size_color = |delta: i64| {
        if delta < 0 {
            format!("{:+} bytes ({:.1}%)", delta, cmp.size_delta_pct).green().to_string()
        } else if delta > 0 {
            format!("{:+} bytes ({:.1}%)", delta, cmp.size_delta_pct).red().to_string()
        } else {
            "no change".dimmed().to_string()
        }
    };
    p::kv(
        "Baseline size",
        &format!("{:.1} KB", cmp.baseline_size_bytes as f64 / 1024.0),
    );
    p::kv(
        "Candidate size",
        &format!("{:.1} KB", cmp.candidate_size_bytes as f64 / 1024.0),
    );
    p::kv_accent("Size delta", &size_color(cmp.size_delta_bytes));

    // Gas cost comparison
    println!();
    p::kv("Baseline gas", &format!("{}", cmp.baseline_gas_cost.total));
    p::kv("Candidate gas", &format!("{}", cmp.candidate_gas_cost.total));
    let gas_color = if cmp.gas_delta < 0 {
        format!("{:+} ({:.1}%)", cmp.gas_delta, cmp.gas_delta_pct)
            .green()
            .to_string()
    } else if cmp.gas_delta > 0 {
        format!("{:+} ({:.1}%)", cmp.gas_delta, cmp.gas_delta_pct)
            .red()
            .to_string()
    } else {
        "no change".dimmed().to_string()
    };
    p::kv_accent("Gas delta", &gas_color);

    // Instruction comparison
    println!();
    p::kv(
        "Baseline instructions",
        &cmp.baseline_instruction_count.to_string(),
    );
    p::kv(
        "Candidate instructions",
        &cmp.candidate_instruction_count.to_string(),
    );
    let instr_color = if cmp.instruction_delta < 0 {
        format!(
            "{:+} ({:.1}%)",
            cmp.instruction_delta, cmp.instruction_delta_pct
        )
        .green()
        .to_string()
    } else if cmp.instruction_delta > 0 {
        format!(
            "{:+} ({:.1}%)",
            cmp.instruction_delta, cmp.instruction_delta_pct
        )
        .red()
        .to_string()
    } else {
        "no change".dimmed().to_string()
    };
    p::kv("Instruction delta", &instr_color);

    // Score comparison
    println!();
    p::kv(
        "Baseline score",
        &format!("{}/100", cmp.baseline_score),
    );
    let cand_score_str = format!("{}/100", cmp.candidate_score);
    let cand_score_colored = if cmp.score_delta >= 0 {
        cand_score_str.green().to_string()
    } else {
        cand_score_str.red().to_string()
    };
    p::kv_accent("Candidate score", &cand_score_colored);
    p::kv(
        "Score delta",
        &format!("{:+}", cmp.score_delta),
    );

    // New / resolved findings
    if cmp.resolved_findings > 0 {
        println!();
        p::success(&format!("{} finding(s) resolved.", cmp.resolved_findings));
    }
    if !cmp.new_findings.is_empty() {
        println!();
        p::warn(&format!(
            "{} new finding(s) introduced:",
            cmp.new_findings.len()
        ));
        for f in &cmp.new_findings {
            println!("  {} [{}] {}", f.id.white(), f.severity.to_string().yellow(), f.description);
        }
    }

    // Verdict
    println!();
    p::kv_accent("Verdict", &cmp.verdict);

    // Profile timing
    for pt in prof.points() {
        p::kv(&format!("Step {}", pt.label), &format!("{:.1}ms", pt.elapsed.as_secs_f64() * 1000.0));
    }

    p::separator();
    Ok(())
}

// ── history ───────────────────────────────────────────────────────────────────

fn history(args: HistoryArgs) -> Result<()> {
    p::header("Gas Report History");

    let mut reports = ga::list_reports()?;
    if let Some(label) = &args.label {
        reports.retain(|r| r.contract_label == *label);
    }
    let shown: Vec<_> = reports.iter().take(args.limit).collect();

    if args.json {
        println!("{}", serde_json::to_string_pretty(&shown)?);
        return Ok(());
    }

    if shown.is_empty() {
        p::info("No gas reports found. Run `starforge gas profile <wasm>` first.");
        return Ok(());
    }

    p::separator();
    println!(
        "  {:<14}  {:<22}  {:<8}  {:<10}  {:<10}  {}",
        "ID".dimmed(),
        "Contract".dimmed(),
        "Score".dimmed(),
        "Size (KB)".dimmed(),
        "Est. Gas".dimmed(),
        "Generated".dimmed(),
    );
    println!("  {}", "─".repeat(78).dimmed());
    for r in &shown {
        let score_str = format!("{}/100", r.optimization_score);
        let score_colored = if r.optimization_score >= 80 {
            score_str.green().to_string()
        } else if r.optimization_score >= 50 {
            score_str.yellow().to_string()
        } else {
            score_str.red().to_string()
        };
        println!(
            "  {:<14}  {:<22}  {:<8}  {:<10.1}  {:<10}  {}",
            r.id.cyan(),
            r.contract_label,
            score_colored,
            r.size_bytes as f64 / 1024.0,
            r.gas_cost.total,
            r.generated_at.get(..16).unwrap_or(&r.generated_at).dimmed(),
        );
    }
    p::separator();
    Ok(())
}

// ── show ──────────────────────────────────────────────────────────────────────

fn show(args: ShowArgs) -> Result<()> {
    p::header("Gas Report");
    let report = ga::load_report(&args.id)?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_profile_report(&report);
    }
    Ok(())
}

// ── guide ─────────────────────────────────────────────────────────────────────

fn guide() -> Result<()> {
    p::header("Gas Optimization Best Practices Guide");
    println!();

    let sections: &[(&str, &[&str])] = &[
        (
            "1. Binary Size Reduction",
            &[
                "Set `opt-level = 'z'` in [profile.release] for minimum size.",
                "Enable `lto = true` and `codegen-units = 1` for Link-Time Optimization.",
                "Add `strip = true` (Rust 1.59+) to remove symbol tables.",
                "Use `wasm-opt -Oz` (binaryen) as a post-build pass.",
                "Enable `default-features = false` on all dependencies.",
                "Audit dependencies with `cargo tree` — remove unused crates.",
            ],
        ),
        (
            "2. Panic & Error Handling",
            &[
                "Set `panic = \"abort\"` in [profile.release] — eliminates unwinding code.",
                "Replace verbose `expect(\"long message\")` with short error codes.",
                "Use `soroban_sdk::panic_with_error!` instead of `panic!`.",
                "Avoid `unwrap()` in hot paths — panics abort the transaction and waste fees.",
            ],
        ),
        (
            "3. Removing Debug Code",
            &[
                "Strip all `println!`, `eprintln!`, and `dbg!` calls before building for release.",
                "Use `#[cfg(not(test))]` or feature flags to gate debug-only code.",
                "Avoid `log::` crates in contract code — they add bloat with no effect on-chain.",
            ],
        ),
        (
            "4. Storage & State Optimization",
            &[
                "Prefer `Temporary` storage for ephemeral data — cheaper to write and auto-expired.",
                "Batch storage reads: cache `env.storage().get()` results in local variables.",
                "Pack related small values into a single storage key using a struct.",
                "Use `soroban_sdk::Map` and `Vec` instead of `std` equivalents.",
                "Minimize the number of distinct storage keys touched per invocation.",
            ],
        ),
        (
            "5. Computation & CPU Gas",
            &[
                "Move expensive off-chain computations off-chain where possible.",
                "Avoid nested loops over storage-backed collections.",
                "Prefer integer arithmetic over floating-point (no f64 in Soroban).",
                "Use bit manipulation instead of division/modulo for powers of two.",
                "Cache repeated calculations in local variables within a function.",
            ],
        ),
        (
            "6. Contract Architecture",
            &[
                "Keep contracts small and focused — deploy large logic as separate contracts.",
                "Use cross-contract calls sparingly; each call adds invocation overhead.",
                "Initialize state in a dedicated `init` function, not a WASM start function.",
                "Remove test helpers and admin utilities from the production build.",
                "Export only public-facing functions; internal helpers should be `fn` not `pub fn`.",
            ],
        ),
        (
            "7. Toolchain & Workflow",
            &[
                "Use `stellar contract build` (Stellar CLI) for the canonical optimized build.",
                "Run `starforge gas profile <wasm>` after every release build.",
                "Run `starforge gas compare <baseline> <candidate>` on every PR.",
                "Add the gas profile step to your CI pipeline with `--fail-on-critical`.",
                "Track score trends with `starforge gas history` to catch regressions early.",
            ],
        ),
        (
            "8. Soroban-Specific Tips",
            &[
                "Understand the fee model: upload fee (per byte) + execution fee (per instruction).",
                "Use `soroban_sdk::symbol_short!()` for short identifiers — cheaper than strings.",
                "Events are cheap — prefer events over storage for append-only audit trails.",
                "Auth overhead: minimize the number of `require_auth()` calls per invocation.",
                "Bump ledger entries proactively to avoid expensive re-initialization.",
            ],
        ),
    ];

    for (title, tips) in sections {
        println!("  {}", title.bright_white().bold());
        for tip in *tips {
            println!("    • {}", tip);
        }
        println!();
    }

    println!(
        "  {}\n  {}\n  {}\n  {}",
        "Quick-start commands:".dimmed(),
        "  starforge gas profile <contract.wasm>".cyan(),
        "  starforge gas compare <baseline.wasm> <optimized.wasm>".cyan(),
        "  starforge gas history".cyan(),
    );
    println!();
    p::separator();
    Ok(())
}
