//! Configuration-dependent parameter tables must remain local to each compiled instance.
mod common;
#[path = "plugin_controls/probe.rs"]
mod probe;
use oxitone_core::{wire::ProjectSnapshot, OxitoneError};
use oxitone_graph::{HostContext, Plugin, PluginDescriptor, PluginInstance, StateSchemaId};
use oxitone_render::{RenderGraph, RenderGraphOptions, SampleStore};
use std::{collections::BTreeMap, sync::Arc};

struct Adaptive {
    inner: probe::Probe,
    descriptor: PluginDescriptor,
}
impl Adaptive {
    fn new(effect: bool) -> Self {
        let inner = probe::Probe::new(effect, false, None);
        let mut descriptor = inner.descriptor().clone();
        descriptor.state_schema = Some(StateSchemaId("fixture.dynamic@1"));
        Self { inner, descriptor }
    }
}
impl Plugin for Adaptive {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }
    fn configuration_dependent(&self) -> bool {
        true
    }
    fn create(&self, host: &HostContext) -> Box<dyn PluginInstance> {
        self.inner.create(host)
    }
    fn try_create_configured(
        &self,
        host: &HostContext,
        _: &BTreeMap<String, f64>,
        _: Option<&serde_json::Value>,
    ) -> Result<Box<dyn PluginInstance>, OxitoneError> {
        Ok(self.create(host))
    }
    fn configured_factory(
        &self,
        _: &HostContext,
        _: &BTreeMap<String, f64>,
        state: Option<&serde_json::Value>,
    ) -> Result<Option<Arc<dyn Plugin>>, OxitoneError> {
        let Some(state) = state else {
            return Ok(None);
        };
        let mut next = Self::new(self.descriptor.kind == oxitone_graph::PluginKind::Effect);
        let spec = &mut next.descriptor.parameters[0];
        spec.id = "expanded".into();
        spec.automation = Some(true);
        if state.as_str() == Some("wrongIdentity") {
            next.descriptor.plugin_version = "2.0.0".into();
        }
        Ok(Some(Arc::new(next)))
    }
}
fn fixture() -> (ProjectSnapshot, oxitone_graph::PluginRegistry) {
    let mut registry = oxitone_render::builtin_registry().unwrap();
    for effect in [false, true] {
        registry.register(Arc::new(Adaptive::new(effect))).unwrap();
    }
    let mut snapshot = common::base_snapshot();
    let mut instrument = common::wavetable_ref(&[("expanded", 0.25)]);
    instrument.plugin_id = "fixture.instrument".into();
    instrument.instance_id = Some("ins_instrument".into());
    instrument.state = Some("expanded".into());
    let expanded = |id: &str| {
        let mut effect = common::effect_ref("fixture.effect", &[("expanded", 0.5)]);
        effect.instance_id = Some(id.into());
        effect.state = Some("expanded".into());
        effect
    };
    let mut unchanged = common::effect_ref("fixture.effect", &[("gain", 0.75)]);
    unchanged.instance_id = Some("ins_unchanged".into());
    snapshot.channels.push(common::channel(
        "chn_test",
        "mix_master",
        instrument,
        vec![expanded("ins_channel"), unchanged],
    ));
    snapshot.mixer_channels.push(common::mixer_channel(
        "mix_master",
        vec![expanded("ins_master")],
        vec![],
    ));
    (snapshot, registry)
}
fn compile(
    snapshot: &ProjectSnapshot,
    registry: &oxitone_graph::PluginRegistry,
) -> Result<RenderGraph, OxitoneError> {
    RenderGraph::compile(
        snapshot,
        registry,
        &SampleStore::new(None),
        &RenderGraphOptions::default(),
    )
}

#[test]
fn every_owner_resolves_its_own_table_and_the_catalog_is_unchanged() {
    let (mut snapshot, registry) = fixture();
    for (index, instance) in ["ins_instrument", "ins_channel", "ins_master"]
        .into_iter()
        .enumerate()
    {
        snapshot.automation.push(serde_json::from_value(serde_json::json!({
            "id": format!("auto_{index}"), "target": {"entityId": instance, "parameterId":"expanded", "scope":"plugin"},
            "source":{"kind":"constant","value":0.25}
        })).unwrap());
    }
    let graph = compile(&snapshot, &registry).unwrap();
    assert_eq!(
        registry
            .lookup_descriptor("fixture.effect", "1.0.0")
            .unwrap()
            .parameters[0]
            .id,
        "gain"
    );
    graph.activate_plugin_controls();
    assert_eq!(graph.plugin_controls().inventory("vst3").instances.len(), 4);
    snapshot.channels[0].effect_chain[1].parameters = BTreeMap::from([("expanded".into(), 0.2)]);
    assert_eq!(
        compile(&snapshot, &registry).err().unwrap().code,
        "InvalidProject"
    );
    assert_eq!(graph.plugin_controls().inventory("vst3").state, "active");
}

#[test]
fn removed_parameters_and_legacy_automation_are_validated_against_restored_state() {
    let (mut snapshot, registry) = fixture();
    snapshot.channels[0]
        .instrument
        .parameters
        .insert("gain".into(), 0.5);
    assert_eq!(
        compile(&snapshot, &registry).err().unwrap().code,
        "InvalidProject"
    );
    snapshot.channels[0].instrument.parameters.remove("gain");
    for parameter in ["expanded", "insert.0.parameter.expanded"] {
        snapshot.automation = vec![serde_json::from_value(serde_json::json!({
            "id":"auto_valid", "target":{"entityId":"chn_test","parameterId":parameter},
            "source":{"kind":"constant","value":0.5}
        }))
        .unwrap()];
        compile(&snapshot, &registry).unwrap();
    }
    snapshot.automation[0].target.parameter_id = "insert.0.parameter.gain".into();
    assert_eq!(
        compile(&snapshot, &registry).err().unwrap().code,
        "AutomationTargetInvalid"
    );
}

#[test]
fn configured_factories_cannot_change_identity_or_bypass_structure_validation() {
    let (mut snapshot, registry) = fixture();
    snapshot.channels[0].instrument.state = Some("wrongIdentity".into());
    assert_eq!(
        compile(&snapshot, &registry).err().unwrap().code,
        "PluginManifestMismatch"
    );
    snapshot.protocol_version = "99.0".into();
    assert_eq!(
        compile(&snapshot, &registry).err().unwrap().code,
        "ProtocolVersionUnsupported"
    );
}
