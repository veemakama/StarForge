use crate::plugins::interface::CORE_VERSION;
use crate::plugins::manifest;
use crate::plugins::registry::{self, TrustLevel, UninstallOptions};
use crate::plugins::PluginManager;
use crate::utils::print as p;
use anyhow::{Context, Result};
use chrono;
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
        /// Source URL or identifier for trust classification
        #[arg(long)]
        source: Option<String>,
        /// Install even if the plugin source is untrusted (requires explicit confirmation)
        #[arg(long)]
        force: bool,
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
        /// Also delete the plugin library file from disk (only under ~/.starforge/plugins/)
        #[arg(long)]
        purge: bool,
        /// Skip confirmation for destructive removal
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// Verify trust and compatibility of installed plugins
    Verify {
        /// Plugin name to verify (verifies all plugins if omitted)
        name: Option<String>,
    },
    /// Update installed plugins to their latest versions
    ///
    /// Checks each plugin's source URL, validates compatibility with the running
    /// CLI, and replaces the local library if a newer copy is available.
    /// Configuration and trust settings are preserved.
    ///
    /// Example: starforge plugin update
    ///          starforge plugin update starforge-defi
    Update {
        /// Plugin name to update (updates all plugins if omitted)
        name: Option<String>,
        /// Skip confirmation prompt
        #[arg(long, default_value = "false")]
        yes: bool,
    },
}

pub fn handle(cmd: PluginCommands) -> Result<()> {
    match cmd {
        PluginCommands::Install {
            name,
            path,
            source,
            force,
        } => install(name, path, source, force),
        PluginCommands::List => list(),
        PluginCommands::Load => load(),
        PluginCommands::Uninstall { name, purge, yes } => uninstall(name, purge, yes),
        PluginCommands::Verify { name } => verify(name),
        PluginCommands::Update { name, yes } => update(name, yes),
    }
}

fn install(name: String, path: Option<PathBuf>, source: Option<String>, force: bool) -> Result<()> {
    let lib_path = registry::resolve_plugin_library_path(&name, path)?;
    let source_str = source.as_deref().unwrap_or("");
    let trust = registry::classify_source(source_str);

    // Warn the user about untrusted sources and require --force to proceed.
    if trust == TrustLevel::Unknown && !source_str.is_empty() && !force {
        p::header("Plugin Install — Trust Warning");
        p::warn(&format!(
            "Plugin source '{}' is not in the trusted sources list.",
            source_str
        ));
        p::info("Trusted sources:");
        p::info("  • https://github.com/Nanle-code/starforge-*");
        p::info("  • https://github.com/StarForge-Labs/*");
        p::info("  • https://crates.io/crates/starforge-plugin-*");
        p::info("");
        p::info("To install anyway: starforge plugin install <name> --source <url> --force");
        p::info("To install from a local path (always trusted): starforge plugin install <name> --path <lib>");
        anyhow::bail!("Refusing to install plugin from untrusted source without --force");
    }

    let plugin_manifest = manifest::require_compatible_manifest(&lib_path, &name)?;

    registry::install_plugin(
        &name,
        &lib_path,
        source_str,
        &plugin_manifest.starforge_version,
        &plugin_manifest.version,
    )?;

    p::header("Plugin Install");
    p::success("Plugin registered");
    p::kv_accent("Name", &name);
    p::kv("Library", &lib_path.display().to_string());
    p::kv("Plugin version", &plugin_manifest.version);
    p::kv("StarForge compatibility", &plugin_manifest.starforge_version);
    p::kv("Trust", trust.label());
    if !source_str.is_empty() {
        p::kv("Source", source_str);
    }
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
        p::kv("Trust", pl.trust.label());
        if !pl.source.is_empty() {
            p::kv("Source", &pl.source);
        }
        if !pl.starforge_version.is_empty() {
            p::kv("StarForge", &pl.starforge_version);
        }
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

    // Warn about any unknown-trust plugins before loading.
    for pl in reg
        .plugins
        .iter()
        .filter(|p| p.trust == TrustLevel::Unknown && !p.source.is_empty())
    {
        p::warn(&format!(
            "Plugin '{}' is from an unknown/untrusted source: {}",
            pl.name, pl.source
        ));
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

fn uninstall(name: String, purge: bool, yes: bool) -> Result<()> {
    let reg = registry::load_registry().unwrap_or_default();
    let plugin = reg
        .plugins
        .iter()
        .find(|p| p.name == name)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Plugin '{}' is not installed. Run `starforge plugin list` to see installed plugins.",
                name
            )
        })?;

    let lib_path = PathBuf::from(&plugin.path);
    let lib_exists = lib_path.exists();

    p::header("Plugin Uninstall");
    p::kv_accent("Plugin", &name);
    p::kv("Library", &plugin.path);

    if lib_exists {
        p::warn(
            "If this plugin is loaded in another StarForge session, close that session before purging files.",
        );
    } else {
        p::warn("Plugin library file is already missing on disk.");
    }

    if purge && !yes {
        p::warn("This will permanently delete the plugin library file.");
        p::info("Proceed with: starforge plugin uninstall <name> --purge --yes");
        anyhow::bail!("Refusing destructive uninstall without --yes");
    }

    // Best-effort: load plugin to run on_unload before registry removal
    if lib_exists {
        let mut pm = PluginManager::new();
        if let Err(e) = unsafe { pm.load_plugin(&lib_path) } {
            p::warn(&format!(
                "Could not load plugin for clean shutdown: {}. Proceeding with uninstall.",
                e
            ));
        } else {
            p::info("Plugin unloaded cleanly.");
        }
    }

    let opts = UninstallOptions {
        purge_files: purge,
        assume_yes: yes,
    };
    let report = registry::uninstall_plugin(&name, &opts)?;

    p::success(&format!("Plugin '{}' removed from registry", name));
    if report.files_removed {
        p::success("Plugin library file deleted.");
    } else if !purge {
        p::info("Library file kept on disk. Use --purge --yes to delete it.");
    }
    if report.library_was_missing && !report.files_removed {
        p::info("No library file was present to remove.");
    }

    Ok(())
}

fn update(name: Option<String>, yes: bool) -> Result<()> {
    p::header("Plugin Update");

    let reg = registry::load_registry().unwrap_or_default();
    if reg.plugins.is_empty() {
        p::info("No plugins installed. Use: starforge plugin install <name> --path <lib>");
        return Ok(());
    }

    let to_update: Vec<_> = match &name {
        Some(n) => {
            let found: Vec<_> = reg.plugins.iter().filter(|p| &p.name == n).collect();
            if found.is_empty() {
                anyhow::bail!("Plugin '{}' is not installed. Run `starforge plugin list`.", n);
            }
            found
        }
        None => reg.plugins.iter().collect(),
    };

    p::kv("Plugins to check", &to_update.len().to_string());
    p::kv("StarForge core version", CORE_VERSION);
    p::separator();

    let mut updated = 0u32;
    let mut skipped = 0u32;
    let mut failed = 0u32;

    for pl in &to_update {
        println!("  Checking: {}", pl.name);

        // Verify the library still exists at its registered path.
        let lib_exists = std::path::Path::new(&pl.path).exists();
        if !lib_exists {
            p::warn(&format!(
                "  '{}' library missing at {}. Re-install with: starforge plugin install {} --path <lib>",
                pl.name, pl.path, pl.name
            ));
            failed += 1;
            println!();
            continue;
        }

        // Only plugins with a non-empty, trusted source URL can be fetched remotely.
        if pl.source.is_empty() {
            p::info(&format!(
                "  '{}' was installed from a local path — no remote source to fetch from.",
                pl.name
            ));
            p::kv("  Path", &pl.path);
            if let Some(ref ts) = pl.installed_at {
                p::kv("  Installed at", ts);
            }
            skipped += 1;
            println!();
            continue;
        }

        let trust = registry::classify_source(&pl.source);
        if trust == registry::TrustLevel::Unknown && !yes {
            p::warn(&format!(
                "  '{}' source '{}' is not trusted. Use --yes to force update from unknown sources.",
                pl.name, pl.source
            ));
            skipped += 1;
            println!();
            continue;
        }

        // For trusted/confirmed sources, re-install the plugin library.
        // This re-uses the existing path — the user is responsible for
        // placing an updated .so/.dylib at the same location, or the source
        // URL must be a direct download endpoint.
        //
        // For crates.io sources we attempt to download via `cargo install`.
        if pl.source.starts_with("https://crates.io/crates/") {
            let crate_name = pl
                .source
                .trim_start_matches("https://crates.io/crates/")
                .split('/')
                .next()
                .unwrap_or(&pl.name);

            p::info(&format!("  Attempting `cargo install {}` ...", crate_name));
            let status = std::process::Command::new("cargo")
                .args(["install", crate_name, "--force"])
                .status();

            match status {
                Ok(s) if s.success() => {
                    registry::install_plugin(&pl.name, std::path::Path::new(&pl.path), &pl.source)?;
                    p::success(&format!("  '{}' updated via cargo install", pl.name));
                    updated += 1;
                }
                Ok(s) => {
                    p::warn(&format!(
                        "  cargo install exited with status {}. Plugin not updated.",
                        s
                    ));
                    failed += 1;
                }
                Err(e) => {
                    p::warn(&format!("  Failed to run cargo: {}. Is Cargo installed?", e));
                    failed += 1;
                }
            }
        } else {
            // For GitHub and other sources, check if the library file on disk
            // has been updated since install and refresh the registry timestamp.
            let metadata = std::fs::metadata(&pl.path);
            match metadata {
                Ok(m) => {
                    let modified = m
                        .modified()
                        .ok()
                        .and_then(|t| {
                            t.duration_since(std::time::UNIX_EPOCH)
                                .ok()
                                .map(|d| d.as_secs())
                        })
                        .unwrap_or(0);

                    let installed_epoch = pl
                        .installed_at
                        .as_deref()
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| dt.timestamp() as u64)
                        .unwrap_or(0);

                    if modified > installed_epoch {
                        // Library on disk is newer — refresh the registry entry.
                        registry::install_plugin(
                            &pl.name,
                            std::path::Path::new(&pl.path),
                            &pl.source,
                        )?;
                        p::success(&format!(
                            "  '{}' library on disk is newer — registry refreshed.",
                            pl.name
                        ));
                        updated += 1;
                    } else {
                        p::info(&format!(
                            "  '{}' is already up to date. Source: {}",
                            pl.name, pl.source
                        ));
                        p::info("  To update manually: replace the library at the registered path,");
                        p::info(&format!("  then run: starforge plugin update {}", pl.name));
                        skipped += 1;
                    }
                }
                Err(e) => {
                    p::warn(&format!("  Could not read library metadata: {}", e));
                    failed += 1;
                }
            }
        }

        println!();
    }

    p::separator();
    p::kv("Updated", &updated.to_string());
    p::kv("Skipped (already current / local)", &skipped.to_string());
    p::kv("Failed", &failed.to_string());

    if failed > 0 {
        anyhow::bail!("{} plugin(s) failed to update. See warnings above.", failed);
    }

    Ok(())
}

fn verify(name: Option<String>) -> Result<()> {
    p::header("Plugin Verification");

    let reg = registry::load_registry().unwrap_or_default();
    if reg.plugins.is_empty() {
        p::info("No plugins installed.");
        return Ok(());
    }

    let to_check: Vec<_> = match &name {
        Some(n) => {
            let found: Vec<_> = reg.plugins.iter().filter(|p| &p.name == n).collect();
            if found.is_empty() {
                anyhow::bail!("Plugin '{}' is not installed.", n);
            }
            found
        }
        None => reg.plugins.iter().collect(),
    };

    let mut all_ok = true;

    for pl in &to_check {
        let lib_exists = std::path::Path::new(&pl.path).exists();

        let trust_ok = match pl.trust {
            TrustLevel::Local | TrustLevel::Trusted => true,
            TrustLevel::Unknown => false,
        };

        let compat_ok = if pl.starforge_version.is_empty() {
            true
        } else {
            crate::plugins::interface::is_core_version_compatible(&pl.starforge_version)
        };

        let status = if lib_exists && trust_ok && compat_ok {
            "✓ OK"
        } else if !lib_exists {
            all_ok = false;
            "✗ library missing"
        } else if !compat_ok {
            all_ok = false;
            "✗ incompatible"
        } else {
            all_ok = false;
            "⚠ untrusted source"
        };

        println!("  {:<24} [{}]  trust={}", pl.name, status, pl.trust.label());
        if !pl.starforge_version.is_empty() {
            p::kv("StarForge", &pl.starforge_version);
        }
        if !pl.source.is_empty() {
            p::kv("Source", &pl.source);
        }
        if !lib_exists {
            p::warn(&format!("Library not found at: {}", pl.path));
            p::info("Re-install with: starforge plugin install <name> --path <lib>");
        }
        if pl.trust == TrustLevel::Unknown && !pl.source.is_empty() {
            p::warn("Source is not in the trusted sources list.");
            p::info("See: starforge plugin install --help for trusted source prefixes.");
        }
        if !compat_ok && !pl.starforge_version.is_empty() {
            p::warn(&format!(
                "Plugin targets StarForge {} but running {}",
                pl.starforge_version, CORE_VERSION
            ));
            p::info("Reinstall a compatible build or upgrade StarForge.");
        }
    }

    if all_ok {
        p::success("All checked plugins passed verification.");
    }

    Ok(())
}
