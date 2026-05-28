use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct TutorialStatus {
    pub active: Option<String>,
    pub started_at: Option<String>,
}

fn status_path() -> Result<PathBuf> {
    let base =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Unable to resolve home directory"))?;
    Ok(base.join(".starforge").join("tutorial_status.json"))
}

pub fn load_status() -> Result<TutorialStatus> {
    let path = status_path()?;
    if !path.exists() {
        return Ok(TutorialStatus::default());
    }
    let bytes = fs::read(&path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn save_status(status: &TutorialStatus) -> Result<()> {
    let path = status_path()?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let bytes = serde_json::to_vec_pretty(status)?;
    fs::write(&path, bytes)?;
    Ok(())
}

pub fn tutorials_dir(repo_root: &Path) -> PathBuf {
    repo_root.join("tutorials")
}
