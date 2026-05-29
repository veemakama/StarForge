use anyhow::{Context, Result};
use std::process::Command;
use std::thread;
use std::time::Duration;

pub const CONTAINER_NAME: &str = "starforge-devnet";
pub const QUICKSTART_IMAGE: &str = "stellar/quickstart:latest";
const CONTAINER_RPC_PORT: u16 = 8000;
const HEALTH_INTERVAL: Duration = Duration::from_secs(2);
const HEALTH_ATTEMPTS: u32 = 60;

/// Soroban RPC URL for a node bound to `host_port` on localhost.
pub fn rpc_url(host_port: u16) -> String {
    format!("http://127.0.0.1:{}/rpc", host_port)
}

pub fn ensure_docker_available() -> Result<()> {
    let output = Command::new("docker")
        .args(["info"])
        .output()
        .context("Docker is not available. Install Docker and ensure the daemon is running.")?;

    if !output.status.success() {
        anyhow::bail!("Docker daemon is not running. Start Docker and try again.");
    }
    Ok(())
}

pub fn container_running() -> Result<bool> {
    let output = Command::new("docker")
        .args(["inspect", "-f", "{{.State.Running}}", CONTAINER_NAME])
        .output()
        .context("Failed to inspect devnet container")?;

    if !output.status.success() {
        return Ok(false);
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim() == "true")
}

fn container_exists() -> Result<bool> {
    let output = Command::new("docker")
        .args(["inspect", CONTAINER_NAME])
        .output()
        .context("Failed to inspect devnet container")?;

    Ok(output.status.success())
}

fn run_container(host_port: u16) -> Result<()> {
    let port_mapping = format!("{}:{}", host_port, CONTAINER_RPC_PORT);
    let output = Command::new("docker")
        .args([
            "run",
            "-d",
            "--name",
            CONTAINER_NAME,
            "-p",
            &port_mapping,
            QUICKSTART_IMAGE,
            "--local",
        ])
        .output()
        .with_context(|| {
            format!(
                "Failed to start {}. Is port {} already in use?",
                QUICKSTART_IMAGE, host_port
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to start local devnet container:\n{}", stderr.trim());
    }
    Ok(())
}

fn start_existing_container() -> Result<()> {
    let output = Command::new("docker")
        .args(["start", CONTAINER_NAME])
        .output()
        .context("Failed to start existing devnet container")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to start devnet container:\n{}", stderr.trim());
    }
    Ok(())
}

pub fn wait_for_healthy(host_port: u16) -> Result<()> {
    let root_url = format!("http://127.0.0.1:{}/", host_port);
    let rpc_url = rpc_url(host_port);

    for attempt in 1..=HEALTH_ATTEMPTS {
        if probe_url(&root_url) || probe_url(&rpc_url) {
            return Ok(());
        }
        if attempt < HEALTH_ATTEMPTS {
            thread::sleep(HEALTH_INTERVAL);
        }
    }

    anyhow::bail!(
        "Local devnet did not become healthy on port {} within {} seconds",
        host_port,
        HEALTH_ATTEMPTS as u64 * HEALTH_INTERVAL.as_secs()
    )
}

fn probe_url(url: &str) -> bool {
    ureq::get(url)
        .timeout(Duration::from_secs(3))
        .call()
        .map(|r| r.status() < 500)
        .unwrap_or(false)
}

/// Start (or reuse) the local quickstart devnet container and wait until healthy.
pub fn start_devnet(host_port: u16) -> Result<()> {
    ensure_docker_available()?;

    if container_running()? {
        wait_for_healthy(host_port)?;
        return Ok(());
    }

    if container_exists()? {
        start_existing_container()?;
    } else {
        run_container(host_port)?;
    }

    wait_for_healthy(host_port)
}

/// Ensure the devnet is running (used by shell / docker-testnet workflows).
pub fn ensure_running(host_port: u16) -> Result<()> {
    start_devnet(host_port)
}

pub fn stop_devnet() -> Result<()> {
    ensure_docker_available()?;

    if !container_exists()? {
        return Ok(());
    }

    let output = Command::new("docker")
        .args(["rm", "-f", CONTAINER_NAME])
        .output()
        .context("Failed to stop devnet container")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to stop devnet container:\n{}", stderr.trim());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rpc_url_uses_host_port() {
        assert_eq!(rpc_url(8000), "http://127.0.0.1:8000/rpc");
        assert_eq!(rpc_url(9000), "http://127.0.0.1:9000/rpc");
    }
}
