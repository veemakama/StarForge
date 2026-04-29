use crate::utils::{print as p, templates};
use anyhow::Result;
use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum TemplateCommands {
    /// Publish a local contract template to the user registry
    Publish {
        /// Local path to the contract template directory
        path: PathBuf,
    },
    /// List known contract templates
    List,
    /// Search templates by name, description, or tags
    Search {
        /// Query text to search for
        query: String,
    },
}

pub fn handle(cmd: TemplateCommands) -> Result<()> {
    match cmd {
        TemplateCommands::Publish { path } => publish(path),
        TemplateCommands::List => list(),
        TemplateCommands::Search { query } => search(query),
    }
}

fn publish(path: PathBuf) -> Result<()> {
    let template = templates::publish_template(&path)?;

    p::header("Template Publish");
    p::success("Template registered successfully");
    p::kv_accent("Name", &template.name);
    p::kv("Version", &template.version);
    p::kv("Source", &template.source);
    if !template.tags.is_empty() {
        p::kv("Tags", &template.tags.join(", "));
    }
    if let Some(path) = template.path.as_ref() {
        p::kv("Path", path);
    }

    Ok(())
}

fn list() -> Result<()> {
    let registry = templates::load_registry()?;
    p::header("Template Registry");
    if registry.templates.is_empty() {
        p::info("No templates found. Publish one with: starforge template publish <path>");
        return Ok(());
    }

    for (i, template) in registry.templates.iter().enumerate() {
        println!("  {:>2}. {}@{}", i + 1, template.name, template.version);
        p::kv("Description", &template.description);
        p::kv("Source", &template.source);
        if !template.tags.is_empty() {
            p::kv("Tags", &template.tags.join(", "));
        }
        if let Some(path) = template.path.as_ref() {
            p::kv("Path", path);
        }
        if i + 1 < registry.templates.len() {
            println!();
        }
    }

    Ok(())
}

fn search(query: String) -> Result<()> {
    let results = templates::search_templates(&query)?;
    p::header(&format!("Template search results for '{}'", query));
    if results.is_empty() {
        p::info("No templates matched that query.");
        return Ok(());
    }

    for (i, template) in results.iter().enumerate() {
        println!("  {:>2}. {}@{}", i + 1, template.name, template.version);
        p::kv("Description", &template.description);
        p::kv("Source", &template.source);
        if !template.tags.is_empty() {
            p::kv("Tags", &template.tags.join(", "));
        }
        if i + 1 < results.len() {
            println!();
        }
    }

    Ok(())
}
