use std::any::Any;

pub trait Plugin: Any + Send + Sync {
    fn name(&self) -> &'static str;
    fn version(&self) -> &'static str;
    fn description(&self) -> &'static str;

    fn on_load(&self) {}
    fn on_unload(&self) {}

    fn execute(&self, args: &[String]) -> Result<(), String>;
}

pub struct PluginDeclaration {
    pub rustc_version: &'static str,
    pub core_version: &'static str,
    pub register: unsafe fn(&mut dyn PluginRegistrar),
}

pub trait PluginRegistrar {
    fn register_plugin(&mut self, plugin: Box<dyn Plugin>);
}

#[macro_export]
macro_rules! export_plugin {
    ($register:expr) => {
        #[doc(hidden)]
        #[no_mangle]
        pub static PLUGIN_DECLARATION: $crate::plugins::PluginDeclaration =
            $crate::plugins::PluginDeclaration {
                rustc_version: $crate::plugins::interface::RUSTC_VERSION,
                core_version: $crate::plugins::interface::CORE_VERSION,
                register: $register,
            };
    };
}

pub const RUSTC_VERSION: &str = env!("RUSTC_VERSION");
pub const CORE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Extract the major version component from a semver string (e.g. "1.2.3" → "1").
/// Returns the full string unchanged if it cannot be parsed.
fn major(version: &str) -> &str {
    match version.find('.') {
        Some(pos) => &version[..pos],
        None => version,
    }
}

/// Returns `true` when `plugin_version` is compatible with the running StarForge core.
///
/// Compatibility rule: the **major** version must match exactly.  A plugin built
/// against `0.x.y` is incompatible with a core running `1.x.y`, and vice-versa.
/// Patch and minor bumps within the same major are considered backwards-compatible.
pub fn is_core_version_compatible(plugin_version: &str) -> bool {
    major(plugin_version) == major(CORE_VERSION)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_version_is_compatible() {
        assert!(is_core_version_compatible(CORE_VERSION));
    }

    #[test]
    fn different_major_is_incompatible() {
        // Construct a version with a different major than CORE_VERSION.
        let core_major: u64 = major(CORE_VERSION).parse().unwrap_or(0);
        let other = format!("{}.0.0", core_major + 1);
        assert!(!is_core_version_compatible(&other));
    }

    #[test]
    fn same_major_different_minor_is_compatible() {
        let core_major = major(CORE_VERSION);
        let other = format!("{}.99.0", core_major);
        assert!(is_core_version_compatible(&other));
    }
}
