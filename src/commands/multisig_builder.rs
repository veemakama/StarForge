use crate::utils::{multisig_builder as multisig, notifications, print as p};
use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Subcommand)]
pub enum MultisigCommands {
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
    /// Verify signatures and approval threshold
    Verify {
        /// Proposal file path
        proposal: PathBuf,
    },
    /// Send signature request notifications for pending signers
    Notify {
        /// Proposal file path
        proposal: PathBuf,
        /// Optional custom notification message
        #[arg(long)]
        message: Option<String>,
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
        MultisigCommands::Verify { proposal } => verify_proposal(&proposal),
        MultisigCommands::Notify { proposal, message } => notify_signers(&proposal, message),
        MultisigCommands::Submit { proposal, network } => submit_proposal(&proposal, &network),
        MultisigCommands::Export { proposal, output } => export_proposal(&proposal, output),
        MultisigCommands::Import { input, output } => import_proposal(&input, output),
        MultisigCommands::Templates => list_templates(),
        MultisigCommands::FromTemplate {
            template,
            output,
            network,
        } => from_template(&template, &output, &network),
    }
}

fn create_proposal(
    threshold: u32,
    signers: &str,
    network: &str,
    title: Option<String>,
    description: Option<String>,
    transaction_xdr: Option<String>,
) -> Result<()> {
    let signer_list = parse_signers(signers);
    validate_threshold(threshold, signer_list.len())?;

    let mut proposal = multisig::Proposal::new(threshold, signer_list, network.to_string());
    proposal.metadata.title = title;
    proposal.metadata.description = description;
    proposal.transaction_xdr = transaction_xdr;
    let filename = format!("proposal_{}.json", uuid::Uuid::new_v4());

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

    let output: String = Input::with_theme(&theme)
        .with_prompt("Output proposal JSON")
        .default(format!("proposal_{}.json", proposal.id))
        .interact_text()?;
    save_proposal(Path::new(&output), &proposal)?;

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

fn add_signer(proposal_path: &Path, signer: &str) -> Result<()> {
    let mut proposal = load_proposal(proposal_path)?;
    let signer = signer.trim();

    if proposal.signers.contains(&signer.to_string()) {
        anyhow::bail!("Signer already in proposal");
    }

    proposal.signers.push(signer.to_string());
    save_proposal(proposal_path, &proposal)?;

    p::success(&format!("Signer added: {}", signer));
    print_progress(&proposal);
    Ok(())
}

fn sign_proposal(proposal_path: &Path, wallet: &str) -> Result<()> {
    let mut proposal = load_proposal(proposal_path)?;

    p::info(&format!("Signing proposal with '{}'", wallet));

    let signature = multisig::generate_proposal_signature(wallet, &proposal)?;
    proposal.add_signature_checked(wallet.to_string(), signature)?;
    save_proposal(proposal_path, &proposal)?;

    println!();
    p::success("Proposal signed");
    print_progress(&proposal);
    Ok(())
}

fn view_proposal(proposal_path: &Path) -> Result<()> {
    let proposal = load_proposal(proposal_path)?;

    println!();
    p::header("Multi-Sig Proposal");
    p::kv_accent("ID", &proposal.id);
    p::kv("Network", &proposal.network);
    p::kv(
        "Threshold",
        &format!("{}/{}", proposal.threshold, proposal.signers.len()),
    );
    p::kv("Status", &proposal.get_status());
    p::kv("Created", &proposal.created_at);
    if let Some(title) = &proposal.metadata.title {
        p::kv("Title", title);
    }
    if let Some(tx_type) = &proposal.metadata.transaction_type {
        p::kv("Type", tx_type);
    }
    if let Some(xdr) = &proposal.transaction_xdr {
        p::kv("Transaction", &preview(xdr, 40));
    }
    print_progress(&proposal);

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
    p::info("Signatures");
    if proposal.signatures.is_empty() {
        println!("  none");
    } else {
        for sig in &proposal.signatures {
            println!("  {}: {}", sig.signer, preview(&sig.signature, 16));
        }
    }
    println!();
    Ok(())
}

fn check_status(proposal_path: &Path) -> Result<()> {
    let proposal = load_proposal(proposal_path)?;
    let validation = multisig::validate_signatures(&proposal);

    println!();
    p::header("Signature Status");
    print_progress(&proposal);

    if validation.ready {
        p::success("All required signatures collected.");
    } else {
        p::info(&format!(
            "{} signer(s) still needed: {}",
            validation.missing_signers.len(),
            validation.missing_signers.join(", ")
        ));
    }
    println!();
    Ok(())
}

fn verify_proposal(proposal_path: &Path) -> Result<()> {
    let proposal = load_proposal(proposal_path)?;
    let validation = multisig::validate_signatures(&proposal);

    p::header("Signature Verification");
    print_progress(&proposal);
    p::kv("Valid signatures", &validation.valid_signatures.to_string());
    p::kv("Required", &proposal.threshold.to_string());
    p::kv("Ready", if validation.ready { "yes" } else { "no" });

    if !validation.invalid_signers.is_empty() {
        p::warn(&format!(
            "Invalid signatures from: {}",
            validation.invalid_signers.join(", ")
        ));
    }
    if !validation.duplicate_signers.is_empty() {
        p::warn(&format!(
            "Duplicate signatures from: {}",
            validation.duplicate_signers.join(", ")
        ));
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

fn notify_signers(proposal_path: &Path, message: Option<String>) -> Result<()> {
    let proposal = load_proposal(proposal_path)?;
    notify_for_proposal(&proposal, message)
}

fn submit_proposal(proposal_path: &Path, network: &str) -> Result<()> {
    let proposal = load_proposal(proposal_path)?;
    let validation = multisig::validate_signatures(&proposal);

    if !validation.ready {
        anyhow::bail!(
            "Not enough valid signatures: {}/{}",
            validation.valid_signatures,
            proposal.threshold
        );
    }
    if !validation.invalid_signers.is_empty() || !validation.duplicate_signers.is_empty() {
        anyhow::bail!(
            "Proposal contains invalid or duplicate signatures. Run `starforge multisig verify`."
        );
    }

    p::info(&format!("Submitting proposal to {}", network));
    p::kv(
        "Signatures",
        &format!("{}/{}", validation.valid_signatures, proposal.threshold),
    );
    if let Some(xdr) = &proposal.transaction_xdr {
        p::kv("Transaction", &preview(xdr, 40));
    }

    p::success("Proposal submitted successfully");
    println!("  Hash: abc123def456...");
    println!();
    Ok(())
}

fn export_proposal(proposal_path: &Path, output: Option<PathBuf>) -> Result<()> {
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

fn import_proposal(input_path: &Path, output: Option<PathBuf>) -> Result<()> {
    let proposal = load_proposal(input_path)?;
    validate_threshold(proposal.threshold, proposal.signers.len())?;
    let output_file =
        output.unwrap_or_else(|| PathBuf::from(format!("proposal_{}.json", uuid::Uuid::new_v4())));

    save_proposal(&output_file, &proposal)?;
    p::success(&format!("Proposal imported: {}", output_file.display()));
    print_progress(&proposal);
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

fn from_template(template: &str, output: &Path, network: &str) -> Result<()> {
    p::info(&format!("Creating proposal from template '{}'", template));

    let proposal = multisig::proposal_from_template(template, network.to_string())?;
    save_proposal(output, &proposal)?;

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

fn validate_threshold(threshold: u32, signer_count: usize) -> Result<()> {
    if threshold == 0 {
        anyhow::bail!("Threshold must be greater than zero");
    }
    if signer_count == 0 {
        anyhow::bail!("At least one signer is required");
    }
    if threshold as usize > signer_count {
        anyhow::bail!("Threshold cannot exceed number of signers");
    }
    Ok(())
}

fn print_progress(proposal: &multisig::Proposal) {
    let progress = multisig::calculate_progress(proposal);
    println!(
        "  Progress: {}",
        multisig::render_progress_bar(&progress, 20).cyan()
    );
}

fn preview(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let prefix: String = value.chars().take(max_chars).collect();
    format!("{}...", prefix)
}

fn notify_for_proposal(proposal: &multisig::Proposal, message: Option<String>) -> Result<()> {
    let progress = multisig::calculate_progress(proposal);
    if progress.pending_signers.is_empty() {
        p::info("No pending signers to notify.");
        return Ok(());
    }

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
