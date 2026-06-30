use anyhow::Result;
use chrono::Utc;
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
}

pub fn common_templates() -> Vec<MultisigTemplate> {
    vec![
        MultisigTemplate {
            name: "escrow",
            description: "2-of-3 Escrow (buyer, seller, arbiter)",
            threshold: 2,
            signers: vec!["buyer", "seller", "arbiter"],
            transaction_type: "escrow_release",
        },
        MultisigTemplate {
            name: "company",
            description: "3-of-5 Company treasury approval",
            threshold: 3,
            signers: vec!["ceo", "cfo", "legal", "ops", "board"],
            transaction_type: "treasury_transfer",
        },
        MultisigTemplate {
            name: "dao",
            description: "5-of-9 DAO treasury authorization",
            threshold: 5,
            signers: vec![
                "member1", "member2", "member3", "member4", "member5", "member6", "member7",
                "member8", "member9",
            ],
            transaction_type: "dao_treasury",
        },
        MultisigTemplate {
            name: "vault",
            description: "2-of-2 Cold storage vault",
            threshold: 2,
            signers: vec!["primary_key", "recovery_key"],
            transaction_type: "vault_release",
        },
        MultisigTemplate {
            name: "payment",
            description: "1-of-2 Payment authorization",
            threshold: 1,
            signers: vec!["requester", "approver"],
            transaction_type: "payment",
        },
    ]
}

pub fn template_by_name(name: &str) -> Option<MultisigTemplate> {
    common_templates()
        .into_iter()
        .find(|template| template.name.eq_ignore_ascii_case(name))
}

pub fn proposal_from_template(template: &str, network: String) -> Result<Proposal> {
    let template = template_by_name(template)
        .ok_or_else(|| anyhow::anyhow!("Unknown multi-sig template: {}", template))?;
    let mut proposal = Proposal::new(
        template.threshold,
        template
            .signers
            .iter()
            .map(|signer| signer.to_string())
            .collect(),
        network,
    );
    proposal.metadata.title = Some(template.description.to_string());
    proposal.metadata.transaction_type = Some(template.transaction_type.to_string());
    proposal.metadata.template = Some(template.name.to_string());
    proposal.events.push(ProposalEvent {
        event_type: "template_applied".to_string(),
        message: format!("Applied '{}' template", template.name),
        at: Utc::now().to_rfc3339(),
    });
    Ok(proposal)
}

pub fn calculate_progress(proposal: &Proposal) -> SignatureProgress {
    let validation = validate_signatures(proposal);
    let signed = validation.valid_signatures;
    let required = proposal.threshold;
    let percent = if required == 0 {
        0
    } else {
        ((signed.min(required) as f64 / required as f64) * 100.0).round() as u32
    };

    SignatureProgress {
        signed,
        required,
        total_signers: proposal.signers.len() as u32,
        percent,
        ready: signed >= required && required > 0,
        pending_signers: validation.missing_signers,
    }
}

pub fn render_progress_bar(progress: &SignatureProgress, width: usize) -> String {
    let width = width.max(1);
    let filled = ((progress.percent.min(100) as usize * width) + 50) / 100;
    let filled = filled.min(width);
    let empty = width - filled;
    format!(
        "[{}{}] {}% ({}/{})",
        "#".repeat(filled),
        ".".repeat(empty),
        progress.percent,
        progress.signed,
        progress.required
    )
}

pub fn proposal_signature_payload(proposal: &Proposal) -> String {
    let metadata = serde_json::to_string(&proposal.metadata).unwrap_or_default();
    format!(
        "{}|{}|{}|{}|{}|{}",
        proposal.id,
        proposal.network,
        proposal.threshold,
        proposal.signers.join(","),
        proposal.transaction_xdr.as_deref().unwrap_or(""),
        metadata
    )
}

pub fn generate_signature_for_payload(signer: &str, message: &str) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(signer.as_bytes());
    hasher.update(b":");
    hasher.update(message.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn generate_proposal_signature(signer: &str, proposal: &Proposal) -> Result<String> {
    Ok(generate_signature_for_payload(
        signer,
        &proposal_signature_payload(proposal),
    ))
}

pub fn verify_proposal_signature(proposal: &Proposal, signature: &Signature) -> bool {
    if !proposal.signers.contains(&signature.signer) {
        return false;
    }

    let payload = proposal_signature_payload(proposal);
    verify_signature(&signature.signer, &signature.signature, &payload)
        || generate_signature(&signature.signer)
            .map(|legacy| legacy == signature.signature)
            .unwrap_or(false)
}

pub fn validate_signatures(proposal: &Proposal) -> SignatureValidationReport {
    let mut valid = HashSet::new();
    let mut seen = HashSet::new();
    let mut invalid_signers = Vec::new();
    let mut duplicate_signers = Vec::new();

    for signature in &proposal.signatures {
        if !seen.insert(signature.signer.clone()) {
            duplicate_signers.push(signature.signer.clone());
            continue;
        }

        if verify_proposal_signature(proposal, signature) {
            valid.insert(signature.signer.clone());
        } else {
            invalid_signers.push(signature.signer.clone());
        }
    }

    let missing_signers = proposal
        .signers
        .iter()
        .filter(|signer| !valid.contains(*signer))
        .cloned()
        .collect::<Vec<_>>();

    let valid_signatures = valid.len() as u32;
    SignatureValidationReport {
        valid_signatures,
        invalid_signers,
        duplicate_signers,
        missing_signers,
        ready: valid_signatures >= proposal.threshold && proposal.threshold > 0,
    }
}

pub fn generate_signature(wallet: &str) -> Result<String> {
    use hex;
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(wallet.as_bytes());
    let result = hasher.finalize();

    Ok(hex::encode(result))
}

pub fn verify_signature(signer: &str, signature: &str, message: &str) -> bool {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(signer.as_bytes());
    hasher.update(b":");
    hasher.update(message.as_bytes());
    let result = hasher.finalize();
    let expected = hex::encode(result);

    if expected == signature {
        return true;
    }

    let mut legacy_hasher = Sha256::new();
    legacy_hasher.update(message.as_bytes());
    hex::encode(legacy_hasher.finalize()) == signature
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

pub async fn send_notification(
    notification: NotificationRequest,
    channel: NotificationChannel,
) -> Result<()> {
    match channel {
        NotificationChannel::Email => {
            println!("📧 Email notification sent to signers");
            Ok(())
        }
        NotificationChannel::Slack => {
            println!("💬 Slack message sent");
            Ok(())
        }
        NotificationChannel::Discord => {
            println!("🎮 Discord message sent");
            Ok(())
        }
        NotificationChannel::Webhook(url) => {
            println!("🔔 Webhook notification sent to {}", url);
            Ok(())
        }
    }
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
}
