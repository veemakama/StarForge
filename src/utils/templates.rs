use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TemplateRegistry {
    #[serde(default)]
    pub templates: Vec<TemplateEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateEntry {
    pub name: String,
    pub description: String,
    pub version: String,
    pub source: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct TemplateManifest {
    name: Option<String>,
    description: Option<String>,
    version: Option<String>,
    source: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
}

const DEFAULT_REGISTRY: &str = include_str!("../../templates/registry.json");

fn registry_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    let dir = home.join(".starforge").join("templates");
    if !dir.exists() {
        fs::create_dir_all(&dir).with_context(|| format!("Failed to create {}", dir.display()))?;
    }
    Ok(dir.join("registry.json"))
}

fn template_storage_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    let dir = home.join(".starforge").join("templates").join("storage");
    if !dir.exists() {
        fs::create_dir_all(&dir).with_context(|| format!("Failed to create {}", dir.display()))?;
    }
    Ok(dir)
}

pub fn load_registry() -> Result<TemplateRegistry> {
    let path = registry_path()?;
    if path.exists() {
        let s = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let reg: TemplateRegistry = serde_json::from_str(&s)
            .with_context(|| format!("Failed to parse {}", path.display()))?;
        return Ok(reg);
    }

    let reg: TemplateRegistry = serde_json::from_str(DEFAULT_REGISTRY)
        .context("Failed to parse embedded template registry")?;
    Ok(reg)
}

pub fn save_registry(reg: &TemplateRegistry) -> Result<()> {
    let path = registry_path()?;
    fs::write(&path, serde_json::to_string_pretty(reg)?)
        .with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

pub fn search_templates(query: &str) -> Result<Vec<TemplateEntry>> {
    let query = query.to_lowercase();
    let reg = load_registry()?;
    let matches = reg
        .templates
        .into_iter()
        .filter(|entry| {
            entry.name.to_lowercase().contains(&query)
                || entry.description.to_lowercase().contains(&query)
                || entry
                    .tags
                    .iter()
                    .any(|tag| tag.to_lowercase().contains(&query))
        })
        .collect();
    Ok(matches)
}

pub fn resolve_template(name: &str) -> Result<Option<TemplateEntry>> {
    let reg = load_registry()?;
    Ok(reg
        .templates
        .into_iter()
        .find(|entry| entry.name == name))
}

pub fn template_source_content(name: &str) -> Result<Option<String>> {
    if let Some(entry) = resolve_template(name)? {
        if let Some(path) = entry.path.as_ref() {
            let path = PathBuf::from(path);
            let candidates = [path.join("src/lib.rs"), path.join("lib.rs")];
            for candidate in candidates {
                if candidate.exists() {
                    return Ok(Some(
                        fs::read_to_string(&candidate)
                            .with_context(|| format!("Failed to read {}", candidate.display()))?,
                    ));
                }
            }
        }
    }
    Ok(None)
}

pub fn publish_template(template_dir: &Path) -> Result<TemplateEntry> {
    if !template_dir.exists() {
        anyhow::bail!("Template path does not exist: {}", template_dir.display());
    }
    if !template_dir.is_dir() {
        anyhow::bail!("Template path must be a directory: {}", template_dir.display());
    }

    let manifest = load_template_manifest(template_dir)?;
    let name = manifest
        .name
        .unwrap_or_else(|| template_dir.file_name().unwrap().to_string_lossy().to_string());
    let version = manifest.version.unwrap_or_else(|| "0.1.0".to_string());
    let description = manifest
        .description
        .unwrap_or_else(|| format!("Community template for {}", name));
    let source = manifest.source.unwrap_or_else(|| "community".to_string());
    let tags = manifest.tags;

    let storage_dir = template_storage_dir()?.join(&name).join(&version);
    if storage_dir.exists() {
        fs::remove_dir_all(&storage_dir)
            .with_context(|| format!("Failed to remove existing template storage {}", storage_dir.display()))?;
    }
    copy_dir_all(template_dir, &storage_dir)?;

    let entry = TemplateEntry {
        name: name.clone(),
        description,
        version: version.clone(),
        source,
        tags,
        path: Some(storage_dir.display().to_string()),
    };

    let mut registry = load_registry().unwrap_or_default();
    registry
        .templates
        .retain(|t| !(t.name == name && t.version == version));
    registry.templates.push(entry.clone());
    registry.sort_by(|a, b| a.name.cmp(&b.name).then(a.version.cmp(&b.version)));
    save_registry(&registry)?;
    Ok(entry)
}

fn load_template_manifest(template_dir: &Path) -> Result<TemplateManifest> {
    let manifest_path = template_dir.join("template.json");
    if manifest_path.exists() {
        let contents = fs::read_to_string(&manifest_path)
            .with_context(|| format!("Failed to read {}", manifest_path.display()))?;
        let manifest: TemplateManifest = serde_json::from_str(&contents)
            .with_context(|| format!("Failed to parse {}", manifest_path.display()))?;
        return Ok(manifest);
    }

    Ok(TemplateManifest {
        name: None,
        description: None,
        version: None,
        source: None,
        tags: vec!["community".to_string()],
    })
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)
        .with_context(|| format!("Failed to create directory {}", dst.display()))?;
    for entry in fs::read_dir(src).with_context(|| format!("Failed to read directory {}", src.display()))? {
        let entry = entry?;
        let path = entry.path();
        let dest = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&path, &dest)?;
        } else {
            fs::copy(&path, &dest)
                .with_context(|| format!("Failed to copy {} to {}", path.display(), dest.display()))?;
        }
    }
    Ok(())
}
