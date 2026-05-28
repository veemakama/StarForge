use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TemplateRegistry {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub templates: Vec<TemplateEntry>,
}

/// Describes where a template's source files live.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum TemplateSource {
    /// Clone from a remote git repository.
    Git {
        url: String,
        #[serde(default)]
        branch: Option<String>,
    },
    /// Copy from a local directory on disk.
    Local { path: String },
    /// A built-in template bundled with StarForge.
    Builtin { id: String },
}

impl std::fmt::Display for TemplateSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TemplateSource::Git { url, branch } => match branch {
                Some(b) => write!(f, "git:{} (branch: {})", url, b),
                None => write!(f, "git:{}", url),
            },
            TemplateSource::Local { path } => write!(f, "local:{}", path),
            TemplateSource::Builtin { id } => write!(f, "builtin:{}", id),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TemplateSource {
    Git { url: String, branch: Option<String> },
    Local { path: String },
    Builtin { id: String },
}

impl std::fmt::Display for TemplateSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TemplateSource::Git { url, branch } => {
                if let Some(branch) = branch {
                    write!(f, "git:{}@{}", url, branch)
                } else {
                    write!(f, "git:{}", url)
                }
            }
            TemplateSource::Local { path } => write!(f, "local:{}", path),
            TemplateSource::Builtin { id } => write!(f, "builtin:{}", id),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TemplateSource {
    Git { url: String, branch: Option<String> },
    Local { path: String },
    Builtin { id: String },
}

impl std::fmt::Display for TemplateSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TemplateSource::Git { url, branch } => {
                if let Some(branch) = branch {
                    write!(f, "git:{}@{}", url, branch)
                } else {
                    write!(f, "git:{}", url)
                }
            }
            TemplateSource::Local { path } => write!(f, "local:{}", path),
            TemplateSource::Builtin { id } => write!(f, "builtin:{}", id),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateEntry {
    pub name: String,
    pub description: String,
    pub author: String,
    pub version: String,
    pub author: String,
    pub source: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub verified: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct TemplateManifest {
    name: Option<String>,
    description: Option<String>,
    version: Option<String>,
    source: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
}

#[allow(dead_code)]
const DEFAULT_REGISTRY: &str = include_str!("../../templates/registry.json");

fn registry_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    let dir = home.join(".starforge").join("templates");
    if !dir.exists() {
        fs::create_dir_all(&dir).with_context(|| format!("Failed to create {}", dir.display()))?;
    }
    Ok(dir.join("registry.json"))
}

fn templates_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    let dir = home.join(".starforge").join("templates").join("storage");
    if !dir.exists() {
        fs::create_dir_all(&dir).with_context(|| format!("Failed to create {}", dir.display()))?;
    }
    Ok(dir)
}

#[allow(dead_code)]
fn template_storage_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    let dir = home.join(".starforge").join("templates").join("storage");
    if !dir.exists() {
        fs::create_dir_all(&dir).with_context(|| format!("Failed to create {}", dir.display()))?;
    }
    Ok(dir)
}

/// Returns the user-level templates directory where published templates are stored.
fn templates_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    let dir = home.join(".starforge").join("templates").join("local");
    if !dir.exists() {
        fs::create_dir_all(&dir).with_context(|| format!("Failed to create {}", dir.display()))?;
    }
    Ok(dir)
}

pub fn load_registry() -> Result<TemplateRegistry> {
    let path = registry_path()?;
    if !path.exists() {
        return Ok(TemplateRegistry::default());
    }
    let contents = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read registry at {}", path.display()))?;
    let registry: TemplateRegistry =
        serde_json::from_str(&contents).with_context(|| "Failed to parse template registry")?;
    Ok(registry)
}

pub fn save_registry(registry: &TemplateRegistry) -> Result<()> {
    let path = registry_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }
    let contents =
        serde_json::to_string_pretty(registry).with_context(|| "Failed to serialize registry")?;
    fs::write(&path, contents)
        .with_context(|| format!("Failed to write registry to {}", path.display()))?;
    Ok(())
}

pub fn search_templates(query: &str, tags: Option<&[String]>) -> Result<Vec<TemplateEntry>> {
    let registry = load_registry()?;
    let query_lower = query.to_lowercase();

    let mut results: Vec<TemplateEntry> = registry
        .templates
        .into_iter()
        .filter(|t| {
            let name_match = t.name.to_lowercase().contains(&query_lower);
            let desc_match = t.description.to_lowercase().contains(&query_lower);
            let tag_match = t
                .tags
                .iter()
                .any(|tag| tag.to_lowercase().contains(&query_lower));

            let text_match = name_match || desc_match || tag_match;

            if let Some(filter_tags) = tags {
                let has_all_tags = filter_tags
                    .iter()
                    .all(|ft| t.tags.iter().any(|t| t.eq_ignore_ascii_case(ft)));
                text_match && has_all_tags
            } else {
                text_match
            }
        })
        .collect();

    // Sort by downloads (popularity) and verified status
    results.sort_by(|a, b| match (a.verified, b.verified) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => b.downloads.cmp(&a.downloads),
    });

    Ok(results)
}

#[allow(dead_code)]
pub fn get_template(name: &str) -> Result<TemplateEntry> {
    let registry = load_registry()?;
    registry
        .templates
        .into_iter()
        .find(|t| t.name == name)
        .ok_or_else(|| anyhow::anyhow!("Template '{}' not found in registry", name))
}

pub fn template_source_content(name: &str) -> Result<Option<String>> {
    let entry = match get_template(name) {
        Ok(entry) => entry,
        Err(_) => return Ok(None),
    };

    let content = match &entry.source {
        TemplateSource::Builtin { id } => {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("templates")
                .join("examples")
                .join(id)
                .join("src")
                .join("lib.rs");
            if path.exists() {
                Some(fs::read_to_string(&path).with_context(|| {
                    format!("Failed to read built-in template at {}", path.display())
                })?)
            } else {
                None
            }
        }
        TemplateSource::Local { path } => {
            let lib_rs = Path::new(path).join("src").join("lib.rs");
            if lib_rs.exists() {
                Some(fs::read_to_string(&lib_rs).with_context(|| {
                    format!("Failed to read template source at {}", lib_rs.display())
                })?)
            } else {
                None
            }
        }
        TemplateSource::Git { .. } => None,
    };

    Ok(content)
}

pub fn add_template(entry: TemplateEntry) -> Result<()> {
    let mut registry = load_registry()?;

    // Check if template already exists
    if let Some(existing) = registry.templates.iter_mut().find(|t| t.name == entry.name) {
        // Update existing template
        *existing = entry;
    } else {
        // Add new template
        registry.templates.push(entry);
    }

    save_registry(&registry)?;
    Ok(())
}

pub fn remove_template(name: &str) -> Result<()> {
    let mut registry = load_registry()?;
    let before = registry.templates.len();
    registry.templates.retain(|t| t.name != name);

    if registry.templates.len() == before {
        anyhow::bail!("Template '{}' not found in registry", name);
    }

    save_registry(&registry)?;
    Ok(())
}

#[allow(dead_code)]
pub fn fetch_template(entry: &TemplateEntry, dest: &Path) -> Result<()> {
    match &entry.source {
        TemplateSource::Git { url, branch } => fetch_git_template(url, branch.as_deref(), dest),
        TemplateSource::Local { path } => fetch_local_template(Path::new(path), dest),
        TemplateSource::Builtin { id } => {
            anyhow::bail!("Built-in template '{}' should be handled separately", id)
        }
    }
}

#[allow(dead_code)]
fn fetch_git_template(url: &str, branch: Option<&str>, dest: &Path) -> Result<()> {
    use std::process::Command;

    let mut cmd = Command::new("git");
    cmd.arg("clone");

    if let Some(b) = branch {
        cmd.arg("--branch").arg(b);
    }

    cmd.arg("--depth").arg("1");
    cmd.arg(url);
    cmd.arg(dest);

    let output = cmd
        .output()
        .with_context(|| "Failed to execute git clone. Is git installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Git clone failed: {}", stderr);
    }

    // Remove .git directory to clean up
    let git_dir = dest.join(".git");
    if git_dir.exists() {
        fs::remove_dir_all(&git_dir).ok();
    }

    Ok(())
}

#[allow(dead_code)]
fn fetch_local_template(source: &Path, dest: &Path) -> Result<()> {
    if !source.exists() {
        anyhow::bail!("Local template path does not exist: {}", source.display());
    }

    copy_dir_recursive(source, dest)
        .with_context(|| format!("Failed to copy template from {}", source.display()))?;

    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();

        // Skip .git directories
        if file_name == ".git" {
            continue;
        }

        let dest_path = dst.join(&file_name);

        if path.is_dir() {
            copy_dir_recursive(&path, &dest_path)?;
        } else {
            fs::copy(&path, &dest_path)?;
        }
    }

    Ok(())
}

pub fn publish_template(
    template_path: &Path,
    name: String,
    description: String,
    author: String,
    tags: Vec<String>,
    version: String,
) -> Result<TemplateEntry> {
    if !template_path.exists() {
        anyhow::bail!("Template path does not exist: {}", template_path.display());
    }

    // Copy template to local templates directory
    let templates_dir = templates_dir()?;
    let dest = templates_dir.join(&name);

    if dest.exists() {
        anyhow::bail!(
            "Template '{}' already exists. Remove it first or use a different name.",
            name
        );
    }

    copy_dir_recursive(template_path, &dest)?;

    // Create template entry
    let entry = TemplateEntry {
        name: name.clone(),
        version,
        description,
        author,
        tags,
        source: TemplateSource::Local {
            path: dest.to_string_lossy().to_string(),
        },
        path: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
        downloads: 0,
        verified: false,
        path: None,
    };

    add_template(entry.clone())?;

    Ok(entry)
}

#[allow(dead_code)]
pub fn validate_template_structure(path: &Path) -> Result<()> {
    // Check for required files
    let cargo_toml = path.join("Cargo.toml");
    if !cargo_toml.exists() {
        anyhow::bail!("Template must contain Cargo.toml");
    }

    let src_dir = path.join("src");
    if !src_dir.exists() || !src_dir.is_dir() {
        anyhow::bail!("Template must contain src/ directory");
    }

    let lib_rs = src_dir.join("lib.rs");
    if !lib_rs.exists() {
        anyhow::bail!("Template must contain src/lib.rs");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_templates() {
        let mut registry = TemplateRegistry::default();
        registry.templates.push(TemplateEntry {
            name: "uniswap-v2".to_string(),
            version: "1.0.0".to_string(),
            description: "Uniswap V2 DEX implementation".to_string(),
            author: "DeFi Team".to_string(),
            tags: vec!["defi".to_string(), "dex".to_string(), "amm".to_string()],
            source: TemplateSource::Builtin {
                id: "uniswap-v2".to_string(),
            },
            path: None,
            created_at: "2025-01-01T00:00:00Z".to_string(),
            updated_at: "2025-01-01T00:00:00Z".to_string(),
            downloads: 100,
            verified: true,
            path: None,
        });

        // Test name search
        let results: Vec<_> = registry
            .templates
            .iter()
            .filter(|t| t.name.contains("uniswap"))
            .collect();
        assert_eq!(results.len(), 1);

        // Test tag search
        let results: Vec<_> = registry
            .templates
            .iter()
            .filter(|t| t.tags.contains(&"defi".to_string()))
            .collect();
        assert_eq!(results.len(), 1);
    }
}
