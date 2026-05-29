use crate::utils::{print as p, templates};
use anyhow::Result;
use clap::Subcommand;
use colored::*;
use dialoguer::{Confirm, Input};
use std::path::PathBuf;
use colored::Colorize;

#[derive(Subcommand)]
pub enum TemplateCommands {
    /// Search for templates in the marketplace
    Search {
        /// Search query (matches name, description, or tags)
        query: String,
        /// Filter by tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
        /// Force refresh of remote registry, ignoring cached copy
        #[arg(long)]
        refresh: bool,
    },
    /// List all available templates
    List,
    /// Show details of a specific template
    Show {
        /// Template name
        name: String,
    },
    /// Publish a template to the local marketplace
    Publish {
        /// Path to the template directory
        path: PathBuf,
        /// Template name
        #[arg(long)]
        name: Option<String>,
        /// Template description
        #[arg(long)]
        description: Option<String>,
        /// Author name
        #[arg(long)]
        author: Option<String>,
        /// Tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
        /// Version
        #[arg(long, default_value = "1.0.0")]
        version: String,
    },
    /// Remove a template from the local marketplace
    Remove {
        /// Template name
        name: String,
    },
    /// Initialize the template registry with example templates
    Init,
}

pub fn handle(cmd: TemplateCommands) -> Result<()> {
    match cmd {
        TemplateCommands::Publish { path, name, description, author, tags, version } => {
            publish(path, name, description, author, tags, version)
        }
        TemplateCommands::List => list(),
        TemplateCommands::Search { query, tags, refresh } => search(query, tags, refresh),
        TemplateCommands::Show { name } => show(name),
        TemplateCommands::Remove { name } => remove(name),
        TemplateCommands::Init => init(),
    }
}

fn publish(
    path: PathBuf,
    name: Option<String>,
    description: Option<String>,
    author: Option<String>,
    tags: Option<String>,
    version: String,
) -> Result<()> {
    use dialoguer::{theme::ColorfulTheme, Input};
    let name = match name {
        Some(n) => n,
        None => Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Template name")
            .interact_text()?,
    };
    let description = match description {
        Some(d) => d,
        None => Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Description")
            .interact_text()?,
    };
    let author = match author {
        Some(a) => a,
        None => Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Author")
            .interact_text()?,
    };
    let tag_list: Vec<String> = tags
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    templates::publish_template(&path, name.clone(), description, author, tag_list, version)?;
    let template = templates::get_template(&name)?;

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

fn search(query: String, tags: Option<String>, refresh: bool) -> Result<()> {
    // Determine tags filter if provided (comma-separated)
    let tag_vec: Option<Vec<String>> = tags.as_ref().map(|t| t.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect());

    // Load registry, optionally forcing a refresh
    let results = if refresh {
        // Temporarily set env var to force refresh
        std::env::set_var("STARFORGE_TEMPLATE_REGISTRY_FORCE_REFRESH", "1");
        let res = templates::search_templates(&query, tag_vec.as_ref().map(|v| &v[..]));
        std::env::remove_var("STARFORGE_TEMPLATE_REGISTRY_FORCE_REFRESH");
        res?
    } else {
        templates::search_templates(&query, tag_vec.as_ref().map(|v| &v[..]))?
    };

fn search(query: String, tags: Option<String>) -> Result<()> {
    let tag_list: Option<Vec<String>> = tags.map(|t| {
        t.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
    });
    let results = templates::search_templates(&query, tag_list.as_deref())?;
    p::header(&format!("Template search results for '{}'", query));
    if results.is_empty() {
        p::info("No templates matched that query.");
        return Ok(());
    }

    for (i, template) in results.iter().enumerate() {
        println!("  {:>2}. {}@{}", i + 1, template.name, template.version);
        p::kv("Description", &template.description);
        p::kv("Downloads", &template.downloads.to_string());
        p::kv("Source", &template.source.to_string());
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

fn show(name: String) -> Result<()> {
    let template = templates::get_template(&name)?;
    p::header(&format!("Template: {}", template.name));
    p::kv("Version", &template.version);
    p::kv("Description", &template.description);
    p::kv("Source", &template.source);
    if !template.author.is_empty() {
        p::kv("Author", &template.author);
    }
    if !template.tags.is_empty() {
        p::kv("Tags", &template.tags.join(", "));
    }
    Ok(())
}

fn remove(name: String) -> Result<()> {
    templates::remove_template(&name)?;
    p::success(&format!("Template '{}' removed", name));
    Ok(())
}

fn init() -> Result<()> {
    p::info("Template registry is ready. Use `starforge template list` to view templates.");
    Ok(())
}
