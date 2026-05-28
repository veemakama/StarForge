use anyhow::{Context, Result};
use rand::Rng;
use serde::Deserialize;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct SorobanEventStream {
    rpc_url: String,
    contract_id: String,
    cursor: Option<String>,
    poll_interval: Duration,
    backoff: Backoff,
}

impl SorobanEventStream {
    pub fn new(rpc_url: String, contract_id: String) -> Self {
        Self {
            rpc_url,
            contract_id,
            cursor: None,
            poll_interval: Duration::from_secs(2),
            backoff: Backoff::default(),
        }
    }

    pub fn with_poll_interval(mut self, seconds: u64) -> Self {
        self.poll_interval = Duration::from_secs(seconds.max(1));
        self
    }

    pub fn next_batch(&mut self) -> Result<Vec<SorobanEvent>> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getEvents",
            "params": {
                "filters": [{
                    "type": "contract",
                    "contractIds": [self.contract_id],
                }],
                "pagination": {
                    "cursor": self.cursor,
                    "limit": 10
                }
            }
        });

        let response: SorobanGetEventsResponse = ureq::post(&self.rpc_url)
            .set("Content-Type", "application/json")
            .send_json(&request)
            .with_context(|| format!("Soroban RPC request to {} failed", self.rpc_url))?
            .into_json()
            .with_context(|| "Failed to decode Soroban getEvents response")?;

        if let Some(error) = response.error {
            anyhow::bail!(
                "Soroban RPC getEvents failed: {}",
                error
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("unknown error")
            );
        }

        let result = response
            .result
            .ok_or_else(|| anyhow::anyhow!("Soroban RPC getEvents returned no result"))?;

        self.cursor = result.cursor;
        self.backoff.reset();
        Ok(result.events)
    }

    pub fn sleep(&self) {
        thread::sleep(self.poll_interval);
    }

    pub fn sleep_backoff(&mut self) {
        thread::sleep(self.backoff.next_delay());
    }
}

#[derive(Debug, Deserialize)]
struct SorobanGetEventsResponse {
    result: Option<SorobanGetEventsResult>,
    error: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct SorobanGetEventsResult {
    cursor: Option<String>,
    events: Vec<SorobanEvent>,
}

#[derive(Debug, Deserialize)]
pub struct SorobanEvent {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    pub event_type: String,
    pub ledger: u32,
    pub id: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub topic: Vec<String>,
    pub value: serde_json::Value,
}
