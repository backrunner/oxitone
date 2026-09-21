//! A graph-scoped native control directory, collected once during compilation.
pub mod request;
use oxitone_core::{
    wire::{InstrumentRef, ProjectSnapshot},
    OxitoneError,
};
use oxitone_graph::control::NativeControl;
use serde::Serialize;
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU64, AtomicU8, Ordering},
        Arc,
    },
    time::Duration,
};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceInfo {
    pub instance_id: String,
    pub plugin_id: String,
    pub plugin_version: String,
}
struct Entry {
    info: InstanceInfo,
    descriptor: oxitone_graph::PluginDescriptor,
    control: Arc<dyn NativeControl>,
}
struct Inner {
    generation: u64,
    state: AtomicU8,
    entries: BTreeMap<String, Entry>,
}
/// The graph is the sole owner of editability; cloned registries do not keep it active.
pub(crate) struct ControlGraph(Arc<Inner>);
#[derive(Clone)]
pub struct ControlRegistry(Arc<Inner>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Inventory {
    instance_control_version: u32,
    pub graph_generation: String,
    pub state: &'static str,
    pub instances: Vec<InstanceInfo>,
}
fn conflict() -> OxitoneError {
    OxitoneError::new(
        "PluginTaskConflict",
        "native control target is not the active graph generation",
    )
}

impl ControlGraph {
    pub fn collect(
        snapshot: &ProjectSnapshot,
        channels: &[crate::channel::ChannelNode],
        mixer: &oxitone_mixer::MixerEngine,
        registry: &oxitone_graph::PluginRegistry,
    ) -> Result<Self, OxitoneError> {
        let mut entries = BTreeMap::new();
        let mut add = |id: &Option<String>,
                       plugin_id: &str,
                       plugin_version: &str,
                       control: Option<Arc<dyn NativeControl>>|
         -> Result<(), OxitoneError> {
            if let (Some(id), Some(control)) = (id, control) {
                let info = InstanceInfo {
                    instance_id: id.clone(),
                    plugin_id: plugin_id.into(),
                    plugin_version: plugin_version.into(),
                };
                let descriptor = registry
                    .instance_descriptor(plugin_id, plugin_version, Some(id))
                    .ok_or_else(|| {
                        OxitoneError::new(
                            "PluginConfigInvalid",
                            "native instance descriptor is missing",
                        )
                    })?
                    .clone();
                if entries
                    .insert(
                        id.clone(),
                        Entry {
                            info,
                            control,
                            descriptor,
                        },
                    )
                    .is_some()
                {
                    return Err(OxitoneError::new(
                        "PluginConfigInvalid",
                        "duplicate native control instance",
                    ));
                }
            }
            Ok(())
        };
        for channel in channels {
            let spec = snapshot
                .channels
                .iter()
                .find(|c| c.id == channel.id)
                .expect("compiled channel exists");
            let reference: &InstrumentRef = &spec.instrument;
            add(
                &reference.instance_id,
                &reference.plugin_id,
                &reference.plugin_version,
                channel.instrument.native_control(),
            )?;
            for (reference, insert) in spec.effect_chain.iter().zip(&channel.inserts) {
                add(
                    &reference.instance_id,
                    &reference.plugin_id,
                    &reference.plugin_version,
                    insert.instance.native_control(),
                )?;
            }
        }
        for bus in &snapshot.mixer_channels {
            for (index, reference) in bus.inserts.iter().enumerate() {
                add(
                    &reference.instance_id,
                    &reference.plugin_id,
                    &reference.plugin_version,
                    mixer.native_insert_control(&bus.id, index),
                )?;
            }
        }
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let generation = NEXT
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| {
                OxitoneError::new("BudgetExceeded", "native graph generation exhausted")
            })?;
        Ok(Self(Arc::new(Inner {
            generation,
            state: AtomicU8::new(0),
            entries,
        })))
    }
    pub fn registry(&self) -> ControlRegistry {
        ControlRegistry(self.0.clone())
    }
    /// RT-safe publication marker only. No plugin or OS calls.
    pub fn activate(&self) {
        let _ = self
            .0
            .state
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire);
    }
    pub fn retire(&self) {
        self.0.state.store(2, Ordering::Release);
    }
}
impl Drop for ControlGraph {
    fn drop(&mut self) {
        self.retire();
    }
}
impl ControlRegistry {
    /// Prepared immutable metadata for UI projection; never invokes a plugin.
    pub fn descriptor(&self, instance_id: &str) -> Option<&oxitone_graph::PluginDescriptor> {
        self.0
            .entries
            .get(instance_id)
            .map(|entry| &entry.descriptor)
    }
    pub fn generation(&self) -> String {
        self.0.generation.to_string()
    }
    pub fn inventory(&self, protocol: &str) -> Inventory {
        Inventory {
            instance_control_version: 1,
            graph_generation: self.generation(),
            state: match self.0.state.load(Ordering::Acquire) {
                0 => "prepared",
                1 => "active",
                _ => "retired",
            },
            instances: self
                .0
                .entries
                .values()
                .filter(|e| e.control.protocol() == protocol)
                .map(|e| e.info.clone())
                .collect(),
        }
    }
    pub fn check(&self, generation: &str) -> Result<(), OxitoneError> {
        if generation != self.generation() || self.0.state.load(Ordering::Acquire) != 1 {
            return Err(conflict());
        }
        Ok(())
    }
    pub fn request(
        &self,
        generation: &str,
        instance_id: &str,
        protocol: &str,
        json: &str,
        timeout: Duration,
    ) -> Result<String, OxitoneError> {
        self.check(generation)?;
        let entry = self.0.entries.get(instance_id).ok_or_else(|| {
            OxitoneError::new("PluginConfigInvalid", "unknown native plugin instance")
        })?;
        if entry.control.protocol() != protocol {
            return Err(OxitoneError::new(
                "PluginCapabilityUnsupported",
                "instance does not support the requested native control protocol",
            ));
        }
        let result = entry.control.request(json, timeout);
        self.check(generation)?;
        result
    }
}

impl crate::RenderGraph {
    /// Clone metadata/control handles before moving the graph to its render worker.
    pub fn plugin_controls(&self) -> ControlRegistry {
        self.controls.registry()
    }
    /// Mark a successfully accepted, stopped graph available to control callers.
    pub fn activate_plugin_controls(&self) {
        self.controls.activate();
    }
}
