//! Host `setParameter` queue semantics (04-api-contracts.md §Wire
//! messages): control-thread enqueue with target/range validation, applied
//! at control rate when the graph processes blocks, and forwarded through
//! `render_wav` via `RenderOptions::parameter_events`.

mod common;

use oxitone_render::{
    builtin_registry, render_wav, ParameterEventInput, RenderGraph, RenderGraphOptions,
    RenderOptions, SampleStore,
};

use common::{base_snapshot, channel, out_dir, pattern, pattern_clip, track, wavetable_ref};

fn event(entity: &str, parameter: &str, value: f64, at_frame: Option<u64>) -> ParameterEventInput {
    ParameterEventInput {
        entity_id: entity.into(),
        parameter_id: parameter.into(),
        value,
        at_frame,
    }
}

/// One wavetable channel playing a four-beat pattern from beat 0.
fn playable_snapshot() -> oxitone_core::wire::ProjectSnapshot {
    let mut snapshot = base_snapshot();
    snapshot
        .tracks
        .push(track("trk_a", &["chn_a"], &["pcl_a"], &[]));
    snapshot.patterns.push(pattern(
        "pat_a",
        (4, 1),
        vec![
            common::note(60, (0, 1), (1, 1), 0.9),
            common::note(64, (1, 1), (1, 1), 0.9),
            common::note(67, (2, 1), (1, 1), 0.9),
            common::note(72, (3, 1), (1, 1), 0.9),
        ],
    ));
    snapshot
        .pattern_clips
        .push(pattern_clip("pcl_a", "pat_a", "trk_a", (0, 1), (4, 1)));
    snapshot
        .channels
        .push(channel("chn_a", "mix_master", wavetable_ref(&[]), vec![]));
    snapshot
}

fn compile(snapshot: &oxitone_core::wire::ProjectSnapshot) -> RenderGraph {
    RenderGraph::compile(
        snapshot,
        &builtin_registry().unwrap(),
        &SampleStore::new(None),
        &RenderGraphOptions::default(),
    )
    .unwrap()
}

fn render_blocks(graph: &mut RenderGraph, blocks: usize) -> (Vec<f32>, Vec<f32>) {
    graph.transport_mut().begin_render(0);
    let block = 128;
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut out_l = vec![0.0f32; block];
    let mut out_r = vec![0.0f32; block];
    for _ in 0..blocks {
        graph.process_block(&mut out_l, &mut out_r);
        left.extend_from_slice(&out_l);
        right.extend_from_slice(&out_r);
    }
    assert!(!graph.faulted(), "render faulted (NaN guard)");
    (left, right)
}

fn peak(buf: &[f32]) -> f32 {
    buf.iter().fold(0.0f32, |acc, x| acc.max(x.abs()))
}

#[test]
fn channel_mute_event_silences_output() {
    let snapshot = playable_snapshot();
    let mut graph = compile(&snapshot);
    graph
        .enqueue_parameter(&event("chn_a", "mute", 1.0, None))
        .unwrap();
    let (left, right) = render_blocks(&mut graph, 64);
    assert_eq!(peak(&left), 0.0);
    assert_eq!(peak(&right), 0.0);
    assert_eq!(graph.pending_parameter_events(), 0);
}

#[test]
fn channel_level_event_ramps_to_silence() {
    let snapshot = playable_snapshot();
    let mut reference = compile(&snapshot);
    let (ref_l, _) = render_blocks(&mut reference, 64);
    assert!(peak(&ref_l) > 0.1, "fixture must be audible");

    // The level smoother (20 ms) ramps rather than snaps; after the ramp
    // the output is far below the reference.
    let mut graph = compile(&snapshot);
    graph
        .enqueue_parameter(&event("chn_a", "level", 0.0, None))
        .unwrap();
    let (left, _) = render_blocks(&mut graph, 64);
    let tail = &left[56 * 128..];
    assert!(
        peak(tail) < 0.01,
        "tail peak {} should be near zero",
        peak(tail)
    );
}

#[test]
fn event_with_future_frame_applies_late() {
    let snapshot = playable_snapshot();
    let block = 128_u64;
    let mut audible = compile(&snapshot);
    let (ref_l, _) = render_blocks(&mut audible, 8);
    assert!(peak(&ref_l) > 0.0, "fixture must be audible");

    // Silence from block 4 on: earlier blocks match the reference.
    let mut graph = compile(&snapshot);
    graph
        .enqueue_parameter(&event("chn_a", "level", 0.0, Some(4 * block)))
        .unwrap();
    let (left, _) = render_blocks(&mut graph, 8);
    let boundary = 4 * block as usize;
    // Blocks before the event are identical to the reference; once the
    // smoother has ramped for a few blocks the level event pulls the
    // output strictly below the reference (final block: gain < 1
    // everywhere).
    assert_eq!(&left[..boundary], &ref_l[..boundary]);
    let tail = 7 * block as usize;
    assert!(peak(&left[tail..]) < peak(&ref_l[tail..]));
}

#[test]
fn mixer_and_instrument_targets_resolve() {
    let snapshot = playable_snapshot();
    let mut graph = compile(&snapshot);
    graph
        .enqueue_parameter(&event("mix_master", "level", 0.5, None))
        .unwrap();
    graph
        .enqueue_parameter(&event("chn_a", "filter.cutoff", 400.0, None))
        .unwrap();
    graph
        .enqueue_parameter(&event("chn_a", "mute", 1.0, None))
        .unwrap();
    let (left, _) = render_blocks(&mut graph, 8);
    assert_eq!(peak(&left), 0.0, "muted channel must be silent");
}

#[test]
fn invalid_targets_and_ranges_are_rejected() {
    let snapshot = playable_snapshot();
    let mut graph = compile(&snapshot);

    let err = graph
        .enqueue_parameter(&event("chn_ghost", "level", 1.0, None))
        .unwrap_err();
    assert_eq!(err.code, "AutomationTargetInvalid");

    let err = graph
        .enqueue_parameter(&event("chn_a", "nonsense", 1.0, None))
        .unwrap_err();
    assert_eq!(err.code, "AutomationTargetInvalid");

    // Swing is automation-only at runtime (dispatch reads the plan value).
    let err = graph
        .enqueue_parameter(&event("chn_a", "swing", 0.5, None))
        .unwrap_err();
    assert_eq!(err.code, "AutomationTargetInvalid");

    // Channel level range is 0..2.
    let err = graph
        .enqueue_parameter(&event("chn_a", "level", 3.0, None))
        .unwrap_err();
    assert_eq!(err.code, "AutomationRange");

    let err = graph
        .enqueue_parameter(&event("chn_a", "level", f64::NAN, None))
        .unwrap_err();
    assert_eq!(err.code, "AutomationRange");
}

#[test]
fn render_wav_applies_parameter_events() {
    let snapshot = playable_snapshot();
    let registry = builtin_registry().unwrap();
    let store = SampleStore::new(None);
    let dir = out_dir("params");

    let mut options = RenderOptions::new(dir.join("muted.wav"));
    options
        .parameter_events
        .push(event("chn_a", "mute", 1.0, None));
    let report = render_wav(&snapshot, &registry, &store, &options).unwrap();
    assert_eq!(report.files.len(), 1);
    assert!(
        report.files[0].peak_dbfs < -90.0,
        "silenced channel must render near-digital-silence, got {} dBFS",
        report.files[0].peak_dbfs
    );
}
