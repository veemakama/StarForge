use anyhow::{Context, Result};
use std::process::Command;

pub fn validate_wasm(bytes: &[u8]) -> Result<()> {
    if bytes.len() < 8 {
        anyhow::bail!("Wasm file too small");
    }
    if &bytes[..4] != b"\0asm" {
        anyhow::bail!("Missing wasm header");
    }
    Ok(())
}

pub fn ensure_docker_sandbox() -> Result<()> {
    let output = Command::new("docker")
        .args(["info"])
        .output()
        .context("Docker is not available. Please install Docker and ensure the daemon is running.")?;

    if !output.status.success() {
        anyhow::bail!("Docker daemon is not running. Please start Docker and try again.");
    }

    let compose_output = Command::new("docker-compose")
        .args(["ps", "-q", "stellar-testnet"])
        .output()
        .context("docker-compose not found. Please install docker-compose.")?;

    let stdout = String::from_utf8_lossy(&compose_output.stdout);
    if stdout.trim().is_empty() {
        let up_output = Command::new("docker-compose")
            .args(["up", "-d", "--wait", "stellar-testnet"])
            .output()
            .context("Failed to start Docker Soroban sandbox via docker-compose.")?;

        if !up_output.status.success() {
            let stderr = String::from_utf8_lossy(&up_output.stderr);
            anyhow::bail!("Failed to start Docker Soroban sandbox:\n{}", stderr.trim());
        }
    }

    Ok(())
}

pub fn stop_docker_sandbox() -> Result<()> {
    let output = Command::new("docker-compose")
        .args(["down"])
        .output()
        .context("Failed to stop Docker Soroban sandbox.")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to stop Docker Soroban sandbox:\n{}", stderr.trim());
    }

    Ok(())
}

pub fn docker_rpc_url() -> &'static str {
    "http://localhost:8000/rpc"
}
