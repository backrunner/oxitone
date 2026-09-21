mod common;
#[path = "plugin_controls/probe.rs"]
mod probe;
use oxitone_render::{
    plugin_controls::ControlRegistry, RenderGraph, RenderGraphOptions, SampleStore,
};
use std::{
    sync::{Arc, Barrier},
    time::Duration,
};

fn graph(seeded: bool, ids: bool, barrier: Option<Arc<Barrier>>) -> RenderGraph {
    let mut registry = oxitone_render::builtin_registry().unwrap();
    for effect in [false, true] {
        registry
            .register(Arc::new(probe::Probe::new(effect, seeded, barrier.clone())))
            .unwrap();
    }
    let mut snapshot = common::base_snapshot();
    let mut instrument = common::wavetable_ref(&[("gain", 0.75)]);
    instrument.plugin_id = "fixture.instrument".into();
    instrument.instance_id = ids.then(|| "ins_instrument".into());
    let effect = |id: &str| {
        let mut effect = common::effect_ref("fixture.effect", &[("gain", 0.75)]);
        effect.instance_id = ids.then(|| id.into());
        effect
    };
    snapshot.channels.push(common::channel(
        "chn_test",
        "mix_master",
        instrument,
        vec![effect("ins_channel")],
    ));
    snapshot.mixer_channels.push(common::mixer_channel(
        "mix_master",
        vec![effect("ins_master")],
        vec![],
    ));
    RenderGraph::compile(
        &snapshot,
        &registry,
        &SampleStore::new(None),
        &RenderGraphOptions::default(),
    )
    .unwrap()
}
fn request(registry: &ControlRegistry, id: &str, command: &str) -> String {
    registry
        .request(
            &registry.generation(),
            id,
            "vst3",
            command,
            Duration::from_secs(1),
        )
        .unwrap()
}

#[test]
fn prepared_graph_is_not_editable_and_drop_retires_cloned_handles() {
    let graph = graph(true, true, None);
    let registry = graph.plugin_controls();
    let generation = registry.generation();
    assert_eq!(registry.inventory("vst3").state, "prepared");
    assert_eq!(
        registry.check(&generation).unwrap_err().code,
        "PluginTaskConflict"
    );
    graph.activate_plugin_controls();
    assert_eq!(
        registry
            .inventory("vst3")
            .instances
            .iter()
            .map(|i| i.instance_id.as_str())
            .collect::<Vec<_>>(),
        ["ins_channel", "ins_instrument", "ins_master"]
    );
    assert_eq!(
        registry
            .request(&generation, "missing", "vst3", "get", Duration::ZERO)
            .unwrap_err()
            .code,
        "PluginConfigInvalid"
    );
    assert_eq!(
        registry
            .request(&generation, "ins_master", "other", "get", Duration::ZERO)
            .unwrap_err()
            .code,
        "PluginCapabilityUnsupported"
    );
    drop(graph);
    assert_eq!(registry.inventory("vst3").state, "retired");
    assert_eq!(
        registry.check(&generation).unwrap_err().code,
        "PluginTaskConflict"
    );
}

#[test]
fn adapter_seeding_preserves_preplay_controls_for_all_instance_locations() {
    for seeded in [false, true] {
        let mut graph = graph(seeded, true, None);
        graph.activate_plugin_controls();
        let registry = graph.plugin_controls();
        for id in ["ins_channel", "ins_instrument", "ins_master"] {
            request(&registry, id, "0.25");
        }
        graph.transport_mut().begin_render(0);
        graph.process_block(&mut [0.; 128], &mut [0.; 128]);
        for id in ["ins_channel", "ins_instrument", "ins_master"] {
            assert_eq!(
                request(&registry, id, "get"),
                if seeded { "0.25" } else { "0.75" }
            );
        }
        request(&registry, "ins_channel", "0.5");
        assert_eq!(
            request(&registry, "ins_master", "get"),
            if seeded { "0.25" } else { "0.75" }
        );
    }
}

#[test]
fn graph_generations_are_unique_and_legacy_instances_get_no_synthetic_id() {
    let first = graph(true, false, None);
    let second = graph(true, true, None);
    assert!(first
        .plugin_controls()
        .inventory("vst3")
        .instances
        .is_empty());
    let old = first.plugin_controls().generation();
    second.activate_plugin_controls();
    assert_ne!(old, second.plugin_controls().generation());
    assert_eq!(
        second.plugin_controls().check(&old).unwrap_err().code,
        "PluginTaskConflict"
    );
}

#[test]
fn retirement_during_a_vendor_call_discards_the_late_reply() {
    let barrier = Arc::new(Barrier::new(2));
    let graph = graph(true, true, Some(barrier.clone()));
    graph.activate_plugin_controls();
    let registry = graph.plugin_controls();
    let call = std::thread::spawn(move || {
        registry.request(
            &registry.generation(),
            "ins_master",
            "vst3",
            "wait",
            Duration::from_secs(1),
        )
    });
    barrier.wait();
    drop(graph);
    barrier.wait();
    assert_eq!(call.join().unwrap().unwrap_err().code, "PluginTaskConflict");
}
