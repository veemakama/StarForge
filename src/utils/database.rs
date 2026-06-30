use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub fn db_path() -> PathBuf {
    crate::utils::config::config_dir().join("starforge.db")
}

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open() -> Result<Self> {
        let path = db_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&path)
            .with_context(|| format!("Failed to open database at {}", path.display()))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        Ok(Self { conn })
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        Ok(Self { conn })
    }

    pub fn initialize(&self) -> Result<()> {
        self.conn.execute_batch(SCHEMA)?;
        self.set_meta("schema_version", "1")?;
        Ok(())
    }

    fn set_meta(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_meta(&self, key: &str) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare("SELECT value FROM meta WHERE key = ?1")?;
        let mut rows = stmt.query(params![key])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    pub fn insert_wallet(&self, wallet: &WalletRow) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO wallets \
             (name, public_key, network, created_at, funded) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                wallet.name,
                wallet.public_key,
                wallet.network,
                wallet.created_at,
                wallet.funded,
            ],
        )?;
        Ok(())
    }

    pub fn list_wallets(&self) -> Result<Vec<WalletRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT name, public_key, network, created_at, funded FROM wallets ORDER BY created_at",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(WalletRow {
                name: row.get(0)?,
                public_key: row.get(1)?,
                network: row.get(2)?,
                created_at: row.get(3)?,
                funded: row.get(4)?,
            })
        })?;
        rows.map(|r| r.map_err(anyhow::Error::from)).collect()
    }

    pub fn get_wallet(&self, name: &str) -> Result<Option<WalletRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT name, public_key, network, created_at, funded FROM wallets WHERE name = ?1",
        )?;
        let mut rows = stmt.query(params![name])?;
        if let Some(row) = rows.next()? {
            Ok(Some(WalletRow {
                name: row.get(0)?,
                public_key: row.get(1)?,
                network: row.get(2)?,
                created_at: row.get(3)?,
                funded: row.get(4)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn delete_wallet(&self, name: &str) -> Result<usize> {
        Ok(self
            .conn
            .execute("DELETE FROM wallets WHERE name = ?1", params![name])?)
    }

    pub fn insert_network(&self, net: &NetworkRow) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO networks \
             (name, horizon_url, soroban_rpc_url, friendbot_url, passphrase) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                net.name,
                net.horizon_url,
                net.soroban_rpc_url,
                net.friendbot_url,
                net.passphrase,
            ],
        )?;
        Ok(())
    }

    pub fn list_networks(&self) -> Result<Vec<NetworkRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT name, horizon_url, soroban_rpc_url, friendbot_url, passphrase FROM networks ORDER BY name",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(NetworkRow {
                name: row.get(0)?,
                horizon_url: row.get(1)?,
                soroban_rpc_url: row.get(2)?,
                friendbot_url: row.get(3)?,
                passphrase: row.get(4)?,
            })
        })?;
        rows.map(|r| r.map_err(anyhow::Error::from)).collect()
    }

    pub fn insert_config_kv(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO config_kv (key, value, updated_at) VALUES (?1, ?2, datetime('now'))",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_config_kv(&self, key: &str) -> Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM config_kv WHERE key = ?1")?;
        let mut rows = stmt.query(params![key])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    pub fn list_config_kv(&self) -> Result<Vec<(String, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT key, value FROM config_kv ORDER BY key")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.map(|r| r.map_err(anyhow::Error::from)).collect()
    }

    pub fn execute_query(&self, sql: &str) -> Result<QueryResult> {
        if sql.trim_start().to_ascii_lowercase().starts_with("select") {
            let mut stmt = self.conn.prepare(sql)?;
            let col_count = stmt.column_count();
            let cols: Vec<String> = (0..col_count)
                .map(|i| stmt.column_name(i).unwrap_or("?").to_string())
                .collect();
            let rows = stmt.query_map([], |row| {
                let values: Vec<String> = (0..col_count)
                    .map(|i| {
                        row.get::<_, rusqlite::types::Value>(i)
                            .map(|v| match v {
                                rusqlite::types::Value::Null => "NULL".to_string(),
                                rusqlite::types::Value::Integer(n) => n.to_string(),
                                rusqlite::types::Value::Real(f) => f.to_string(),
                                rusqlite::types::Value::Text(s) => s,
                                rusqlite::types::Value::Blob(b) => {
                                    format!("<blob:{} bytes>", b.len())
                                }
                            })
                            .unwrap_or_else(|_| "?".to_string())
                    })
                    .collect();
                Ok(values)
            })?;

            let result_rows: Vec<Vec<String>> = rows
                .map(|r| r.map_err(anyhow::Error::from))
                .collect::<Result<_>>()?;
            let row_count = result_rows.len();

            Ok(QueryResult {
                columns: cols,
                rows: result_rows,
                rows_affected: row_count,
            })
        } else {
            let affected = self.conn.execute(sql, [])?;
            Ok(QueryResult {
                columns: vec![],
                rows: vec![],
                rows_affected: affected,
            })
        }
    }

    pub fn backup(&self, dest: &std::path::Path) -> Result<()> {
        let src = db_path();
        std::fs::copy(&src, dest)?;
        Ok(())
    }

    pub fn integrity_check(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("PRAGMA integrity_check")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.map(|r| r.map_err(anyhow::Error::from)).collect()
    }

    pub fn stats(&self) -> Result<DbStats> {
        let wallets: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM wallets", [], |r| r.get(0))
            .unwrap_or(0);
        let networks: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM networks", [], |r| r.get(0))
            .unwrap_or(0);
        let config_entries: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM config_kv", [], |r| r.get(0))
            .unwrap_or(0);
        let events_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))
            .unwrap_or(0);
        let schema_version = self
            .get_meta("schema_version")?
            .unwrap_or_else(|| "unknown".to_string());
        let db_size = std::fs::metadata(db_path()).map(|m| m.len()).unwrap_or(0);
        Ok(DbStats {
            wallets: wallets as usize,
            networks: networks as usize,
            config_entries: config_entries as usize,
            events: events_count as usize,
            schema_version,
            db_size_bytes: db_size,
        })
    }

    pub fn insert_event(&self, event: &EventRow) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO events \
             (id, event_type, contract_id, ledger, topics, value, timestamp, network) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                event.id,
                event.event_type,
                event.contract_id,
                event.ledger,
                event.topics,
                event.value,
                event.timestamp,
                event.network,
            ],
        )?;
        Ok(())
    }

    pub fn search_events(&self, filters: &EventSearchFilters) -> Result<Vec<EventRow>> {
        let mut conditions = vec!["1=1".to_string()];
        let mut params = Vec::new();

        if let Some(ref contract_id) = filters.contract_id {
            conditions.push("contract_id = ?".to_string());
            params.push(contract_id.clone());
        }
        if let Some(ref event_type) = filters.event_type {
            conditions.push("event_type = ?".to_string());
            params.push(event_type.clone());
        }
        if let Some(min_ledger) = filters.min_ledger {
            conditions.push("ledger >= ?".to_string());
            params.push(min_ledger.to_string());
        }
        if let Some(max_ledger) = filters.max_ledger {
            conditions.push("ledger <= ?".to_string());
            params.push(max_ledger.to_string());
        }
        if let Some(ref start_time) = filters.start_time {
            conditions.push("timestamp >= ?".to_string());
            params.push(start_time.clone());
        }
        if let Some(ref end_time) = filters.end_time {
            conditions.push("timestamp <= ?".to_string());
            params.push(end_time.clone());
        }
        if let Some(ref network) = filters.network {
            conditions.push("network = ?".to_string());
            params.push(network.clone());
        }

        let limit = filters.limit.unwrap_or(100).to_string();
        let offset = filters.offset.unwrap_or(0).to_string();

        let sql = format!(
            "SELECT id, event_type, contract_id, ledger, topics, value, timestamp, network \
             FROM events \
             WHERE {} \
             ORDER BY timestamp DESC \
             LIMIT {} OFFSET {}",
            conditions.join(" AND "),
            limit,
            offset
        );

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params), |row| {
            Ok(EventRow {
                id: row.get(0)?,
                event_type: row.get(1)?,
                contract_id: row.get(2)?,
                ledger: row.get(3)?,
                topics: row.get(4)?,
                value: row.get(5)?,
                timestamp: row.get(6)?,
                network: row.get(7)?,
            })
        })?;

        rows.map(|r| r.map_err(anyhow::Error::from)).collect()
    }

    pub fn aggregate_events(&self, bucket: &AggregationBucket, filters: &EventSearchFilters) -> Result<Vec<EventAggregation>> {
        let bucket_sql = match bucket {
            AggregationBucket::Hour => "strftime('%Y-%m-%d %H:00:00', timestamp) AS bucket",
            AggregationBucket::Day => "strftime('%Y-%m-%d', timestamp) AS bucket",
            AggregationBucket::Week => "strftime('%Y-%W', timestamp) AS bucket",
            AggregationBucket::Month => "strftime('%Y-%m', timestamp) AS bucket",
        };

        let mut conditions = vec!["1=1".to_string()];
        let mut params = Vec::new();

        if let Some(ref contract_id) = filters.contract_id {
            conditions.push("contract_id = ?".to_string());
            params.push(contract_id.clone());
        }
        if let Some(ref event_type) = filters.event_type {
            conditions.push("event_type = ?".to_string());
            params.push(event_type.clone());
        }
        if let Some(min_ledger) = filters.min_ledger {
            conditions.push("ledger >= ?".to_string());
            params.push(min_ledger.to_string());
        }
        if let Some(max_ledger) = filters.max_ledger {
            conditions.push("ledger <= ?".to_string());
            params.push(max_ledger.to_string());
        }
        if let Some(ref start_time) = filters.start_time {
            conditions.push("timestamp >= ?".to_string());
            params.push(start_time.clone());
        }
        if let Some(ref end_time) = filters.end_time {
            conditions.push("timestamp <= ?".to_string());
            params.push(end_time.clone());
        }
        if let Some(ref network) = filters.network {
            conditions.push("network = ?".to_string());
            params.push(network.clone());
        }

        let sql = format!(
            "SELECT {}, COUNT(*) AS count \
             FROM events \
             WHERE {} \
             GROUP BY bucket \
             ORDER BY bucket DESC",
            bucket_sql,
            conditions.join(" AND ")
        );

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params), |row| {
            Ok(EventAggregation {
                bucket: row.get(0)?,
                count: row.get(1)?,
            })
        })?;

        rows.map(|r| r.map_err(anyhow::Error::from)).collect()
    }

    pub fn export_events(&self, filters: &EventSearchFilters, format: ExportFormat, writer: &mut impl std::io::Write) -> Result<()> {
        let events = self.search_events(filters)?;

        match format {
            ExportFormat::Json => {
                serde_json::to_writer_pretty(writer, &events)?;
            }
            ExportFormat::Csv => {
                let mut wtr = csv::Writer::from_writer(writer);
                wtr.write_record(&["id", "event_type", "contract_id", "ledger", "topics", "value", "timestamp", "network"])?;
                for event in events {
                    wtr.write_record(&[
                        &event.id,
                        &event.event_type,
                        &event.contract_id,
                        &event.ledger.to_string(),
                        &event.topics.unwrap_or_default(),
                        &event.value,
                        &event.timestamp,
                        &event.network,
                    ])?;
                }
                wtr.flush()?;
            }
        }

        Ok(())
    }
}

pub fn migrate_from_toml(db: &Database) -> Result<MigrationReport> {
    let cfg = crate::utils::config::load()?;
    let mut report = MigrationReport::default();

    for wallet in &cfg.wallets {
        db.insert_wallet(&WalletRow {
            name: wallet.name.clone(),
            public_key: wallet.public_key.clone(),
            network: wallet.network.clone(),
            created_at: wallet.created_at.clone(),
            funded: wallet.funded,
        })?;
        report.wallets_migrated += 1;
    }

    for (name, net) in &cfg.networks {
        db.insert_network(&NetworkRow {
            name: name.clone(),
            horizon_url: net.horizon_url.clone(),
            soroban_rpc_url: net.soroban_rpc_url.clone(),
            friendbot_url: net.friendbot_url.clone(),
            passphrase: net.passphrase.clone(),
        })?;
        report.networks_migrated += 1;
    }

    db.insert_config_kv("network", &cfg.network)?;
    if let Some(telemetry) = cfg.telemetry_enabled {
        db.insert_config_kv("telemetry_enabled", &telemetry.to_string())?;
    }
    db.insert_config_kv("schema_version", &cfg.version)?;
    report.config_keys_migrated += 3;

    db.set_meta("migrated_from_toml", "true")?;
    db.set_meta("migration_timestamp", &chrono::Utc::now().to_rfc3339())?;

    Ok(report)
}

pub fn export_to_toml(db: &Database) -> Result<String> {
    use std::collections::HashMap;

    let wallets = db.list_wallets()?;
    let networks = db.list_networks()?;
    let kv = db.list_config_kv()?;

    let active_network = kv
        .iter()
        .find(|(k, _)| k == "network")
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| "testnet".to_string());

    let telemetry = kv
        .iter()
        .find(|(k, _)| k == "telemetry_enabled")
        .and_then(|(_, v)| v.parse::<bool>().ok());

    let mut cfg_map: HashMap<String, toml::Value> = HashMap::new();
    cfg_map.insert("version".to_string(), toml::Value::String("1".to_string()));
    cfg_map.insert("network".to_string(), toml::Value::String(active_network));

    if let Some(t) = telemetry {
        cfg_map.insert("telemetry_enabled".to_string(), toml::Value::Boolean(t));
    }

    let wallet_array: Vec<toml::Value> = wallets
        .iter()
        .map(|w| {
            let mut m = toml::map::Map::new();
            m.insert("name".to_string(), toml::Value::String(w.name.clone()));
            m.insert(
                "public_key".to_string(),
                toml::Value::String(w.public_key.clone()),
            );
            m.insert(
                "network".to_string(),
                toml::Value::String(w.network.clone()),
            );
            m.insert(
                "created_at".to_string(),
                toml::Value::String(w.created_at.clone()),
            );
            m.insert("funded".to_string(), toml::Value::Boolean(w.funded));
            toml::Value::Table(m)
        })
        .collect();
    cfg_map.insert("wallets".to_string(), toml::Value::Array(wallet_array));

    let mut net_table = toml::map::Map::new();
    for net in &networks {
        let mut nm = toml::map::Map::new();
        nm.insert(
            "horizon_url".to_string(),
            toml::Value::String(net.horizon_url.clone()),
        );
        if let Some(rpc) = &net.soroban_rpc_url {
            nm.insert(
                "soroban_rpc_url".to_string(),
                toml::Value::String(rpc.clone()),
            );
        }
        if let Some(fb) = &net.friendbot_url {
            nm.insert("friendbot_url".to_string(), toml::Value::String(fb.clone()));
        }
        if let Some(pp) = &net.passphrase {
            nm.insert("passphrase".to_string(), toml::Value::String(pp.clone()));
        }
        net_table.insert(net.name.clone(), toml::Value::Table(nm));
    }
    cfg_map.insert("networks".to_string(), toml::Value::Table(net_table));

    Ok(toml::to_string_pretty(&toml::Value::Table(
        cfg_map.into_iter().collect(),
    ))?)
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS wallets (
    name        TEXT PRIMARY KEY,
    public_key  TEXT NOT NULL,
    network     TEXT NOT NULL DEFAULT 'testnet',
    created_at  TEXT NOT NULL,
    funded      INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS networks (
    name            TEXT PRIMARY KEY,
    horizon_url     TEXT NOT NULL,
    soroban_rpc_url TEXT,
    friendbot_url   TEXT,
    passphrase      TEXT
);

CREATE TABLE IF NOT EXISTS config_kv (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS plugins (
    name        TEXT PRIMARY KEY,
    path        TEXT NOT NULL,
    source      TEXT,
    installed_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS templates (
    name        TEXT PRIMARY KEY,
    description TEXT,
    tags        TEXT,
    source_url  TEXT,
    cached_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS events (
    id TEXT PRIMARY KEY,
    event_type TEXT NOT NULL,
    contract_id TEXT NOT NULL,
    ledger INTEGER NOT NULL,
    topics TEXT,
    value TEXT NOT NULL,
    timestamp TEXT NOT NULL DEFAULT (datetime('now')),
    network TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_events_contract ON events(contract_id);
CREATE INDEX IF NOT EXISTS idx_events_ledger ON events(ledger);
CREATE INDEX IF NOT EXISTS idx_events_type ON events(event_type);
CREATE INDEX IF NOT EXISTS idx_events_network ON events(network);
CREATE INDEX IF NOT EXISTS idx_events_timestamp ON events(timestamp);
CREATE INDEX IF NOT EXISTS idx_events_contract_ledger ON events(contract_id, ledger);
CREATE INDEX IF NOT EXISTS idx_wallets_network ON wallets(network);
CREATE INDEX IF NOT EXISTS idx_config_kv_key   ON config_kv(key);
";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletRow {
    pub name: String,
    pub public_key: String,
    pub network: String,
    pub created_at: String,
    pub funded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkRow {
    pub name: String,
    pub horizon_url: String,
    pub soroban_rpc_url: Option<String>,
    pub friendbot_url: Option<String>,
    pub passphrase: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub rows_affected: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbStats {
    pub wallets: usize,
    pub networks: usize,
    pub config_entries: usize,
    pub events: usize,
    pub schema_version: String,
    pub db_size_bytes: u64,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct MigrationReport {
    pub wallets_migrated: usize,
    pub networks_migrated: usize,
    pub config_keys_migrated: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRow {
    pub id: String,
    pub event_type: String,
    pub contract_id: String,
    pub ledger: u32,
    pub topics: Option<String>,
    pub value: String,
    pub timestamp: String,
    pub network: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EventSearchFilters {
    pub contract_id: Option<String>,
    pub event_type: Option<String>,
    pub min_ledger: Option<u32>,
    pub max_ledger: Option<u32>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub network: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggregationBucket {
    Hour,
    Day,
    Week,
    Month,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventAggregation {
    pub bucket: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExportFormat {
    Json,
    Csv,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn in_memory_db() -> Database {
        let db = Database::open_in_memory().unwrap();
        db.initialize().unwrap();
        db
    }

    #[test]
    fn insert_and_list_wallet() {
        let db = in_memory_db();
        db.insert_wallet(&WalletRow {
            name: "alice".to_string(),
            public_key: "GABC".to_string(),
            network: "testnet".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            funded: false,
        })
        .unwrap();
        let wallets = db.list_wallets().unwrap();
        assert_eq!(wallets.len(), 1);
        assert_eq!(wallets[0].name, "alice");
    }

    #[test]
    fn get_wallet_returns_none_for_missing() {
        let db = in_memory_db();
        let w = db.get_wallet("missing").unwrap();
        assert!(w.is_none());
    }

    #[test]
    fn config_kv_roundtrip() {
        let db = in_memory_db();
        db.insert_config_kv("network", "mainnet").unwrap();
        let v = db.get_config_kv("network").unwrap();
        assert_eq!(v, Some("mainnet".to_string()));
    }

    #[test]
    fn integrity_check_passes_on_fresh_db() {
        let db = in_memory_db();
        let result = db.integrity_check().unwrap();
        assert_eq!(result, vec!["ok".to_string()]);
    }

    #[test]
    fn stats_reflect_inserted_data() {
        let db = in_memory_db();
        db.insert_wallet(&WalletRow {
            name: "bob".to_string(),
            public_key: "GXYZ".to_string(),
            network: "testnet".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            funded: true,
        })
        .unwrap();
        let stats = db.stats().unwrap();
        assert_eq!(stats.wallets, 1);
    }

    #[test]
    fn delete_wallet_removes_entry() {
        let db = in_memory_db();
        db.insert_wallet(&WalletRow {
            name: "temp".to_string(),
            public_key: "GTEMP".to_string(),
            network: "testnet".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            funded: false,
        })
        .unwrap();
        let removed = db.delete_wallet("temp").unwrap();
        assert_eq!(removed, 1);
        assert!(db.get_wallet("temp").unwrap().is_none());
    }

    #[test]
    fn insert_and_search_event() {
        let db = in_memory_db();
        let event = EventRow {
            id: "evt123".to_string(),
            event_type: "contract".to_string(),
            contract_id: "CABC123".to_string(),
            ledger: 12345,
            topics: Some(serde_json::to_string(&vec!["topic1", "topic2"]).unwrap()),
            value: serde_json::json!({"key": "value"}).to_string(),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            network: "testnet".to_string(),
        };
        db.insert_event(&event).unwrap();
        
        let filters = EventSearchFilters {
            contract_id: Some("CABC123".to_string()),
            ..Default::default()
        };
        let events = db.search_events(&filters).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, "evt123");
    }

    #[test]
    fn aggregate_events() {
        let db = in_memory_db();
        let event1 = EventRow {
            id: "evt1".to_string(),
            event_type: "contract".to_string(),
            contract_id: "CABC".to_string(),
            ledger: 1,
            topics: None,
            value: "{}".to_string(),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            network: "testnet".to_string(),
        };
        let event2 = EventRow {
            id: "evt2".to_string(),
            event_type: "contract".to_string(),
            contract_id: "CABC".to_string(),
            ledger: 2,
            topics: None,
            value: "{}".to_string(),
            timestamp: "2024-01-01T01:00:00Z".to_string(),
            network: "testnet".to_string(),
        };
        db.insert_event(&event1).unwrap();
        db.insert_event(&event2).unwrap();

        let aggregates = db.aggregate_events(&AggregationBucket::Hour, &EventSearchFilters::default()).unwrap();
        assert_eq!(aggregates.len(), 1);
        assert_eq!(aggregates[0].count, 2);
    }
}
