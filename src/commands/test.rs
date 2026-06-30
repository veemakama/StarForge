use crate::utils::{config, contract_testing, print as p, test_automation, test_runner};
use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

#[derive(Args)]
pub struct TestArgs {
    /// Path to the compiled wasm
    #[arg(long)]
    pub wasm: PathBuf,

    /// JSON/TOML contract testing fixture with mocks, scenarios, and assertions
    #[arg(long)]
    pub fixture: Option<PathBuf>,

    /// Path to contract source for generation/coverage
    #[arg(long)]
    pub source: Option<PathBuf>,

    /// Collect coverage analysis (requires --source)
    #[arg(long, default_value = "false")]
    pub coverage: bool,

    /// Auto-generate test cases from source
    #[arg(long, default_value = "false")]
    pub generate: bool,

    /// Run tests in parallel
    #[arg(long, default_value = "false")]
    pub parallel: bool,

    /// Number of parallel workers
    #[arg(long, default_value = "4")]
    pub workers: usize,

    /// Output report format (html, json) — also generates dashboard
    #[arg(long)]
    pub report: Option<String>,

    /// Path to contract source directory for test generation
    #[arg(long)]
    pub contract_path: Option<PathBuf>,

    /// Verify Soroban testnet integration for this run
    #[arg(long, default_value = "false")]
    pub testnet: bool,

    /// Network used by --testnet
    #[arg(long, default_value = "testnet")]
    pub network: String,

    /// Deployed contract ID used by --testnet checks
    #[arg(long)]
    pub contract_id: Option<String>,

    /// Skip the live RPC health probe while still validating testnet wiring
    #[arg(long, default_value = "false")]
    pub testnet_dry_run: bool,

    /// Disable the Soroban RPC health probe during --testnet
    #[arg(long, default_value = "false")]
    pub skip_rpc_health: bool,
}

pub async fn handle(args: TestArgs) -> Result<()> {
    config::validate_file_path(&args.wasm, Some("wasm"))?;
    if let Some(fixture) = &args.fixture {
        config::validate_file_path(fixture, None)?;
    }
    if let Some(contract_id) = &args.contract_id {
        config::validate_contract_id(contract_id)?;
    }
    if args.coverage && args.source.is_none() {
        anyhow::bail!("--coverage requires --source");
    }
    if args.generate && args.source.is_none() {
        anyhow::bail!("--generate requires --source");
    }

    p::header("Contract Test Runner");
    p::kv("Wasm", &args.wasm.display().to_string());
    p::kv("Coverage", if args.coverage { "yes" } else { "no" });
    p::kv("Generate", if args.generate { "yes" } else { "no" });
    p::kv("Parallel", if args.parallel { "yes" } else { "no" });
    if let Some(fixture) = &args.fixture {
        p::kv("Fixture", &fixture.display().to_string());
    }
    if args.testnet {
        p::kv("Testnet integration", "yes");
        p::kv("Network", &args.network);
    }
    if let Some(r) = &args.report {
        p::kv("Report", r);
    }
    p::kv("Generate tests", if args.generate { "yes" } else { "no" });
    p::kv(
        "Parallel execution",
        if args.parallel { "yes" } else { "no" },
    );
    if args.parallel {
        p::kv("Workers", &args.workers.to_string());
    }

    if let Some(fixture) = &args.fixture {
        let report = contract_testing::run_contract_framework(
            &args.wasm,
            fixture,
            contract_testing::FrameworkRunOptions {
                coverage: args.coverage,
                report_format: args.report.clone(),
                source: args.source.clone(),
                testnet: build_testnet_config(&args),
            },
        )
        .await?;

        print_framework_report(&report);

        if report.failures > 0 {
            anyhow::bail!("Some contract framework tests failed");
        }

        p::success("All contract framework tests passed");
        return Ok(());
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
                        "html" => {
                            test_automation::TestReportExporter::export_html(&report, &report_path)?
                        }
                        "json" => {
                            test_automation::TestReportExporter::export_json(&report, &report_path)?
                        }
                        "junit" => test_automation::TestReportExporter::export_junit(
                            &report,
                            &report_path,
                        )?,
                        _ => {
                            test_automation::TestReportExporter::export_html(&report, &report_path)?
                        }
                    }

                    p::kv("Report saved", &report_path.display().to_string());
                }

                println!();
                p::separator();
                p::kv("Total tests", &report.total_tests.to_string());
                p::kv("Passed", &report.passed.to_string());
                p::kv("Failed", &report.failed.to_string());
                p::kv(
                    "Coverage",
                    &format!(
                        "{}%",
                        if report.coverage_summary.lines_total > 0 {
                            (report.coverage_summary.lines_covered as f64
                                / report.coverage_summary.lines_total as f64
                                * 100.0) as u32
                        } else {
                            0
                        }
                    ),
                );
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
            parallel: args.parallel,
            generate: args.generate,
            source: args.source.clone(),
            workers: args.workers,
        },
    )?;

    println!();
    p::separator();
    p::kv_accent("SHA256", &result.sha256);
    p::kv("Wasm bytes", &result.size_bytes.to_string());
    p::kv("Cases executed", &result.cases_executed.to_string());
    p::kv("Failures", &result.failures.to_string());
    p::kv("Generated cases", &result.generated_cases.len().to_string());

    if let Some(cov) = &result.coverage {
        p::kv("Coverage", &format!("{:.1}%", cov.coverage_percent));
    }
    if let Some(path) = &result.report_path {
        p::kv("Report path", &path.display().to_string());
    }
    if let Some(path) = &result.dashboard_path {
        p::kv("Dashboard", &path.display().to_string());
    }

    if args.testnet {
        let status = contract_testing::verify_testnet_integration(
            build_testnet_config(&args).expect("testnet config available"),
        )
        .await?;
        println!();
        p::header("Testnet Integration");
        p::kv("Network", &status.network);
        p::kv("RPC", &status.rpc_url);
        p::kv("RPC healthy", if status.rpc_healthy { "yes" } else { "no" });
        for check in &status.checks {
            p::info(check);
        }
    }

    if !result.failure_analysis.is_empty() {
        println!();
        p::header("Failure Analysis");
        for fa in &result.failure_analysis {
            println!("  {} [{}]: {}", fa.test_name, fa.category, fa.suggestion);
        }
    }

    p::separator();

    if result.failures > 0 {
        anyhow::bail!("Some contract tests failed");
    }

    p::success("All contract tests passed");
    Ok(())
}

fn build_testnet_config(args: &TestArgs) -> Option<contract_testing::TestnetIntegrationConfig> {
    args.testnet
        .then(|| contract_testing::TestnetIntegrationConfig {
            network: args.network.clone(),
            contract_id: args.contract_id.clone(),
            verify_rpc_health: !args.skip_rpc_health,
            dry_run: args.testnet_dry_run,
        })
}

fn print_framework_report(report: &contract_testing::ContractFrameworkReport) {
    println!();
    p::separator();
    p::kv_accent("SHA256", &report.wasm_sha256);
    p::kv("Suite", &report.suite_name);
    p::kv("Wasm bytes", &report.size_bytes.to_string());
    p::kv("Fixtures", &report.fixtures_loaded.to_string());
    p::kv("Mocks", &report.mocks_available.to_string());
    p::kv("Cases executed", &report.cases_executed.to_string());
    p::kv("Failures", &report.failures.to_string());

    if let Some(coverage) = &report.coverage {
        p::kv("Coverage", &format!("{:.1}%", coverage.coverage_percent));
    }
    if let Some(path) = &report.report_path {
        p::kv("Report path", &path.display().to_string());
    }
    if let Some(status) = &report.testnet {
        p::kv("Testnet RPC", &status.rpc_url);
        p::kv("RPC healthy", if status.rpc_healthy { "yes" } else { "no" });
    }

    for case in &report.cases {
        let status = if case.passed { "PASS" } else { "FAIL" };
        p::kv(&format!("Case {}", case.name), status);
        for error in &case.errors {
            p::warn(error);
        }
    }

    p::separator();
}
