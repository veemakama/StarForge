//! StarForge Plugin SDK
//!
//! Implement the [`StarForgePlugin`] trait and use [`export_plugin!`] to
//! expose your plugin to the StarForge CLI loader.

/// Metadata every plugin must provide.
pub struct PluginMeta {
    pub name: &'static str,
    pub version: &'static str,
    pub description: &'static str,
}

/// Core trait all StarForge plugins must implement.
pub trait StarForgePlugin {
    fn meta(&self) -> PluginMeta;
    fn run(&self, args: &[String]) -> Result<(), String>;
}

/// Exports a plugin so the StarForge CLI can load it via `libloading`.
///
/// # Example
/// ```rust,ignore
/// use starforge_plugin_sdk::{export_plugin, PluginMeta, StarForgePlugin};
///
/// struct MyPlugin;
///
/// impl StarForgePlugin for MyPlugin {
///     fn meta(&self) -> PluginMeta {
///         PluginMeta { name: "my-plugin", version: "0.1.0", description: "Does something cool" }
///     }
///     fn run(&self, args: &[String]) -> Result<(), String> {
///         println!("my-plugin args: {:?}", args);
///         Ok(())
///     }
/// }
///
/// export_plugin!(MyPlugin);
/// ```
#[macro_export]
macro_rules! export_plugin {
    ($plugin_type:ty) => {
        #[no_mangle]
        pub extern "C" fn _starforge_plugin_create(
        ) -> *mut dyn $crate::StarForgePlugin {
            let plugin = <$plugin_type>::default();
            Box::into_raw(Box::new(plugin))
        }
    };
}
