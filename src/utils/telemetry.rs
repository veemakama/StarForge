use crate::utils::config;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct TelemetryData {
    pub timestamp: DateTime<Utc>,
    pub event: String,
    pub properties: serde_json::Value,
    pub anonymous_id: String,
}

pub fn track_event(event: &str, properties: serde_json::Value) -> Result<()> {
    let cfg = config::load()?;

    // Check if telemetry is enabled (default to true, but respect opt-out)
    if !cfg.telemetry_enabled.unwrap_or(true) {
        return Ok(());
    }

    let anonymous_id = get_or_create_anonymous_id()?;

    let data = TelemetryData {
        timestamp: Utc::now(),
        event: event.to_string(),
        properties,
        anonymous_id,
    };

    // In a real app, we would send this to a service.
    // For now, we'll log it to a local file in the data directory.
    save_telemetry_locally(&data)?;

    Ok(())
}

fn get_or_create_anonymous_id() -> Result<String> {
    let data_dir = config::get_data_dir()?;
    let id_file = data_dir.join("anonymous_id");

    if id_file.exists() {
        Ok(fs::read_to_string(id_file)?.trim().to_string())
    } else {
        let id = Uuid::new_v4().to_string();
        fs::write(id_file, &id)?;
        Ok(id)
    }
}

fn save_telemetry_locally(data: &TelemetryData) -> Result<()> {
    let data_dir = config::get_data_dir()?;
    let telemetry_log = data_dir.join("telemetry.log");

    let json = serde_json::to_string(data)?;

    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(telemetry_log)?;

    writeln!(file, "{}", json)?;

    Ok(())
}

#[allow(dead_code)]
pub fn set_telemetry_enabled(enabled: bool) -> Result<()> {
    let mut cfg = config::load()?;
    cfg.telemetry_enabled = Some(enabled);
    config::save(&cfg)?;
    Ok(())
}
