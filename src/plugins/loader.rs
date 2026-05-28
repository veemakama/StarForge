use crate::plugins::interface::{
    is_core_version_compatible, Plugin, PluginDeclaration, PluginRegistrar, CORE_VERSION,
    RUSTC_VERSION,
};
use anyhow::{Context, Result};
use libloading::{Library, Symbol};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::rc::Rc;

pub struct PluginManager {
    /// Maps plugin name → (plugin, core_version it was built against).
    plugins: HashMap<String, (Box<dyn Plugin>, String)>,
    libraries: Vec<Rc<Library>>,
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            libraries: Vec::new(),
        }
    }

    /// # Safety
    /// The caller must ensure the plugin at `path` is a valid StarForge plugin
    /// compiled with a compatible Rust toolchain and ABI.
    pub unsafe fn load_plugin<P: AsRef<OsStr>>(&mut self, path: P) -> Result<()> {
        let path_display = path.as_ref().to_string_lossy().to_string();
        let library = Rc::new(Library::new(path).context("Failed to load library")?);

        let decl: Symbol<*mut PluginDeclaration> = library
            .get(b"PLUGIN_DECLARATION")
            .context("Failed to find PLUGIN_DECLARATION symbol — is this a StarForge plugin?")?;

        let decl = &**decl;

        // ── rustc ABI check ──────────────────────────────────────────────────
        if decl.rustc_version != RUSTC_VERSION {
            anyhow::bail!(
                "Plugin ABI mismatch in '{path_display}':\n  \
                 Plugin was compiled with rustc {plugin_rustc}\n  \
                 StarForge requires rustc {core_rustc}\n\n  \
                 Rebuild the plugin with the same Rust toolchain used to build StarForge.",
                path_display = path_display,
                plugin_rustc = decl.rustc_version,
                core_rustc = RUSTC_VERSION,
            );
        }

        // ── StarForge core version check ─────────────────────────────────────
        if !is_core_version_compatible(decl.core_version) {
            anyhow::bail!(
                "Plugin version incompatibility in '{path_display}':\n  \
                 Plugin was built for StarForge {plugin_core}\n  \
                 Running StarForge {core}\n\n  \
                 The major version must match. Rebuild the plugin against \
                 StarForge {core} or install a compatible StarForge version.\n  \
                 See DEVELOPER_GUIDE.md § \"Plugin Version Compatibility\" for details.",
                path_display = path_display,
                plugin_core = decl.core_version,
                core = CORE_VERSION,
            );
        }

        let mut registrar = ProxyRegistrar::new();
        (decl.register)(&mut registrar);

        let plugin_core_version = decl.core_version.to_string();
        for plugin in registrar.plugins {
            let name = plugin.name().to_string();
            plugin.on_load();
            self.plugins
                .insert(name, (plugin, plugin_core_version.clone()));
        }

        self.libraries.push(library);

        Ok(())
    }

    /// Returns `(name, description, built_for_core_version)` for every loaded plugin.
    pub fn list_plugins(&self) -> Vec<(&str, &str, &str)> {
        self.plugins
            .iter()
            .map(|(n, (p, cv))| (n.as_str(), p.description(), cv.as_str()))
            .collect()
    }

    pub fn execute(&self, name: &str, args: &[String]) -> Result<(), String> {
        if let Some((plugin, _)) = self.plugins.get(name) {
            plugin.execute(args)
        } else {
            Err(format!("Plugin '{}' not found", name))
        }
    }
}

struct ProxyRegistrar {
    plugins: Vec<Box<dyn Plugin>>,
}

impl ProxyRegistrar {
    fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }
}

impl PluginRegistrar for ProxyRegistrar {
    fn register_plugin(&mut self, plugin: Box<dyn Plugin>) {
        self.plugins.push(plugin);
    }
}
