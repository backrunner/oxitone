#[path = "common/allocations.rs"]
mod allocations;
mod common;
use common::*;
use oxitone_render::{builtin_registry, RenderGraph, RenderGraphOptions, SampleStore};
use std::sync::atomic::Ordering;

#[test]
fn preview_is_bit_exact_bounded_and_allocation_free_with_sample_accurate_notes() {
    let mut s = base_snapshot();
    s.tracks.push(track("trk_a", &["chn_a"], &["pcl_a"], &[]));
    // 120 BPM / 48 kHz: exact frame offsets 60 and 180.
    s.patterns.push(pattern(
        "pat_a",
        (4, 1),
        vec![note(60, (1, 400), (1, 200), 0.8)],
    ));
    s.pattern_clips
        .push(pattern_clip("pcl_a", "pat_a", "trk_a", (0, 1), (4, 1)));
    s.channels
        .push(channel("chn_a", "mix_master", wavetable_ref(&[]), vec![]));
    let registry = builtin_registry().unwrap();
    let store = SampleStore::new(None);
    let compile =
        || RenderGraph::compile(&s, &registry, &store, &RenderGraphOptions::default()).unwrap();
    let mut reference = compile();
    let mut graph = compile();
    let telemetry = graph.enable_preview();
    reference.transport_mut().begin_render(0);
    graph.transport_mut().begin_render(0);
    let (mut l, mut r, mut expected_l, mut expected_r) =
        ([0.; 128], [0.; 128], [0.; 128], [0.; 128]);
    for _ in 0..40 {
        reference.process_block(&mut expected_l, &mut expected_r);
        assert_eq!(
            allocations::count(|| graph.process_block(&mut l, &mut r)),
            (0, 0)
        );
        assert_eq!(l, expected_l);
        assert_eq!(r, expected_r);
    }
    let node = &telemetry.channels[0];
    let on = node.notes.pop().unwrap();
    let off = node.notes.pop().unwrap();
    assert_eq!((on.frame, on.pitch, on.on, on.epoch), (60, 60, true, 0));
    assert_eq!((off.frame, off.on), (180, false));
    assert!(node.notes.pop().is_none());
    assert!(node.dropped.load(Ordering::Relaxed) > 0);
    assert!(node.meter().0 > 0.);
    let master = telemetry
        .buses
        .iter()
        .find(|n| n.id == "mix_master")
        .unwrap();
    let mut captured = [0.; 8192];
    assert_eq!(master.audio.read_frames(&mut captured), 4096);
    assert!(captured.iter().any(|v| v.abs() > 0.001));
    assert_eq!(allocations::count(|| graph.seek(0)), (0, 0));
    assert_eq!(telemetry.epoch.load(Ordering::Relaxed), 1);
    graph.process_block(&mut l, &mut r);
    assert_eq!(node.notes.pop().unwrap().epoch, 1);
}
