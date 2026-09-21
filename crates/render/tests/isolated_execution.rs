#[path = "common/allocations.rs"]
mod allocations;
mod common;
#[path = "isolated_execution/probe.rs"]
mod probe;

use oxitone_graph::execution::IsolatedMode;
use oxitone_render::{RenderGraph, RenderGraphOptions, SampleStore, TransportState};
use std::sync::{Arc, Mutex};

fn graph(effect: bool, fail: bool) -> (RenderGraph, Arc<Mutex<Vec<probe::Call>>>) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut registry = oxitone_render::builtin_registry().unwrap();
    registry
        .register(Arc::new(probe::Probe {
            calls: calls.clone(),
            effect,
            fail,
        }))
        .unwrap();
    let mut snapshot = common::base_snapshot();
    snapshot.time_signature_map[0].numerator = 6;
    snapshot.time_signature_map[0].denominator = 8;
    let mut instrument = common::wavetable_ref(&[]);
    if !effect {
        instrument.plugin_id = "fixture.isolated".into();
    }
    snapshot.channels.push(common::channel(
        "chn_test",
        "mix_master",
        instrument,
        vec![],
    ));
    if effect {
        snapshot.mixer_channels.push(common::mixer_channel(
            "mix_master",
            vec![common::effect_ref("fixture.isolated", &[])],
            vec![],
        ));
    }
    let mut graph = RenderGraph::compile(
        &snapshot,
        &registry,
        &SampleStore::new(None),
        &RenderGraphOptions {
            master_limiter: false,
            ..Default::default()
        },
    )
    .unwrap();
    graph.transport_mut().begin_render(96000);
    (graph, calls)
}

#[test]
fn direct_execution_rejects_isolated_instruments_and_master_inserts_without_allocating() {
    for effect in [false, true] {
        let (mut graph, calls) = graph(effect, false);
        let mut left = [1.0; 128];
        let mut right = left;
        let counts = allocations::count(|| {
            graph.process_block(&mut left, &mut right);
            graph.seek(0);
        });
        assert_eq!(counts, (0, 0));
        assert!(graph.faulted());
        assert_eq!(left, [0.0; 128]);
        assert!(calls.lock().unwrap().is_empty());
    }
}

#[test]
fn isolated_execution_maps_exact_short_segments_meter_and_continuous_time_across_seek() {
    for effect in [false, true] {
        let (mut graph, calls) = graph(effect, false);
        let mut left = [0.0; 17];
        let mut right = left;
        graph.process_offline_block(&mut left, &mut right).unwrap();
        graph.seek(24000);
        graph.process_isolated_block(&mut left, &mut right).unwrap();
        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].frames, 17);
        assert_eq!(calls[0].position.project_frame, 96000);
        assert_eq!(calls[0].position.project_beat, 4.0);
        assert_eq!(calls[0].position.bar_beat, 3.0);
        assert_eq!(calls[0].position.time_signature, [6, 8]);
        assert_eq!(calls[0].position.mode, IsolatedMode::Offline);
        assert_eq!(calls[1].position.project_frame, 24000);
        assert_eq!(calls[1].position.continuous_frame, 17);
        assert_eq!(calls[1].position.mode, IsolatedMode::Realtime);
        assert_eq!(calls[1].resets, 1);
    }
}

#[test]
fn external_failure_silences_the_entire_output_and_returns_the_error() {
    let (mut graph, _) = graph(false, true);
    let mut left = [1.0; 128];
    let mut right = left;
    let error = graph
        .process_offline_block(&mut left, &mut right)
        .unwrap_err();
    assert_eq!(error.code, "PluginHostTimeout");
    assert_eq!(left, [0.0; 128]);
    assert_eq!(right, left);
    assert_eq!(graph.state(), TransportState::Stopped);
}

#[test]
fn loop_boundary_starts_a_new_state_epoch_at_the_exact_frame() {
    let (mut graph, calls) = graph(false, false);
    graph.transport_mut().play_from(96000, Some((96000, 96019)));
    let mut left = [0.0; 37];
    let mut right = left;
    graph.process_isolated_block(&mut left, &mut right).unwrap();
    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!((calls[0].frames, calls[1].frames), (19, 18));
    assert_eq!(calls[1].position.project_frame, 96000);
    assert_eq!(calls[1].position.continuous_frame, 19);
    assert_eq!(calls[1].resets, 1);
}
