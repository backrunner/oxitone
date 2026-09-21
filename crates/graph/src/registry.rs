//! Static plugin registry (08-plugin-abi.md §加载模型). Built-ins register
//! from `crates/instruments` / `crates/mixer`; third-party dylib plugins will
//! register through the same API after dlopen validation. Registering
//! the same `plugin_id + plugin_version` twice is idempotent and keeps the
//! first registration.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::abi::Plugin;
use crate::descriptor::PluginDescriptor;

/// Process-wide catalog of available plugins, keyed by (id, version).
#[derive(Default, Clone)]
pub struct PluginRegistry {
    plugins: BTreeMap<(String, String), Arc<dyn Plugin>>,
    instances: BTreeMap<String, Arc<dyn Plugin>>,
    configuration_dependent: bool,
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
        self.configuration_dependent |= plugin.configuration_dependent();
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
    ) -> Option<&PluginDescriptor> {
        self.plugins
            .get(&(plugin_id.to_string(), plugin_version.to_string()))
            .map(|plugin| plugin.descriptor())
    }

    /// Graph-local capability override. Never modifies the engine's shared registration.
    pub fn register_instance(
        &mut self,
        instance_id: &str,
        plugin: Arc<dyn Plugin>,
    ) -> Result<(), oxitone_core::OxitoneError> {
        plugin.descriptor().validate()?;
        if self.instances.contains_key(instance_id) {
            return Err(oxitone_core::OxitoneError::new(
                "InvalidProject",
                "duplicate configured instance",
            ));
        }
        self.instances.insert(instance_id.to_owned(), plugin);
        Ok(())
    }

    fn instance(
        &self,
        id: &str,
        version: &str,
        instance: Option<&str>,
    ) -> Option<&Arc<dyn Plugin>> {
        let plugin = self.instances.get(instance?)?;
        let descriptor = plugin.descriptor();
        (descriptor.plugin_id == id && descriptor.plugin_version == version).then_some(plugin)
    }

    pub fn lookup_instance(
        &self,
        id: &str,
        version: &str,
        instance: Option<&str>,
    ) -> Option<Arc<dyn Plugin>> {
        self.instance(id, version, instance)
            .cloned()
            .or_else(|| self.lookup(id, version))
    }

    pub fn instance_descriptor(
        &self,
        id: &str,
        version: &str,
        instance: Option<&str>,
    ) -> Option<&PluginDescriptor> {
        self.instance(id, version, instance)
            .map(|plugin| plugin.descriptor())
            .or_else(|| self.lookup_descriptor(id, version))
    }

    /// Whether any version of `plugin_id` is registered.
    pub fn contains_id(&self, plugin_id: &str) -> bool {
        self.plugins.keys().any(|(id, _)| id == plugin_id)
    }

    pub fn has_configured_factories(&self) -> bool {
        self.configuration_dependent
    }

    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }
}
