mod common;

use common::*;
use oxitone_core::wire::EffectRef;
use oxitone_graph::validate;
use std::collections::BTreeMap;

#[test]
fn vst3_never_silently_discards_project_resource_bindings() {
    let id = "vst3.11111111111111111111111111111111";
    for slot in 0..3 {
        let mut snapshot = base_snapshot();
        snapshot.protocol_version = "1.3".into();
        let mut registry = base_registry();
        let mut descriptor = synth_descriptor();
        if slot != 0 {
            descriptor.kind = oxitone_graph::PluginKind::Effect;
            descriptor.input_layout = oxitone_graph::ChannelLayout::Stereo;
            descriptor.max_polyphony = None;
        }
        descriptor.plugin_id = id.into();
        registry.register(mock_plugin(descriptor)).unwrap();
        let resources = Some(BTreeMap::from([("sample".into(), "smp_missing".into())]));
        if slot == 0 {
            snapshot.channels[0].instrument.plugin_id = id.into();
            snapshot.channels[0].instrument.resources = resources;
        } else {
            let effect = EffectRef {
                plugin_id: id.into(),
                plugin_version: PLUGIN_VERSION.into(),
                resources,
                state: None,
                instance_id: None,
                parameters: Default::default(),
                mix: None,
                bypass: None,
            };
            if slot == 1 {
                snapshot.channels[0].effect_chain.push(effect);
            } else {
                snapshot.mixer_channels[0].inserts.push(effect);
            }
        }
        let error = validate(&snapshot, &registry).unwrap_err();
        assert_eq!(error.code, "PluginCapabilityUnsupported");
        assert!(error.path.unwrap().ends_with(".resources"));
    }
}
