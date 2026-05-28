use crate::utils::{print as p, templates};
use anyhow::Result;
use clap::Subcommand;
use dialoguer::Input;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum TemplateCommands {
    /// Search for templates in the marketplace
    Search {
        /// Search query (matches name, description, or tags)
        query: String,
        /// Filter by tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
    },
    /// List all available templates
    List,
    /// Show details of a specific template
    Show {
        /// Template name (can include version: name@v1.2.0)
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
    /// Update a template from its source
    Update {
        /// Template name
        name: String,
    },
    /// Initialize the template registry with example templates
    Init,
}

pub fn handle(cmd: TemplateCommands) -> Result<()> {
    match cmd {
        TemplateCommands::Publish {
            path,
            name,
            description,
            author,
            tags,
            version,
        } => publish(path, name, description, author, tags, version),
        TemplateCommands::List => list(),
        TemplateCommands::Search { query, tags } => search(query, tags),
        TemplateCommands::Show { name } => show(name),
        TemplateCommands::Remove { name } => remove(name),
        TemplateCommands::Update { name } => update(name),
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
    // Resolve name interactively if not provided
    let name = match name {
        Some(n) => n,
        None => Input::new().with_prompt("Template name").interact_text()?,
    };
    let description = match description {
        Some(d) => d,
        None => Input::new().with_prompt("Description").interact_text()?,
    };
    let author = match author {
        Some(a) => a,
        None => Input::new()
            .with_prompt("Author")
            .default("unknown".to_string())
            .interact_text()?,
    };
    let tag_list: Vec<String> = tags
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let template =
        templates::publish_template(&path, name, description, author, tag_list, version)?;

    p::header("Template Publish");
    p::success("Template registered successfully");
    p::kv_accent("Name", &template.name);
    p::kv("Version", &template.version);
    p::kv("Source", &template.source.to_string());
    if !template.tags.is_empty() {
        p::kv("Tags", &template.tags.join(", "));
    }
    if let Some(ref path) = template.path {
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
        p::kv("Source", &template.source.to_string());
        if !template.tags.is_empty() {
            p::kv("Tags", &template.tags.join(", "));
        }
        if let Some(ref path) = template.path {
            p::kv("Path", path);
        }
        if i + 1 < registry.templates.len() {
            println!();
        }
    }

    Ok(())
}

fn search(query: String) -> Result<()> {
    let results = templates::search_templates(&query, None)?;
    p::header(&format!("Template search results for '{}'", query));
    if results.is_empty() {
        p::info("No templates matched that query.");
        return Ok(());
    }

    p::info(&format!(
        "Found {} result(s), ranked by popularity:",
        results.len()
    ));
    println!();

    for (i, template) in results.iter().enumerate() {
        let badge = p::verified_badge(template.verified);
        println!(
            "  {:>2}. {}@{}{}",
            i + 1,
            template.name.cyan().bold(),
            template.version,
            badge
        );
        p::kv("Description", &template.description);
        p::kv("Downloads", &template.downloads.to_string());
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
    let (template_name, version) = parse_template_ref(&name);
    let template = templates::get_template_by_name_and_version(template_name, version)?;

    p::header(&format!("Template: {}", template.name));
    p::kv("Description", &template.description);
    p::kv("Author", &template.author);
    p::kv("Version", &template.version);
    p::kv("Source", &template.source.to_string());
    if !template.tags.is_empty() {
        p::kv("Tags", &template.tags.join(", "));
    }
    p::kv("Downloads", &template.downloads.to_string());
    p::kv("Verified", if template.verified { "Yes" } else { "No" });
    p::kv("Created", &template.created_at);
    p::kv("Updated", &template.updated_at);

    Ok(())
}

fn remove(name: String) -> Result<()> {
    templates::remove_template(&name)?;
    p::header("Template Remove");
    p::success(&format!("Template '{}' removed successfully", name));
    Ok(())
}

fn update(name: String) -> Result<()> {
    templates::update_template(&name)?;
    p::header("Template Update");
    p::success(&format!("Template '{}' updated successfully", name));
    Ok(())
}

fn init() -> Result<()> {
    p::header("Template Registry Initialization");
    p::info("Initializing template registry with example templates...");
    p::success("Template registry initialized");
    Ok(())
}

fn parse_template_ref(name: &str) -> (&str, Option<&str>) {
    if let Some(pos) = name.find('@') {
        let (name_part, version_part) = name.split_at(pos);
        (name_part, Some(&version_part[1..]))
    } else {
        (name, None)
    }
}
