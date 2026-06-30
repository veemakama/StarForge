use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DeployStatus {
    Success,
    Failed,
    RolledBack,
    Pending,
}

impl std::fmt::Display for DeployStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeployStatus::Success => write!(f, "success"),
            DeployStatus::Failed => write!(f, "failed"),
            DeployStatus::RolledBack => write!(f, "rolled-back"),
            DeployStatus::Pending => write!(f, "pending"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployRecord {
    pub id: String,
    pub contract_id: Option<String>,
    pub wasm_path: String,
    pub wasm_hash: String,
    pub network: String,
    pub wallet: String,
    pub timestamp: String,
    pub status: DeployStatus,
    pub error: Option<String>,
    pub previous_id: Option<String>,
    pub approved_by: Option<String>,
    pub verification_passed: bool,
}

impl DeployRecord {
    pub fn new(
        wasm_path: &str,
        wasm_hash: &str,
        network: &str,
        wallet: &str,
        previous_id: Option<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            contract_id: None,
            wasm_path: wasm_path.to_string(),
            wasm_hash: wasm_hash.to_string(),
            network: network.to_string(),
            wallet: wallet.to_string(),
            timestamp: Utc::now().to_rfc3339(),
            status: DeployStatus::Pending,
            error: None,
            previous_id,
            approved_by: None,
            verification_passed: false,
        }
    }

    /// Build a new record that reverts the active deployment back to `target`.
    ///
    /// The rollback re-applies `target`'s WASM/contract, so the resulting record
    /// inherits its `wasm_path`/`wasm_hash`/`contract_id` but gets a fresh id, is
    /// marked `Success`, and links `previous_id` to the deployment it reverted to
    /// (preserving the upgrade/rollback lineage).
    pub fn rollback_of(target: &DeployRecord, wallet: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            contract_id: target.contract_id.clone(),
            wasm_path: target.wasm_path.clone(),
            wasm_hash: target.wasm_hash.clone(),
            network: target.network.clone(),
            wallet: wallet.to_string(),
            timestamp: Utc::now().to_rfc3339(),
            status: DeployStatus::Success,
            error: None,
            previous_id: Some(target.id.clone()),
            approved_by: None,
            verification_passed: target.verification_passed,
        }
    }
}

fn history_path() -> PathBuf {
    crate::utils::config::config_dir().join("deploy_history.json")
}

pub fn load_history() -> Result<Vec<DeployRecord>> {
    let path = history_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let data = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&data).unwrap_or_default())
}

pub fn save_history(records: &[DeployRecord]) -> Result<()> {
    let path = history_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_string_pretty(records)?;
    fs::write(&path, data)?;
    Ok(())
}

pub fn record_deployment(record: DeployRecord) -> Result<String> {
    let mut history = load_history()?;
    let id = record.id.clone();
    history.push(record);
    save_history(&history)?;
    Ok(id)
}

pub fn update_status(id: &str, status: DeployStatus, error: Option<String>) -> Result<()> {
    let mut history = load_history()?;
    if let Some(rec) = history.iter_mut().find(|r| r.id == id) {
        rec.status = status;
        rec.error = error;
    }
    save_history(&history)
}

pub fn set_contract_id(id: &str, contract_id: &str) -> Result<()> {
    let mut history = load_history()?;
    if let Some(rec) = history.iter_mut().find(|r| r.id == id) {
        rec.contract_id = Some(contract_id.to_string());
    }
    save_history(&history)
}

pub fn set_verified(id: &str, passed: bool) -> Result<()> {
    let mut history = load_history()?;
    if let Some(rec) = history.iter_mut().find(|r| r.id == id) {
        rec.verification_passed = passed;
    }
    save_history(&history)
}

pub fn get_record(id: &str) -> Result<Option<DeployRecord>> {
    let history = load_history()?;
    Ok(history
        .into_iter()
        .find(|r| r.id == id || r.id.starts_with(id)))
}

pub fn last_successful(network: &str) -> Result<Option<DeployRecord>> {
    let history = load_history()?;
    Ok(history
        .into_iter()
        .rev()
        .find(|r| r.network == network && r.status == DeployStatus::Success))
}

/// Mark `target` as the active deployment again by appending a rollback record,
/// and flag the deployment(s) it superseded as rolled back. Returns the new
/// rollback record's id.
pub fn record_rollback(target: &DeployRecord, wallet: &str) -> Result<String> {
    let mut history = load_history()?;

    // Any successful deployment on this network that came *after* the target is
    // being reverted away from — mark it rolled back so the dashboard reflects it.
    if let Some(target_pos) = history.iter().position(|r| r.id == target.id) {
        for rec in history.iter_mut().skip(target_pos + 1) {
            if rec.network == target.network && rec.status == DeployStatus::Success {
                rec.status = DeployStatus::RolledBack;
            }
        }
    }

    let record = DeployRecord::rollback_of(target, wallet);
    let id = record.id.clone();
    history.push(record);
    save_history(&history)?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deploy_record_new_has_pending_status() {
        let r = DeployRecord::new("a.wasm", "abc123", "testnet", "alice", None);
        assert_eq!(r.status, DeployStatus::Pending);
        assert!(r.contract_id.is_none());
    }

    #[test]
    fn deploy_status_display() {
        assert_eq!(DeployStatus::Success.to_string(), "success");
        assert_eq!(DeployStatus::RolledBack.to_string(), "rolled-back");
    }

    #[test]
    fn rollback_of_inherits_target_artifact_and_links_lineage() {
        let mut target = DeployRecord::new("v1.wasm", "hash-v1", "testnet", "alice", None);
        target.contract_id = Some("CABC".to_string());
        target.status = DeployStatus::Success;
        target.verification_passed = true;

        let rb = DeployRecord::rollback_of(&target, "bob");

        // Re-applies the target's artifact...
        assert_eq!(rb.wasm_hash, "hash-v1");
        assert_eq!(rb.wasm_path, "v1.wasm");
        assert_eq!(rb.contract_id.as_deref(), Some("CABC"));
        assert_eq!(rb.network, "testnet");
        // ...but is a distinct, successful record that links back to the target.
        assert_ne!(rb.id, target.id);
        assert_eq!(rb.status, DeployStatus::Success);
        assert_eq!(rb.previous_id.as_deref(), Some(target.id.as_str()));
        assert_eq!(rb.wallet, "bob");
    }
}
