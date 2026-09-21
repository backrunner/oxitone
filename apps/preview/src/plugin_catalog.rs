//! Owned descriptor copies: detail windows never keep factories or library handles alive.
use oxitone_core::wire::ProjectSnapshot;
use oxitone_graph::{PluginDescriptor, PluginRegistry};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct LibraryInfo {
    pub path: String,
    pub sha256: String,
    pub display_name: Option<String>,
}
#[derive(Clone, Debug)]
pub struct PluginInfo {
    pub descriptor: PluginDescriptor,
    pub library: Option<LibraryInfo>,
    pub instances: BTreeMap<String, std::sync::Arc<PluginDescriptor>>,
}
impl PluginInfo {
    pub fn descriptor_for(&self, instance: Option<&str>) -> &PluginDescriptor {
        instance
            .and_then(|id| self.instances.get(id))
            .map_or(&self.descriptor, |value| value.as_ref())
    }
}
pub type Catalog = BTreeMap<(String, String), PluginInfo>;
pub type Libraries = BTreeMap<(String, String), LibraryInfo>;

pub fn display_name(catalog: &Catalog, id: &str, version: &str) -> String {
    catalog
        .get(&(id.into(), version.into()))
        .and_then(|info| info.library.as_ref())
        .and_then(|library| library.display_name.clone())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| crate::mixer_model::plugin_name(id))
}

pub fn collect(
    snapshot: &ProjectSnapshot,
    registry: &PluginRegistry,
    libraries: Libraries,
) -> Catalog {
    let instruments = snapshot
        .channels
        .iter()
        .map(|c| (&c.instrument.plugin_id, &c.instrument.plugin_version));
    let effects = snapshot
        .channels
        .iter()
        .flat_map(|c| &c.effect_chain)
        .chain(snapshot.mixer_channels.iter().flat_map(|b| &b.inserts))
        .map(|e| (&e.plugin_id, &e.plugin_version));
    instruments
        .chain(effects)
        .filter_map(|(id, version)| {
            let key = (id.clone(), version.clone());
            registry.lookup_descriptor(id, version).map(|descriptor| {
                let info = PluginInfo {
                    descriptor: descriptor.clone(),
                    library: libraries.get(&key).cloned(),
                    instances: BTreeMap::new(),
                };
                (key, info)
            })
        })
        .collect()
}

pub fn collect_instances(
    catalog: &mut Catalog,
    controls: &oxitone_render::plugin_controls::ControlRegistry,
) {
    for instance in controls.inventory("vst3").instances {
        if let (Some(info), Some(descriptor)) = (
            catalog.get_mut(&(instance.plugin_id, instance.plugin_version)),
            controls.descriptor(&instance.instance_id),
        ) {
            info.instances.insert(
                instance.instance_id,
                std::sync::Arc::new(descriptor.clone()),
            );
        }
    }
}
