//! Control-thread registration for both in-process C plugins and isolated VST3 plugins.
use crate::{
    plugin_catalog::{Libraries, LibraryInfo},
    wire,
};
use oxitone_core::{
    wire::{AllowPlugins, RegisterPluginOptions},
    OxitoneError,
};
use oxitone_graph::PluginRegistry;
use oxitone_render::{
    builtin_registry,
    plugins::{load_plugin, CPlugin},
};
use std::sync::Arc;

#[derive(Default)]
pub(super) struct LoadedPlugins {
    native: Vec<Arc<CPlugin>>,
    #[cfg(target_os = "macos")]
    vst3: Vec<Arc<oxitone_render::vst3::Vst3Plugin>>,
}
impl LoadedPlugins {
    pub fn load(
        native: Vec<RegisterPluginOptions>,
        vst3: Vec<serde_json::Value>,
        policy: AllowPlugins,
    ) -> Result<(PluginRegistry, Self, Libraries), OxitoneError> {
        let mut registry = builtin_registry()?;
        let mut loaded = Self::default();
        let mut libraries = Libraries::new();
        for options in native {
            if options.manifest.plugin_id.starts_with("oxitone.")
                || registry
                    .lookup_descriptor(
                        &options.manifest.plugin_id,
                        &options.manifest.plugin_version,
                    )
                    .is_some()
            {
                return Err(wire::invalid("duplicate or reserved plugin ID/version"));
            }
            let plugin = unsafe { load_plugin(&options, policy)? };
            registry.register(plugin.clone())?;
            libraries.insert(
                (
                    options.manifest.plugin_id.clone(),
                    options.manifest.plugin_version.clone(),
                ),
                LibraryInfo {
                    path: options.library_path.clone(),
                    sha256: plugin.registration.sha256.clone(),
                    display_name: None,
                },
            );
            loaded.native.push(plugin);
        }
        for value in vst3 {
            #[cfg(target_os = "macos")]
            {
                let options: oxitone_render::vst3::RegistrationOptions =
                    serde_json::from_value(value).map_err(|e| wire::invalid(&e.to_string()))?;
                let path = options.source.bundle_path.clone();
                let plugin = oxitone_render::vst3::Vst3Plugin::load(options, policy)?;
                let registration = &plugin.registration;
                if registry
                    .lookup_descriptor(&registration.plugin_id, &registration.plugin_version)
                    .is_some()
                {
                    return Err(wire::invalid("duplicate or reserved VST3 ID/version"));
                }
                registry.register(plugin.clone())?;
                libraries.insert(
                    (
                        registration.plugin_id.clone(),
                        registration.plugin_version.clone(),
                    ),
                    LibraryInfo {
                        path,
                        sha256: registration.sha256.clone(),
                        display_name: Some(plugin.display_name().into()),
                    },
                );
                loaded.vst3.push(plugin);
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = value;
                return Err(OxitoneError::new(
                    "PluginCapabilityUnsupported",
                    "VST3 requires macOS",
                ));
            }
        }
        Ok((registry, loaded, libraries))
    }
    pub fn faults(&self) -> (u64, String) {
        let mut counts: Vec<_> = self
            .native
            .iter()
            .map(|p| (&p.registration.plugin_id, p.fault_count()))
            .collect();
        #[cfg(target_os = "macos")]
        counts.extend(
            self.vst3
                .iter()
                .map(|p| (&p.registration.plugin_id, p.fault_count())),
        );
        (
            counts.iter().map(|(_, count)| *count).sum(),
            counts
                .into_iter()
                .filter(|(_, count)| *count > 0)
                .map(|(id, count)| format!("{id}: {count}"))
                .collect::<Vec<_>>()
                .join(", "),
        )
    }
}
