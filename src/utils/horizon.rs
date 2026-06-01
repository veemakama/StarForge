use crate::utils::config;
use anyhow::{Context, Result};
use serde::Deserialize;

pub fn network_config(network: &str) -> Result<config::NetworkConfig> {
    let cfg = config::load()?;
    config::get_network_config(&cfg, network)
}

pub fn horizon_url(network: &str) -> Result<String> {
    Ok(network_config(network)?.horizon_url)
}

pub fn friendbot_url(network: &str) -> Result<Option<String>> {
    Ok(network_config(network)?.friendbot_url)
}

#[derive(Debug, Deserialize)]
pub struct AccountResponse {
    #[allow(dead_code)]
    pub id: String,
    pub sequence: String,
    pub balances: Vec<Balance>,
    #[allow(dead_code)]
    pub subentry_count: u32,
}

#[derive(Debug, Deserialize)]
pub struct Balance {
    pub balance: String,
    pub asset_type: String,
    pub asset_code: Option<String>,
}

pub fn fund_account(public_key: &str, network: &str) -> Result<()> {
    let friendbot =
        friendbot_url(network)?.unwrap_or_else(|| "https://friendbot.stellar.org".to_string());
    let separator = if friendbot.contains('?') { '&' } else { '?' };
    let url = format!("{}{}addr={}", friendbot, separator, public_key);
    let res = ureq::get(&url)
        .call()
        .with_context(|| format!("Friendbot request failed for {}", network))?;
    if res.status() == 200 {
        Ok(())
    } else {
        anyhow::bail!("Friendbot returned status {}", res.status())
    }
}

pub fn fetch_account(public_key: &str, network: &str) -> Result<AccountResponse> {
    let horizon = horizon_url(network)?;
    let url = format!("{}/accounts/{}", horizon, public_key);
    let res = ureq::get(&url)
        .call()
        .with_context(|| format!("Failed to reach Horizon on {}", network))?;
    if res.status() == 200 {
        let account: AccountResponse = res
            .into_json()
            .with_context(|| "Failed to parse account response")?;
        Ok(account)
    } else {
        anyhow::bail!("Account not found on {}", network)
    }
}

pub fn check_network(network: &str) -> bool {
    if let Ok(horizon) = horizon_url(network) {
        let url = format!("{}/", horizon);
        ureq::get(&url)
            .call()
            .map(|r| r.status() == 200)
            .unwrap_or(false)
    } else {
        false
    }
}

pub fn build_transaction_query_url(
    public_key: &str,
    network: &str,
    filter: &TxFilter,
) -> Result<String> {
    let horizon = horizon_url(network)?;
    let mut url = format!(
        "{}/accounts/{}/transactions?order={}&limit={}",
        horizon,
        public_key,
        filter.order.as_deref().unwrap_or("desc"),
        filter.limit.min(200)
    );

    if let Some(ref cursor) = filter.cursor {
        url.push_str(&format!("&cursor={}", cursor));
    }
    if let Some(ref type_filter) = filter.type_filter {
        url.push_str(&format!("&type={}", type_filter));
    }

    Ok(url)
}

#[derive(Debug, Deserialize, Clone)]
pub struct TransactionRecord {
    pub hash: String,
    pub successful: bool,
    pub operation_count: u32,
    pub fee_charged: String,
    pub created_at: String,
    pub memo_type: Option<String>,
    pub memo: Option<String>,
    pub source_account: Option<String>,
    #[serde(rename = "type")]
    pub transaction_type: Option<String>,
    pub paging_token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FeeStats {
    #[serde(rename = "low_fee")]
    pub low_fee: String,
    #[serde(rename = "mode_fee")]
    pub mode_fee: String,
    #[serde(rename = "high_fee")]
    pub high_fee: String,
}

pub fn fetch_fee_stats(network: &str) -> Result<FeeStats> {
    let horizon = horizon_url(network)?;
    let url = format!("{}/fee_stats", horizon);
    let res = ureq::get(&url)
        .call()
        .with_context(|| format!("Failed to fetch fee stats from {}", network))?;
    if res.status() == 200 {
        let stats: FeeStats = res.into_json().with_context(|| "Failed to parse fee stats response")?;
        Ok(stats)
    } else {
        anyhow::bail!("Failed to get fee stats: HTTP {}", res.status())
    }
}

#[derive(Debug, Deserialize)]
struct TransactionsResponse {
    #[serde(rename = "_embedded")]
    embedded: TransactionsEmbedded,
}

#[derive(Debug, Deserialize)]
struct TransactionsEmbedded {
    records: Vec<TransactionRecord>,
}

pub struct TxFilter {
    pub limit: u8,
    pub cursor: Option<String>,
    pub order: Option<String>,
    pub type_filter: Option<String>,
    pub after: Option<String>,
    pub before: Option<String>,
    pub successful_only: Option<bool>,
}

#[allow(dead_code)]
pub fn fetch_transactions(
    public_key: &str,
    network: &str,
    limit: u8,
) -> Result<Vec<TransactionRecord>> {
    fetch_transactions_filtered(
        public_key,
        network,
        TxFilter {
            limit,
            cursor: None,
            order: None,
            type_filter: None,
            after: None,
            before: None,
            successful_only: None,
        },
    )
}

pub fn fetch_transactions_filtered(
    public_key: &str,
    network: &str,
    filter: TxFilter,
) -> Result<Vec<TransactionRecord>> {
    let url = build_transaction_query_url(public_key, network, &filter)?;
    let res = ureq::get(&url).call().with_context(|| {
        format!(
            "Account '{}' not found on {}. Has it been funded?",
            public_key, network
        )
    })?;

    let parsed: TransactionsResponse = res
        .into_json()
        .with_context(|| "Failed to parse transactions response")?;

    let mut records = parsed.embedded.records;

    // Client-side filtering for Horizon features not universally supported
    if let Some(ref type_filter) = filter.type_filter {
        records.retain(|tx| tx.transaction_type.as_deref() == Some(type_filter.as_str()));
    }
    if let Some(ref after) = filter.after {
        records.retain(|tx| tx.created_at.as_str() >= after.as_str());
    }
    if let Some(ref before) = filter.before {
        records.retain(|tx| tx.created_at.as_str() <= before.as_str());
    }
    if let Some(successful_only) = filter.successful_only {
        records.retain(|tx| tx.successful == successful_only);
    }

    Ok(records)
}

#[derive(Debug, Deserialize)]
pub struct TransactionSimulationResult {
    pub transaction_xdr: String,
    pub fee: u64,
}

#[derive(Debug, Deserialize)]
pub struct TransactionSubmitResult {
    pub hash: String,
    pub successful: bool,
}

#[derive(Debug, Deserialize)]
struct HorizonError {
    pub title: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BatchPaymentOp {
    pub destination: String,
    pub amount: String,
    pub asset_code: Option<String>,
    pub asset_issuer: Option<String>,
}

pub fn build_and_simulate_batch(
    source: &str,
    operations: &[BatchPaymentOp],
    sequence: &str,
    network: &str,
) -> Result<TransactionSimulationResult> {
    let tx_xdr = build_batch_transaction_xdr(source, operations, sequence, network)?;

    let base_fee_per_op = 100_000u64;
    let estimated_fee = base_fee_per_op.saturating_mul(operations.len() as u64);

    Ok(TransactionSimulationResult {
        transaction_xdr: tx_xdr,
        fee: estimated_fee,
    })
}

pub fn build_and_simulate_account_merge(
    source: &str,
    destination: &str,
    sequence: &str,
    network: &str,
) -> Result<TransactionSimulationResult> {
    let tx_xdr = build_account_merge_transaction_xdr(source, destination, sequence, network)?;

    Ok(TransactionSimulationResult {
        transaction_xdr: tx_xdr,
        fee: 100_000,
    })
}

pub fn build_and_simulate_payment(
    source: &str,
    destination: &str,
    amount: &str,
    asset_code: Option<&str>,
    asset_issuer: Option<&str>,
    sequence: &str,
    network: &str,
) -> Result<TransactionSimulationResult> {
    // For now, we'll use a simplified approach by calling the stellar CLI
    // In a production implementation, you'd use stellar-xdr to build the transaction properly

    // Build transaction XDR using stellar-sdk patterns
    let tx_xdr = build_payment_transaction_xdr(
        source,
        destination,
        amount,
        asset_code,
        asset_issuer,
        sequence,
        network,
    )?;

    // Simulate the transaction
    let horizon = horizon_url(network)?;
    let _url = format!("{}/transactions", horizon);
    let _form_data = format!("tx={}", urlencoding::encode(&tx_xdr));

    // For simulation, we'll estimate the fee
    let estimated_fee = 100000u64; // 0.00001 XLM in stroops

    Ok(TransactionSimulationResult {
        transaction_xdr: tx_xdr,
        fee: estimated_fee,
    })
}

pub fn submit_payment_transaction(
    transaction_xdr: &str,
    secret_key: &str,
    network: &str,
) -> Result<TransactionSubmitResult> {
    // Sign the transaction
    let signed_xdr = sign_transaction_xdr(transaction_xdr, secret_key, network)?;

    // Submit to Horizon
    let horizon = horizon_url(network)?;
    let url = format!("{}/transactions", horizon);
    let form_data = format!("tx={}", urlencoding::encode(&signed_xdr));

    let res = ureq::post(&url)
        .set("Content-Type", "application/x-www-form-urlencoded")
        .send_string(&form_data)
        .with_context(|| "Failed to submit transaction to Horizon")?;

    let status = res.status();

    if status == 200 {
        let result: serde_json::Value = res
            .into_json()
            .with_context(|| "Failed to parse transaction response")?;

        let hash = result
            .get("hash")
            .and_then(|h| h.as_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(TransactionSubmitResult {
            hash,
            successful: true,
        })
    } else {
        let error_text = res
            .into_string()
            .unwrap_or_else(|_| "Unknown error".to_string());

        // Try to parse Horizon error format
        if let Ok(horizon_error) = serde_json::from_str::<HorizonError>(&error_text) {
            let detail = horizon_error
                .detail
                .unwrap_or_else(|| "No additional details".to_string());
            anyhow::bail!("Transaction failed: {} - {}", horizon_error.title, detail);
        } else {
            anyhow::bail!("Transaction failed with status {}: {}", status, error_text);
        }
    }
}

pub fn submit_multisig_transaction(
    signed_transaction_xdr: &str,
    network: &str,
) -> Result<TransactionSubmitResult> {
    // Submit a pre-signed transaction (e.g. multisig envelope) to Horizon.
    let horizon = horizon_url(network)?;
    let url = format!("{}/transactions", horizon);
    let form_data = format!("tx={}", urlencoding::encode(signed_transaction_xdr));

    let res = ureq::post(&url)
        .set("Content-Type", "application/x-www-form-urlencoded")
        .send_string(&form_data)
        .with_context(|| "Failed to submit transaction to Horizon")?;

    let status = res.status();
    if status == 200 {
        let result: serde_json::Value = res
            .into_json()
            .with_context(|| "Failed to parse transaction response")?;

        let hash = result
            .get("hash")
            .and_then(|h| h.as_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(TransactionSubmitResult {
            hash,
            successful: true,
        })
    } else {
        let error_text = res
            .into_string()
            .unwrap_or_else(|_| "Unknown error".to_string());

        if let Ok(horizon_error) = serde_json::from_str::<HorizonError>(&error_text) {
            let detail = horizon_error
                .detail
                .unwrap_or_else(|| "No additional details".to_string());
            anyhow::bail!("Transaction failed: {} - {}", horizon_error.title, detail);
        } else {
            anyhow::bail!("Transaction failed with status {}: {}", status, error_text);
        }
    }
}

fn build_batch_transaction_xdr(
    source: &str,
    operations: &[BatchPaymentOp],
    sequence: &str,
    network: &str,
) -> Result<String> {
    if operations.is_empty() {
        anyhow::bail!("Batch transaction requires at least one operation");
    }

    let _network_passphrase = config::get_network_passphrase(network);

    let op_parts: Vec<String> = operations
        .iter()
        .enumerate()
        .map(|(i, op)| {
            let asset_info = match (&op.asset_code, &op.asset_issuer) {
                (None, None) => "native".to_string(),
                (Some(code), Some(issuer)) => format!("{}:{}", code, issuer),
                _ => return Err(anyhow::anyhow!("Invalid asset in operation {}", i + 1)),
            };
            Ok(format!(
                "pay:{}:{}:{}",
                op.destination, op.amount, asset_info
            ))
        })
        .collect::<Result<Vec<_>>>()?;

    let mock_xdr = format!(
        "mock_batch_tx_{}_{}_{}_{}",
        source,
        op_parts.join("|"),
        sequence,
        network
    );

    use base64::{engine::general_purpose, Engine as _};
    Ok(general_purpose::STANDARD.encode(mock_xdr))
}

fn build_account_merge_transaction_xdr(
    source: &str,
    destination: &str,
    sequence: &str,
    network: &str,
) -> Result<String> {
    let _network_passphrase = network_passphrase(network);

    let mock_xdr = format!(
        "mock_merge_tx_{}_{}_{}_{}",
        source, destination, sequence, network
    );

    use base64::{engine::general_purpose, Engine as _};
    Ok(general_purpose::STANDARD.encode(mock_xdr))
}

fn network_passphrase(network: &str) -> String {
    config::get_network_passphrase(network)
}

fn build_payment_transaction_xdr(
    source: &str,
    destination: &str,
    amount: &str,
    asset_code: Option<&str>,
    asset_issuer: Option<&str>,
    sequence: &str,
    network: &str,
) -> Result<String> {
    // This is a simplified mock implementation
    // In production, you'd use stellar-xdr crate to build proper transaction XDR

    let _network_passphrase = network_passphrase(network);

    // Mock XDR generation - in reality this would be much more complex
    let asset_info = match (asset_code, asset_issuer) {
        (None, None) => "native".to_string(),
        (Some(code), Some(issuer)) => format!("{}:{}", code, issuer),
        _ => return Err(anyhow::anyhow!("Invalid asset specification")),
    };

    // Generate a mock transaction XDR
    // In production, use stellar-xdr to build proper TransactionEnvelope
    let mock_xdr = format!(
        "mock_payment_tx_{}_{}_{}_{}_{}",
        source, destination, amount, asset_info, sequence
    );

    use base64::{engine::general_purpose, Engine as _};
    Ok(general_purpose::STANDARD.encode(mock_xdr))
}

fn sign_transaction_xdr(transaction_xdr: &str, secret_key: &str, network: &str) -> Result<String> {
    // This is a simplified mock implementation
    // In production, you'd use stellar-xdr and ed25519 signing

    let _network_passphrase = config::get_network_passphrase(network);

    // Mock signing - in reality this would involve:
    // 1. Decode the transaction XDR
    // 2. Hash the transaction with network passphrase
    // 3. Sign with ed25519 private key
    // 4. Create TransactionEnvelope with signature
    // 5. Re-encode to XDR

    let signed_mock = format!("signed_{}_with_{}", transaction_xdr, &secret_key[..8]);
    use base64::{engine::general_purpose, Engine as _};
    Ok(general_purpose::STANDARD.encode(signed_mock))
}
