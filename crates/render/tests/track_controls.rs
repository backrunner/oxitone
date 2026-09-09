mod common;
use common::*;
use oxitone_core::wire::*;
use oxitone_graph::{
    compile::{binding_value_at, compile_plan},
    CompileOptions,
};
use oxitone_render::{builtin_registry, RenderGraph, RenderGraphOptions, SampleStore};
use oxitone_transport::{event::EventPayload, EvalContext};

fn song() -> ProjectSnapshot {
    let mut s = base_snapshot();
    s.channels
        .push(channel("chn_a", "mix_master", wavetable_ref(&[]), vec![]));
    for (id, pitch, end) in [("a", 60, 4), ("b", 72, 8)] {
        s.patterns.push(pattern(
            &format!("pat_{id}"),
            (end, 1),
            vec![note(pitch, (0, 1), (1, 1), 0.7)],
        ));
        s.pattern_clips.push(pattern_clip(
            &format!("pcl_{id}"),
            &format!("pat_{id}"),
            &format!("trk_{id}"),
            (0, 1),
            (end, 1),
        ));
        s.tracks.push(track(
            &format!("trk_{id}"),
            &["chn_a"],
            &[&format!("pcl_{id}")],
            &[],
        ));
    }
    s
}
fn pitches(s: &ProjectSnapshot, respect_solo: bool) -> Vec<u8> {
    let plan = compile_plan(
        s,
        &builtin_registry().unwrap(),
        &SampleStore::new(None),
        &CompileOptions {
            respect_solo,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        plan.content_end_beat.to_f64(),
        8.,
        "Mute/Solo never resizes the timeline"
    );
    plan.scheduler
        .events()
        .iter()
        .filter_map(|e| match e.payload {
            EventPayload::NoteOn { pitch, .. } => Some(pitch),
            _ => None,
        })
        .collect()
}

#[test]
fn track_gating_preserves_shared_channel_and_export_policy() {
    let mut s = song();
    assert_eq!(pitches(&s, true), vec![60, 72]);
    s.tracks[0].mute = Some(true);
    assert_eq!(pitches(&s, true), vec![72]);
    assert_eq!(pitches(&s, false), vec![72]);
    s.tracks[0].mute = None;
    s.tracks[0].solo = Some(true);
    assert_eq!(pitches(&s, true), vec![60]);
    assert_eq!(pitches(&s, false), vec![60, 72]);
    s.tracks[1].solo = Some(true);
    assert_eq!(pitches(&s, true), vec![60, 72]);
    s.tracks[1].mute = Some(true);
    assert_eq!(pitches(&s, true), vec![60]);
    s.tracks[0].mute = Some(true);
    assert!(pitches(&s, true).is_empty());
    assert_eq!(s.channels[0].mute, None);
    assert_eq!(s.channels[0].solo, None);
    s.protocol_version = "1.1".into();
    assert!(oxitone_core::wire::encode_project_snapshot(&s).is_err());
}

#[test]
fn sample_and_playlist_automation_share_track_gating() {
    let mut s = song();
    let dir = out_dir("track-controls");
    s.samples
        .push(sample_asset(&dir, "smp_a", &vec![0.2; 4800], (1, 1)));
    s.sample_clips
        .push(sample_clip("scl_a", "smp_a", "trk_a", Some((1, 1))));
    s.tracks[0].sample_clip_ids.push("scl_a".into());
    s.automation.push(AutomationLaneSpec {
        id: "auto_a".into(),
        target: AutomationTarget {
            entity_id: "chn_a".into(),
            parameter_id: "level".into(),
            scope: None,
        },
        source: AutomationSourceSpec::Constant { value: 0.1 },
        playback: Some(AutomationPlayback::Playlist),
        combine: None,
        loop_spec: None,
        last_beat: None,
    });
    s.automation_clips = Some(vec![AutomationClipSpec {
        id: "acl_a".into(),
        lane_id: "auto_a".into(),
        track_id: "trk_a".into(),
        start_beat: beat(0, 1),
        duration_beats: Some(beat(4, 1)),
        enabled: None,
    }]);
    for (mute, solo_b, respect, active) in [
        (false, false, true, true),
        (true, false, true, false),
        (false, true, true, false),
        (false, true, false, true),
    ] {
        s.tracks[0].mute = Some(mute);
        s.tracks[1].solo = Some(solo_b);
        let plan = compile_plan(
            &s,
            &builtin_registry().unwrap(),
            &SampleStore::new(Some(dir.clone())),
            &CompileOptions {
                respect_solo: respect,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(!plan.sample_clips[0].channels.is_empty(), active);
        assert_eq!(
            binding_value_at(
                &plan.bindings[0],
                1.,
                &EvalContext {
                    origin_beat: 0.,
                    loop_iteration: 0
                }
            ),
            if active { 0.1 } else { 0.5 }
        );
        assert_eq!(plan.content_end_beat.to_f64(), 8.);
    }
}

#[test]
fn render_graph_forwards_solo_and_muted_notes_produce_silent_pcm() {
    let mut s = song();
    s.tracks[0].solo = Some(true);
    let compile = |s: &ProjectSnapshot, respect_solo| {
        RenderGraph::compile(
            s,
            &builtin_registry().unwrap(),
            &SampleStore::new(None),
            &RenderGraphOptions {
                respect_solo,
                ..Default::default()
            },
        )
        .unwrap()
    };
    let graph = compile(&s, true);
    assert_eq!(
        graph
            .plan()
            .scheduler
            .events()
            .iter()
            .filter(|e| matches!(e.payload, EventPayload::NoteOn { .. }))
            .count(),
        1
    );
    let graph = compile(&s, false);
    assert_eq!(
        graph
            .plan()
            .scheduler
            .events()
            .iter()
            .filter(|e| matches!(e.payload, EventPayload::NoteOn { .. }))
            .count(),
        2
    );
    s.tracks[0].mute = Some(true);
    let mut graph = compile(&s, true);
    graph.transport_mut().begin_render(0);
    let (mut l, mut r) = ([0.; 128], [0.; 128]);
    for _ in 0..100 {
        graph.process_block(&mut l, &mut r);
        assert!(l.iter().chain(&r).all(|v| *v == 0.));
    }
}
