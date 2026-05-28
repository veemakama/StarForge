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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum TemplateSource {
    Git {
        url: String,
        #[serde(default)]
        branch: Option<String>,
    },
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
    #[serde(default)]
    pub source: serde_json::Value,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub verified: bool,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub scaffold: Option<ScaffoldConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaffoldConfig {
    pub questions: Vec<ScaffoldQuestion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaffoldQuestion {
    pub name: String,
    pub prompt: String,
    pub r#type: String,
    #[serde(default)]
    pub default: Option<String>,
    #[serde(default)]
    pub choices: Vec<String>,
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

pub fn get_template_by_name_and_version(name: &str, version: Option<&str>) -> Result<TemplateEntry> {
    let registry = load_registry()?;
    let mut matching: Vec<_> = registry
        .templates
        .into_iter()
        .filter(|t| t.name == name)
        .collect();

    if matching.is_empty() {
        return Err(anyhow::anyhow!("Template '{}' not found", name));
    }

    if let Some(v) = version {
        matching.sort_by(|a, b| {
            semver_cmp(&b.version, &a.version)
        });
        matching.into_iter().find(|t| t.version == v)
            .ok_or_else(|| anyhow::anyhow!("Template '{}@{}' not found", name, v))
    } else {
        matching.sort_by(|a, b| {
            semver_cmp(&b.version, &a.version)
        });
        Ok(matching.into_iter().next().unwrap())
    }
}

fn semver_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let parse_version = |v: &str| {
        v.strip_prefix('v')
            .unwrap_or(v)
            .split('.')
            .filter_map(|s| s.parse::<u64>().ok())
            .collect::<Vec<_>>()
    };
    parse_version(a).cmp(&parse_version(b))
}

pub fn template_source_content(name: &str) -> Result<Option<String>> {
    let entry = match get_template(name) {
        Ok(entry) => entry,
        Err(_) => return Ok(None),
    };

    let content = match &entry.source {
        serde_json::Value::Object(obj) => {
            if let Some(type_val) = obj.get("type").and_then(|v| v.as_str()) {
                match type_val {
                    "builtin" => {
                        if let Some(id) = obj.get("id").and_then(|v| v.as_str()) {
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
                        } else {
                            None
                        }
                    }
                    "local" => {
                        if let Some(path_val) = obj.get("path").and_then(|v| v.as_str()) {
                            let lib_rs = Path::new(path_val).join("src").join("lib.rs");
                            if lib_rs.exists() {
                                Some(fs::read_to_string(&lib_rs).with_context(|| {
                                    format!("Failed to read template source at {}", lib_rs.display())
                                })?)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            } else {
                None
            }
        }
        _ => None,
    };

    Ok(content)
}

pub fn add_template(entry: TemplateEntry) -> Result<()> {
    let mut registry = load_registry()?;

    if let Some(existing) = registry.templates.iter_mut().find(|t| t.name == entry.name && t.version == entry.version) {
        *existing = entry;
    } else {
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

pub fn update_template(name: &str) -> Result<()> {
    let entry = get_template(name)?;

    match &entry.source {
        serde_json::Value::Object(obj) => {
            if let Some(type_val) = obj.get("type").and_then(|v| v.as_str()) {
                match type_val {
                    "git" => {
                        if let Some(url) = obj.get("url").and_then(|v| v.as_str()) {
                            let branch = obj.get("branch").and_then(|v| v.as_str());
                            let dest = std::env::temp_dir().join(&entry.name);
                            if dest.exists() {
                                fs::remove_dir_all(&dest).ok();
                            }
                            fetch_git_template(url, branch, &dest)?;
                        }
                    }
                    _ => anyhow::bail!("Template source type '{}' does not support updates", type_val),
                }
            }
        }
        _ => {}
    }

    Ok(())
}

#[allow(dead_code)]
pub fn fetch_template(entry: &TemplateEntry, dest: &Path) -> Result<()> {
    match &entry.source {
        serde_json::Value::Object(obj) => {
            if let Some(type_val) = obj.get("type").and_then(|v| v.as_str()) {
                match type_val {
                    "git" => {
                        let url = obj.get("url").and_then(|v| v.as_str())
                            .ok_or_else(|| anyhow::anyhow!("Git URL not found"))?;
                        let branch = obj.get("branch").and_then(|v| v.as_str());
                        fetch_git_template(url, branch, dest)
                    }
                    "local" => {
                        let path = obj.get("path").and_then(|v| v.as_str())
                            .ok_or_else(|| anyhow::anyhow!("Local path not found"))?;
                        fetch_local_template(Path::new(path), dest)
                    }
                    "builtin" => {
                        anyhow::bail!("Built-in template should be handled separately")
                    }
                    _ => anyhow::bail!("Unknown template source type"),
                }
            } else {
                anyhow::bail!("Template source type not specified")
            }
        }
        _ => anyhow::bail!("Invalid template source"),
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

    let templates_dir = templates_dir()?;
    let dest = templates_dir.join(&name);

    if dest.exists() {
        anyhow::bail!(
            "Template '{}' already exists. Remove it first or use a different name.",
            name
        );
    }

    copy_dir_recursive(template_path, &dest)?;

    let scaffold = load_scaffold_config(&dest);

    let entry = TemplateEntry {
        name: name.clone(),
        version,
        description,
        author,
        tags,
        source: serde_json::json!({
            "type": "local",
            "path": dest.to_string_lossy().to_string()
        }),
        path: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
        downloads: 0,
        verified: false,
        scaffold,
    };

    add_template(entry.clone())?;

    Ok(entry)
}

fn load_scaffold_config(template_path: &Path) -> Option<ScaffoldConfig> {
    let scaffold_path = template_path.join("scaffold.json");
    if !scaffold_path.exists() {
        return None;
    }

    match fs::read_to_string(&scaffold_path) {
        Ok(content) => serde_json::from_str(&content).ok(),
        Err(_) => None,
    }
}

#[allow(dead_code)]
pub fn validate_template_structure(path: &Path) -> Result<()> {
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
            source: serde_json::json!({
                "type": "builtin",
                "id": "uniswap-v2"
            }),
            path: None,
            created_at: "2025-01-01T00:00:00Z".to_string(),
            updated_at: "2025-01-01T00:00:00Z".to_string(),
            downloads: 100,
            verified: true,
            scaffold: None,
        });

        let results: Vec<_> = registry
            .templates
            .iter()
            .filter(|t| t.name.contains("uniswap"))
            .collect();
        assert_eq!(results.len(), 1);

        let results: Vec<_> = registry
            .templates
            .iter()
            .filter(|t| t.tags.contains(&"defi".to_string()))
            .collect();
        assert_eq!(results.len(), 1);
    }
}
