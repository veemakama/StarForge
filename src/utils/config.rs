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
        // Accept both 3-part (legacy: salt:nonce:ciphertext) and 5-part (with KDF: salt:nonce:ciphertext:mem:iterations)
        if parts.len() != 3 && parts.len() != 5 {
            anyhow::bail!("Invalid encrypted secret bundle format: expected 3 or 5 parts, got {}", parts.len());
        }
        
        // Validate base64 parts (first 3 parts are always base64)
        for i in 0..3 {
            BASE64
                .decode(parts[i])
                .map_err(|_| anyhow::anyhow!("Invalid base64 in encrypted secret bundle at part {}", i))?;
        }
        
        // If 5-part bundle, validate KDF parameters are valid u32
        if parts.len() == 5 {
            parts[3]
                .parse::<u32>()
                .map_err(|_| anyhow!("Invalid KDF memory cost: must be a valid u32"))?;
            parts[4]
                .parse::<u32>()
                .map_err(|_| anyhow!("Invalid KDF iteration count: must be a valid u32"))?;
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    #[serde(default = "default_version")]
    pub version: String,
    pub network: String,
    pub wallets: Vec<WalletEntry>,
    #[serde(default)]
    pub networks: std::collections::HashMap<String, NetworkConfig>,
    pub telemetry_enabled: Option<bool>,
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
            telemetry_enabled: Some(true),
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

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn load() -> Result<Config> {
    let path = config_path();
    if !path.exists() {
        return Ok(Config::default());
    }
    let contents = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config at {:?}", path))?;
    let mut config: Config =
        toml::from_str(&contents).with_context(|| "Failed to parse config file")?;

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
    }

    #[test]
    fn test_invalid_secret_key() {
        assert!(validate_secret_key("not-a-key").is_err());
        assert!(validate_secret_key("S123").is_err());
        assert!(validate_secret_key("bad:bundle").is_err());
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
    cfg.networks.entry("testnet".to_string()).or_insert_with(|| NetworkConfig {
        horizon_url: "https://horizon-testnet.stellar.org".to_string(),
        soroban_rpc_url: Some("https://soroban-testnet.stellar.org".to_string()),
        friendbot_url: Some("https://friendbot.stellar.org".to_string()),
        passphrase: Some("Test SDF Network ; September 2015".to_string()),
    });
    cfg.networks.entry("mainnet".to_string()).or_insert_with(|| NetworkConfig {
        horizon_url: "https://horizon.stellar.org".to_string(),
        soroban_rpc_url: Some("https://mainnet.sorobanrpc.com".to_string()),
        friendbot_url: None,
        passphrase: Some("Public Global Stellar Network ; September 2015".to_string()),
    });
    cfg.networks.entry("docker-testnet".to_string()).or_insert_with(|| NetworkConfig {
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

const RESERVED_NETWORKS: &[&str] = &["testnet", "mainnet", "docker-testnet"];

pub fn add_custom_network(
    config: &mut Config,
    name: String,
    horizon_url: String,
    soroban_rpc_url: Option<String>,
    friendbot_url: Option<String>,
    passphrase: Option<String>,
) -> Result<()> {
    if RESERVED_NETWORKS.contains(&name.as_str()) {
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
