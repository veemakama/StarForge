use crate::utils::mock_soroban;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct TestOptions {
    pub coverage: bool,
    pub report_format: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestRunResult {
    pub size_bytes: usize,
    pub sha256: String,
    pub cases_executed: u32,
    pub failures: u32,
    pub report_path: Option<PathBuf>,
}

pub fn run_contract_tests(wasm: &Path, opts: TestOptions) -> Result<TestRunResult> {
    let bytes = fs::read(wasm).with_context(|| format!("Failed to read {}", wasm.display()))?;
    let sha256 = hex::encode(Sha256::digest(&bytes));

    // Stubbed engine: we validate the wasm is at least plausible.
    mock_soroban::validate_wasm(&bytes).context("Invalid/unsupported wasm")?;

    // For now, emulate a single happy-path test execution.
    let cases_executed = 1;
    let failures = 0;

    let report_path = opts
        .report_format
        .as_deref()
        .map(|fmt| format_report(&sha256, fmt, opts.coverage))
        .transpose()?;

    Ok(TestRunResult {
        size_bytes: bytes.len(),
        sha256,
        cases_executed,
        failures,
        report_path,
    })
}

fn format_report(sha256: &str, format: &str, coverage: bool) -> Result<PathBuf> {
    let dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?
        .join(".starforge")
        .join("reports");
    if !dir.exists() {
        fs::create_dir_all(&dir).with_context(|| format!("Failed to create {}", dir.display()))?;
    }

    let filename = format!(
        "contract-test-{}{}.{ext}",
        &sha256[..12],
        if coverage { "-coverage" } else { "" },
        ext = format
    );
    let path = dir.join(filename);

    match format {
        "json" => {
            let payload = serde_json::json!({
                "sha256": sha256,
                "coverage": coverage,
                "note": "This is a lightweight placeholder report (no VM execution yet)."
            });
            fs::write(&path, serde_json::to_string_pretty(&payload)?)
                .with_context(|| format!("Failed to write {}", path.display()))?;
        }
        "html" => {
            let html = format!(
                "<!doctype html><meta charset=\"utf-8\" /><title>StarForge Contract Test Report</title><h1>Contract Test Report</h1><p>sha256: <code>{}</code></p><p>coverage: {}</p><p><em>Placeholder report.</em></p>",
                sha256,
                coverage
            );
            fs::write(&path, html)
                .with_context(|| format!("Failed to write {}", path.display()))?;
        }
        other => {
            anyhow::bail!(
                "Unsupported report format '{}'. Use 'html' or 'json'.",
                other
            );
        }
    }

    Ok(path)
}
