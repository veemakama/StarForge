use crate::utils::{print as p, templates};
use anyhow::Result;
use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum TemplateCommands {
    /// Search for templates in the marketplace
    Search {
        /// Search query (matches name, description, or tags). Use "" to list all.
        #[arg(default_value = "")]
        query: String,
        /// Filter by tags (comma-separated); a template must have all of them
        #[arg(long)]
        tags: Option<String>,
        /// Only show verified templates
        #[arg(long)]
        verified: bool,
        /// Only show templates with at least this quality score (0-100)
        #[arg(long, default_value_t = 0)]
        min_quality: u8,
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
    /// Install a template from a directory or .zip archive into the local registry
    Install {
        /// Path to template directory or .zip package
        path: PathBuf,
        /// Template name (defaults to directory/archive stem)
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
        /// Minimum StarForge CLI version required
        #[arg(long)]
        cli_version_min: Option<String>,
        /// Maximum StarForge CLI version supported
        #[arg(long)]
        cli_version_max: Option<String>,
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
        /// Minimum StarForge CLI version required (semver, e.g. "0.1.0")
        #[arg(long)]
        cli_version_min: Option<String>,
        /// Maximum StarForge CLI version supported (semver, e.g. "1.99.99")
        #[arg(long)]
        cli_version_max: Option<String>,
        /// SPDX license identifier (e.g. "MIT", "Apache-2.0")
        #[arg(long)]
        license: Option<String>,
        /// Source repository URL
        #[arg(long)]
        repository: Option<String>,
        /// Project homepage URL
        #[arg(long)]
        homepage: Option<String>,
        /// Extended documentation URL
        #[arg(long)]
        documentation: Option<String>,
    },
    /// Remove a template from the local marketplace
    Remove {
        /// Template name
        name: String,
    },
    /// Initialize the template registry with example templates
    Init,
    /// Show full metadata for a template: author, version, license, repository, trust badges
    Info {
        /// Template name
        name: String,
    },
    /// Install a template from a Git URL, local path, or marketplace registry name
    Install {
        /// Source: git URL (https://...), local filesystem path, or registry template name
        source: String,
        /// Override the installed template name (defaults to the template name or URL basename)
        #[arg(long)]
        name: Option<String>,
        /// Pin to a specific version when installing from the marketplace registry
        #[arg(long)]
        version: Option<String>,
        /// Overwrite the template if it is already installed
        #[arg(long, default_value = "false")]
        force: bool,
    },
    /// Update installed templates to their latest versions
    Update {
        /// Name of the template to update (omit when using --all)
        #[arg(long, conflicts_with = "all")]
        name: Option<String>,
        /// Update all installed git-sourced templates
        #[arg(long, short, conflicts_with = "name")]
        all: bool,
    },
}

pub fn handle(cmd: TemplateCommands) -> Result<()> {
    match cmd {
        TemplateCommands::Install {
            path,
            name,
            description,
            author,
            tags,
            version,
            cli_version_min,
            cli_version_max,
        } => install(
            path,
            name,
            description,
            author,
            tags,
            version,
            cli_version_min,
            cli_version_max,
        ),
        TemplateCommands::Publish {
            path,
            name,
            description,
            author,
            tags,
            version,
            cli_version_min,
            cli_version_max,
            license,
            repository,
            homepage,
            documentation,
        } => publish(
            path,
            name,
            description,
            author,
            tags,
            version,
            cli_version_min,
            cli_version_max,
            license,
            repository,
            homepage,
            documentation,
        ),
        TemplateCommands::List => list(),
        TemplateCommands::Search {
            query,
            tags,
            verified,
            min_quality,
            refresh,
        } => search(query, tags, verified, min_quality, refresh),
        TemplateCommands::Show { name } => show(name),
        TemplateCommands::Remove { name } => remove(name),
        TemplateCommands::Init => init(),
        TemplateCommands::Info { name } => info(name),
        TemplateCommands::Install {
            source,
            name,
            version,
            force,
        } => install(source, name, version, force),
        TemplateCommands::Update { name, all } => update(name, all),
    }
}

fn install(
    path: PathBuf,
    name: Option<String>,
    description: Option<String>,
    author: Option<String>,
    tags: Option<String>,
    version: String,
    cli_version_min: Option<String>,
    cli_version_max: Option<String>,
) -> Result<()> {
    publish(
        path,
        name,
        description,
        author,
        tags,
        version,
        cli_version_min,
        cli_version_max,
        None,
        None,
        None,
        None,
    )?;
    p::header("Template Install");
    p::info("Template package installed into the local registry.");
    Ok(())
}

fn publish(
    path: PathBuf,
    name: Option<String>,
    description: Option<String>,
    author: Option<String>,
    tags: Option<String>,
    version: String,
    cli_version_min: Option<String>,
    cli_version_max: Option<String>,
    license: Option<String>,
    repository: Option<String>,
    homepage: Option<String>,
    documentation: Option<String>,
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

    templates::publish_template_versioned(
        &path,
        name.clone(),
        description,
        author,
        tag_list,
        version,
        cli_version_min,
        cli_version_max,
        license,
        repository,
        homepage,
        documentation,
    )?;
    let template = templates::get_template(&name)?;

    p::header("Template Publish");
    p::success("Template registered successfully");
    p::kv_accent("Name", &template.name);
    p::kv("Version", &template.version);
    p::kv("Source", &template.source.to_string());
    if !template.tags.is_empty() {
        p::kv("Tags", &template.tags.join(", "));
    }
    if let Some(lic) = template.license.as_ref() {
        p::kv("License", lic);
    }
    if let Some(repo) = template.repository.as_ref() {
        p::kv("Repository", repo);
    }
    if let Some(path) = template.path.as_ref() {
        p::kv("Path", path);
    }

    Ok(())
}

fn list() -> Result<()> {
    use crate::utils::templates::{check_template_compatibility, CompatibilityStatus};

    let registry = templates::load_registry()?;
    p::header("Template Registry");
    if registry.templates.is_empty() {
        p::info("No templates found. Publish one with: starforge template publish <path>");
        return Ok(());
    }

    for (i, template) in registry.templates.iter().enumerate() {
        let compat_badge = match check_template_compatibility(template) {
            CompatibilityStatus::Compatible => "[COMPATIBLE]",
            CompatibilityStatus::TooOld { .. } | CompatibilityStatus::TooNew { .. } => {
                "[INCOMPATIBLE]"
            }
            CompatibilityStatus::MalformedMetadata { .. } => "[BAD-META]",
        };
        let mut badges = template.trust_indicators();
        badges.push(compat_badge.to_string());
        println!(
            "  {:>2}. {}@{}  [quality {}/100]  {}",
            i + 1,
            template.name,
            template.version,
            template.quality_score(),
            badges.join(" "),
        );
        p::kv("Description", &template.description);
        p::kv("Source", &template.source.to_string());
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

fn search(
    query: String,
    tags: Option<String>,
    verified: bool,
    min_quality: u8,
    refresh: bool,
) -> Result<()> {
    use crate::utils::templates::{check_template_compatibility, CompatibilityStatus};
    let tag_list: Vec<String> = tags
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let filters = templates::SearchFilters {
        tags: tag_list,
        verified_only: verified,
        min_quality,
    };

    // Load registry, optionally forcing a refresh of the remote copy.
    let results = if refresh {
        std::env::set_var("STARFORGE_TEMPLATE_REGISTRY_FORCE_REFRESH", "1");
        let res = templates::search_templates_ranked(&query, &filters);
        std::env::remove_var("STARFORGE_TEMPLATE_REGISTRY_FORCE_REFRESH");
        res?
    } else {
        templates::search_templates_ranked(&query, &filters)?
    };

    let heading = if query.trim().is_empty() {
        "Template search results".to_string()
    } else {
        format!("Template search results for '{}'", query)
    };
    p::header(&heading);

    // Summarize the active filters so users understand the result set.
    let mut active_filters = Vec::new();
    if !filters.tags.is_empty() {
        active_filters.push(format!("tags: {}", filters.tags.join(", ")));
    }
    if filters.verified_only {
        active_filters.push("verified only".to_string());
    }
    if filters.min_quality > 0 {
        active_filters.push(format!("min quality: {}", filters.min_quality));
    }
    if !active_filters.is_empty() {
        p::kv("Filters", &active_filters.join("  |  "));
    }

    if results.is_empty() {
        p::info("No templates matched. Try a broader query or relaxing the filters.");
        return Ok(());
    }

    p::kv("Matches", &results.len().to_string());
    println!();

    for (i, result) in results.iter().enumerate() {
        let template = &result.entry;
        let compat_badge = match check_template_compatibility(template) {
            CompatibilityStatus::Compatible => "[COMPATIBLE]",
            CompatibilityStatus::TooOld { .. } | CompatibilityStatus::TooNew { .. } => {
                "[INCOMPATIBLE]"
            }
            CompatibilityStatus::MalformedMetadata { .. } => "[BAD-META]",
        };
        let mut badges = template.trust_indicators();
        badges.push(compat_badge.to_string());
        println!(
            "  {:>2}. {}@{}  [quality {}/100]  {}",
            i + 1,
            template.name,
            template.version,
            template.quality_score(),
            badges.join(" "),
        );
        p::kv("Description", &template.description);
        p::kv("Downloads", &template.downloads.to_string());
        if !template.tags.is_empty() {
            p::kv("Tags", &template.tags.join(", "));
        }
        // Explain why this result matched, helping users scan the list.
        if !result.reasons.is_empty() {
            p::kv(
                "Matched",
                &format!(
                    "{} (relevance {})",
                    result.reasons.join(", "),
                    result.relevance
                ),
            );
        }
        p::kv("Source", &template.source.to_string());
        if i + 1 < results.len() {
            println!();
        }
    }

    Ok(())
}

fn show(name: String) -> Result<()> {
    use crate::utils::templates::{check_template_compatibility, CompatibilityStatus};

    let template = templates::get_template(&name)?;
    p::header(&format!("Template: {}", template.name));
    p::kv("Version", &template.version);
    p::kv("Description", &template.description);
    p::kv("Source", &template.source.to_string());
    if !template.author.is_empty() {
        p::kv("Author", &template.author);
    }
    if !template.tags.is_empty() {
        p::kv("Tags", &template.tags.join(", "));
    }
    if let Some(ref license) = template.license {
        p::kv("License", license);
    }
    if let Some(ref repo) = template.repository {
        p::kv("Repository", repo);
    }
    if let Some(ref hp) = template.homepage {
        p::kv("Homepage", hp);
    }
    if let Some(ref doc_url) = template.documentation {
        p::kv("Documentation", doc_url);
    }
    if let Some(ref min) = template.cli_version_min {
        p::kv("Requires StarForge >=", min);
    }
    if let Some(ref max) = template.cli_version_max {
        p::kv("Requires StarForge <=", max);
    }
    match check_template_compatibility(&template) {
        CompatibilityStatus::Compatible => p::success("Compatible with this StarForge version"),
        CompatibilityStatus::TooOld {
            required_min,
            running,
        } => {
            p::warn(&format!(
                "Incompatible: requires >= {} (running {})",
                required_min, running
            ));
        }
        CompatibilityStatus::TooNew {
            required_max,
            running,
        } => {
            p::warn(&format!(
                "Incompatible: requires <= {} (running {})",
                required_max, running
            ));
        }
        CompatibilityStatus::MalformedMetadata { reason } => {
            p::warn(&format!("Malformed version metadata: {}", reason));
        }
    }
    print_quality_signals(&template);
    Ok(())
}

/// Render the quality / trust signals for a template so users can quickly
/// gauge how dependable it is.
fn print_quality_signals(template: &templates::TemplateEntry) {
    p::kv(
        "Quality score",
        &format!("{}/100", template.quality_score()),
    );
    p::kv("Maintenance", template.maintenance.label());
    p::kv(
        "Documentation",
        if template.documented {
            "Available"
        } else {
            "Not provided"
        },
    );
    p::kv("Downloads", &template.downloads.to_string());
    let badges = template.trust_indicators();
    if !badges.is_empty() {
        p::kv("Trust signals", &badges.join("  "));
    }
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

fn info(name: String) -> Result<()> {
    use crate::utils::templates::{check_template_compatibility, CompatibilityStatus};

    let template = templates::get_template(&name)?;

    p::header(&format!("Template Info: {}", template.name));
    p::separator();

    p::kv_accent("Name", &template.name);
    p::kv("Version", &template.version);

    if !template.author.is_empty() {
        p::kv("Author", &template.author);
    }
    if !template.description.is_empty() {
        p::kv("Description", &template.description);
    }
    if !template.tags.is_empty() {
        p::kv("Tags", &template.tags.join(", "));
    }

    println!();
    p::info("Source & Repository");
    p::kv("Source", &template.source.to_string());
    if let Some(ref repo) = template.repository_url {
        p::kv("Repository", repo);
    }

    println!();
    p::info("Licensing & Compatibility");
    if let Some(ref license) = template.license {
        p::kv("License", license);
    } else {
        p::kv("License", "Not declared");
    }
    match (
        template.cli_version_min.as_deref(),
        template.cli_version_max.as_deref(),
    ) {
        (Some(min), Some(max)) => p::kv("CLI Version Range", &format!(">= {}  <=  {}", min, max)),
        (Some(min), None) => p::kv("CLI Version Range", &format!(">= {}", min)),
        (None, Some(max)) => p::kv("CLI Version Range", &format!("<= {}", max)),
        (None, None) => p::kv("CLI Version Range", "Any version"),
    }
    match check_template_compatibility(&template) {
        CompatibilityStatus::Compatible => p::success("Compatible with this StarForge version"),
        CompatibilityStatus::TooOld {
            required_min,
            running,
        } => p::warn(&format!(
            "Incompatible: requires >= {} (running {})",
            required_min, running
        )),
        CompatibilityStatus::TooNew {
            required_max,
            running,
        } => p::warn(&format!(
            "Incompatible: requires <= {} (running {})",
            required_max, running
        )),
        CompatibilityStatus::MalformedMetadata { reason } => {
            p::warn(&format!("Malformed version metadata: {}", reason))
        }
    }

    println!();
    p::info("Quality & Trust");
    p::kv(
        "Quality Score",
        &format!("{}/100", template.quality_score()),
    );
    p::kv("Maintenance", template.maintenance.label());
    p::kv(
        "Documentation",
        if template.documented {
            "Available"
        } else {
            "Not provided"
        },
    );
    p::kv("Downloads", &template.downloads.to_string());

    let badges = template.trust_indicators();
    if !badges.is_empty() {
        p::kv("Trust Badges", &badges.join("  "));
    }

    if !template.created_at.is_empty() {
        println!();
        p::info("Timestamps");
        p::kv("Published", &template.created_at);
        if !template.updated_at.is_empty() {
            p::kv("Last Updated", &template.updated_at);
        }
    }

    p::separator();
    Ok(())
}

fn install(
    source: String,
    name: Option<String>,
    version: Option<String>,
    force: bool,
) -> Result<()> {
    p::header("Template Install");
    p::kv("Source", &source);
    if let Some(ref n) = name {
        p::kv("Name override", n);
    }
    if let Some(ref v) = version {
        p::kv("Version", v);
    }
    println!();

    p::step(1, 2, "Resolving and fetching template...");
    let entry = templates::install_template(&source, name.as_deref(), version.as_deref(), force)?;

    p::step(2, 2, "Registering in local registry...");
    println!();
    p::success(&format!("Template '{}' installed", entry.name));
    p::kv_accent("Name", &entry.name);
    p::kv("Version", &entry.version);
    p::kv("Source", &entry.source.to_string());
    if let Some(ref path) = entry.path {
        p::kv("Local path", path);
    }
    p::info(&format!(
        "Use it with: starforge template info {}",
        entry.name
    ));
    Ok(())
}

fn update(name: Option<String>, all: bool) -> Result<()> {
    if all {
        p::header("Template Update — All");
        p::step(1, 1, "Updating all git-sourced templates...");
        let results = templates::update_all_installed_templates()?;

        if results.is_empty() {
            p::info("No git-sourced templates are installed.");
            return Ok(());
        }

        println!();
        for (tpl_name, result) in &results {
            match result {
                Ok(()) => p::success(&format!("  {} updated", tpl_name)),
                Err(e) => p::warn(&format!("  {} — {}", tpl_name, e)),
            }
        }

        let ok = results.iter().filter(|(_, r)| r.is_ok()).count();
        println!();
        p::kv("Updated", &format!("{}/{}", ok, results.len()));
        return Ok(());
    }

    let name = name.ok_or_else(|| {
        anyhow::anyhow!("Provide a template name or --all to update all templates")
    })?;

    p::header(&format!("Template Update: {}", name));
    p::step(1, 1, "Re-fetching from source...");
    templates::update_installed_template(&name)?;
    println!();
    p::success(&format!("Template '{}' updated", name));
    Ok(())
}
