use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasReport {
    pub size_bytes: usize,
    pub sha256: String,
    pub score: u32,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizeResult {
    pub input_size_bytes: usize,
    pub output_size_bytes: usize,
    pub output_path: PathBuf,
}

pub fn analyze_wasm(path: &Path) -> Result<GasReport> {
    let bytes = fs::read(path).with_context(|| format!("Failed to read {}", path.display()))?;
    let sha256 = hex::encode(Sha256::digest(&bytes));
    let size = bytes.len();

    // Heuristics only: keep this lightweight and deterministic.
    let mut suggestions = Vec::new();
    if size > 500_000 {
        suggestions.push(
            "Wasm is large; consider stripping symbols and removing unused features.".to_string(),
        );
    }
    if bytes.windows(4).any(|w| w == b"panic") {
        suggestions.push(
            "Panic strings detected; consider `panic = \"abort\"` and removing verbose messages."
                .to_string(),
        );
    }
    if bytes.windows(7).any(|w| w == b"println") {
        suggestions.push("Debug printing detected; remove logs for production builds.".to_string());
    }

    // A simple, stable scoring function.
    let score = (1_000_000usize.saturating_sub(size)).min(1_000_000) as u32;

    Ok(GasReport {
        size_bytes: size,
        sha256,
        score,
        suggestions,
    })
}

pub fn optimize_wasm(input: &Path, output: &Path) -> Result<OptimizeResult> {
    let bytes = fs::read(input).with_context(|| format!("Failed to read {}", input.display()))?;

    // No binary rewriting here (keeps this safe and dependency-light).
    // We simply copy the wasm through so the command is usable today,
    // while leaving room for real optimizations later.
    if let Some(parent) = output.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }
    }
    fs::write(output, &bytes).with_context(|| format!("Failed to write {}", output.display()))?;

    Ok(OptimizeResult {
        input_size_bytes: bytes.len(),
        output_size_bytes: bytes.len(),
        output_path: output.to_path_buf(),
    })
}
