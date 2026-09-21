#[path = "common/allocations.rs"]
mod allocations;
mod common;
#[path = "output_routes/fixture.rs"]
mod fixture;
use oxitone_core::wire::ProjectSnapshot;
use oxitone_graph::{PluginKind, PluginRegistry};
use oxitone_render::{RenderGraph, RenderGraphOptions, SampleStore};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

fn setup() -> (
    ProjectSnapshot,
    PluginRegistry,
    Arc<AtomicUsize>,
    Arc<AtomicUsize>,
) {
    let mut registry = oxitone_render::builtin_registry().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let activation = Arc::new(AtomicUsize::new(0));
    for (id, base, kind) in [
        (
            "fixture.outputs",
            "oxitone.wavetable",
            fixture::Kind::Instrument,
        ),
        ("fixture.delay", "oxitone.utility", fixture::Kind::Delay),
    ] {
        let mut descriptor = registry.lookup_descriptor(base, "1.0.0").unwrap().clone();
        descriptor.plugin_id = id.into();
        descriptor.parameters.clear();
        registry
            .register(Arc::new(fixture::Factory {
                descriptor,
                kind,
                calls: calls.clone(),
                activation: activation.clone(),
            }))
            .unwrap();
    }
    let mut snapshot = common::base_snapshot();
    let mut instrument = common::wavetable_ref(&[]);
    instrument.plugin_id = "fixture.outputs".into();
    snapshot.channels.push(common::channel(
        "chn_a",
        "mix_main",
        instrument,
        vec![common::effect_ref("fixture.delay", &[])],
    ));
    snapshot.channels[0].output_routes = Some(
        [
            ("1".into(), "mix_one".into()),
            ("2".into(), "mix_two".into()),
        ]
        .into(),
    );
    for id in ["mix_main", "mix_one", "mix_two", "mix_master"] {
        snapshot
            .mixer_channels
            .push(common::mixer_channel(id, vec![], vec![]));
    }
    (snapshot, registry, calls, activation)
}
fn compile(snapshot: &ProjectSnapshot, registry: &PluginRegistry) -> RenderGraph {
    let mut graph = RenderGraph::compile(
        snapshot,
        registry,
        &SampleStore::new(None),
        &RenderGraphOptions {
            master_limiter: false,
            ..Default::default()
        },
    )
    .unwrap();
    graph.transport_mut().begin_render(0);
    graph
}

#[test]
fn auxiliary_outputs_process_once_share_fader_and_align_with_main_insert_latency() {
    let (mut snapshot, registry, calls, activation) = setup();
    let gain = oxitone_dsp::gain_pan::equal_power_gains(0.).0;
    for selected in ["mix_main", "mix_one", "mix_two"] {
        for bus in &mut snapshot.mixer_channels {
            if bus.id != "mix_master" {
                bus.master_send_ratio = Some(if bus.id == selected { 1. } else { 0. });
            }
        }
        let mut graph = compile(&snapshot, &registry);
        assert_eq!(graph.graph_latency_frames(), 12);
        assert_eq!(activation.load(Ordering::Relaxed), 7);
        calls.store(0, Ordering::Relaxed);
        let mut l = [0.; 128];
        let mut r = l;
        let measured = allocations::count(|| {
            graph
                .process_offline_block(&mut l[..9], &mut r[..9])
                .unwrap();
            graph
                .process_offline_block(&mut l[9..], &mut r[9..])
                .unwrap();
        });
        assert_eq!(measured, (0, 0));
        assert_eq!(
            calls.load(Ordering::Relaxed),
            2,
            "one processor call per segment, irrespective of output count"
        );
        let raw = match selected {
            "mix_main" => 0.05,
            "mix_one" => 0.2,
            _ => 0.3,
        };
        for (i, sample) in l.iter().enumerate() {
            assert!(
                (*sample - if i == 12 { raw * gain.powi(3) } else { 0. }).abs() < 1e-6,
                "{selected} frame {i}"
            );
        }
        assert_eq!(l, r);
        graph.seek(0);
        let mut repeat_l = [0.; 128];
        let mut repeat_r = repeat_l;
        graph
            .process_offline_block(&mut repeat_l, &mut repeat_r)
            .unwrap();
        assert_eq!(l, repeat_l, "seek clears every route's compensation delay");
    }
    snapshot.channels[0].mute = Some(true);
    let mut graph = compile(&snapshot, &registry);
    let mut l = [1.; 128];
    let mut r = l;
    graph.process_offline_block(&mut l, &mut r).unwrap();
    assert_eq!(l, [0.; 128]);
    snapshot.channels[0].output_routes = Some([("2".into(), "mix_two".into())].into());
    let _ = compile(&snapshot, &registry);
    assert_eq!(
        activation.load(Ordering::Relaxed),
        5,
        "unmapped bus 1 is disabled"
    );
}

#[test]
fn route_validation_rejects_unknown_indices_destinations_and_old_formats() {
    let (mut snapshot, registry, _, _) = setup();
    for (index, destination) in [
        ("0", "mix_one"),
        ("3", "mix_one"),
        ("16", "mix_one"),
        ("01", "mix_one"),
        ("2", "mix_missing"),
    ] {
        snapshot.channels[0].output_routes = Some([(index.into(), destination.into())].into());
        assert!(RenderGraph::compile(
            &snapshot,
            &registry,
            &SampleStore::new(None),
            &Default::default()
        )
        .is_err());
    }
    snapshot.channels[0].output_routes = Some(Default::default());
    snapshot.protocol_version = "1.3".into();
    assert_eq!(
        oxitone_core::wire::check_snapshot_version(&snapshot)
            .unwrap_err()
            .code,
        "ProtocolVersionUnsupported"
    );
    assert_eq!(
        registry
            .lookup_descriptor("fixture.outputs", "1.0.0")
            .unwrap()
            .kind,
        PluginKind::Instrument
    );
}
