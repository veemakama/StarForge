use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

use crate::utils::config;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IncidentStatus {
    Open,
    Acknowledged,
    Mitigated,
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentRecord {
    pub id: String,
    pub contract_id: String,
    pub severity: String,
    pub title: String,
    pub description: String,
    pub status: IncidentStatus,
    pub created_at: String,
    pub updated_at: String,
    pub actions_taken: Vec<String>,
}

pub struct IncidentStore;

impl IncidentStore {
    fn dir() -> Result<PathBuf> {
        let dir = config::config_dir().join("security").join("incidents");
        if !dir.exists() {
            fs::create_dir_all(&dir)?;
        }
        Ok(dir)
    }

    fn index_path() -> Result<PathBuf> {
        Ok(Self::dir()?.join("incidents.json"))
    }

    pub fn load_all() -> Result<Vec<IncidentRecord>> {
        let path = Self::index_path()?;
        if !path.exists() {
            return Ok(vec![]);
        }
        let raw = fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    pub fn save_all(records: &[IncidentRecord]) -> Result<()> {
        fs::write(
            Self::index_path()?,
            serde_json::to_string_pretty(records)?,
        )
        .context("Failed to save incidents")
    }

    pub fn create(
        contract_id: &str,
        severity: &str,
        title: &str,
        description: &str,
    ) -> Result<IncidentRecord> {
        let mut records = Self::load_all()?;
        let now = Utc::now().to_rfc3339();
        let incident = IncidentRecord {
            id: Uuid::new_v4().to_string(),
            contract_id: contract_id.to_string(),
            severity: severity.to_string(),
            title: title.to_string(),
            description: description.to_string(),
            status: IncidentStatus::Open,
            created_at: now.clone(),
            updated_at: now,
            actions_taken: vec!["Incident auto-created by security monitor".into()],
        };
        records.push(incident.clone());
        Self::save_all(&records)?;
        Ok(incident)
    }

    pub fn update_status(id: &str, status: IncidentStatus) -> Result<IncidentRecord> {
        let mut records = Self::load_all()?;
        let incident = records
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or_else(|| anyhow::anyhow!("Incident '{}' not found", id))?;
        incident.status = status.clone();
        incident.updated_at = Utc::now().to_rfc3339();
        incident
            .actions_taken
            .push(format!("Status changed to {:?}", status));
        let updated = incident.clone();
        Self::save_all(&records)?;
        Ok(updated)
    }
}

pub struct IncidentResponse;

impl IncidentResponse {
    pub fn auto_respond(
        contract_id: &str,
        severity: &str,
        title: &str,
        description: &str,
    ) -> Result<IncidentRecord> {
        let incident = IncidentStore::create(contract_id, severity, title, description)?;
        if severity == "critical" || severity == "high" {
            crate::utils::notifications::alert(&format!(
                "Security incident [{}]: {} — {}",
                incident.id, title, description
            ));
        }
        Ok(incident)
    }
}
