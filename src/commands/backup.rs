use crate::utils::backup::{self, BackupStatus};
use crate::utils::crypto::prompt_password;
use crate::utils::print as p;
use anyhow::Result;
use clap::{Args, Subcommand};
use colored::Colorize;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum BackupCommands {
    /// Create a backup of contract code/state
    Create(CreateArgs),
    /// List recorded backups
    List(ListArgs),
    /// Show details of a backup
    Show(ShowArgs),
    /// Verify a backup's integrity (checksum)
    Verify(VerifyArgs),
    /// Restore a backup into a destination directory
    Restore(RestoreArgs),
    /// Restore the most recent backup at or before a given time (point-in-time recovery)
    RestorePointInTime(RestorePitArgs),
    /// Replicate an existing backup to another region
    Replicate(ReplicateArgs),
    /// Run a non-destructive recovery test (restore into a temp dir and verify)
    TestRecovery(VerifyArgs),
    /// Configure a recurring automated backup
    AutoConfigure(AutoConfigureArgs),
    /// Run any automated backups that are due
    AutoRun(AutoRunArgs),
}

#[derive(Args)]
pub struct CreateArgs {
    /// Files or directories to back up (contract WASM/source, state dirs, etc.)
    #[arg(required = true)]
    pub sources: Vec<PathBuf>,
    /// Logical label for this backup set (used for point-in-time recovery lookups)
    #[arg(long)]
    pub label: String,
    /// Encrypt the backup archive with a passphrase
    #[arg(long, default_value_t = false)]
    pub encrypt: bool,
    /// Region label to replicate the backup to
    #[arg(long, default_value = "primary")]
    pub region: String,
}

#[derive(Args)]
pub struct ListArgs {
    #[arg(long)]
    pub label: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct ShowArgs {
    pub id: String,
}

#[derive(Args)]
pub struct VerifyArgs {
    pub id: String,
}

#[derive(Args)]
pub struct RestoreArgs {
    pub id: String,
    /// Directory to restore the backup's files into
    #[arg(long)]
    pub dest: PathBuf,
}

#[derive(Args)]
pub struct RestorePitArgs {
    /// Backup label to search within
    #[arg(long)]
    pub label: String,
    /// Restore the most recent backup at or before this time (RFC3339)
    #[arg(long)]
    pub at: String,
    #[arg(long)]
    pub dest: PathBuf,
}

#[derive(Args)]
pub struct ReplicateArgs {
    pub id: String,
    #[arg(long)]
    pub region: String,
}

#[derive(Args)]
pub struct AutoConfigureArgs {
    #[arg(long)]
    pub label: String,
    #[arg(required = true)]
    pub sources: Vec<PathBuf>,
    /// How often the backup should run, in hours
    #[arg(long, default_value_t = 24)]
    pub interval_hours: u64,
    #[arg(long, default_value_t = false)]
    pub encrypt: bool,
    #[arg(long, default_value = "primary")]
    pub region: String,
}

#[derive(Args)]
pub struct AutoRunArgs {}

pub async fn handle(cmd: BackupCommands) -> Result<()> {
    match cmd {
        BackupCommands::Create(args) => handle_create(args),
        BackupCommands::List(args) => handle_list(args),
        BackupCommands::Show(args) => handle_show(args),
        BackupCommands::Verify(args) => handle_verify(args),
        BackupCommands::Restore(args) => handle_restore(args),
        BackupCommands::RestorePointInTime(args) => handle_restore_pit(args),
        BackupCommands::Replicate(args) => handle_replicate(args),
        BackupCommands::TestRecovery(args) => handle_test_recovery(args),
        BackupCommands::AutoConfigure(args) => handle_auto_configure(args),
        BackupCommands::AutoRun(args) => handle_auto_run(args),
    }
}

fn handle_create(args: CreateArgs) -> Result<()> {
    p::header("Create Backup");
    let passphrase = if args.encrypt {
        Some(prompt_password("Backup encryption passphrase", true)?)
    } else {
        None
    };

    let record = backup::create_backup(
        &args.sources,
        &args.label,
        args.encrypt,
        passphrase.as_deref(),
        &args.region,
    )?;

    p::kv("Backup ID", &record.id);
    p::kv("Label", &record.label);
    p::kv("Size", &format!("{} bytes", record.size_bytes));
    p::kv("Encrypted", if record.encrypted { "yes" } else { "no" });
    p::kv("Checksum", &record.checksum);
    p::kv("Replicated to", &record.replicated_regions.join(", "));
    p::success("Backup created");
    Ok(())
}

fn handle_list(args: ListArgs) -> Result<()> {
    p::header("Backups");
    let mut records = backup::list_backups()?;
    if let Some(label) = &args.label {
        records.retain(|r| &r.label == label);
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&records)?);
        return Ok(());
    }

    if records.is_empty() {
        p::info("No backups recorded.");
        return Ok(());
    }

    p::separator();
    for r in &records {
        let status_str = match r.status {
            BackupStatus::Verified => r.status.to_string().green().to_string(),
            BackupStatus::VerificationFailed => r.status.to_string().red().to_string(),
            BackupStatus::Created => r.status.to_string().cyan().to_string(),
        };
        println!(
            "  {} {:<14} {:<20} {:<10} {}",
            &r.id[..8.min(r.id.len())].cyan(),
            r.label,
            r.created_at.get(..19).unwrap_or(&r.created_at),
            status_str,
            r.replicated_regions.join(","),
        );
    }
    p::separator();
    Ok(())
}

fn handle_show(args: ShowArgs) -> Result<()> {
    p::header("Backup Details");
    let record = backup::load_backup(&args.id)?;
    println!("{}", serde_json::to_string_pretty(&record)?);
    Ok(())
}

fn handle_verify(args: VerifyArgs) -> Result<()> {
    p::header("Verify Backup");
    let record = backup::load_backup(&args.id)?;
    let passphrase = if record.encrypted {
        Some(prompt_password("Backup encryption passphrase", false)?)
    } else {
        None
    };
    let updated = backup::verify_backup(&args.id, passphrase.as_deref())?;
    p::kv("Backup ID", &updated.id);
    p::kv("Status", &updated.status.to_string());
    p::success("Backup verified successfully");
    Ok(())
}

fn handle_restore(args: RestoreArgs) -> Result<()> {
    p::header("Restore Backup");
    let record = backup::load_backup(&args.id)?;
    let passphrase = if record.encrypted {
        Some(prompt_password("Backup encryption passphrase", false)?)
    } else {
        None
    };
    let extracted = backup::restore_backup(&args.id, &args.dest, passphrase.as_deref())?;
    p::kv("Files restored", &extracted.len().to_string());
    for f in &extracted {
        println!("  - {}", f);
    }
    p::success("Backup restored");
    Ok(())
}

fn handle_restore_pit(args: RestorePitArgs) -> Result<()> {
    p::header("Point-in-Time Recovery");
    let at = backup::find_point_in_time(&args.label, chrono_parse(&args.at)?)?;
    p::kv("Selected backup", &at.id);
    p::kv("Created at", &at.created_at);

    let passphrase = if at.encrypted {
        Some(prompt_password("Backup encryption passphrase", false)?)
    } else {
        None
    };
    let extracted = backup::restore_backup(&at.id, &args.dest, passphrase.as_deref())?;
    p::kv("Files restored", &extracted.len().to_string());
    p::success("Point-in-time recovery complete");
    Ok(())
}

fn handle_replicate(args: ReplicateArgs) -> Result<()> {
    p::header("Replicate Backup");
    let record = backup::replicate_existing(&args.id, &args.region)?;
    p::kv("Backup ID", &record.id);
    p::kv("Replicated to", &record.replicated_regions.join(", "));
    p::success("Backup replicated");
    Ok(())
}

fn handle_test_recovery(args: VerifyArgs) -> Result<()> {
    p::header("Recovery Test");
    let record = backup::load_backup(&args.id)?;
    let passphrase = if record.encrypted {
        Some(prompt_password("Backup encryption passphrase", false)?)
    } else {
        None
    };
    let count = backup::test_restore(&args.id, passphrase.as_deref())?;
    p::kv("Files verified", &count.to_string());
    p::success("Recovery test passed — backup can be restored");
    Ok(())
}

fn handle_auto_configure(args: AutoConfigureArgs) -> Result<()> {
    p::header("Configure Automated Backup");
    let cfg = backup::configure_automation(
        &args.label,
        args.sources,
        args.interval_hours,
        args.encrypt,
        &args.region,
    )?;
    p::kv("Label", &cfg.label);
    p::kv("Interval", &format!("{}h", cfg.interval_hours));
    p::success("Automated backup configured. Run `starforge backup auto-run` periodically (e.g. via cron) to execute due backups.");
    Ok(())
}

fn handle_auto_run(_args: AutoRunArgs) -> Result<()> {
    p::header("Run Automated Backups");
    let passphrase = if backup::list_automation()?.iter().any(|c| c.encrypt) {
        Some(prompt_password("Backup encryption passphrase", false)?)
    } else {
        None
    };
    let ran = backup::run_automation(passphrase.as_deref())?;
    if ran.is_empty() {
        p::info("No automated backups were due.");
    } else {
        for label in &ran {
            p::success(&format!("Backed up '{}'", label));
        }
    }
    Ok(())
}

fn chrono_parse(s: &str) -> Result<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&chrono::Utc))
        .map_err(|e| anyhow::anyhow!("Invalid RFC3339 timestamp '{}': {}", s, e))
}
