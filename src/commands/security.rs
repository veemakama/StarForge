use crate::utils::print as p;
use crate::utils::security::{
    apply_hardening, generate_hardening_report, run_checklist, validate_security, write_report,
    AnomalyDetector, HardeningOptions, IncidentResponse, IncidentStore, ThreatFeed,
    evaluate_event, default_rules,
};
use crate::utils::stream::{EventStreamFilters, SorobanEventStream};
use crate::utils::{config, notifications, soroban};
use anyhow::Result;
use clap::{Args, Subcommand};
use std::fs;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[derive(Subcommand)]
pub enum SecurityCommands {
    /// Apply automated security hardening transforms
    Harden(HardenArgs),
    /// Run security checklist against contract source
    Checklist(ChecklistArgs),
    /// Validate contract against security patterns
    Validate(ValidateArgs),
    /// Generate hardening report (json or html)
    Report(ReportArgs),
    /// Continuous security monitoring for deployed contracts
    Monitor(SecurityMonitorArgs),
    /// Manage security incidents
    Incident(IncidentArgs),
}

#[derive(Args)]
pub struct HardenArgs {
    /// Path to Soroban contract source (.rs)
    pub path: PathBuf,
    /// Apply auto-fix transforms (writes .hardened.rs)
    #[arg(long, default_value = "false")]
    pub apply: bool,
    /// Preview changes without writing files
    #[arg(long, default_value = "false")]
    pub dry_run: bool,
}

#[derive(Args)]
pub struct ChecklistArgs {
    pub path: PathBuf,
}

#[derive(Args)]
pub struct ValidateArgs {
    pub path: PathBuf,
}

#[derive(Args)]
pub struct ReportArgs {
    pub path: PathBuf,
    #[arg(long, default_value = "json")]
    pub format: String,
}

#[derive(Args)]
pub struct SecurityMonitorArgs {
    #[arg(long)]
    pub contract: String,
    #[arg(long, default_value = "testnet")]
    pub network: String,
    #[arg(long, default_value = "2")]
    pub interval: u64,
    #[arg(long, default_value = "true")]
    pub follow: bool,
    #[arg(long, default_value = "false")]
    pub auto_incident: bool,
}

#[derive(Subcommand)]
pub enum IncidentCommands {
    List,
    Ack { #[arg(long)] id: String },
}

#[derive(Args)]
pub struct IncidentArgs {
    #[command(subcommand)]
    pub command: IncidentCommands,
}

pub fn handle(cmd: SecurityCommands) -> Result<()> {
    match cmd {
        SecurityCommands::Harden(args) => handle_harden(args),
        SecurityCommands::Checklist(args) => handle_checklist(args),
        SecurityCommands::Validate(args) => handle_validate(args),
        SecurityCommands::Report(args) => handle_report(args),
        SecurityCommands::Monitor(args) => handle_monitor(args),
        SecurityCommands::Incident(args) => handle_incident(args),
    }
}

fn handle_harden(args: HardenArgs) -> Result<()> {
    config::validate_file_path(&args.path, Some("rs"))?;
    p::header("Security Hardening");

    let result = apply_hardening(
        &args.path,
        &HardeningOptions {
            apply_fixes: args.apply,
            dry_run: args.dry_run || !args.apply,
            pattern_ids: None,
        },
    )?;

    p::kv("File", &result.file);
    p::kv("Findings", &result.findings.len().to_string());
    p::kv("Transforms applied", &result.transforms_applied.to_string());
    if let Some(out) = &result.output_path {
        p::kv("Output", &out.display().to_string());
    }

    for finding in &result.findings {
        println!(
            "  [{}] line {}: {} ({})",
            finding.severity, finding.line, finding.pattern_name, finding.pattern_id
        );
    }

    p::success("Hardening scan complete");
    Ok(())
}

fn handle_checklist(args: ChecklistArgs) -> Result<()> {
    config::validate_file_path(&args.path, Some("rs"))?;
    p::header("Security Checklist");

    let result = run_checklist(&args.path)?;
    p::kv("Score", &format!("{:.1}%", result.score_percent));
    p::kv("Passed", &result.passed.to_string());
    p::kv("Failed", &result.failed.to_string());

    for item in &result.items {
        let icon = if item.passed { "✓" } else { "✗" };
        println!(
            "  {} [{}] {} — {}",
            icon, item.severity, item.title, item.category
        );
    }

    Ok(())
}

fn handle_validate(args: ValidateArgs) -> Result<()> {
    config::validate_file_path(&args.path, Some("rs"))?;
    p::header("Security Validation");

    let result = validate_security(&args.path)?;
    p::kv("Valid", if result.valid { "yes" } else { "no" });
    p::kv("Critical", &result.critical.to_string());
    p::kv("High", &result.high.to_string());
    p::kv("Medium", &result.medium.to_string());
    p::kv("Low", &result.low.to_string());

    if !result.valid {
        anyhow::bail!("Security validation failed");
    }
    p::success("Security validation passed");
    Ok(())
}

fn handle_report(args: ReportArgs) -> Result<()> {
    config::validate_file_path(&args.path, Some("rs"))?;
    p::header("Security Hardening Report");

    let hardening = apply_hardening(
        &args.path,
        &HardeningOptions {
            apply_fixes: false,
            dry_run: true,
            pattern_ids: None,
        },
    )?;
    let checklist = run_checklist(&args.path)?;
    let validation = validate_security(&args.path)?;
    let report = generate_hardening_report(&args.path, hardening, checklist, validation)?;
    let path = write_report(&report, &args.format)?;

    p::kv("Report", &path.display().to_string());
    p::kv("Security score", &format!("{:.1}%", report.summary.security_score));
    p::success("Hardening report generated");
    Ok(())
}

fn handle_monitor(args: SecurityMonitorArgs) -> Result<()> {
    config::validate_contract_id(&args.contract)?;
    config::validate_network(&args.network)?;

    p::header("Security Monitoring");
    p::kv("Contract", &args.contract);
    p::kv("Network", &args.network);

    let rpc_url = soroban::rpc_url(&args.network)?;
    let rules = default_rules();
    let threat_feed = ThreatFeed::default_feed();
    let mut anomaly = AnomalyDetector::new(&args.contract);

    let running = Arc::new(AtomicBool::new(true));
    {
        let running = Arc::clone(&running);
        ctrlc::set_handler(move || running.store(false, Ordering::SeqCst))?;
    }

    let mut stream = SorobanEventStream::new(rpc_url, args.contract.clone())
        .with_poll_interval(args.interval)
        .with_filters(EventStreamFilters::default());

    let report_dir = config::config_dir().join("security").join("reports");
    fs::create_dir_all(&report_dir)?;

    while running.load(Ordering::SeqCst) {
        match stream.next_batch() {
            Ok(batch) => {
                for event in batch {
                    let security_events = evaluate_event(
                        &rules,
                        &args.contract,
                        event.ledger,
                        &event.id,
                        &event.topic,
                        &event.value,
                    );

                    for se in &security_events {
                        notifications::alert(&format!("[{}] {}", se.severity, se.message));

                        if args.auto_incident {
                            IncidentResponse::auto_respond(
                                &args.contract,
                                &se.severity,
                                &se.rule_name,
                                &se.message,
                            )?;
                        }
                    }

                    let threats = threat_feed.match_event(&event.value.to_string());
                    for threat in threats {
                        notifications::alert(&format!(
                            "Threat intel match [{}]: {}",
                            threat.severity, threat.description
                        ));
                    }

                    if let Some(anomaly_finding) = anomaly.record_event(None) {
                        notifications::warn(&anomaly_finding.message);
                    }
                }

                if !args.follow {
                    break;
                }
                stream.sleep();
            }
            Err(err) => {
                notifications::warn(&format!("Stream error: {}. Retrying…", err));
                stream.sleep_backoff();
            }
        }
    }

    p::success("Security monitoring session ended");
    Ok(())
}

fn handle_incident(args: IncidentArgs) -> Result<()> {
    match args.command {
        IncidentCommands::List => {
            p::header("Security Incidents");
            let incidents = IncidentStore::load_all()?;
            if incidents.is_empty() {
                p::info("No incidents recorded");
                return Ok(());
            }
            for inc in incidents {
                println!(
                    "  {} [{}] {} — {:?} ({})",
                    inc.id, inc.severity, inc.title, inc.status, inc.created_at
                );
            }
            Ok(())
        }
        IncidentCommands::Ack { id } => {
            let updated =
                IncidentStore::update_status(&id, crate::utils::security::IncidentStatus::Acknowledged)?;
            p::success(&format!("Incident {} acknowledged", updated.id));
            Ok(())
        }
    }
}
