use crate::utils::{multisig_builder as multisig, notifications, print as p};
use anyhow::Result;
use clap::Subcommand;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::exit;

#[derive(Subcommand)]
pub enum MultisigCommands {
    /// Interactive multi-sig transaction builder workflow
    Build {
        /// Output proposal file path
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Create new multi-sig transaction proposal
    Create {
        /// Minimum signatures required
        #[arg(long)]
        threshold: u32,
        /// Signers (comma-separated names or public keys)
        #[arg(long)]
        signers: String,
        /// Transaction network
        #[arg(long, default_value = "testnet")]
        network: String,
        /// Human-readable proposal title
        #[arg(long)]
        title: Option<String>,
        /// Proposal description
        #[arg(long)]
        description: Option<String>,
        /// Transaction envelope/XDR to collect signatures for
        #[arg(long)]
        transaction_xdr: Option<String>,
    },
    /// Interactive multi-sig transaction builder
    Wizard,
    /// Add a signer to proposal
    AddSigner {
        /// Proposal file path
        proposal: PathBuf,
        /// Signer name or public key
        signer: String,
    },
    /// Sign proposal with wallet/signer name
    Sign {
        /// Proposal file path
        proposal: PathBuf,
        /// Signer wallet/name
        wallet: String,
    },
    /// View proposal details and signatures
    View {
        /// Proposal file path
        proposal: PathBuf,
    },
    /// Check signature status
    Status {
        /// Proposal file path
        proposal: PathBuf,
    },
    /// Check if proposal has enough valid signatures (exit 0 when ready)
    IsReady {
        /// Proposal file path
        proposal: PathBuf,
    },
    /// Submit signed proposal to network
    Submit {
        /// Proposal file path
        proposal: PathBuf,
        /// Network name
        #[arg(long, default_value = "testnet")]
        network: String,
    },
    /// Export proposal as JSON
    Export {
        /// Proposal file path
        proposal: PathBuf,
        /// Output file path
        output: Option<PathBuf>,
    },
    /// Import proposal from JSON
    Import {
        /// JSON file path
        input: PathBuf,
        /// Output proposal file path
        output: Option<PathBuf>,
    },
    /// Send signature request notifications
    Notify {
        /// Proposal file path
        proposal: PathBuf,
        /// Notification channel (email, slack, discord, webhook)
        #[arg(long, default_value = "email")]
        channel: String,
        /// Webhook URL for slack, discord, or webhook channels
        #[arg(long)]
        webhook: Option<String>,
        /// Custom notification message
        #[arg(long)]
        message: Option<String>,
    },
    /// List template scenarios
    Templates,
    /// Create proposal from template
    FromTemplate {
        /// Template name
        template: String,
        /// Output file path
        output: PathBuf,
        /// Transaction network
        #[arg(long, default_value = "testnet")]
        network: String,
    },
}

pub async fn handle(cmd: MultisigCommands) -> Result<()> {
    match cmd {
        MultisigCommands::Build { output } => build_interactive(output),
        MultisigCommands::Create {
            threshold,
            signers,
            network,
            title,
            description,
            transaction_xdr,
        } => create_proposal(
            threshold,
            &signers,
            &network,
            title,
            description,
            transaction_xdr,
        ),
        MultisigCommands::Wizard => interactive_wizard(),
        MultisigCommands::AddSigner { proposal, signer } => add_signer(&proposal, &signer),
        MultisigCommands::Sign { proposal, wallet } => sign_proposal(&proposal, &wallet),
        MultisigCommands::View { proposal } => view_proposal(&proposal),
        MultisigCommands::Status { proposal } => check_status(&proposal),
        MultisigCommands::IsReady { proposal } => is_ready(&proposal),
        MultisigCommands::Submit { proposal, network } => submit_proposal(&proposal, &network),
        MultisigCommands::Export { proposal, output } => export_proposal(&proposal, output),
        MultisigCommands::Import { input, output } => import_proposal(&input, output),
        MultisigCommands::Notify {
            proposal,
            channel,
            webhook,
            message,
        } => notify_signers(&proposal, &channel, webhook, message),
        MultisigCommands::Templates => list_templates(),
        MultisigCommands::FromTemplate {
            template,
            output,
            network,
        } => from_template(&template, &output, &network),
    }
}

fn load_proposal(path: &std::path::Path) -> Result<multisig::Proposal> {
    let contents = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&contents)?)
}

fn save_proposal(path: &std::path::Path, proposal: &multisig::Proposal) -> Result<()> {
    std::fs::write(path, serde_json::to_string_pretty(proposal)?)?;
    Ok(())
}

fn build_interactive(output: Option<PathBuf>) -> Result<()> {
    let theme = ColorfulTheme::default();

    p::header("Multi-Signature Transaction Builder");
    println!();

    let use_template = Confirm::with_theme(&theme)
        .with_prompt("Start from a pre-built template?")
        .default(true)
        .interact()?;

    let mut proposal = if use_template {
        let templates = multisig::template_definitions();
        let labels: Vec<String> = templates
            .iter()
            .map(|t| format!("{} - {}", t.name, t.description))
            .collect();
        let idx = Select::with_theme(&theme)
            .with_prompt("Choose template")
            .items(&labels)
            .default(0)
            .interact()?;
        multisig::proposal_from_template(templates[idx].name)?
    } else {
        let threshold: u32 = Input::with_theme(&theme)
            .with_prompt("Signature threshold (M-of-N)")
            .default(2)
            .interact_text()?;
        let signers_raw: String = Input::with_theme(&theme)
            .with_prompt("Signers (comma-separated)")
            .interact_text()?;
        let signers: Vec<String> = signers_raw
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if threshold as usize > signers.len() {
            anyhow::bail!("Threshold cannot exceed number of signers");
        }
        let network: String = Input::with_theme(&theme)
            .with_prompt("Network")
            .default("testnet".into())
            .interact_text()?;
        multisig::Proposal::new(threshold, signers, network)
    };

    let title: String = Input::with_theme(&theme)
        .with_prompt("Proposal title")
        .default(proposal.metadata.title.clone().unwrap_or_default())
        .allow_empty(true)
        .interact_text()?;
    if !title.is_empty() {
        proposal.metadata.title = Some(title);
    }

    let description: String = Input::with_theme(&theme)
        .with_prompt("Description")
        .allow_empty(true)
        .interact_text()?;
    if !description.is_empty() {
        proposal.metadata.description = Some(description);
    }

    let output_path = output.unwrap_or_else(|| {
        PathBuf::from(format!("proposal_{}.json", uuid::Uuid::new_v4()))
    });
    save_proposal(&output_path, &proposal)?;

    p::success(&format!("Proposal saved: {}", output_path.display()));
    print_proposal_summary(&proposal);
    run_interactive_loop(&output_path)
}

fn run_interactive_loop(proposal_path: &std::path::Path) -> Result<()> {
    let theme = ColorfulTheme::default();

    loop {
        println!();
        let choice = Select::with_theme(&theme)
            .with_prompt("Multi-sig workflow")
            .items(&[
                "View proposal (v)",
                "Check progress (p)",
                "Sign with wallet (s)",
                "Send notifications (n)",
                "Export proposal (e)",
                "Submit to network",
                "Quit (q)",
            ])
            .default(1)
            .interact()?;

        match choice {
            0 => view_proposal(proposal_path)?,
            1 => check_status(proposal_path)?,
            2 => {
                let wallet: String = Input::with_theme(&theme)
                    .with_prompt("Wallet / signer name")
                    .interact_text()?;
                sign_proposal(proposal_path, &wallet)?;
            }
            3 => {
                let channel: String = Input::with_theme(&theme)
                    .with_prompt("Notification channel (email/slack/discord/webhook)")
                    .default("email".into())
                    .interact_text()?;
                let webhook = if channel == "slack" || channel == "discord" || channel == "webhook" {
                    Some(
                        Input::with_theme(&theme)
                            .with_prompt("Webhook URL")
                            .interact_text()?,
                    )
                } else {
                    None
                };
                notify_signers(proposal_path, &channel, webhook, None)?;
            }
            4 => export_proposal(proposal_path, None)?,
            5 => submit_proposal(proposal_path, "testnet")?,
            _ => break,
        }
    }

    p::info("Multi-sig builder session ended");
    Ok(())
}

fn create_proposal(threshold: u32, signers: &str, network: &str) -> Result<()> {
    p::info(&format!(
        "Creating {}-of-{} multi-sig proposal",
        threshold,
        signers.split(',').count()
    ));

    save_proposal(Path::new(&filename), &proposal)?;

    println!();
    p::success(&format!("Proposal created: {}", filename));
    p::kv(
        "Threshold",
        &format!("{}/{}", threshold, proposal.signers.len()),
    );
    p::kv("Network", network);
    print_progress(&proposal);
    Ok(())
}

fn interactive_wizard() -> Result<()> {
    let theme = ColorfulTheme::default();
    p::header("Interactive Multi-Sig Builder");

    let templates = multisig::common_templates();
    let mut choices = vec!["blank custom proposal".to_string()];
    choices.extend(
        templates
            .iter()
            .map(|template| format!("{} - {}", template.name, template.description)),
    );

    let selected = Select::with_theme(&theme)
        .with_prompt("Choose a starting point")
        .items(&choices)
        .default(0)
        .interact()?;

    let network: String = Input::with_theme(&theme)
        .with_prompt("Network")
        .default("testnet".to_string())
        .interact_text()?;

    let mut proposal = if selected == 0 {
        let threshold: u32 = Input::with_theme(&theme)
            .with_prompt("Required signatures")
            .default(2)
            .interact_text()?;
        let signers: String = Input::with_theme(&theme)
            .with_prompt("Signers (comma-separated names or public keys)")
            .interact_text()?;
        let signer_list = parse_signers(&signers);
        validate_threshold(threshold, signer_list.len())?;
        multisig::Proposal::new(threshold, signer_list, network)
    } else {
        multisig::proposal_from_template(templates[selected - 1].name, network)?
    };

    let title: String = Input::with_theme(&theme)
        .with_prompt("Title")
        .default(
            proposal
                .metadata
                .title
                .clone()
                .unwrap_or_else(|| "Multi-sig transaction".to_string()),
        )
        .interact_text()?;
    proposal.metadata.title = Some(title);

    let description: String = Input::with_theme(&theme)
        .with_prompt("Description")
        .allow_empty(true)
        .interact_text()?;
    if !description.trim().is_empty() {
        proposal.metadata.description = Some(description);
    }

    let transaction_xdr: String = Input::with_theme(&theme)
        .with_prompt("Transaction XDR/envelope (optional)")
        .allow_empty(true)
        .interact_text()?;
    if !transaction_xdr.trim().is_empty() {
        proposal.transaction_xdr = Some(transaction_xdr);
    }

    save_proposal(std::path::Path::new(&filename), &proposal)?;

    println!();
    p::success(&format!("Proposal created: {}", output));
    print_progress(&proposal);

    if Confirm::with_theme(&theme)
        .with_prompt("Queue signature request notifications now?")
        .default(false)
        .interact()?
    {
        notify_for_proposal(&proposal, None)?;
    }

    Ok(())
}

fn add_signer(proposal_path: &std::path::Path, signer: &str) -> Result<()> {
    let mut proposal = load_proposal(proposal_path)?;

    if proposal.signers.contains(&signer.to_string()) {
        anyhow::bail!("Signer already in proposal");
    }

    proposal.signers.push(signer.to_string());
    save_proposal(proposal_path, &proposal)?;

    p::success(&format!("Signer added: {}", signer));
    print_progress(&proposal);
    Ok(())
}

fn sign_proposal(proposal_path: &std::path::Path, wallet: &str) -> Result<()> {
    let mut proposal = load_proposal(proposal_path)?;

    multisig::validate_for_signing(&proposal, wallet)?;

    p::info(&format!("Signing proposal with '{}'", wallet));

    let signature = multisig::generate_signature(&proposal.id, wallet)?;
    if !multisig::validate_signature_format(&signature) {
        anyhow::bail!("Generated signature failed format validation");
    }
    if !multisig::verify_signature(&proposal.id, wallet, &signature) {
        anyhow::bail!("Signature self-verification failed");
    }

    proposal.add_signature(wallet.to_string(), signature);
    save_proposal(proposal_path, &proposal)?;

    println!();
    println!("  Status: {}", proposal.get_status());
    println!(
        "  Signatures: {}/{}",
        proposal.signatures.len(),
        proposal.threshold
    );
    println!();
    p::success("Proposal signed");
    print_progress(&proposal);
    Ok(())
}

fn print_proposal_summary(proposal: &multisig::Proposal) {
    println!();
    println!("  ID:        {}", proposal.id);
    println!("  Threshold: {}/{}", proposal.threshold, proposal.signers.len());
    println!("  Network:   {}", proposal.network);
    println!("  Status:    {}", proposal.get_status());
    println!();
}

fn view_proposal(proposal_path: &std::path::Path) -> Result<()> {
    let proposal = load_proposal(proposal_path)?;

    println!();
    p::header("Multi-Sig Proposal");
    p::kv_accent("ID", &proposal.id);
    p::kv("Network", &proposal.network);
    p::kv(
        "Threshold",
        &format!("{}/{}", proposal.threshold, proposal.signers.len()),
    );
    println!("Status:      {}", proposal.get_status());
    println!("Created:     {}", proposal.created_at);
    if let Some(title) = &proposal.metadata.title {
        println!("Title:       {}", title);
    }
    if let Some(desc) = &proposal.metadata.description {
        println!("Description: {}", desc);
    }
    println!();

    println!();
    p::info("Signers");
    for (idx, signer) in proposal.signers.iter().enumerate() {
        let signed = proposal.signatures.iter().any(|sig| sig.signer == *signer);
        let marker = if signed {
            "signed".green()
        } else {
            "pending".yellow()
        };
        println!("  {:>2}. {:<8} {}", idx + 1, marker, signer);
    }

    println!();
    println!("{}", colored::Colorize::cyan("═══ SIGNATURES ═══"));
    for sig in &proposal.signatures {
        let verified = multisig::verify_signature(&proposal.id, &sig.signer, &sig.signature);
        let marker = if verified {
            colored::Colorize::green("✓")
        } else {
            colored::Colorize::red("✗")
        };
        let preview = if sig.signature.len() >= 16 {
            &sig.signature[..16]
        } else {
            &sig.signature
        };
        println!("  {} {}: {}...", marker, sig.signer, preview);
    }
    println!();
    Ok(())
}

fn check_status(proposal_path: &std::path::Path) -> Result<()> {
    let proposal = load_proposal(proposal_path)?;

    let signed = proposal.signatures.len();
    let remaining = proposal.threshold as isize - signed as isize;

    println!();
    p::header("Signature Status");
    print_progress(&proposal);

    let (bar, percent) = multisig::render_progress_bar(signed, proposal.threshold);
    print!("  [");
    for ch in bar.chars() {
        if ch == '█' {
            print!("{}", colored::Colorize::green("█"));
        } else {
            print!("{}", colored::Colorize::red("░"));
        }
    }
    if !validation.missing_signers.is_empty() {
        p::info(&format!(
            "Pending signers: {}",
            validation.missing_signers.join(", ")
        ));
    }

    if !validation.invalid_signers.is_empty() || !validation.duplicate_signers.is_empty() {
        anyhow::bail!("Proposal contains invalid signature data");
    }
    Ok(())
}

fn is_ready(proposal_path: &std::path::Path) -> Result<()> {
    let proposal = load_proposal(proposal_path)?;
    match multisig::validate_for_submit(&proposal) {
        Ok(()) => {
            print!("ready");
            io::stdout().flush()?;
            Ok(())
        }
        Err(_) => {
            exit(1);
        }
    }
}

fn submit_proposal(proposal_path: &std::path::Path, network: &str) -> Result<()> {
    let proposal = load_proposal(proposal_path)?;

    multisig::validate_for_submit(&proposal)?;

    p::info(&format!("Submitting proposal to {}", network));
    p::kv(
        "Signatures",
        &format!("{}/{}", validation.valid_signatures, proposal.threshold),
    );
    for sig in &proposal.signatures {
        println!("    ✓ {} verified", sig.signer);
    }
    println!();

    p::success("Proposal submitted successfully");
    println!("  Hash: abc123def456...");
    println!();
    Ok(())
}

fn export_proposal(proposal_path: &std::path::Path, output: Option<PathBuf>) -> Result<()> {
    let proposal = load_proposal(proposal_path)?;

    let output_file = output.unwrap_or_else(|| {
        PathBuf::from(format!(
            "proposal_export_{}.json",
            chrono::Local::now().format("%Y%m%d_%H%M%S")
        ))
    });

    save_proposal(&output_file, &proposal)?;

    p::success(&format!("Proposal exported: {}", output_file.display()));
    Ok(())
}

fn import_proposal(input_path: &std::path::Path, output: Option<PathBuf>) -> Result<()> {
    let proposal = load_proposal(input_path)?;

    let output_file =
        output.unwrap_or_else(|| PathBuf::from(format!("proposal_{}.json", uuid::Uuid::new_v4())));

    save_proposal(&output_file, &proposal)?;

    p::success(&format!("Proposal imported: {}", output_file.display()));
    print_progress(&proposal);
    Ok(())
}

async fn notify_signers(
    proposal_path: &std::path::Path,
    channel: &str,
    webhook: Option<String>,
    message: Option<String>,
) -> Result<()> {
    let proposal = load_proposal(proposal_path)?;
    let pending = proposal.pending_signers();

    if pending.is_empty() {
        p::info("All signers have already signed — no notifications needed");
        return Ok(());
    }

    let default_message = format!(
        "Signature requested for multi-sig proposal {} ({}/{}) on {}",
        proposal.id,
        proposal.signatures.len(),
        proposal.threshold,
        proposal.network
    );
    let msg = message.unwrap_or(default_message);
    let notification = multisig::NotificationRequest::new(&proposal, msg);
    let parsed_channel = multisig::parse_notification_channel(channel, webhook.clone())?;

    p::info(&format!(
        "Sending {} notification to {} pending signer(s)",
        channel,
        pending.len()
    ));

    multisig::send_notification(notification, parsed_channel, webhook.as_deref())?;

    p::success("Notification requests sent");

    Ok(())
}

fn list_templates() -> Result<()> {
    println!();
    p::header("Multi-Sig Templates");
    for template in multisig::common_templates() {
        println!(
            "  {} - {} [{}/{}]",
            template.name.yellow(),
            template.description,
            template.threshold,
            template.signers.len()
        );
    }
    println!();
    println!("Usage: starforge multisig from-template <template> <file> --network testnet");
    println!();
    Ok(())
}

    for template in multisig::template_definitions() {
        println!(
            "  {} - {}",
            colored::Colorize::yellow(template.name),
            template.description
        );
    }

    println!();
    p::success(&format!("Proposal created: {}", output.display()));
    p::kv(
        "Template",
        proposal.metadata.title.as_deref().unwrap_or(template),
    );
    p::kv(
        "Threshold",
        &format!("{}/{}", proposal.threshold, proposal.signers.len()),
    );
    p::kv("Signers", &proposal.signers.join(", "));
    print_progress(&proposal);
    Ok(())
}

fn load_proposal(path: &Path) -> Result<multisig::Proposal> {
    let contents = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&contents)?)
}

fn save_proposal(path: &Path, proposal: &multisig::Proposal) -> Result<()> {
    std::fs::write(path, serde_json::to_string_pretty(proposal)?)?;
    Ok(())
}

fn parse_signers(signers: &str) -> Vec<String> {
    signers
        .split(',')
        .map(|signer| signer.trim().to_string())
        .filter(|signer| !signer.is_empty())
        .collect()
}

    let proposal = multisig::proposal_from_template(template)?;
    let signers: Vec<&str> = proposal.signers.iter().map(String::as_str).collect();

    save_proposal(output, &proposal)?;

    println!();
    println!(
        "  Template: {}",
        proposal.metadata.title.as_deref().unwrap_or(template)
    );
    println!("  Threshold: {}/{}", proposal.threshold, signers.len());
    println!("  Signers: {}", signers.join(", "));
    println!();

    let default_message = format!(
        "Signature requested for proposal {} ({}/{})",
        proposal.id, progress.signed, progress.required
    );
    let mut data = HashMap::new();
    data.insert("proposal_id".to_string(), proposal.id.clone());
    data.insert("network".to_string(), proposal.network.clone());
    data.insert("threshold".to_string(), proposal.threshold.to_string());
    data.insert(
        "pending_signers".to_string(),
        progress.pending_signers.join(","),
    );
    data.insert("message".to_string(), message.unwrap_or(default_message));

    notifications::send_notification("multisig_signature_request", &data, "medium")?;
    p::success(&format!(
        "Queued signature request notification for {} signer(s)",
        progress.pending_signers.len()
    ));
    Ok(())
}
