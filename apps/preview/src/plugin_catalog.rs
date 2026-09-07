//! Owned descriptor copies: detail windows never keep factories or library handles alive.
use oxitone_core::wire::ProjectSnapshot;
use oxitone_graph::{PluginDescriptor, PluginRegistry};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct LibraryInfo {
    pub path: String,
    pub sha256: String,
}
#[derive(Clone, Debug)]
pub struct PluginInfo {
    pub descriptor: PluginDescriptor,
    pub library: Option<LibraryInfo>,
}
pub type Catalog = BTreeMap<(String, String), PluginInfo>;
pub type Libraries = BTreeMap<(String, String), LibraryInfo>;

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
                };
                (key, info)
            })
        })
        .collect()
}
