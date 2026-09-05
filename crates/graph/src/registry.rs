//! Static plugin registry (08-plugin-abi.md §加载模型). Built-ins register
//! from `crates/instruments` / `crates/mixer`; third-party dylib plugins will
//! register through the same API after dlopen validation (M5). Registering
//! the same `plugin_id + plugin_version` twice is idempotent and keeps the
//! first registration.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::abi::Plugin;
use crate::descriptor::PluginDescriptor;

/// Process-wide catalog of available plugins, keyed by (id, version).
#[derive(Default)]
pub struct PluginRegistry {
    plugins: BTreeMap<(String, String), Arc<dyn Plugin>>,
}

impl PluginRegistry {
    /// Empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registry pre-populated with the given built-in plugins. This crate
    /// cannot depend on `oxitone-instruments`/`oxitone-mixer` (they depend on
    /// this crate for the ABI traits), so the caller assembles the list —
    /// see `oxitone_render::builtin_registry` for the standard assembly.
    /// Registering the same id+version twice keeps the first registration.
    pub fn with_builtins(
        plugins: impl IntoIterator<Item = Arc<dyn Plugin>>,
    ) -> Result<Self, oxitone_core::OxitoneError> {
        let mut registry = Self::new();
        for plugin in plugins {
            registry.register(plugin)?;
        }
        Ok(registry)
    }

    /// Validate the descriptor and insert the plugin. Re-registering an
    /// existing id+version is a no-op returning the original registration.
    pub fn register(&mut self, plugin: Arc<dyn Plugin>) -> Result<(), oxitone_core::OxitoneError> {
        let descriptor = plugin.descriptor();
        descriptor.validate()?;
        self.plugins
            .entry((
                descriptor.plugin_id.to_string(),
                descriptor.plugin_version.to_string(),
            ))
            .or_insert(plugin);
        Ok(())
    }

    /// Exact id+version lookup.
    pub fn lookup(&self, plugin_id: &str, plugin_version: &str) -> Option<Arc<dyn Plugin>> {
        self.plugins
            .get(&(plugin_id.to_string(), plugin_version.to_string()))
            .cloned()
    }

    /// Descriptor for an exact id+version pair.
    pub fn lookup_descriptor(
        &self,
        plugin_id: &str,
        plugin_version: &str,
    ) -> Option<&'static PluginDescriptor> {
        self.plugins
            .get(&(plugin_id.to_string(), plugin_version.to_string()))
            .map(|plugin| plugin.descriptor())
    }

    /// Whether any version of `plugin_id` is registered.
    pub fn contains_id(&self, plugin_id: &str) -> bool {
        self.plugins.keys().any(|(id, _)| id == plugin_id)
    }

    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }
}
