use crate::utils::{config, print as p, test_automation, test_runner};
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

    /// Generate automated test cases from contract
    #[arg(long, default_value = "false")]
    pub generate: bool,

    /// Run tests in parallel
    #[arg(long, default_value = "false")]
    pub parallel: bool,

    /// Number of parallel workers (default: 4)
    #[arg(long, default_value = "4")]
    pub workers: usize,

    /// Path to contract source directory for test generation
    #[arg(long)]
    pub contract_path: Option<PathBuf>,
}

pub fn handle(args: TestArgs) -> Result<()> {
    config::validate_file_path(&args.wasm, Some("wasm"))?;

    p::header("Contract Test Runner");
    p::kv("Wasm", &args.wasm.display().to_string());
    p::kv("Coverage", if args.coverage { "yes" } else { "no" });
    if let Some(r) = &args.report {
        p::kv("Report", r);
    }
    p::kv("Generate tests", if args.generate { "yes" } else { "no" });
    p::kv("Parallel execution", if args.parallel { "yes" } else { "no" });
    if args.parallel {
        p::kv("Workers", &args.workers.to_string());
    }

    // Handle automated test generation
    if args.generate {
        if let Some(contract_path) = &args.contract_path {
            p::info("Generating automated test cases...");
            let generator = test_automation::TestCaseGenerator::new(contract_path.clone());
            let suite = generator.generate_from_contract()?;
            
            p::success(&format!("Generated {} test cases", suite.test_cases.len()));
            
            // Save test suite
            let suite_path = contract_path.join("test_suite.json");
            let json = serde_json::to_string_pretty(&suite)?;
            std::fs::write(&suite_path, json)?;
            p::kv("Test suite saved", &suite_path.display().to_string());
        }
    }

    // Run tests with automation if parallel is enabled
    if args.parallel {
        if let Some(contract_path) = &args.contract_path {
            let suite_path = contract_path.join("test_suite.json");
            if suite_path.exists() {
                let suite_content = std::fs::read_to_string(&suite_path)?;
                let suite: test_automation::TestSuite = serde_json::from_str(&suite_content)?;
                
                p::info("Running tests in parallel...");
                let runner = test_automation::ParallelTestRunner::new(args.workers);
                let report = runner.run_tests(&suite, &args.wasm)?;
                
                // Export report
                if let Some(report_format) = &args.report {
                    let report_path = match report_format.as_str() {
                        "html" => PathBuf::from("test_report.html"),
                        "json" => PathBuf::from("test_report.json"),
                        "junit" => PathBuf::from("test_report.xml"),
                        _ => PathBuf::from("test_report.html"),
                    };
                    
                    match report_format.as_str() {
                        "html" => test_automation::TestReportExporter::export_html(&report, &report_path)?,
                        "json" => test_automation::TestReportExporter::export_json(&report, &report_path)?,
                        "junit" => test_automation::TestReportExporter::export_junit(&report, &report_path)?,
                        _ => test_automation::TestReportExporter::export_html(&report, &report_path)?,
                    }
                    
                    p::kv("Report saved", &report_path.display().to_string());
                }
                
                println!();
                p::separator();
                p::kv("Total tests", &report.total_tests.to_string());
                p::kv("Passed", &report.passed.to_string());
                p::kv("Failed", &report.failed.to_string());
                p::kv("Coverage", &format!("{}%", 
                    if report.coverage_summary.lines_total > 0 {
                        (report.coverage_summary.lines_covered as f64 / report.coverage_summary.lines_total as f64 * 100.0) as u32
                    } else { 0 }
                ));
                p::kv("Duration", &format!("{}ms", report.total_duration_ms));
                p::separator();
                
                if report.failed > 0 {
                    anyhow::bail!("Some contract tests failed");
                }
                
                p::success("All contract tests passed");
                return Ok(());
            }
        }
    }

    // Fall back to original test runner
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
