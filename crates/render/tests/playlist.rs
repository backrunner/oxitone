#[path = "common/allocations.rs"]
mod allocations;
mod common;
use common::*;
use oxitone_core::wire::*;
use oxitone_graph::compile::{binding_value_at, compile_plan, CompileOptions};
use oxitone_render::{builtin_registry, RenderGraph, RenderGraphOptions, SampleStore};
use oxitone_transport::{event::EventPayload, EvalContext};

fn song() -> ProjectSnapshot {
    let mut s = base_snapshot();
    s.channels
        .push(channel("chn_a", "mix_master", wavetable_ref(&[]), vec![]));
    s.channels
        .push(channel("chn_b", "mix_master", wavetable_ref(&[]), vec![]));
    s.patterns = vec![
        pattern("pat_a", (4, 1), vec![note(60, (0, 1), (1, 1), 0.7)]),
        pattern("pat_b", (4, 1), vec![note(72, (1, 1), (1, 2), 0.4)]),
        pattern("pat_song", (4, 1), vec![]),
    ];
    s.patterns[2].parts = Some(vec![
        PatternPartSpec {
            channel_id: "chn_a".into(),
            pattern_id: "pat_a".into(),
        },
        PatternPartSpec {
            channel_id: "chn_b".into(),
            pattern_id: "pat_b".into(),
        },
    ]);
    s.pattern_clips.push(pattern_clip(
        "pcl_song",
        "pat_song",
        "trk_a",
        (0, 1),
        (8, 1),
    ));
    s.tracks.push(track("trk_a", &[], &["pcl_song"], &[]));
    s
}
fn placement(id: &str, start: i64, duration: i64) -> AutomationClipSpec {
    AutomationClipSpec {
        id: id.into(),
        lane_id: "auto_a".into(),
        track_id: "trk_a".into(),
        start_beat: beat(start, 1),
        duration_beats: Some(beat(duration, 1)),
        enabled: None,
    }
}
fn automate(s: &mut ProjectSnapshot, parameter: &str, source: AutomationSourceSpec) {
    s.automation.push(AutomationLaneSpec {
        id: "auto_a".into(),
        target: AutomationTarget {
            entity_id: "chn_a".into(),
            parameter_id: parameter.into(),
            scope: None,
        },
        playback: Some(AutomationPlayback::Playlist),
        source,
        combine: None,
        loop_spec: None,
        last_beat: None,
    });
}

#[test]
fn independent_parts_repeat_and_move_without_track_routing() {
    let registry = builtin_registry().unwrap();
    let store = SampleStore::new(None);
    let mut s = song();
    for start in [0, 3] {
        s.pattern_clips[0].start_beat = beat(start, 1);
        let plan = compile_plan(&s, &registry, &store, &CompileOptions::default()).unwrap();
        let notes: Vec<_> = plan
            .scheduler
            .events()
            .iter()
            .filter_map(|e| match e.payload {
                EventPayload::NoteOn { pitch, .. } => Some((e.channel_id.as_str(), pitch, e.frame)),
                _ => None,
            })
            .collect();
        assert_eq!(
            notes,
            vec![
                ("chn_a", 60, start as u64 * 24000),
                ("chn_b", 72, (start + 1) as u64 * 24000),
                ("chn_a", 60, (start + 4) as u64 * 24000),
                ("chn_b", 72, (start + 5) as u64 * 24000)
            ]
        );
    }
    let encoded = oxitone_core::wire::encode_project_snapshot(&s).unwrap();
    assert_eq!(
        oxitone_core::wire::decode_project_snapshot(&encoded).unwrap(),
        s
    );
    s.protocol_version = "1.1".into();
    assert!(compile_plan(&s, &registry, &store, &CompileOptions::default()).is_err());
}

#[test]
fn shorter_parts_wait_for_the_composite_period_before_repeating() {
    let registry = builtin_registry().unwrap();
    let store = SampleStore::new(None);
    let mut s = song();
    s.patterns[0].length_beats = beat(8, 1);
    s.patterns[2].length_beats = beat(8, 1);
    s.pattern_clips[0].duration_beats = Some(beat(16, 1));
    let plan = compile_plan(&s, &registry, &store, &CompileOptions::default()).unwrap();
    let starts: Vec<_> = plan
        .scheduler
        .events()
        .iter()
        .filter_map(|e| match e.payload {
            EventPayload::NoteOn { pitch: 72, .. } => Some(e.frame),
            _ => None,
        })
        .collect();
    assert_eq!(starts, vec![24000, 9 * 24000]);
    s.patterns[0].length_beats = beat(9, 1);
    assert!(compile_plan(&s, &registry, &store, &CompileOptions::default()).is_err());
    // A wire fraction slightly past the root must not disappear in f64 rounding.
    s.patterns[2].length_beats = beat(9_007_199_254_740_992, 1_000_000_000);
    s.patterns[0].length_beats = beat(9_007_199_254_740_993, 1_000_000_000);
    assert!(compile_plan(&s, &registry, &store, &CompileOptions::default()).is_err());
}

#[test]
fn automation_local_time_overlap_gaps_seek_and_last_removal() {
    let registry = builtin_registry().unwrap();
    let store = SampleStore::new(None);
    let mut s = song();
    s.channels[0].level = 0.6;
    automate(
        &mut s,
        "level",
        AutomationSourceSpec::Wave {
            wave: WaveKind::Saw,
            period_beats: beat(4, 1),
            phase: None,
            min: None,
            max: None,
            pulse_width: None,
        },
    );
    s.automation_clips = Some(vec![placement("acl_a", 2, 4), placement("acl_b", 4, 1)]);
    let plan = compile_plan(&s, &registry, &store, &CompileOptions::default()).unwrap();
    let ctx = EvalContext {
        origin_beat: 3.2,
        loop_iteration: 0,
    };
    // Later-starting clip wins only during its interval; older clip resumes at its own phase.
    for (t, v) in [
        (7., 0.3),
        (6., 0.3),
        (5., 0.75),
        (4., 0.),
        (3., 0.25),
        (2., 0.),
        (1., 0.3),
    ] {
        assert!((binding_value_at(&plan.bindings[0], t, &ctx) - v).abs() < 1e-12);
    }
    assert_eq!(
        allocations::count(|| {
            for i in 0..48000 {
                std::hint::black_box(binding_value_at(&plan.bindings[0], i as f64 / 6000., &ctx));
            }
        }),
        (0, 0)
    );
    s.automation_clips.as_mut().unwrap().clear();
    let plan = compile_plan(&s, &registry, &store, &CompileOptions::default()).unwrap();
    assert_eq!(binding_value_at(&plan.bindings[0], 3., &ctx), 0.3);
    s.automation_clips = Some(vec![placement("acl_a", 2, 4)]);
    s.tracks[0].enabled = Some(false);
    let plan = compile_plan(&s, &registry, &store, &CompileOptions::default()).unwrap();
    assert_eq!(binding_value_at(&plan.bindings[0], 3., &ctx), 0.3);
}

#[test]
fn automation_changes_pcm_only_in_placed_range_across_block_sizes() {
    let mut s = song();
    s.patterns[0].notes[0].duration = beat(4, 1);
    s.channels[1].mute = Some(true);
    automate(&mut s, "mute", AutomationSourceSpec::Constant { value: 1. });
    let mut clip = placement("acl_a", 1, 1);
    // Deliberately not aligned to a block.
    clip.start_beat = beat(101, 100);
    s.automation_clips = Some(vec![clip]);
    let mut reference = None;
    for block in [64, 128, 256] {
        s.block_size = block;
        let registry = builtin_registry().unwrap();
        let store = SampleStore::new(None);
        let mut graph = RenderGraph::compile(
            &s,
            &registry,
            &store,
            &RenderGraphOptions {
                master_limiter: false,
                ..Default::default()
            },
        )
        .unwrap();
        graph.transport_mut().begin_render(0);
        let (mut l, mut r) = (vec![0.; block as usize], vec![0.; block as usize]);
        let mut output = vec![0.; 72000];
        assert_eq!(
            allocations::count(|| {
                for chunk in output.chunks_mut(block as usize) {
                    graph.process_block(&mut l, &mut r);
                    chunk.copy_from_slice(&l[..chunk.len()]);
                }
                graph.seek(60000);
                graph.process_block(&mut l, &mut r);
            }),
            (0, 0)
        );
        assert!(output[30000..47000].iter().all(|v| *v == 0.));
        assert!(output[8000..20000].iter().any(|v| v.abs() > 0.01));
        assert!(output[55000..65000].iter().any(|v| v.abs() > 0.01));
        if let Some(ref reference) = reference {
            assert_eq!(&output, reference);
        } else {
            reference = Some(output);
        }
    }
}
