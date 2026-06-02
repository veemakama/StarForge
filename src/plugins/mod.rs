pub mod interface;
pub mod loader;
pub mod manifest;
pub mod registry;

pub use interface::{Plugin, PluginDeclaration};
pub use loader::{PluginLoadError, PluginManager};
