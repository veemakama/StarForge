#![allow(clippy::items_after_test_module)]

use crate::utils::crypto;
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Validates that a string is a well-formed Stellar Ed25519 public key.
///
/// A valid Stellar public key:
/// - Starts with 'G'
/// - Is exactly 56 characters long
/// - Contains only valid base32 characters (A-Z, 2-7)
///
/// Returns `Ok(())` if the key is valid, or an error with a descriptive message.
pub fn validate_public_key(key: &str) -> Result<()> {
    if !key.starts_with('G') {
        anyhow::bail!(
            "Invalid public key: must start with 'G'.\n  \
             A valid Stellar public key looks like: GABC...XYZ (56 characters, starting with G)."
        );
    }

    if key.len() != 56 {
        anyhow::bail!(
            "Invalid public key: expected 56 characters, got {}.\n  \
             A valid Stellar public key is exactly 56 characters long.",
            key.len()
        );
    }

    // Validate base32 character set (A-Z, 2-7)
    if let Some(bad_char) = key.chars().find(|c| !matches!(c, 'A'..='Z' | '2'..='7')) {
        anyhow::bail!(
            "Invalid public key: contains invalid character '{}'.\n  \
             A valid Stellar public key uses only uppercase letters A-Z and digits 2-7.",
            bad_char
        );
    }
    Ok(())
}

/// Validates a Soroban contract ID.
/// Must start with 'C', be exactly 56 chars long, and use valid base32 chars.
pub fn validate_contract_id(id: &str) -> Result<()> {
    if !id.starts_with('C') {
        anyhow::bail!("Invalid contract ID: must start with 'C'.");
    }
    if id.len() != 56 {
        anyhow::bail!(
            "Invalid contract ID: expected 56 characters, got {}.",
            id.len()
        );
    }
    if let Some(bad_char) = id.chars().find(|c| !matches!(c, 'A'..='Z' | '2'..='7')) {
        anyhow::bail!(
            "Invalid contract ID: contains invalid character '{}'.",
            bad_char
        );
    }
    Ok(())
}

/// Validates a file path exists and optionally matches an extension.
pub fn validate_file_path(path: &std::path::Path, expected_ext: Option<&str>) -> Result<()> {
    if !path.exists() {
        anyhow::bail!("Path does not exist: {}", path.display());
    }
    if !path.is_file() {
        anyhow::bail!("Path is not a file: {}", path.display());
    }
    if let Some(ext) = expected_ext {
        if path.extension().and_then(|e| e.to_str()) != Some(ext) {
            anyhow::bail!("Invalid file type: expected '{}' extension.", ext);
        }
    }
    Ok(())
}

/// Validates network setting.
pub fn validate_network(network: &str) -> Result<()> {
    match network {
        "testnet" | "mainnet" | "docker-testnet" => Ok(()),
        _ => {
            let cfg = load()?;
            if cfg.networks.contains_key(network) {
                Ok(())
            } else {
                anyhow::bail!(
                    "Unsupported network '{}'. Use 'testnet', 'mainnet', 'docker-testnet', or a configured custom network.",
                    network
                )
            }
        }
    }
}

/// Validates a Stellar secret key or encrypted bundle.
pub fn validate_secret_key(secret: &str) -> Result<()> {
    if secret.contains(':') {
        let parts: Vec<&str> = secret.split(':').collect();
        // Accept:
        // - 3-part (legacy: salt:nonce:ciphertext)
        // - 5-part (KDF without p_cost: salt:nonce:ciphertext:mem:iterations)
        // - 6-part (KDF with p_cost: salt:nonce:ciphertext:mem:iterations:parallelism)
        if parts.len() != 3 && parts.len() != 5 && parts.len() != 6 {
            anyhow::bail!(
                "Invalid encrypted secret bundle format: expected 3, 5, or 6 parts, got {}",
                parts.len()
            );
        }

        // Validate base64 parts (first 3 parts are always base64)
        for part in parts.iter().take(3) {
            BASE64
                .decode(part)
                .map_err(|_| anyhow::anyhow!("Invalid base64 in encrypted secret bundle"))?;
        }

        // If 5 or 6-part bundle, validate KDF parameters are valid u32
        if parts.len() >= 5 {
            parts[3]
                .parse::<u32>()
                .map_err(|_| anyhow::anyhow!("Invalid KDF memory cost: must be a valid u32"))?;
            parts[4]
                .parse::<u32>()
                .map_err(|_| anyhow::anyhow!("Invalid KDF iteration count: must be a valid u32"))?;
        }
        if parts.len() == 6 {
            parts[5].parse::<u32>().map_err(|_| {
                anyhow::anyhow!("Invalid KDF parallelism factor: must be a valid u32")
            })?;
        }

        return Ok(());
    }

    if !secret.starts_with('S') {
        anyhow::bail!("Invalid secret key: must start with 'S'.");
    }
    if secret.len() != 56 {
        anyhow::bail!(
            "Invalid secret key: expected 56 characters, got {}.",
            secret.len()
        );
    }
    if let Some(bad_char) = secret.chars().find(|c| !matches!(c, 'A'..='Z' | '2'..='7')) {
        anyhow::bail!(
            "Invalid secret key: contains invalid character '{}'.",
            bad_char
        );
    }
    Ok(())
}

/// Validates that a network exists in the current configuration.
pub fn validate_network_exists(cfg: &Config, network: &str) -> Result<()> {
    if cfg.networks.contains_key(network) {
        return Ok(());
    }
    validate_network(network)
}

/// Validates an amount string parses to a positive f64.
pub fn validate_amount(amount: &str) -> Result<f64> {
    let amt: f64 = amount
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid amount format: '{}'", amount))?;
    if amt <= 0.0 {
        anyhow::bail!("Amount must be strictly positive, got {}", amt);
    }
    Ok(amt)
}

/// Validates a wallet name.
/// Must not be empty and must contain only alphanumeric chars, dashes, or underscores.
pub fn validate_wallet_name(name: &str) -> Result<()> {
    if name.is_empty() {
        anyhow::bail!("Wallet name cannot be empty.");
    }
    if let Some(bad_char) = name
        .chars()
        .find(|c| !c.is_alphanumeric() && *c != '-' && *c != '_')
    {
        anyhow::bail!("Invalid wallet name '{}': contains invalid character '{}'. Use alphanumeric, dash, or underscore.", name, bad_char);
    }
    Ok(())
}

/// Validates the full configuration schema and wallet entries.
pub fn validate_config(cfg: &Config) -> Result<()> {
    if cfg.version.is_empty() {
        anyhow::bail!("Config version is missing");
    }

    if cfg.network.trim().is_empty() {
        anyhow::bail!("Active network is not set");
    }

    validate_network_exists(cfg, &cfg.network)?;

    if cfg.networks.is_empty() {
        anyhow::bail!("No networks configured");
    }

    for (name, net_cfg) in &cfg.networks {
        validate_endpoint_url(
            &net_cfg.horizon_url,
            &format!("network '{}'.horizon_url", name),
        )?;
        if let Some(ref soroban_url) = net_cfg.soroban_rpc_url {
            validate_endpoint_url(soroban_url, &format!("network '{}'.soroban_rpc_url", name))?;
        }
        if let Some(ref friendbot_url) = net_cfg.friendbot_url {
            validate_endpoint_url(friendbot_url, &format!("network '{}'.friendbot_url", name))?;
        }
    }

    for wallet in &cfg.wallets {
        validate_wallet_name(&wallet.name)?;
        validate_public_key(&wallet.public_key)?;
        if let Some(ref secret) = wallet.secret_key {
            validate_secret_key(secret)?;
        }
        validate_network_exists(cfg, &wallet.network)?;
    }

    for source in &cfg.plugin_trust.trusted_sources {
        validate_plugin_trust_source(source)?;
    }

    Ok(())
}

fn validate_endpoint_url(url: &str, label: &str) -> Result<()> {
    if url.starts_with("http://") || url.starts_with("https://") {
        Ok(())
    } else {
        anyhow::bail!(
            "Invalid {}: must start with http:// or https:// (got '{}')",
            label,
            url
        )
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    #[serde(default = "default_version")]
    pub version: String,
    pub network: String,
    pub wallets: Vec<WalletEntry>,
    #[serde(default)]
    pub networks: std::collections::HashMap<String, NetworkConfig>,
    #[serde(default)]
    pub plugin_trust: PluginTrustConfig,
    pub telemetry_enabled: Option<bool>,
    pub wallet_encryption: Option<crypto::KdfOptions>,
}

fn default_version() -> String {
    "1".to_string()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkConfig {
    pub horizon_url: String,
    pub soroban_rpc_url: Option<String>,
    pub friendbot_url: Option<String>,
    #[serde(default)]
    pub passphrase: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct PluginTrustConfig {
    /// Trusted plugin source allowlist entries. Entries may be domains
    /// (`plugins.example.com`) or URL prefixes (`https://plugins.example.com/releases/`).
    #[serde(default = "default_trusted_plugin_sources")]
    pub trusted_sources: Vec<String>,
}

impl Default for PluginTrustConfig {
    fn default() -> Self {
        Self {
            trusted_sources: default_trusted_plugin_sources(),
        }
    }
}

pub fn default_trusted_plugin_sources() -> Vec<String> {
    vec![
        "https://github.com/Nanle-code/starforge-*".to_string(),
        "https://github.com/StarForge-Labs/*".to_string(),
        "https://crates.io/crates/starforge-plugin-*".to_string(),
    ]
}

pub fn validate_plugin_trust_source(source: &str) -> Result<()> {
    let source = source.trim();
    if source.is_empty() {
        anyhow::bail!("Trusted plugin source cannot be empty");
    }
    if source.chars().any(char::is_whitespace) {
        anyhow::bail!("Trusted plugin source cannot contain whitespace");
    }

    let wildcard_count = source.matches('*').count();
    if wildcard_count > 1 || (wildcard_count == 1 && !source.ends_with('*')) {
        anyhow::bail!("Trusted plugin source may only use '*' as a trailing wildcard");
    }

    let without_wildcard = source.strip_suffix('*').unwrap_or(source);
    if without_wildcard.contains("://") {
        let scheme = without_wildcard
            .split_once("://")
            .map(|(scheme, _)| scheme.to_ascii_lowercase())
            .unwrap_or_default();
        if !matches!(scheme.as_str(), "http" | "https" | "git+https") {
            anyhow::bail!("Trusted plugin source URL must use http, https, or git+https scheme");
        }
        let after_scheme = without_wildcard
            .split_once("://")
            .map(|(_, rest)| rest)
            .unwrap_or("");
        let host = after_scheme
            .split(['/', '?', '#'])
            .next()
            .unwrap_or("")
            .rsplit('@')
            .next()
            .unwrap_or("")
            .split(':')
            .next()
            .unwrap_or("");
        if host.is_empty() || host.starts_with('.') || host.ends_with('.') {
            anyhow::bail!("Trusted plugin source URL must include a valid host");
        }
        return Ok(());
    }

    let domain = without_wildcard.trim_start_matches("*.");
    if domain.contains('/')
        || domain.contains(':')
        || domain.starts_with('.')
        || domain.ends_with('.')
    {
        anyhow::bail!("Trusted plugin domain must be a domain name, not a path or URL fragment");
    }
    if domain.is_empty() || !domain.contains('.') {
        anyhow::bail!("Trusted plugin domain must include a dot, such as plugins.example.com");
    }
    if !domain
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
    {
        anyhow::bail!("Trusted plugin domain contains invalid characters");
    }

    Ok(())
}

pub fn add_trusted_plugin_source(config: &mut Config, source: String) -> Result<bool> {
    validate_plugin_trust_source(&source)?;
    let source = source.trim().to_string();
    if config
        .plugin_trust
        .trusted_sources
        .iter()
        .any(|existing| existing.eq_ignore_ascii_case(&source))
    {
        return Ok(false);
    }
    config.plugin_trust.trusted_sources.push(source);
    config
        .plugin_trust
        .trusted_sources
        .sort_by_key(|entry| entry.to_ascii_lowercase());
    Ok(true)
}

pub fn remove_trusted_plugin_source(config: &mut Config, source: &str) -> bool {
    let before = config.plugin_trust.trusted_sources.len();
    config
        .plugin_trust
        .trusted_sources
        .retain(|existing| !existing.eq_ignore_ascii_case(source.trim()));
    before != config.plugin_trust.trusted_sources.len()
}

pub fn reset_trusted_plugin_sources(config: &mut Config) {
    config.plugin_trust.trusted_sources = default_trusted_plugin_sources();
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WalletEntry {
    pub name: String,
    pub public_key: String,
    pub secret_key: Option<String>,
    pub network: String,
    pub created_at: String,
    pub funded: bool,
    #[serde(default)]
    pub rotation_history: Vec<WalletRotationRecord>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WalletRotationRecord {
    pub rotated_at: String,
    pub previous_public_key: String,
    pub previous_network: String,
    pub previous_funded: bool,
    /// The previous secret key (plaintext or encrypted bundle), preserved when
    /// `--backup` is passed to `wallet rotate`.  `None` when not requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_secret_key: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        let mut networks = HashMap::new();
        networks.insert(
            "testnet".to_string(),
            NetworkConfig {
                horizon_url: "https://horizon-testnet.stellar.org".to_string(),
                soroban_rpc_url: Some("https://soroban-testnet.stellar.org".to_string()),
                friendbot_url: Some("https://friendbot.stellar.org".to_string()),
                passphrase: Some("Test SDF Network ; September 2015".to_string()),
            },
        );
        networks.insert(
            "mainnet".to_string(),
            NetworkConfig {
                horizon_url: "https://horizon.stellar.org".to_string(),
                soroban_rpc_url: Some("https://mainnet.sorobanrpc.com".to_string()),
                friendbot_url: None,
                passphrase: Some("Public Global Stellar Network ; September 2015".to_string()),
            },
        );
        networks.insert(
            "docker-testnet".to_string(),
            NetworkConfig {
                horizon_url: "http://localhost:8000".to_string(),
                soroban_rpc_url: Some("http://localhost:8000/rpc".to_string()),
                friendbot_url: None,
                passphrase: Some("Test SDF Network ; September 2015".to_string()),
            },
        );

        Self {
            version: "1".to_string(),
            network: "testnet".to_string(),
            wallets: vec![],
            networks,
            plugin_trust: PluginTrustConfig::default(),
            telemetry_enabled: Some(true),
            wallet_encryption: None,
        }
    }
}

const CURRENT_CONFIG_VERSION: &str = "1";

pub fn migrate_config(mut config: Config) -> Result<Config> {
    let config_version = config.version.as_str();

    if config_version == CURRENT_CONFIG_VERSION {
        return Ok(config);
    }

    // Create backup before migration
    backup_config(&config)?;

    // Apply migrations in sequence
    match config_version {
        "" | "0" => {
            // Migration from v0 to v1: Add version field
            config.version = "1".to_string();
        }
        _ => {
            anyhow::bail!(
                "Unknown config version '{}'. Current version is '{}'.",
                config_version,
                CURRENT_CONFIG_VERSION
            );
        }
    }

    Ok(config)
}

fn backup_config(config: &Config) -> Result<()> {
    let backup_path = config_dir().join(format!(
        "config.backup.v{}.{}.toml",
        config.version,
        chrono::Utc::now().timestamp()
    ));

    let contents =
        toml::to_string_pretty(config).with_context(|| "Failed to serialize config for backup")?;

    fs::write(&backup_path, contents)
        .with_context(|| format!("Failed to write backup to {:?}", backup_path))?;

    Ok(())
}

#[allow(dead_code)]
pub fn rollback_config(version: &str) -> Result<()> {
    let config_dir = config_dir();
    let backup_pattern = format!("config.backup.v{}", version);

    let mut backups: Vec<_> = fs::read_dir(&config_dir)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(&backup_pattern)
        })
        .collect();

    if backups.is_empty() {
        anyhow::bail!("No backup found for version '{}'", version);
    }

    // Sort by timestamp (newest first)
    backups.sort_by_key(|b| std::cmp::Reverse(b.file_name()));

    let latest_backup = &backups[0];
    let backup_path = latest_backup.path();

    fs::copy(&backup_path, config_path())
        .with_context(|| format!("Failed to restore backup from {:?}", backup_path))?;

    Ok(())
}

pub fn config_dir() -> PathBuf {
    let home = dirs::home_dir().expect("Could not find home directory");
    home.join(".starforge")
}

pub fn get_data_dir() -> Result<PathBuf> {
    let dir = config_dir().join("data");
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    Ok(dir)
}

pub fn get_config_path() -> Result<PathBuf> {
    Ok(config_path())
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn load() -> Result<Config> {
    let path = config_path();
    if !path.exists() {
        return Ok(Config::default());
    }
    let contents = fs::read_to_string(&path).with_context(|| {
        format!(
            "Failed to read config file at '{}'.\n\
             Check that the file exists and is readable.",
            path.display()
        )
    })?;
    let mut config: Config = toml::from_str(&contents).with_context(|| {
        format!(
            "Failed to parse config file at '{}'.\n\
             The file may be corrupted or contain invalid TOML.\n\
             Run `starforge config doctor` to diagnose, or delete the file to reset to defaults.",
            path.display()
        )
    })?;

    // Migrate config if needed
    config = migrate_config(config)?;

    // Guarantee built-in networks are always present
    ensure_default_networks(&mut config);

    // Save migrated config
    if config.version != CURRENT_CONFIG_VERSION {
        save(&config)?;
    }

    Ok(config)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DoctorStatus {
    Pass,
    Fail,
}

#[derive(Debug, Clone)]
pub struct DoctorFinding {
    pub category: &'static str,
    pub status: DoctorStatus,
    pub message: String,
}

impl DoctorFinding {
    pub fn pass(category: &'static str, message: impl Into<String>) -> Self {
        Self {
            category,
            status: DoctorStatus::Pass,
            message: message.into(),
        }
    }

    pub fn fail(category: &'static str, message: impl Into<String>) -> Self {
        Self {
            category,
            status: DoctorStatus::Fail,
            message: message.into(),
        }
    }
}

/// Read and parse `config.toml` without migration or default-network injection.
pub fn parse_config_file() -> Result<Config> {
    let path = config_path();
    if !path.exists() {
        return Ok(Config::default());
    }
    let contents = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config at {}", path.display()))?;
    toml::from_str(&contents).with_context(|| "Failed to parse config.toml")
}

fn validate_service_url(url: &str, label: &str) -> Result<()> {
    if url.trim().is_empty() {
        anyhow::bail!("{label} cannot be empty");
    }
    if !url.starts_with("http://") && !url.starts_with("https://") {
        anyhow::bail!("{label} must use http or https");
    }
    Ok(())
}

/// Run structural validation checks against a loaded configuration.
pub fn validate_config_integrity(cfg: &Config) -> Vec<DoctorFinding> {
    let mut findings = Vec::new();

    if cfg.version == CURRENT_CONFIG_VERSION {
        findings.push(DoctorFinding::pass(
            "schema",
            format!("config version is {}", cfg.version),
        ));
    } else {
        findings.push(DoctorFinding::fail(
            "schema",
            format!(
                "unsupported config version '{}' (expected {})",
                cfg.version, CURRENT_CONFIG_VERSION
            ),
        ));
    }

    match validate_network_exists(cfg, &cfg.network) {
        Ok(()) => findings.push(DoctorFinding::pass(
            "network",
            format!("active network '{}' is configured", cfg.network),
        )),
        Err(e) => findings.push(DoctorFinding::fail("network", e.to_string())),
    }

    if cfg.wallets.is_empty() {
        findings.push(DoctorFinding::pass("wallet", "no wallets configured"));
    } else {
        let mut wallet_ok = true;
        let mut wallet_errors = Vec::new();
        for wallet in &cfg.wallets {
            let label = format!("wallet '{}'", wallet.name);
            if let Err(e) = validate_wallet_name(&wallet.name) {
                wallet_ok = false;
                wallet_errors.push(format!("{label}: {e}"));
            }
            if let Err(e) = validate_public_key(&wallet.public_key) {
                wallet_ok = false;
                wallet_errors.push(format!("{label} public key: {e}"));
            }
            if let Some(ref secret) = wallet.secret_key {
                if let Err(e) = validate_secret_key(secret) {
                    wallet_ok = false;
                    wallet_errors.push(format!("{label} secret key: {e}"));
                }
            }
            if let Err(e) = validate_network_exists(cfg, &wallet.network) {
                wallet_ok = false;
                wallet_errors.push(format!("{label} network: {e}"));
            }
        }
        if wallet_ok {
            findings.push(DoctorFinding::pass(
                "wallet",
                format!("{} wallet(s) validated", cfg.wallets.len()),
            ));
        } else {
            findings.push(DoctorFinding::fail("wallet", wallet_errors.join("; ")));
        }
    }

    let mut network_ok = true;
    let mut network_errors = Vec::new();
    for (name, net) in &cfg.networks {
        if let Err(e) = validate_service_url(&net.horizon_url, "horizon_url") {
            network_ok = false;
            network_errors.push(format!("network '{name}': {e}"));
        }
        if let Some(ref rpc) = net.soroban_rpc_url {
            if let Err(e) = validate_service_url(rpc, "soroban_rpc_url") {
                network_ok = false;
                network_errors.push(format!("network '{name}' soroban RPC: {e}"));
            }
        }
    }
    if network_ok {
        findings.push(DoctorFinding::pass(
            "network",
            format!("{} network(s) have valid endpoint URLs", cfg.networks.len()),
        ));
    } else {
        findings.push(DoctorFinding::fail("network", network_errors.join("; ")));
    }

    let mut trust_ok = true;
    let mut trust_errors = Vec::new();
    for source in &cfg.plugin_trust.trusted_sources {
        if let Err(e) = validate_plugin_trust_source(source) {
            trust_ok = false;
            trust_errors.push(format!("'{source}': {e}"));
        }
    }
    if trust_ok {
        findings.push(DoctorFinding::pass(
            "plugin_trust",
            format!(
                "{} trusted plugin source(s) validated",
                cfg.plugin_trust.trusted_sources.len()
            ),
        ));
    } else {
        findings.push(DoctorFinding::fail("plugin_trust", trust_errors.join("; ")));
    }

    if let Some(ref kdf) = cfg.wallet_encryption {
        let mut enc_ok = true;
        let mut enc_errors = Vec::new();
        for (field, value) in [
            ("mem", kdf.mem),
            ("iterations", kdf.iterations),
            ("parallelism", kdf.parallelism),
        ] {
            if let Some(v) = value {
                if v == 0 {
                    enc_ok = false;
                    enc_errors.push(format!("{field} must be > 0"));
                }
            }
        }
        if enc_ok {
            findings.push(DoctorFinding::pass(
                "encryption",
                "wallet encryption parameters are valid",
            ));
        } else {
            findings.push(DoctorFinding::fail("encryption", enc_errors.join("; ")));
        }
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_public_key() {
        // Well-formed Stellar public key (56 chars, starts with G, valid base32)
        let key = "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWNT";
        assert!(validate_public_key(key).is_ok());
    }

    #[test]
    fn test_rejects_key_not_starting_with_g() {
        let key = "SAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN";
        let err = validate_public_key(key).unwrap_err();
        assert!(err.to_string().contains("must start with 'G'"));
    }

    #[test]
    fn test_rejects_key_wrong_length() {
        let key = "GAAZI4TCR3TY5";
        let err = validate_public_key(key).unwrap_err();
        assert!(err.to_string().contains("expected 56 characters"));
    }

    #[test]
    fn test_rejects_key_invalid_characters() {
        // Lowercase letters are not valid base32
        let key = "Gaazi4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWNT";
        let err = validate_public_key(key).unwrap_err();
        assert!(err.to_string().contains("invalid character"));
    }

    #[test]
    fn test_rejects_empty_key() {
        let err = validate_public_key("").unwrap_err();
        assert!(err.to_string().contains("must start with 'G'"));
    }

    #[test]
    fn test_valid_contract_id() {
        let id = "CAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWNW";
        assert!(validate_contract_id(id).is_ok());
    }

    #[test]
    fn test_rejects_contract_id_not_starting_with_c() {
        // Starts with 'G'
        let id = "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWNW";
        let err = validate_contract_id(id).unwrap_err();
        assert!(err.to_string().contains("must start with 'C'"));
    }

    #[test]
    fn test_valid_amount() {
        assert_eq!(validate_amount("10.5").unwrap(), 10.5);
        assert_eq!(validate_amount("1").unwrap(), 1.0);
    }

    #[test]
    fn test_invalid_amount() {
        assert!(validate_amount("-5").is_err());
        assert!(validate_amount("0").is_err());
        assert!(validate_amount("abc").is_err());
    }

    #[test]
    fn test_valid_wallet_name() {
        assert!(validate_wallet_name("alice-123_DEPLOY").is_ok());
    }

    #[test]
    fn test_invalid_wallet_name() {
        assert!(validate_wallet_name("").is_err());
        assert!(validate_wallet_name("alice!").is_err());
        assert!(validate_wallet_name("my wallet").is_err());
    }

    #[test]
    fn test_valid_plain_secret_key() {
        let Ok(secret) = std::env::var("STARFORGE_TEST_SECRET_KEY") else {
            eprintln!("skipping test_valid_plain_secret_key: STARFORGE_TEST_SECRET_KEY is not set");
            return;
        };
        assert!(validate_secret_key(&secret).is_ok());
    }

    #[test]
    fn test_valid_encrypted_secret_bundle() {
        let salt = BASE64.encode([0u8; 16]);
        let nonce = BASE64.encode([1u8; 12]);
        let cipher = BASE64.encode([2u8; 32]);
        let bundle = format!("{}:{}:{}", salt, nonce, cipher);
        assert!(validate_secret_key(&bundle).is_ok());

        // 5-part
        let bundle_5 = format!("{}:{}:{}:32768:4", salt, nonce, cipher);
        assert!(validate_secret_key(&bundle_5).is_ok());

        // 6-part
        let bundle_6 = format!("{}:{}:{}:32768:4:2", salt, nonce, cipher);
        assert!(validate_secret_key(&bundle_6).is_ok());
    }

    #[test]
    fn test_invalid_secret_key() {
        assert!(validate_secret_key("not-a-key").is_err());
        assert!(validate_secret_key("S123").is_err());
        assert!(validate_secret_key("bad:bundle").is_err());
    }

    #[test]
    fn validate_config_accepts_default_config() {
        let cfg = Config::default();
        assert!(validate_config(&cfg).is_ok());
    }

    #[test]
    fn validate_config_rejects_missing_active_network() {
        let cfg = Config {
            network: "unknown-net".to_string(),
            ..Default::default()
        };
        let err = validate_config(&cfg).unwrap_err();
        assert!(err.to_string().contains("unknown-net"));
    }

    #[test]
    fn validate_config_rejects_invalid_horizon_url() {
        let mut cfg = Config::default();
        cfg.networks.get_mut("testnet").unwrap().horizon_url = "ftp://bad.example.com".to_string();
        let err = validate_config(&cfg).unwrap_err();
        assert!(err.to_string().contains("horizon_url"));
    }

    #[test]
    fn default_config_includes_plugin_trust_sources() {
        let cfg = Config::default();
        assert_eq!(
            cfg.plugin_trust.trusted_sources,
            default_trusted_plugin_sources()
        );
    }

    #[test]
    fn config_without_plugin_trust_deserializes_with_defaults() {
        let toml = r#"
version = "1"
network = "testnet"
wallets = []
telemetry_enabled = true
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(
            cfg.plugin_trust.trusted_sources,
            default_trusted_plugin_sources()
        );
    }

    #[test]
    fn trusted_plugin_source_management_deduplicates_and_resets() {
        let mut cfg = Config::default();
        assert!(add_trusted_plugin_source(&mut cfg, "plugins.example.com".to_string()).unwrap());
        assert!(!add_trusted_plugin_source(&mut cfg, "PLUGINS.EXAMPLE.COM".to_string()).unwrap());
        assert!(cfg
            .plugin_trust
            .trusted_sources
            .contains(&"plugins.example.com".to_string()));

        assert!(remove_trusted_plugin_source(
            &mut cfg,
            "plugins.example.com"
        ));
        assert!(!remove_trusted_plugin_source(
            &mut cfg,
            "plugins.example.com"
        ));

        cfg.plugin_trust.trusted_sources.clear();
        reset_trusted_plugin_sources(&mut cfg);
        assert_eq!(
            cfg.plugin_trust.trusted_sources,
            default_trusted_plugin_sources()
        );
    }

    #[test]
    fn invalid_trusted_plugin_sources_are_rejected() {
        for source in [
            "",
            "plugins example.com",
            "https://",
            "ftp://example.com",
            "example",
            "example.com/path",
            "https://example.com/*/bad",
        ] {
            assert!(
                validate_plugin_trust_source(source).is_err(),
                "{source} should be invalid"
            );
        }
    }

    #[test]
    fn validate_config_integrity_passes_default_config() {
        let cfg = Config::default();
        let findings = validate_config_integrity(&cfg);
        assert!(
            findings.iter().all(|f| f.status == DoctorStatus::Pass),
            "expected all pass, got: {:?}",
            findings
        );
    }

    #[test]
    fn validate_config_integrity_catches_bad_wallet_key() {
        let mut cfg = Config::default();
        cfg.wallets.push(WalletEntry {
            name: "bad".to_string(),
            public_key: "not-a-key".to_string(),
            secret_key: None,
            network: "testnet".to_string(),
            created_at: String::new(),
            funded: false,
            rotation_history: Vec::new(),
        });
        let findings = validate_config_integrity(&cfg);
        assert!(
            findings
                .iter()
                .any(|f| f.category == "wallet" && f.status == DoctorStatus::Fail),
            "expected wallet failure, got: {:?}",
            findings
        );
    }
}

/// Returns the network passphrase for transaction signing.
/// Checks the config for a custom passphrase; falls back to well-known defaults.
pub fn get_network_passphrase(network: &str) -> String {
    if let Ok(cfg) = load() {
        if let Some(net_cfg) = cfg.networks.get(network) {
            if let Some(passphrase) = &net_cfg.passphrase {
                return passphrase.clone();
            }
        }
    }
    match network {
        "mainnet" => "Public Global Stellar Network ; September 2015".to_string(),
        _ => "Test SDF Network ; September 2015".to_string(),
    }
}

/// Ensures the three built-in networks are present in the config's network map.
/// Safe to call on any Config — existing entries are never overwritten.
pub fn ensure_default_networks(cfg: &mut Config) {
    cfg.networks
        .entry("testnet".to_string())
        .or_insert_with(|| NetworkConfig {
            horizon_url: "https://horizon-testnet.stellar.org".to_string(),
            soroban_rpc_url: Some("https://soroban-testnet.stellar.org".to_string()),
            friendbot_url: Some("https://friendbot.stellar.org".to_string()),
            passphrase: Some("Test SDF Network ; September 2015".to_string()),
        });
    cfg.networks
        .entry("mainnet".to_string())
        .or_insert_with(|| NetworkConfig {
            horizon_url: "https://horizon.stellar.org".to_string(),
            soroban_rpc_url: Some("https://mainnet.sorobanrpc.com".to_string()),
            friendbot_url: None,
            passphrase: Some("Public Global Stellar Network ; September 2015".to_string()),
        });
    cfg.networks
        .entry("docker-testnet".to_string())
        .or_insert_with(|| NetworkConfig {
            horizon_url: "http://localhost:8000".to_string(),
            soroban_rpc_url: Some("http://localhost:8000/rpc".to_string()),
            friendbot_url: None,
            passphrase: Some("Test SDF Network ; September 2015".to_string()),
        });
}

pub fn save(config: &Config) -> Result<()> {
    let dir = config_dir();
    if !dir.exists() {
        fs::create_dir_all(&dir)
            .with_context(|| format!("Failed to create config dir {:?}", dir))?;
    }
    let contents = toml::to_string_pretty(config).with_context(|| "Failed to serialize config")?;
    fs::write(config_path(), contents).with_context(|| "Failed to write config file")?;
    Ok(())
}

pub fn get_network_config(cfg: &Config, network: &str) -> Result<NetworkConfig> {
    cfg.networks
        .get(network)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Network '{}' not found in configuration", network))
}

pub const RESERVED_NETWORKS: &[&str] = &["testnet", "mainnet", "docker-testnet"];

/// Returns true for built-in networks that cannot be removed or renamed.
pub fn is_reserved_network(name: &str) -> bool {
    RESERVED_NETWORKS.contains(&name)
}

pub fn add_custom_network(
    config: &mut Config,
    name: String,
    horizon_url: String,
    soroban_rpc_url: Option<String>,
    friendbot_url: Option<String>,
    passphrase: Option<String>,
) -> Result<()> {
    if is_reserved_network(&name) {
        anyhow::bail!(
            "'{}' is a reserved network name ('testnet', 'mainnet', 'docker-testnet'). Choose a different name.",
            name
        );
    }
    if config.networks.contains_key(&name) {
        anyhow::bail!("Network '{}' already exists", name);
    }
    config.networks.insert(
        name,
        NetworkConfig {
            horizon_url,
            soroban_rpc_url,
            friendbot_url,
            passphrase,
        },
    );
    Ok(())
}

/// Remove a custom network from config. Built-in networks are protected.
pub fn remove_custom_network(config: &mut Config, name: &str) -> Result<()> {
    if is_reserved_network(name) {
        anyhow::bail!(
            "'{}' is a built-in network and cannot be removed. Only custom networks can be removed.",
            name
        );
    }
    if !config.networks.contains_key(name) {
        anyhow::bail!("Network '{}' not found", name);
    }
    // Only remove if it is not a built-in re-injected entry (custom keys are user-added).
    config.networks.remove(name);

    if config.network == name {
        config.network = "testnet".to_string();
    }

    for wallet in &mut config.wallets {
        if wallet.network == name {
            wallet.network = config.network.clone();
        }
    }

    Ok(())
}

/// Rename a custom network. Built-in networks cannot be renamed.
pub fn rename_custom_network(config: &mut Config, old_name: &str, new_name: &str) -> Result<()> {
    if is_reserved_network(old_name) {
        anyhow::bail!(
            "'{}' is a built-in network and cannot be renamed.",
            old_name
        );
    }
    if is_reserved_network(new_name) {
        anyhow::bail!(
            "'{}' is a reserved network name. Choose a different name.",
            new_name
        );
    }
    if !config.networks.contains_key(old_name) {
        anyhow::bail!("Network '{}' not found", old_name);
    }
    if config.networks.contains_key(new_name) {
        anyhow::bail!("Network '{}' already exists", new_name);
    }
    if old_name == new_name {
        anyhow::bail!("Old and new network names are the same");
    }

    let net_cfg = config.networks.remove(old_name).expect("network exists");
    config.networks.insert(new_name.to_string(), net_cfg);

    if config.network == old_name {
        config.network = new_name.to_string();
    }

    for wallet in &mut config.wallets {
        if wallet.network == old_name {
            wallet.network = new_name.to_string();
        }
    }

    Ok(())
}
