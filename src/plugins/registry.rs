use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginRegistry {
    #[serde(default)]
    pub plugins: Vec<InstalledPlugin>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPlugin {
    pub name: String,
    /// Stored as a string for portability (and easy display)
    pub path: String,
}

fn registry_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    let dir = home.join(".starforge").join("plugins");
    if !dir.exists() {
        fs::create_dir_all(&dir).with_context(|| format!("Failed to create {}", dir.display()))?;
    }
    Ok(dir.join("registry.json"))
}

pub fn load_registry() -> Result<PluginRegistry> {
    let path = registry_path()?;
    if !path.exists() {
        return Ok(PluginRegistry::default());
    }
    let s =
        fs::read_to_string(&path).with_context(|| format!("Failed to read {}", path.display()))?;
    let reg: PluginRegistry =
        serde_json::from_str(&s).with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(reg)
}

pub fn save_registry(reg: &PluginRegistry) -> Result<()> {
    let path = registry_path()?;
    fs::write(&path, serde_json::to_string_pretty(reg)?)
        .with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

pub fn install_plugin(name: &str, library_path: &Path) -> Result<()> {
    if !library_path.exists() {
        anyhow::bail!("Plugin library not found: {}", library_path.display());
    }

    let mut reg = load_registry().unwrap_or_default();
    reg.plugins.retain(|p| p.name != name);
    reg.plugins.push(InstalledPlugin {
        name: name.to_string(),
        path: library_path.display().to_string(),
    });
    reg.plugins.sort_by(|a, b| a.name.cmp(&b.name));
    save_registry(&reg)?;
    Ok(())
}

pub fn resolve_plugin_library_path(name: &str, explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(p) = explicit {
        return Ok(p);
    }

    // Heuristic locations:
    // - ./libstarforge_<name>.<ext>
    // - ~/.starforge/plugins/<name>/libstarforge_<name>.<ext>
    let cwd = std::env::current_dir().context("Failed to get current dir")?;
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    let plugin_dir = home.join(".starforge").join("plugins").join(name);

    let candidates = candidate_library_names(name)
        .into_iter()
        .flat_map(|fname| [cwd.join(&fname), plugin_dir.join(&fname)])
        .collect::<Vec<_>>();

    for c in candidates {
        if c.exists() {
            return Ok(c);
        }
    }

    anyhow::bail!(
        "No plugin library found for '{}'. Provide `--path` to the plugin shared library.",
        name
    );
}

fn candidate_library_names(name: &str) -> Vec<String> {
    let base = format!("libstarforge_{}", name);
    if cfg!(target_os = "windows") {
        vec![format!("{base}.dll")]
    } else if cfg!(target_os = "macos") {
        vec![format!("{base}.dylib"), format!("{base}.so")]
    } else {
        vec![format!("{base}.so")]
    }
}
