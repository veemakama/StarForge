use crate::utils::mock_soroban;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct LocalSorobanSandbox {
    wasm_path: PathBuf,
    network: String,
}

impl LocalSorobanSandbox {
    pub fn new<P: AsRef<Path>>(wasm_path: P, network: &str) -> Result<Self> {
        let wasm_path = wasm_path.as_ref().to_path_buf();
        if !wasm_path.exists() {
            anyhow::bail!("Contract wasm not found: {}", wasm_path.display());
        }

        if network == "docker-testnet" {
            mock_soroban::ensure_docker_sandbox()?;
        }

        Ok(Self {
            wasm_path,
            network: network.to_string(),
        })
    }

    pub fn invoke(&self, function: &str, args: &[String]) -> Result<String> {
        if self.network == "docker-testnet" {
            self.invoke_via_docker(function, args)
        } else {
            self.invoke_via_local_cli(function, args)
        }
    }

    fn invoke_via_local_cli(&self, function: &str, args: &[String]) -> Result<String> {
        let mut cmd = Command::new("stellar");
        cmd.arg("contract")
            .arg("invoke")
            .arg("--wasm")
            .arg(&self.wasm_path)
            .arg("--fn")
            .arg(function);

        if !args.is_empty() {
            cmd.arg("--");
            for arg in args {
                cmd.arg(arg);
            }
        }

        let out = cmd
            .output()
            .with_context(|| "Failed to run `stellar contract invoke` (is `stellar` installed?)")?;

        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let stdout = String::from_utf8_lossy(&out.stdout);
            anyhow::bail!(
                "Local invoke failed.\nstdout:\n{}\nstderr:\n{}",
                stdout.trim(),
                stderr.trim()
            );
        }

        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    fn invoke_via_docker(&self, function: &str, args: &[String]) -> Result<String> {
        let mut docker_args = vec![
            "run".to_string(),
            "--rm".to_string(),
            "--network".to_string(),
            "starforge_default".to_string(),
            "-v".to_string(),
            format!(
                "{}:/workspace/contract.wasm",
                self.wasm_path.canonicalize().unwrap_or_else(|_| self.wasm_path.clone()).display()
            ),
            "stellar/quickstart:latest".to_string(),
            "stellar".to_string(),
            "contract".to_string(),
            "invoke".to_string(),
            "--wasm".to_string(),
            "/workspace/contract.wasm".to_string(),
            "--rpc-url".to_string(),
            "http://soroban-rpc:8000".to_string(),
            "--network-passphrase".to_string(),
            "Test SDF Network ; September 2015".to_string(),
            "--fn".to_string(),
            function.to_string(),
        ];

        if !args.is_empty() {
            docker_args.push("--".to_string());
            docker_args.extend(args.iter().cloned());
        }

        let out = Command::new("docker")
            .args(&docker_args)
            .output()
            .with_context(|| "Failed to run `docker` for Soroban sandbox invocation.")?;

        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let stdout = String::from_utf8_lossy(&out.stdout);
            anyhow::bail!(
                "Docker sandbox invoke failed.\nstdout:\n{}\nstderr:\n{}",
                stdout.trim(),
                stderr.trim()
            );
        }

        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }
}
