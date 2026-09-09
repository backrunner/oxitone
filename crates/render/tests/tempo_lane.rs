//! Tempo-lane baking tests (02-domain-spec.md §Project 与时间轴,
//! 03-audio-runtime-spec.md §Tempo automation 烘焙): when a tempo lane
//! exists it replaces the tempoMap as the effective clock, and scheduled
//! events land on the baked table's frames.

mod common;

use common::*;
use oxitone_core::wire::{
    AutomationLaneSpec, AutomationSourceSpec, AutomationTarget, CurveKind, WaveKind,
};
use oxitone_core::Beat;
use oxitone_render::{builtin_registry, SampleStore};

fn tempo_lane(id: &str, project: &str, source: AutomationSourceSpec) -> AutomationLaneSpec {
    AutomationLaneSpec {
        playback: None,
        id: id.into(),
        target: AutomationTarget {
            scope: None,
            entity_id: project.into(),
            parameter_id: "tempo".into(),
        },
        source,
        combine: None,
        loop_spec: None,
        last_beat: None,
    }
}

/// One note per beat over 8 beats; tempoMap is constant 120 (ignored when
/// the lane exists).
fn lane_snapshot(source: AutomationSourceSpec) -> oxitone_core::wire::ProjectSnapshot {
    let mut snapshot = base_snapshot();
    snapshot.tracks = vec![track("trk_notes", &["chn_notes"], &["pcl_notes"], &[])];
    snapshot.patterns = vec![pattern(
        "pat_notes",
        (4, 1),
        (0..4).map(|b| note(72, (b, 1), (1, 8), 0.8)).collect(),
    )];
    snapshot.pattern_clips = vec![pattern_clip(
        "pcl_notes",
        "pat_notes",
        "trk_notes",
        (0, 1),
        (8, 1),
    )];
    snapshot.channels = vec![channel(
        "chn_notes",
        "mix_notes",
        wavetable_ref(&[]),
        vec![],
    )];
    snapshot.mixer_channels = vec![mixer_channel("mix_notes", vec![], vec![])];
    snapshot.automation = vec![tempo_lane("auto_tempo", &snapshot.id.clone(), source)];
    snapshot
}

#[test]
fn constant_lane_replaces_tempo_map() {
    // Lane constant 0.5 → bpm = 20 * (999/20)^0.5 ≈ 141.3; the static 120
    // BPM tempoMap must be ignored.
    let snapshot = lane_snapshot(AutomationSourceSpec::Constant { value: 0.5 });
    let registry = builtin_registry().unwrap();
    let store = SampleStore::new(None);
    let plan = oxitone_graph::compile_plan(
        &snapshot,
        &registry,
        &store,
        &oxitone_graph::CompileOptions::default(),
    )
    .unwrap();
    let expected_bpm = 20.0 * (999.0f64 / 20.0).powf(0.5);
    let bpm = plan.tempo.bpm_at_beat(3.0);
    assert!(
        (bpm - expected_bpm).abs() < 0.5,
        "effective bpm {bpm}, expected {expected_bpm}"
    );
    // Events land on the baked table's frames (sample-exact).
    let events = plan.scheduler.events();
    let note_ons: Vec<u64> = events
        .iter()
        .filter(|e| {
            matches!(
                e.payload,
                oxitone_transport::event::EventPayload::NoteOn { .. }
            )
        })
        .map(|e| e.frame)
        .collect();
    assert_eq!(note_ons.len(), 8);
    for (k, frame) in note_ons.iter().enumerate() {
        let expected = plan.tempo.beat_to_frame(Beat::new(k as i64, 1).unwrap());
        assert_eq!(*frame, expected, "note {k} frame mismatch");
    }
    // And the frames match the baked BPM within bake-grid tolerance.
    let frame_of_beat1 = note_ons[1];
    let ideal = 60.0 / expected_bpm * 48_000.0;
    assert!(
        (frame_of_beat1 as f64 - ideal).abs() < 2.0,
        "beat 1 at {frame_of_beat1}, ideal {ideal}"
    );
}

#[test]
fn wave_lane_events_follow_baked_bpm() {
    // Sine tempo lane: events must land exactly on the baked table, and
    // the baked BPM at a probe beat must track the lane's log mapping.
    let snapshot = lane_snapshot(AutomationSourceSpec::Wave {
        wave: WaveKind::Sine,
        period_beats: beat(8, 1),
        phase: None,
        min: Some(0.2),
        max: Some(0.8),
        pulse_width: None,
    });
    let registry = builtin_registry().unwrap();
    let store = SampleStore::new(None);
    let plan = oxitone_graph::compile_plan(
        &snapshot,
        &registry,
        &store,
        &oxitone_graph::CompileOptions::default(),
    )
    .unwrap();
    let events = plan.scheduler.events();
    for event in events {
        let beat = match event.payload {
            oxitone_transport::event::EventPayload::NoteOn { .. } => event.frame,
            _ => continue,
        };
        let _ = beat;
    }
    let note_ons: Vec<u64> = events
        .iter()
        .filter(|e| {
            matches!(
                e.payload,
                oxitone_transport::event::EventPayload::NoteOn { .. }
            )
        })
        .map(|e| e.frame)
        .collect();
    for (k, frame) in note_ons.iter().enumerate() {
        assert_eq!(
            *frame,
            plan.tempo.beat_to_frame(Beat::new(k as i64, 1).unwrap()),
            "note {k} off the baked table"
        );
    }
    // BPM at beat 2: lane value = 0.5 + 0.3 * sin(pi/2 * 2/4 ... ) — just
    // assert the value lies inside the lane's mapped range.
    let bpm = plan.tempo.bpm_at_beat(2.0);
    let lo = 20.0 * (999.0f64 / 20.0).powf(0.2);
    let hi = 20.0 * (999.0f64 / 20.0).powf(0.8);
    assert!(
        bpm >= lo - 1.0 && bpm <= hi + 1.0,
        "bpm {bpm} outside lane range"
    );
}

#[test]
fn curve_lane_with_step_lands_on_exact_frames() {
    // Piecewise-step tempo via a curve lane (transport-invariant): 0.0 → 60
    // BPM at beat 0, 1.0 → 999 BPM at beat 4.
    let snapshot = lane_snapshot(AutomationSourceSpec::Curve {
        interpolation: CurveKind::Step,
        points: vec![
            oxitone_core::wire::AutomationPoint {
                beat: Beat::ZERO,
                value: 0.0,
                curve: None,
            },
            oxitone_core::wire::AutomationPoint {
                beat: beat(4, 1),
                value: 1.0,
                curve: None,
            },
        ],
    });
    let registry = builtin_registry().unwrap();
    let store = SampleStore::new(None);
    let plan = oxitone_graph::compile_plan(
        &snapshot,
        &registry,
        &store,
        &oxitone_graph::CompileOptions::default(),
    )
    .unwrap();
    let note_ons: Vec<u64> = plan
        .scheduler
        .events()
        .iter()
        .filter(|e| {
            matches!(
                e.payload,
                oxitone_transport::event::EventPayload::NoteOn { .. }
            )
        })
        .map(|e| e.frame)
        .collect();
    // Beats 0..4 at 20 BPM (lane value 0 → 20 BPM): 3 s per beat.
    for (k, &frame) in note_ons.iter().enumerate().take(4) {
        let expected = (k as u64) * 3 * 48_000;
        assert!(
            (frame as i64 - expected as i64).abs() <= 1,
            "beat {k}: {frame} vs {expected}"
        );
    }
    // Beat 4 lands on the exact 999-BPM grid from the discontinuity.
    let beat4 = note_ons[4];
    assert_eq!(beat4, 4 * 3 * 48_000, "step boundary must be sample-exact");
    let bpm_after = plan.tempo.bpm_at_frame(beat4 + 480);
    assert!(
        (bpm_after - 999.0).abs() < 1.0,
        "bpm after step {bpm_after}"
    );
}
