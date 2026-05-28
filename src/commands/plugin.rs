use crate::plugins::interface::CORE_VERSION;
use crate::plugins::{registry, PluginManager};
use crate::utils::print as p;
use anyhow::{Context, Result};
use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum PluginCommands {
    /// Register a plugin shared library for StarForge to load
    ///
    /// Example: starforge plugin install starforge-defi --path ./libstarforge_defi.so
    Install {
        /// Plugin name (used as the command name)
        name: String,
        /// Path to the plugin shared library (.so/.dylib/.dll)
        #[arg(long)]
        path: Option<PathBuf>,
    },
    /// List installed plugins from the local registry
    List,
    /// Load installed plugins and show those successfully loaded
    Load,
    /// Remove a plugin from the registry
    ///
    /// Example: starforge plugin uninstall starforge-defi
    Uninstall {
        /// Plugin name to remove
        name: String,
    },
}

pub fn handle(cmd: PluginCommands) -> Result<()> {
    match cmd {
        PluginCommands::Install { name, path } => install(name, path),
        PluginCommands::List => list(),
        PluginCommands::Load => load(),
        PluginCommands::Uninstall { name } => uninstall(name),
    }
}

fn install(name: String, path: Option<PathBuf>) -> Result<()> {
    let lib_path = registry::resolve_plugin_library_path(&name, path)?;
    registry::install_plugin(&name, &lib_path)?;

    p::header("Plugin Install");
    p::success("Plugin registered");
    p::kv_accent("Name", &name);
    p::kv("Library", &lib_path.display().to_string());
    p::info("Load plugins with: starforge plugin load");
    Ok(())
}

fn list() -> Result<()> {
    p::header("Installed Plugins");
    let reg = registry::load_registry().unwrap_or_default();
    if reg.plugins.is_empty() {
        p::info("No plugins installed. Use: starforge plugin install <name> --path <lib>");
        return Ok(());
    }

    p::kv("StarForge core version", CORE_VERSION);
    p::separator();
    for (i, pl) in reg.plugins.iter().enumerate() {
        println!("  {:>2}. {}", i + 1, pl.name);
        p::kv("Path", &pl.path);
        if i < reg.plugins.len() - 1 {
            println!();
        }
    }
    p::separator();
    Ok(())
}

fn load() -> Result<()> {
    p::header("Plugin Loader");

    let reg = registry::load_registry().unwrap_or_default();
    if reg.plugins.is_empty() {
        p::info("No plugins installed. Use: starforge plugin install <name> --path <lib>");
        return Ok(());
    }

    let mut pm = PluginManager::new();
    for pl in &reg.plugins {
        unsafe {
            pm.load_plugin(&pl.path)
                .with_context(|| format!("Failed to load plugin '{}' from {}", pl.name, pl.path))?;
        }
    }

    let loaded = pm.list_plugins();
    if loaded.is_empty() {
        p::warn("No plugins loaded.");
        return Ok(());
    }

    p::kv("StarForge core version", CORE_VERSION);
    p::separator();
    for (name, desc, built_for) in loaded {
        p::kv_accent(name, desc);
        p::kv("Built for StarForge", built_for);
    }
    p::separator();
    Ok(())
}

fn uninstall(name: String) -> Result<()> {
    let mut reg = registry::load_registry().unwrap_or_default();

    let before = reg.plugins.len();
    reg.plugins.retain(|p| p.name != name);

    if reg.plugins.len() == before {
        anyhow::bail!(
            "Plugin '{}' is not installed. Run `starforge plugin list` to see installed plugins.",
            name
        );
    }

    registry::save_registry(&reg)?;

    p::header("Plugin Uninstall");
    p::success(&format!("Plugin '{}' removed from registry", name));
    p::info("The plugin library file on disk was not deleted.");
    Ok(())
}
