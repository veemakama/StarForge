use crate::utils::{config, print as p, telemetry};
use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum TelemetryCommands {
    /// Enable telemetry collections
    Enable,
    /// Disable telemetry collections
    Disable,
    /// Show current telemetry status
    Status,
}

pub fn handle(cmd: TelemetryCommands) -> Result<()> {
    match cmd {
        TelemetryCommands::Enable => {
            telemetry::set_telemetry_enabled(true)?;
            p::success("Telemetry collections enabled.");
        }
        TelemetryCommands::Disable => {
            telemetry::set_telemetry_enabled(false)?;
            p::success("Telemetry collections disabled.");
        }
        TelemetryCommands::Status => {
            let cfg = config::load()?;
            let enabled = cfg.telemetry_enabled.unwrap_or(true);
            let env_override = std::env::var("STARFORGE_TELEMETRY").ok();
            
            p::header("Telemetry Status");
            p::separator();
            p::kv("Configured Enabled", &enabled.to_string());
            if let Some(env_val) = env_override {
                p::kv("Environment Override (STARFORGE_TELEMETRY)", &env_val);
            }
            p::separator();
        }
    }
    Ok(())
}
