use crate::utils::print as p;
use crate::utils::security::{
    apply_hardening, default_rules, evaluate_event, format_report, generate_hardening_report,
    run_audit, run_checklist, run_pentest, track_findings, validate_security, write_report,
    AnomalyDetector, AuditConfig, HardeningOptions, IncidentResponse, IncidentStore,
    RemediationStatus, ThreatFeed,
};
use crate::utils::stream::{EventStreamFilters, SorobanEventStream};
use crate::utils::{config, notifications, soroban};
use anyhow::Result;
use clap::{Args, Subcommand};
use colored::Colorize;
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
    /// Run full security audit with external tools (Slither, Mythril) and built-in analysis
    Audit(AuditArgs),
    /// Run simulated penetration test cases against contract source
    Pentest(PentestArgs),
    /// Track remediation of findings from audit/pentest/checklist runs
    Remediation(RemediationArgs),
}

#[derive(Args)]
pub struct AuditArgs {
    /// Path to Soroban contract source (.rs)
    pub path: PathBuf,
    /// Run Slither if installed
    #[arg(long, default_value = "true")]
    pub slither: bool,
    /// Run Mythril if installed
    #[arg(long, default_value = "true")]
    pub mythril: bool,
    /// Output format: text or json
    #[arg(long, default_value = "text")]
    pub format: String,
    /// Save report to file instead of stdout
    #[arg(long)]
    pub out: Option<PathBuf>,
    /// CI mode: exit non-zero if score is below threshold (0-100)
    #[arg(long)]
    pub min_score: Option<f64>,
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
    Ack {
        #[arg(long)]
        id: String,
    },
}

#[derive(Args)]
pub struct IncidentArgs {
    #[command(subcommand)]
    pub command: IncidentCommands,
}

#[derive(Args)]
pub struct PentestArgs {
    /// Path to Soroban contract source (.rs)
    pub path: PathBuf,
    /// Output format: text or json
    #[arg(long, default_value = "text")]
    pub format: String,
    /// Automatically create remediation tracking items for exploited cases
    #[arg(long, default_value_t = true)]
    pub track: bool,
}

#[derive(Subcommand)]
pub enum RemediationCommands {
    /// List tracked remediation items
    List {
        #[arg(long)]
        status: Option<String>,
    },
    /// Assign a remediation item to someone
    Assign {
        id: String,
        #[arg(long)]
        to: String,
    },
    /// Update the status of a remediation item
    Status {
        id: String,
        /// New status: open, in-progress, resolved, verified, wont-fix
        status: String,
    },
    /// Add a note to a remediation item
    Note { id: String, note: String },
}

#[derive(Args)]
pub struct RemediationArgs {
    #[command(subcommand)]
    pub command: RemediationCommands,
}

pub async fn handle(cmd: SecurityCommands) -> Result<()> {
    match cmd {
        SecurityCommands::Harden(args) => handle_harden(args),
        SecurityCommands::Checklist(args) => handle_checklist(args),
        SecurityCommands::Validate(args) => handle_validate(args),
        SecurityCommands::Report(args) => handle_report(args),
        SecurityCommands::Monitor(args) => handle_monitor(args),
        SecurityCommands::Incident(args) => handle_incident(args),
        SecurityCommands::Audit(args) => handle_audit(args),
        SecurityCommands::Pentest(args) => handle_pentest(args),
        SecurityCommands::Remediation(args) => handle_remediation(args),
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
    p::kv(
        "Security score",
        &format!("{:.1}%", report.summary.security_score),
    );
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
            let updated = IncidentStore::update_status(
                &id,
                crate::utils::security::IncidentStatus::Acknowledged,
            )?;
            p::success(&format!("Incident {} acknowledged", updated.id));
            Ok(())
        }
    }
}

fn handle_audit(args: AuditArgs) -> Result<()> {
    config::validate_file_path(&args.path, Some("rs"))?;
    p::header("Contract Security Audit");
    p::kv("Contract", &args.path.display().to_string());

    let cfg = AuditConfig {
        run_slither: args.slither,
        run_mythril: args.mythril,
    };

    let result = run_audit(&args.path, &cfg)?;

    let score_label = match result.score as u32 {
        90..=100 => "Excellent",
        70..=89 => "Good",
        50..=69 => "Fair",
        _ => "Poor",
    };

    p::separator();
    p::kv("Tools used", &result.tools_used.join(", "));
    p::kv(
        "Security score",
        &format!("{:.1}/100  ({})", result.score, score_label),
    );
    p::kv("Critical", &result.summary.critical.to_string());
    p::kv("High    ", &result.summary.high.to_string());
    p::kv("Medium  ", &result.summary.medium.to_string());
    p::kv("Low     ", &result.summary.low.to_string());
    p::kv("Info    ", &result.summary.info.to_string());

    if !result.findings.is_empty() {
        println!();
        p::header("Findings");
        for (i, f) in result.findings.iter().enumerate() {
            println!(
                "  {}. [{}] {}  ({})",
                i + 1,
                f.severity.to_uppercase(),
                f.title,
                f.tool
            );
            println!("     {}", f.description);
            println!("     Remediation: {}", f.remediation);
            if let Some(loc) = &f.location {
                println!("     Location: {}", loc);
            }
            println!();
        }
    } else {
        println!();
        p::success("No security issues found.");
    }

    match args.format.as_str() {
        "json" => {
            let json = serde_json::to_string_pretty(&result)?;
            if let Some(out) = &args.out {
                fs::write(out, &json)?;
                p::kv("Report saved", &out.display().to_string());
            } else {
                println!("{}", json);
            }
        }
        _ => {
            if let Some(out) = &args.out {
                let text = format_report(&result);
                fs::write(out, &text)?;
                p::kv("Report saved", &out.display().to_string());
            }
        }
    }

    if let Some(min) = args.min_score {
        if result.score < min {
            anyhow::bail!(
                "Security score {:.1} is below required minimum {:.1}",
                result.score,
                min
            );
        }
    }

    p::success("Security audit complete");
    Ok(())
}

fn handle_pentest(args: PentestArgs) -> Result<()> {
    config::validate_file_path(&args.path, Some("rs"))?;
    p::header("Penetration Test Simulation");
    p::kv("Contract", &args.path.display().to_string());

    let report = run_pentest(&args.path)?;

    if args.track {
        let findings: Vec<_> = report
            .results
            .iter()
            .filter(|r| r.exploited)
            .map(|r| {
                (
                    r.name.clone(),
                    r.severity.clone(),
                    r.evidence.clone(),
                    r.remediation.clone(),
                )
            })
            .collect();
        let created = track_findings("pentest", &findings)?;
        if !created.is_empty() {
            p::info(&format!(
                "Created {} new remediation item(s)",
                created.len()
            ));
        }
    }

    if args.format == "json" {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    p::separator();
    p::kv("Cases run", &report.cases_run.to_string());
    p::kv("Cases exploited", &report.cases_exploited.to_string());
    p::kv("Security score", &format!("{:.1}/100", report.score));
    println!();

    for r in &report.results {
        let icon = if r.exploited { "✗".red() } else { "✓".green() };
        println!(
            "  {} [{}] {} ({})",
            icon,
            r.severity.to_uppercase(),
            r.name,
            r.id
        );
        println!("      Attack vector: {}", r.attack_vector);
        if r.exploited {
            println!("      Evidence: {}", r.evidence);
            println!("      Remediation: {}", r.remediation);
        }
    }

    p::separator();
    if report.cases_exploited == 0 {
        p::success("No exploitable findings from simulated penetration tests");
    } else {
        p::warn(&format!(
            "{} simulated attack(s) succeeded — see remediation items",
            report.cases_exploited
        ));
    }
    Ok(())
}

fn handle_remediation(args: RemediationArgs) -> Result<()> {
    match args.command {
        RemediationCommands::List { status } => {
            p::header("Remediation Tracker");
            let mut items = crate::utils::security::remediation::load_all()?;
            if let Some(status) = &status {
                items.retain(|i| i.status.to_string() == *status);
            }
            if items.is_empty() {
                p::info("No remediation items recorded.");
                return Ok(());
            }
            for item in &items {
                println!(
                    "  {} [{}] {} — {} ({})",
                    &item.id[..8.min(item.id.len())].cyan(),
                    item.severity.to_uppercase(),
                    item.title,
                    item.status,
                    item.source,
                );
                if let Some(assignee) = &item.assignee {
                    println!("      Assigned to: {}", assignee);
                }
            }
            Ok(())
        }
        RemediationCommands::Assign { id, to } => {
            let item = crate::utils::security::remediation::assign(&id, &to)?;
            p::success(&format!("Assigned '{}' to {}", item.title, to));
            Ok(())
        }
        RemediationCommands::Status { id, status } => {
            let parsed = match status.as_str() {
                "open" => RemediationStatus::Open,
                "in-progress" => RemediationStatus::InProgress,
                "resolved" => RemediationStatus::Resolved,
                "verified" => RemediationStatus::Verified,
                "wont-fix" => RemediationStatus::WontFix,
                other => anyhow::bail!(
                    "Unknown status '{}'. Use one of: open, in-progress, resolved, verified, wont-fix",
                    other
                ),
            };
            let item = crate::utils::security::remediation::update_status(&id, parsed)?;
            p::success(&format!("'{}' is now {}", item.title, item.status));
            Ok(())
        }
        RemediationCommands::Note { id, note } => {
            let item = crate::utils::security::remediation::add_note(&id, &note)?;
            p::success(&format!("Note added to '{}'", item.title));
            Ok(())
        }
    }
}
