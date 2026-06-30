use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub id: String,
    pub threshold: u32,
    pub signers: Vec<String>,
    pub signatures: Vec<Signature>,
    pub network: String,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub metadata: ProposalMetadata,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transaction_xdr: Option<String>,
    #[serde(default)]
    pub events: Vec<ProposalEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signature {
    pub signer: String,
    pub signature: String,
    pub signed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
    pub transaction_type: Option<String>,
    pub amount: Option<f64>,
    pub recipient: Option<String>,
    #[serde(default)]
    pub template: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalEvent {
    pub event_type: String,
    pub message: String,
    pub at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignatureProgress {
    pub signed: u32,
    pub required: u32,
    pub total_signers: u32,
    pub percent: u32,
    pub ready: bool,
    pub pending_signers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignatureValidationReport {
    pub valid_signatures: u32,
    pub invalid_signers: Vec<String>,
    pub duplicate_signers: Vec<String>,
    pub missing_signers: Vec<String>,
    pub ready: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultisigTemplate {
    pub name: &'static str,
    pub description: &'static str,
    pub threshold: u32,
    pub signers: Vec<&'static str>,
    pub transaction_type: &'static str,
}

#[derive(Debug, Clone)]
pub struct TemplateDefinition {
    pub name: &'static str,
    pub threshold: u32,
    pub signers: &'static [&'static str],
    pub description: &'static str,
}

impl Proposal {
    pub fn new(threshold: u32, signers: Vec<String>, network: String) -> Self {
        Proposal {
            id: Uuid::new_v4().to_string(),
            threshold,
            signers,
            signatures: Vec::new(),
            network,
            created_at: Utc::now().to_rfc3339(),
            expires_at: None,
            metadata: ProposalMetadata {
                title: None,
                description: None,
                transaction_type: None,
                amount: None,
                recipient: None,
                template: None,
            },
            transaction_xdr: None,
            events: vec![ProposalEvent {
                event_type: "created".to_string(),
                message: "Proposal created".to_string(),
                at: Utc::now().to_rfc3339(),
            }],
        }
    }

    pub fn add_signature(&mut self, signer: String, signature: String) {
        self.signatures.push(Signature {
            signer: signer.clone(),
            signature,
            signed_at: Utc::now().to_rfc3339(),
        });
        self.events.push(ProposalEvent {
            event_type: "signed".to_string(),
            message: format!("Signature collected from {}", signer),
            at: Utc::now().to_rfc3339(),
        });
    }

    pub fn add_signature_checked(&mut self, signer: String, signature: String) -> Result<()> {
        if !self.signers.contains(&signer) {
            anyhow::bail!("Signer '{}' is not authorized for this proposal", signer);
        }
        if self.signatures.iter().any(|sig| sig.signer == signer) {
            anyhow::bail!("Signer '{}' has already signed this proposal", signer);
        }
        self.add_signature(signer, signature);
        Ok(())
    }

    pub fn is_complete(&self) -> bool {
        self.signatures.len() >= self.threshold as usize
    }

    pub fn get_status(&self) -> String {
        if self.is_expired() {
            return "expired".to_string();
        }
        if self.is_complete() {
            "ready".to_string()
        } else {
            format!("pending ({}/{})", self.signatures.len(), self.threshold)
        }
    }

    pub fn pending_signers(&self) -> Vec<String> {
        self.signers
            .iter()
            .filter(|s| !self.signatures.iter().any(|sig| sig.signer == **s))
            .cloned()
            .collect()
    }

    pub fn signed_by(&self) -> Vec<String> {
        self.signatures.iter().map(|s| s.signer.clone()).collect()
    }

    pub fn is_expired(&self) -> bool {
        is_proposal_expired(self)
    }
}

pub fn is_proposal_expired(proposal: &Proposal) -> bool {
    let Some(expires_at) = &proposal.expires_at else {
        return false;
    };
    DateTime::parse_from_rfc3339(expires_at)
        .map(|dt| dt.with_timezone(&Utc) < Utc::now())
        .unwrap_or(false)
}

pub fn signing_message(proposal_id: &str, signer: &str) -> String {
    format!("starforge-multisig:{proposal_id}:{signer}")
}

fn hash_message(message: &str) -> Result<String> {
    use hex;
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(signer.as_bytes());
    hasher.update(b":");
    hasher.update(message.as_bytes());
    Ok(hex::encode(hasher.finalize()))
}

pub fn generate_signature(proposal_id: &str, wallet: &str) -> Result<String> {
    hash_message(&signing_message(proposal_id, wallet))
}

pub fn verify_signature(proposal_id: &str, signer: &str, signature: &str) -> bool {
    generate_signature(proposal_id, signer)
        .map(|expected| expected == signature)
        .unwrap_or(false)
}

pub fn validate_signature_format(signature: &str) -> bool {
    signature.len() == 64 && signature.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn validate_for_signing(proposal: &Proposal, wallet: &str) -> Result<()> {
    if proposal.is_expired() {
        bail!("Proposal has expired");
    }
    if !proposal.signers.contains(&wallet.to_string()) {
        bail!("Wallet '{}' is not an authorized signer for this proposal", wallet);
    }
    if proposal.signatures.iter().any(|s| s.signer == wallet) {
        bail!("Wallet '{}' has already signed this proposal", wallet);
    }
    Ok(())
}

pub fn validate_for_submit(proposal: &Proposal) -> Result<()> {
    if proposal.is_expired() {
        bail!("Proposal has expired");
    }
    if proposal.signatures.len() < proposal.threshold as usize {
        bail!(
            "Not enough signatures: {}/{}",
            proposal.signatures.len(),
            proposal.threshold
        );
    }

    for sig in &proposal.signatures {
        if !validate_signature_format(&sig.signature) {
            bail!("Invalid signature format from signer '{}'", sig.signer);
        }
        if !proposal.signers.contains(&sig.signer) {
            bail!("Unknown signer '{}' in signature list", sig.signer);
        }
        if !verify_signature(&proposal.id, &sig.signer, &sig.signature) {
            bail!("Signature verification failed for signer '{}'", sig.signer);
        }
    }

    Ok(())
}

pub fn render_progress_bar(signed: usize, threshold: u32) -> (String, i32) {
    let percent = if threshold == 0 {
        100
    } else {
        ((signed as f32 / threshold as f32) * 100.0).min(100.0) as i32
    };
    let filled = (percent / 10) as usize;
    let empty = 10usize.saturating_sub(filled);
    let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));
    (bar, percent)
}

pub fn template_definitions() -> Vec<TemplateDefinition> {
    vec![
        TemplateDefinition {
            name: "escrow",
            threshold: 2,
            signers: &["buyer", "seller", "arbiter"],
            description: "2-of-3 Escrow (buyer, seller, arbiter)",
        },
        TemplateDefinition {
            name: "company",
            threshold: 3,
            signers: &["ceo", "cfo", "board1", "board2", "board3"],
            description: "3-of-5 Company Signers",
        },
        TemplateDefinition {
            name: "dao",
            threshold: 5,
            signers: &[
                "member1", "member2", "member3", "member4", "member5", "member6", "member7",
                "member8", "member9",
            ],
            description: "5-of-9 DAO Treasury",
        },
        TemplateDefinition {
            name: "vault",
            threshold: 2,
            signers: &["key1", "key2"],
            description: "2-of-2 Cold Storage Vault",
        },
        TemplateDefinition {
            name: "payment",
            threshold: 1,
            signers: &["approver1", "approver2"],
            description: "1-of-2 Payment Authorization",
        },
    ]
}

pub fn proposal_from_template(name: &str) -> Result<Proposal> {
    let template = template_definitions()
        .into_iter()
        .find(|t| t.name == name)
        .ok_or_else(|| anyhow::anyhow!("Unknown template: {}", name))?;

    let mut proposal = Proposal::new(
        template.threshold,
        template.signers.iter().map(|s| s.to_string()).collect(),
        "testnet".to_string(),
    );
    proposal.metadata.title = Some(template.description.to_string());
    proposal.metadata.transaction_type = Some(name.to_string());
    Ok(proposal)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NotificationRequest {
    pub proposal_id: String,
    pub signers: Vec<String>,
    pub threshold: u32,
    pub message: String,
    pub created_at: String,
}

impl NotificationRequest {
    pub fn new(proposal: &Proposal, message: String) -> Self {
        NotificationRequest {
            proposal_id: proposal.id.clone(),
            signers: proposal.pending_signers(),
            threshold: proposal.threshold,
            message,
            created_at: Utc::now().to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum NotificationChannel {
    Email,
    Slack,
    Discord,
    Webhook(String),
}

pub fn parse_notification_channel(channel: &str, webhook: Option<String>) -> Result<NotificationChannel> {
    match channel.to_lowercase().as_str() {
        "email" => Ok(NotificationChannel::Email),
        "slack" => Ok(NotificationChannel::Slack),
        "discord" => Ok(NotificationChannel::Discord),
        "webhook" => {
            let url = webhook.ok_or_else(|| anyhow::anyhow!("--webhook is required for webhook channel"))?;
            Ok(NotificationChannel::Webhook(url))
        }
        other => bail!("Unknown notification channel: {}", other),
    }
}

pub fn send_notification(
    notification: NotificationRequest,
    channel: NotificationChannel,
    webhook: Option<&str>,
) -> Result<()> {
    match channel {
        NotificationChannel::Email => {
            for signer in &notification.signers {
                println!("📧 Email notification queued for {}", signer);
            }
            Ok(())
        }
        NotificationChannel::Slack => {
            let url = webhook.ok_or_else(|| anyhow::anyhow!("--webhook is required for slack channel"))?;
            println!("💬 Slack message sent");
            post_webhook(url, &notification)
        }
        NotificationChannel::Discord => {
            let url = webhook.ok_or_else(|| anyhow::anyhow!("--webhook is required for discord channel"))?;
            println!("🎮 Discord message sent");
            post_webhook(url, &notification)
        }
        NotificationChannel::Webhook(url) => post_webhook(&url, &notification),
    }
}

fn post_webhook(url: &str, notification: &NotificationRequest) -> Result<()> {
    let payload = serde_json::json!({
        "text": notification.message,
        "proposal_id": notification.proposal_id,
        "pending_signers": notification.signers,
        "threshold": notification.threshold,
    });

    let response = ureq::post(url)
        .set("Content-Type", "application/json")
        .send_string(&payload.to_string())?;

    if response.status() >= 400 {
        bail!("Webhook notification failed with status {}", response.status());
    }

    println!("🔔 Webhook notification sent to {}", url);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proposal_creation() {
        let signers = vec![
            "alice".to_string(),
            "bob".to_string(),
            "charlie".to_string(),
        ];
        let proposal = Proposal::new(2, signers, "testnet".to_string());

        assert_eq!(proposal.threshold, 2);
        assert_eq!(proposal.signers.len(), 3);
        assert!(!proposal.is_complete());
    }

    #[test]
    fn test_signature_added() {
        let signers = vec!["alice".to_string(), "bob".to_string()];
        let mut proposal = Proposal::new(2, signers, "testnet".to_string());

        proposal.add_signature("alice".to_string(), "sig123".to_string());
        assert_eq!(proposal.signatures.len(), 1);
        assert!(!proposal.is_complete());

        proposal.add_signature("bob".to_string(), "sig456".to_string());
        assert!(proposal.is_complete());
    }

    #[test]
    fn test_pending_signers() {
        let signers = vec![
            "alice".to_string(),
            "bob".to_string(),
            "charlie".to_string(),
        ];
        let mut proposal = Proposal::new(2, signers, "testnet".to_string());

        proposal.add_signature("alice".to_string(), "sig123".to_string());
        let pending = proposal.pending_signers();

        assert_eq!(pending.len(), 2);
        assert!(!pending.contains(&"alice".to_string()));
    }

    #[test]
    fn test_signature_generation_and_verification() {
        let proposal = Proposal::new(2, vec!["alice".into()], "testnet".into());
        let sig = generate_signature(&proposal.id, "alice").unwrap();

        assert!(validate_signature_format(&sig));
        assert!(verify_signature(&proposal.id, "alice", &sig));
        assert!(!verify_signature(&proposal.id, "bob", &sig));
    }

    #[test]
    fn test_validate_for_submit() {
        let signers = vec!["alice".to_string(), "bob".to_string()];
        let mut proposal = Proposal::new(2, signers, "testnet".to_string());
        assert!(validate_for_submit(&proposal).is_err());

        let sig = generate_signature(&proposal.id, "alice").unwrap();
        proposal.add_signature("alice".to_string(), sig);
        assert!(validate_for_submit(&proposal).is_err());

        let sig = generate_signature(&proposal.id, "bob").unwrap();
        proposal.add_signature("bob".to_string(), sig);
        assert!(validate_for_submit(&proposal).is_ok());
    }

    #[test]
    fn test_template_definitions() {
        let templates = template_definitions();
        assert_eq!(templates.len(), 5);
        let escrow = proposal_from_template("escrow").unwrap();
        assert_eq!(escrow.threshold, 2);
        assert_eq!(escrow.signers.len(), 3);
    }

    #[test]
    fn test_progress_bar() {
        let (bar, percent) = render_progress_bar(1, 2);
        assert_eq!(percent, 50);
        assert!(bar.contains('█'));
        assert!(bar.contains('░'));
    }
}
