use crate::utils::{print as p, tutorial_engine};
use anyhow::Result;
use clap::Subcommand;
use colored::*;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum TutorialCommands {
    /// List available tutorials
    List,
    /// Start a tutorial by slug (e.g. hello-world)
    Start { slug: String },
    /// Show current tutorial status
    Status,
}

pub fn handle(cmd: TutorialCommands) -> Result<()> {
    match cmd {
        TutorialCommands::List => list(),
        TutorialCommands::Start { slug } => start(slug),
        TutorialCommands::Status => status(),
    }
}

fn repo_root() -> Result<PathBuf> {
    Ok(std::env::current_dir()?)
}

fn list() -> Result<()> {
    let root = repo_root()?;
    let dir = tutorial_engine::tutorials_dir(&root);
    p::header("Tutorials");

    if !dir.exists() {
        p::warn(&format!(
            "No tutorials directory found at {}",
            dir.display()
        ));
        return Ok(());
    }

    let mut entries: Vec<_> = std::fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    if entries.is_empty() {
        p::info("No tutorials installed yet.");
        return Ok(());
    }

    p::separator();
    for (i, entry) in entries.iter().enumerate() {
        let slug = entry.file_name().to_string_lossy().to_string();
        println!("  {:>2}. {}", i + 1, slug.cyan().bold());
    }
    p::separator();
    Ok(())
}

fn start(slug: String) -> Result<()> {
    let root = repo_root()?;
    let dir = tutorial_engine::tutorials_dir(&root).join(&slug);
    if !dir.exists() {
        anyhow::bail!(
            "Tutorial '{}' not found. Run {} to see available tutorials.",
            slug,
            "starforge tutorial list".cyan()
        );
    }

    let mut status = tutorial_engine::load_status()?;
    status.active = Some(slug.clone());
    status.started_at = Some(chrono::Utc::now().to_rfc3339());
    tutorial_engine::save_status(&status)?;

    p::header(&format!("Tutorial: {}", slug));
    p::separator();
    p::info("This is an initial interactive tutorial scaffold.");
    p::info(&format!(
        "Open the tutorial content in: {}",
        dir.display().to_string().cyan()
    ));
    p::info("Track progress with: starforge tutorial status");
    Ok(())
}

fn status() -> Result<()> {
    let status = tutorial_engine::load_status()?;
    p::header("Tutorial Status");
    match status.active {
        Some(active) => {
            p::kv_accent("Active", &active);
            if let Some(ts) = status.started_at {
                p::kv("Started", &ts);
            }
        }
        None => {
            p::info(&format!(
                "No active tutorial. Start one with: {}",
                "starforge tutorial start hello-world".cyan()
            ));
        }
    }
    Ok(())
}
