use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::Deserialize;
use stellar_xdr::curr::{Limited, Limits, ScSymbol, ScVal, WriteXdr};
use std::thread;
use std::time::Duration;

/// RPC and client-side filters for Soroban `getEvents`.
#[derive(Debug, Clone, Default)]
pub struct EventStreamFilters {
    pub event_type: Option<String>,
    pub topic_segments: Option<Vec<String>>,
    pub value_match: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SorobanEventStream {
    rpc_url: String,
    contract_id: String,
    cursor: Option<String>,
    poll_interval: Duration,
    backoff: Backoff,
    filters: EventStreamFilters,
}

#[derive(Debug, Clone)]
struct Backoff {
    attempt: u32,
    base_ms: u64,
    max_ms: u64,
}

impl Default for Backoff {
    fn default() -> Self {
        Self {
            attempt: 0,
            base_ms: 500,
            max_ms: 30_000,
        }
    }
}

impl Backoff {
    fn reset(&mut self) {
        self.attempt = 0;
    }

    fn next_delay(&mut self) -> Duration {
        let exp = self.attempt.min(6);
        self.attempt = self.attempt.saturating_add(1);
        let ms = (self.base_ms.saturating_mul(1_u64 << exp)).min(self.max_ms);
        Duration::from_millis(ms)
    }
}

impl SorobanEventStream {
    pub fn new(rpc_url: String, contract_id: String) -> Self {
        Self {
            rpc_url,
            contract_id,
            cursor: None,
            poll_interval: Duration::from_secs(2),
            backoff: Backoff::default(),
            filters: EventStreamFilters::default(),
        }
    }

    pub fn with_poll_interval(mut self, seconds: u64) -> Self {
        self.poll_interval = Duration::from_secs(seconds.max(1));
        self
    }

    pub fn with_filters(mut self, filters: EventStreamFilters) -> Self {
        self.filters = filters;
        self
    }

    pub fn with_event_type(mut self, event_type: impl Into<String>) -> Self {
        self.filters.event_type = Some(event_type.into());
        self
    }

    pub fn with_topic_segments(mut self, segments: Vec<String>) -> Self {
        self.filters.topic_segments = Some(segments);
        self
    }

    pub fn with_value_match(mut self, pattern: impl Into<String>) -> Self {
        self.filters.value_match = Some(pattern.into());
        self
    }

    pub fn next_batch(&mut self) -> Result<Vec<SorobanEvent>> {
        let filter = self.build_rpc_filter();
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getEvents",
            "params": {
                "filters": [filter],
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

        let events = result
            .events
            .into_iter()
            .filter(|event| event_matches_value(event, &self.filters))
            .collect();

        Ok(events)
    }

    pub fn sleep(&self) {
        thread::sleep(self.poll_interval);
    }

    pub fn sleep_backoff(&mut self) {
        thread::sleep(self.backoff.next_delay());
    }

    fn build_rpc_filter(&self) -> serde_json::Value {
        let event_type = self
            .filters
            .event_type
            .as_deref()
            .unwrap_or("contract");

        let mut filter = serde_json::json!({
            "type": event_type,
            "contractIds": [self.contract_id],
        });

        if let Some(ref segments) = self.filters.topic_segments {
            let encoded: Result<Vec<String>> =
                segments.iter().map(|s| encode_topic_segment(s)).collect();
            if let Ok(topic_row) = encoded {
                if !topic_row.is_empty() {
                    filter["topics"] = serde_json::json!([topic_row]);
                }
            }
        }

        filter
    }
}

fn event_matches_value(event: &SorobanEvent, filters: &EventStreamFilters) -> bool {
    let Some(ref pattern) = filters.value_match else {
        return true;
    };
    if pattern.is_empty() {
        return true;
    }
    let haystack = event.value.to_string().to_lowercase();
    haystack.contains(&pattern.to_lowercase())
}

fn encode_topic_segment(segment: &str) -> Result<String> {
    let trimmed = segment.trim();
    if trimmed.is_empty() {
        anyhow::bail!("topic segment cannot be empty");
    }
    if trimmed == "*" || trimmed == "**" {
        return Ok(trimmed.to_string());
    }
    if looks_like_base64(trimmed) {
        return Ok(trimmed.to_string());
    }

    let symbol = ScSymbol(
        trimmed
            .as_bytes()
            .try_into()
            .with_context(|| format!("invalid topic symbol '{}'", trimmed))?,
    );
    let scval = ScVal::Symbol(symbol);
    let mut bytes = Vec::new();
    scval.write_xdr(&mut Limited::new(&mut bytes, Limits::none()))?;
    Ok(BASE64.encode(bytes))
}

fn looks_like_base64(value: &str) -> bool {
    value.len() >= 8
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')
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
    pub topic: Vec<String>,
    pub value: serde_json::Value,
}
