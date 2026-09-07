//! Full graph parity and realtime boundaries for morph/layers/note-triggered LFO.
#[path = "common/allocations.rs"]
mod allocations;
mod common;
use common::*;
use oxitone_core::wire::ProjectSnapshot;
use oxitone_render::{RenderGraph, SampleStore};

fn project(block: u32, voices: u8) -> ProjectSnapshot {
    let mut s = base_snapshot();
    s.block_size = block;
    s.channels = vec![channel(
        "chn_s",
        "mix_master",
        wavetable_ref(&[
            ("oscA.unison", 3.),
            ("oscA.morphTo", 5.),
            ("oscA.position", 0.4),
            ("oscA.phase", 0.2),
            ("oscA.phaseSpread", 0.6),
            ("oscB.wavetable", 4.),
            ("oscB.morphTo", 2.),
            ("oscB.position", 0.3),
            ("osc.mix", 0.2),
            ("sub.level", 0.1),
            ("noise.level", 0.02),
            ("filter.cutoff", 2400.),
            ("filterEnv.amount", 12.),
            ("lfo.rateHz", 5.3),
            ("lfo.pitch", 0.7),
            ("lfo.cutoff", 9.),
            ("lfo.positionA", 0.3),
            ("lfo.positionB", -0.2),
            ("lfo.level", 0.2),
            ("amp.release", 0.02),
        ]),
        vec![],
    )];
    s.tracks = vec![track("trk_s", &["chn_s"], &["pcl_s"], &[])];
    // Start at frame 17: a voice's control cycle must survive host block splits.
    let mut notes = (0..voices)
        .map(|i| note(36 + i, (17, 24000), (1, 2), 0.3))
        .collect::<Vec<_>>();
    notes.push(note(72, (17041, 24000), (1, 2), 0.4));
    s.patterns = vec![pattern("pat_s", (4, 1), notes)];
    s.pattern_clips = vec![pattern_clip("pcl_s", "pat_s", "trk_s", (0, 1), (4, 1))];
    s
}

#[test]
fn note_triggered_motion_matches_across_host_block_sizes() {
    let (reference_l, reference_r, _) = render_frames(&project(128, 3), 48000);
    assert!(reference_l.iter().any(|v| v.abs() > 0.01));
    for block in [64, 256] {
        let (left, right, _) = render_frames(&project(block, 3), 48000);
        let delta = left
            .iter()
            .chain(&right)
            .zip(reference_l.iter().chain(&reference_r))
            .map(|(a, b)| (a - b).abs())
            .fold(0f32, f32::max);
        assert!(delta < 1e-5, "block {block}: max delta {delta}");
    }
}

#[test]
fn mono_and_legato_motion_keep_pitch_changes_sample_aligned() {
    for mode in [1., 2.] {
        let source = |block| {
            let mut s = project(block, 1);
            s.channels[0]
                .instrument
                .parameters
                .extend([("voiceMode".into(), mode), ("glide".into(), 0.03)]);
            s.patterns[0]
                .notes
                .push(note(67, (617, 24000), (1, 8), 0.4));
            s
        };
        let (a, b, _) = render_frames(&source(128), 24000);
        for block in [64, 256] {
            let (l, r, _) = render_frames(&source(block), 24000);
            let delta = a
                .iter()
                .chain(&b)
                .zip(l.iter().chain(&r))
                .map(|(a, b)| (a - b).abs())
                .fold(0f32, f32::max);
            assert!(delta < 1e-5, "mode {mode}, block {block}: {delta}");
        }
    }
}

#[test]
fn motion_voice_stealing_seek_and_parameter_events_never_allocate_or_free() {
    let registry = oxitone_render::builtin_registry().unwrap();
    // 65 notes force stealing from the fixed 64-voice pool.
    let mut graph = RenderGraph::compile(
        &project(128, 65),
        &registry,
        &SampleStore::new(None),
        &Default::default(),
    )
    .unwrap();
    graph
        .enqueue_parameter(&oxitone_render::ParameterEventInput {
            entity_id: "chn_s".into(),
            parameter_id: "lfo.positionA".into(),
            value: 0.5,
            at_frame: Some(97),
        })
        .unwrap();
    graph.transport_mut().begin_render(0);
    let (mut left, mut right) = ([0.; 128], [0.; 128]);
    let counts = allocations::count(|| {
        for block in 0..150 {
            if block == 90 {
                graph.seek(0);
            }
            graph.process_block(&mut left, &mut right);
        }
    });
    assert_eq!(counts, (0, 0));
    assert!(!graph.faulted());
    assert!(left.iter().chain(&right).all(|v| v.is_finite()));
    assert!(left.iter().any(|v| v.abs() > 0.001));
}
