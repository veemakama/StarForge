use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use zip::write::FileOptions;
use zip::ZipWriter;

use crate::utils::config;
use crate::utils::crypto::{decrypt_secret, encrypt_secret};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BackupStatus {
    Created,
    Verified,
    VerificationFailed,
}

impl std::fmt::Display for BackupStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            BackupStatus::Created => "created",
            BackupStatus::Verified => "verified",
            BackupStatus::VerificationFailed => "verification-failed",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupRecord {
    pub id: String,
    pub label: String,
    pub created_at: String,
    pub sources: Vec<String>,
    pub archive_path: PathBuf,
    pub encrypted: bool,
    pub checksum: String,
    pub size_bytes: u64,
    pub status: BackupStatus,
    pub verified_at: Option<String>,
    pub replicated_regions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationConfig {
    pub label: String,
    pub sources: Vec<String>,
    pub interval_hours: u64,
    pub encrypt: bool,
    pub region: String,
    pub last_run: Option<String>,
}

fn backups_dir() -> Result<PathBuf> {
    let dir = config::config_dir().join("backups");
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    Ok(dir)
}

fn replicas_dir(region: &str) -> Result<PathBuf> {
    let dir = backups_dir()?.join("replicas").join(region);
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    Ok(dir)
}

fn records_file() -> Result<PathBuf> {
    Ok(backups_dir()?.join("records.json"))
}

fn automation_file() -> Result<PathBuf> {
    Ok(backups_dir()?.join("automation.json"))
}

pub fn list_backups() -> Result<Vec<BackupRecord>> {
    let path = records_file()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&raw).unwrap_or_default())
}

fn save_records(records: &[BackupRecord]) -> Result<()> {
    fs::write(records_file()?, serde_json::to_string_pretty(records)?)?;
    Ok(())
}

pub fn load_backup(id: &str) -> Result<BackupRecord> {
    list_backups()?
        .into_iter()
        .find(|r| r.id == id || r.id.starts_with(id))
        .ok_or_else(|| anyhow::anyhow!("No backup found with ID prefix '{}'", id))
}

fn zip_sources(sources: &[PathBuf]) -> Result<Vec<u8>> {
    let buf = Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(buf);
    let options: FileOptions =
        FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    for src in sources {
        if !src.exists() {
            anyhow::bail!("Backup source not found: {}", src.display());
        }
        add_path_to_zip(&mut zip, src, src, options)?;
    }

    let buf = zip.finish()?;
    Ok(buf.into_inner())
}

fn add_path_to_zip<W: Write + std::io::Seek>(
    zip: &mut ZipWriter<W>,
    base: &Path,
    path: &Path,
    options: FileOptions,
) -> Result<()> {
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            add_path_to_zip(zip, base, &entry.path(), options)?;
        }
    } else {
        let rel = path.strip_prefix(base.parent().unwrap_or(base)).unwrap_or(path);
        zip.start_file(rel.to_string_lossy(), options)?;
        let mut f = fs::File::open(path)?;
        let mut contents = Vec::new();
        f.read_to_end(&mut contents)?;
        zip.write_all(&contents)?;
    }
    Ok(())
}

pub fn create_backup(
    sources: &[PathBuf],
    label: &str,
    encrypt: bool,
    passphrase: Option<&str>,
    region: &str,
) -> Result<BackupRecord> {
    if encrypt && passphrase.is_none() {
        anyhow::bail!("A passphrase is required to create an encrypted backup");
    }

    let archive_bytes = zip_sources(sources)?;
    let checksum = hex::encode(Sha256::digest(&archive_bytes));
    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    let (filename, write_bytes): (String, Vec<u8>) = if encrypt {
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &archive_bytes);
        let bundle = encrypt_secret(passphrase.unwrap(), &b64, None)?;
        (format!("{}.bak.enc", id), bundle.into_bytes())
    } else {
        (format!("{}.zip", id), archive_bytes.clone())
    };

    let archive_path = backups_dir()?.join(&filename);
    fs::write(&archive_path, &write_bytes)
        .with_context(|| format!("Failed to write backup archive {}", archive_path.display()))?;

    let mut record = BackupRecord {
        id,
        label: label.to_string(),
        created_at: now,
        sources: sources.iter().map(|p| p.display().to_string()).collect(),
        archive_path,
        encrypted: encrypt,
        checksum,
        size_bytes: archive_bytes.len() as u64,
        status: BackupStatus::Created,
        verified_at: None,
        replicated_regions: Vec::new(),
    };

    replicate(&mut record, region)?;

    let mut records = list_backups()?;
    records.push(record.clone());
    save_records(&records)?;
    Ok(record)
}

fn replicate(record: &mut BackupRecord, region: &str) -> Result<()> {
    let dest_dir = replicas_dir(region)?;
    let filename = record
        .archive_path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Invalid archive path"))?;
    let dest = dest_dir.join(filename);
    fs::copy(&record.archive_path, &dest)?;
    record.replicated_regions.push(region.to_string());
    Ok(())
}

/// Replicate an existing backup to an additional region.
pub fn replicate_existing(id: &str, region: &str) -> Result<BackupRecord> {
    let mut records = list_backups()?;
    let record = records
        .iter_mut()
        .find(|r| r.id == id || r.id.starts_with(id))
        .ok_or_else(|| anyhow::anyhow!("No backup found with ID prefix '{}'", id))?;
    if record.replicated_regions.iter().any(|r| r == region) {
        anyhow::bail!("Backup already replicated to region '{}'", region);
    }
    replicate(record, region)?;
    let updated = record.clone();
    save_records(&records)?;
    Ok(updated)
}

fn read_archive_bytes(record: &BackupRecord, passphrase: Option<&str>) -> Result<Vec<u8>> {
    let raw = fs::read(&record.archive_path)
        .with_context(|| format!("Failed to read archive {}", record.archive_path.display()))?;
    if record.encrypted {
        let passphrase = passphrase
            .ok_or_else(|| anyhow::anyhow!("A passphrase is required to read this backup"))?;
        let bundle = String::from_utf8(raw).context("Encrypted backup file is not valid UTF-8")?;
        let b64 = decrypt_secret(passphrase, &bundle)?;
        let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64)
            .context("Failed to decode decrypted backup payload")?;
        Ok(bytes)
    } else {
        Ok(raw)
    }
}

/// Verify a backup's archive integrity by recomputing its checksum.
pub fn verify_backup(id: &str, passphrase: Option<&str>) -> Result<BackupRecord> {
    let mut records = list_backups()?;
    let record = records
        .iter_mut()
        .find(|r| r.id == id || r.id.starts_with(id))
        .ok_or_else(|| anyhow::anyhow!("No backup found with ID prefix '{}'", id))?;

    let bytes = read_archive_bytes(record, passphrase)?;
    let checksum = hex::encode(Sha256::digest(&bytes));

    record.status = if checksum == record.checksum {
        BackupStatus::Verified
    } else {
        BackupStatus::VerificationFailed
    };
    record.verified_at = Some(Utc::now().to_rfc3339());
    let updated = record.clone();
    save_records(&records)?;

    if updated.status == BackupStatus::VerificationFailed {
        anyhow::bail!("Backup '{}' failed integrity verification", updated.id);
    }
    Ok(updated)
}

/// Restore a backup's archive contents into `dest_dir`.
pub fn restore_backup(id: &str, dest_dir: &Path, passphrase: Option<&str>) -> Result<Vec<String>> {
    let record = load_backup(id)?;
    let bytes = read_archive_bytes(&record, passphrase)?;
    extract_zip(&bytes, dest_dir)
}

fn extract_zip(bytes: &[u8], dest_dir: &Path) -> Result<Vec<String>> {
    fs::create_dir_all(dest_dir)?;
    let reader = Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader)?;
    let mut extracted = Vec::new();

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = match file.enclosed_name() {
            Some(path) => dest_dir.join(path),
            None => continue,
        };
        if file.is_dir() {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(parent) = outpath.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut outfile = fs::File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
            extracted.push(outpath.display().to_string());
        }
    }
    Ok(extracted)
}

/// Restore a backup into a temporary directory and confirm the extracted files exist,
/// without disturbing the real environment. Used for periodic recovery testing.
pub fn test_restore(id: &str, passphrase: Option<&str>) -> Result<usize> {
    let tmp = tempfile::tempdir()?;
    let extracted = restore_backup(id, tmp.path(), passphrase)?;
    for path in &extracted {
        if !Path::new(path).exists() {
            anyhow::bail!("Recovery test failed: expected file '{}' missing after restore", path);
        }
    }
    Ok(extracted.len())
}

/// Find the most recent backup for `label` created at or before `at`.
pub fn find_point_in_time(label: &str, at: DateTime<Utc>) -> Result<BackupRecord> {
    let mut candidates: Vec<BackupRecord> = list_backups()?
        .into_iter()
        .filter(|r| r.label == label)
        .filter(|r| {
            DateTime::parse_from_rfc3339(&r.created_at)
                .map(|dt| dt.with_timezone(&Utc) <= at)
                .unwrap_or(false)
        })
        .collect();
    candidates.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    candidates
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("No backup found for label '{}' at or before {}", label, at))
}

pub fn list_automation() -> Result<Vec<AutomationConfig>> {
    let path = automation_file()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&raw).unwrap_or_default())
}

fn save_automation(configs: &[AutomationConfig]) -> Result<()> {
    fs::write(automation_file()?, serde_json::to_string_pretty(configs)?)?;
    Ok(())
}

pub fn configure_automation(
    label: &str,
    sources: Vec<PathBuf>,
    interval_hours: u64,
    encrypt: bool,
    region: &str,
) -> Result<AutomationConfig> {
    let mut configs = list_automation()?;
    configs.retain(|c| c.label != label);
    let cfg = AutomationConfig {
        label: label.to_string(),
        sources: sources.iter().map(|p| p.display().to_string()).collect(),
        interval_hours,
        encrypt,
        region: region.to_string(),
        last_run: None,
    };
    configs.push(cfg.clone());
    save_automation(&configs)?;
    Ok(cfg)
}

/// Run any automated backup configs that are due. Returns labels backed up.
/// Encrypted automation configs require `passphrase` to be supplied.
pub fn run_automation(passphrase: Option<&str>) -> Result<Vec<String>> {
    let mut configs = list_automation()?;
    let mut ran = Vec::new();
    let now = Utc::now();

    for cfg in configs.iter_mut() {
        let due = match &cfg.last_run {
            None => true,
            Some(last) => DateTime::parse_from_rfc3339(last)
                .map(|dt| now.signed_duration_since(dt.with_timezone(&Utc)).num_hours() as u64 >= cfg.interval_hours)
                .unwrap_or(true),
        };
        if !due {
            continue;
        }
        let sources: Vec<PathBuf> = cfg.sources.iter().map(PathBuf::from).collect();
        create_backup(&sources, &cfg.label, cfg.encrypt, passphrase, &cfg.region)?;
        cfg.last_run = Some(now.to_rfc3339());
        ran.push(cfg.label.clone());
    }

    save_automation(&configs)?;
    Ok(ran)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_source_file(dir: &Path, name: &str, contents: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn zip_and_extract_roundtrip() {
        let src_dir = tempdir().unwrap();
        let f1 = make_source_file(src_dir.path(), "a.wasm", "hello-wasm");

        let bytes = zip_sources(&[f1.clone()]).unwrap();
        let out_dir = tempdir().unwrap();
        let extracted = extract_zip(&bytes, out_dir.path()).unwrap();
        assert_eq!(extracted.len(), 1);
        let contents = fs::read_to_string(&extracted[0]).unwrap();
        assert_eq!(contents, "hello-wasm");
    }

    #[test]
    fn checksum_changes_when_contents_change() {
        let dir = tempdir().unwrap();
        let f1 = make_source_file(dir.path(), "a.txt", "v1");
        let b1 = zip_sources(&[f1.clone()]).unwrap();
        let f2 = make_source_file(dir.path(), "a.txt", "v2");
        let b2 = zip_sources(&[f2]).unwrap();
        assert_ne!(
            hex::encode(Sha256::digest(&b1)),
            hex::encode(Sha256::digest(&b2))
        );
    }
}
