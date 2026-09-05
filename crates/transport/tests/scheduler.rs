use oxitone_core::beat::Beat;
use oxitone_core::error::codes;
use oxitone_core::wire::{NoteSpec, PatternClipSpec, PatternSpec};
use oxitone_transport::{ClipSource, EventPayload, EventPriority, Scheduler, TempoMap};

const SR: u32 = 48_000;
const FRAMES_PER_BEAT: u64 = 24_000; // 120 BPM @ 48 kHz

fn beat(n: i64, d: u32) -> Beat {
    Beat::new(n, d).unwrap()
}

fn tempo() -> oxitone_transport::CompiledTempoMap {
    TempoMap::compile(
        &[oxitone_core::wire::TempoSegment {
            start_beat: Beat::ZERO,
            bpm: 120.0,
            curve: None,
        }],
        SR,
    )
    .unwrap()
}

fn note(pitch: u8, start: (i64, u32), duration: (i64, u32), velocity: f64) -> NoteSpec {
    NoteSpec {
        id: None,
        pitch,
        start: beat(start.0, start.1),
        duration: beat(duration.0, duration.1),
        velocity,
        off_velocity: None,
        chance: None,
        voice: None,
        tags: None,
    }
}

fn pattern(length: (i64, u32), notes: Vec<NoteSpec>) -> PatternSpec {
    PatternSpec {
        id: "pat1".to_string(),
        name: None,
        length_beats: beat(length.0, length.1),
        notes,
    }
}

fn clip(start: (i64, u32)) -> PatternClipSpec {
    PatternClipSpec {
        id: "clip1".to_string(),
        pattern_id: "pat1".to_string(),
        track_id: "track1".to_string(),
        start_beat: beat(start.0, start.1),
        duration_beats: None,
        loop_count: None,
        last_beat: None,
        transpose: None,
        velocity_scale: None,
        probability: None,
        enabled: None,
    }
}

fn chan1() -> &'static String {
    static CHAN: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    CHAN.get_or_init(|| "chan1".to_string())
}

fn source<'a>(clip: &'a PatternClipSpec, pattern: &'a PatternSpec, swing: f64) -> ClipSource<'a> {
    ClipSource {
        clip,
        pattern,
        channel_id: chan1(),
        swing,
        track_tempo: None,
    }
}

fn on_frames(sched: &Scheduler) -> Vec<u64> {
    sched
        .events()
        .iter()
        .filter(|e| matches!(e.payload, EventPayload::NoteOn { .. }))
        .map(|e| e.frame)
        .collect()
}

#[test]
fn one_shot_clip_schedules_on_and_off() {
    let tempo = tempo();
    let pat = pattern(
        (4, 1),
        vec![note(60, (0, 1), (1, 1), 0.8), note(64, (2, 1), (1, 1), 0.5)],
    );
    let cl = clip((1, 1));
    let sched = Scheduler::compile(&[source(&cl, &pat, 0.0)], &tempo, 7).unwrap();
    let events = sched.events();
    assert_eq!(events.len(), 4);
    let kinds: Vec<(u64, EventPriority, u64)> = events
        .iter()
        .map(|e| (e.frame, e.priority, e.sequence))
        .collect();
    assert_eq!(
        kinds,
        vec![
            (24_000, EventPriority::NoteOn, 0),
            (48_000, EventPriority::NoteOff, 0),
            (72_000, EventPriority::NoteOn, 1),
            (96_000, EventPriority::NoteOff, 1),
        ]
    );
    match events[0].payload {
        EventPayload::NoteOn { pitch, velocity } => {
            assert_eq!(pitch, 60);
            assert!((velocity - 0.8).abs() < 1e-12);
        }
        _ => panic!("expected note on"),
    }
    assert_eq!(events[0].track_id, "track1");
    assert_eq!(events[0].channel_id, "chan1");
}

#[test]
fn loop_count_repeats_at_pattern_boundaries() {
    let tempo = tempo();
    let pat = pattern((4, 1), vec![note(60, (0, 1), (1, 1), 0.8)]);
    let mut cl = clip((1, 1));
    cl.loop_count = Some(2);
    let sched = Scheduler::compile(&[source(&cl, &pat, 0.0)], &tempo, 7).unwrap();
    assert_eq!(on_frames(&sched), vec![24_000, 120_000]);
}

#[test]
fn last_beat_is_exclusive_and_cuts_notes() {
    let tempo = tempo();
    let pat = pattern(
        (4, 1),
        vec![note(60, (0, 1), (1, 1), 0.8), note(64, (7, 2), (1, 2), 0.8)],
    );
    let mut cl = clip((0, 1));
    cl.last_beat = Some(beat(5, 1));
    let sched = Scheduler::compile(&[source(&cl, &pat, 0.0)], &tempo, 7).unwrap();
    // Iteration 0: beats 0 and 3.5; iteration 1: beat 4 kept, 7.5 cut.
    assert_eq!(on_frames(&sched), vec![0, 84_000, 96_000]);
}

#[test]
fn duration_beats_truncates_clip() {
    let tempo = tempo();
    let pat = pattern(
        (4, 1),
        vec![
            note(60, (0, 1), (1, 1), 0.8),
            note(62, (1, 1), (1, 1), 0.8),
            note(64, (2, 1), (1, 1), 0.8),
        ],
    );
    let mut cl = clip((0, 1));
    cl.duration_beats = Some(beat(2, 1));
    let sched = Scheduler::compile(&[source(&cl, &pat, 0.0)], &tempo, 7).unwrap();
    assert_eq!(on_frames(&sched), vec![0, 24_000]);
}

#[test]
fn end_spec_kinds_are_mutually_exclusive() {
    let tempo = tempo();
    let pat = pattern((4, 1), vec![note(60, (0, 1), (1, 1), 0.8)]);
    for cl in [
        PatternClipSpec {
            duration_beats: Some(beat(2, 1)),
            loop_count: Some(2),
            ..clip((0, 1))
        },
        PatternClipSpec {
            duration_beats: Some(beat(2, 1)),
            last_beat: Some(beat(8, 1)),
            ..clip((0, 1))
        },
        PatternClipSpec {
            loop_count: Some(2),
            last_beat: Some(beat(8, 1)),
            ..clip((0, 1))
        },
    ] {
        let err = Scheduler::compile(&[source(&cl, &pat, 0.0)], &tempo, 7).unwrap_err();
        assert_eq!(err.code, codes::INVALID_PROJECT);
    }
}

#[test]
fn disabled_clip_schedules_nothing() {
    let tempo = tempo();
    let pat = pattern((4, 1), vec![note(60, (0, 1), (1, 1), 0.8)]);
    let mut cl = clip((0, 1));
    cl.enabled = Some(false);
    let sched = Scheduler::compile(&[source(&cl, &pat, 0.0)], &tempo, 7).unwrap();
    assert!(sched.is_empty());
    assert_eq!(sched.len(), 0);
}

#[test]
fn transpose_and_velocity_scale_apply() {
    let tempo = tempo();
    let pat = pattern(
        (4, 1),
        vec![note(60, (0, 1), (1, 1), 0.5), note(62, (2, 1), (1, 1), 0.9)],
    );
    let mut cl = clip((0, 1));
    cl.transpose = Some(12);
    cl.velocity_scale = Some(1.5);
    let sched = Scheduler::compile(&[source(&cl, &pat, 0.0)], &tempo, 7).unwrap();
    let ons: Vec<(u8, f64)> = sched
        .events()
        .iter()
        .filter_map(|e| match e.payload {
            EventPayload::NoteOn { pitch, velocity } => Some((pitch, velocity)),
            _ => None,
        })
        .collect();
    assert_eq!(ons.len(), 2);
    assert_eq!(ons[0].0, 72);
    assert!((ons[0].1 - 0.75).abs() < 1e-12);
    assert_eq!(ons[1].0, 74);
    assert_eq!(ons[1].1, 1.0); // 0.9 * 1.5 clamps to 1.0
}

#[test]
fn probability_is_deterministic_per_seed() {
    let tempo = tempo();
    let notes: Vec<NoteSpec> = (0..8)
        .map(|i| note(60 + i, (i64::from(i), 8), (1, 8), 0.8))
        .collect();
    let pat = pattern((4, 1), notes);
    let mut cl = clip((0, 1));
    cl.probability = Some(0.5);
    let a = Scheduler::compile(&[source(&cl, &pat, 0.0)], &tempo, 42).unwrap();
    let b = Scheduler::compile(&[source(&cl, &pat, 0.0)], &tempo, 42).unwrap();
    assert_eq!(a.events(), b.events());

    cl.probability = Some(1.0);
    let all = Scheduler::compile(&[source(&cl, &pat, 0.0)], &tempo, 42).unwrap();
    assert_eq!(all.events().len(), 16);

    cl.probability = Some(0.0);
    let none = Scheduler::compile(&[source(&cl, &pat, 0.0)], &tempo, 42).unwrap();
    assert!(none.is_empty());
}

#[test]
fn swing_shifts_only_odd_sixteenth_grid_notes() {
    let tempo = tempo();
    let pat = pattern(
        (1, 1),
        vec![
            note(60, (0, 1), (1, 8), 0.8),
            note(61, (1, 4), (1, 8), 0.8),
            note(62, (1, 2), (1, 8), 0.8),
            note(63, (3, 4), (1, 8), 0.8),
            note(64, (1, 8), (1, 16), 0.8),
        ],
    );
    let cl = clip((0, 1));
    let sched = Scheduler::compile(&[source(&cl, &pat, 1.0)], &tempo, 7).unwrap();
    // Odd grid indices 1 and 3 shift by 0.125 beat; even and off-grid stay.
    assert_eq!(on_frames(&sched), vec![0, 3_000, 9_000, 12_000, 21_000]);

    let sched = Scheduler::compile(&[source(&cl, &pat, 0.5)], &tempo, 7).unwrap();
    assert_eq!(on_frames(&sched), vec![0, 3_000, 7_500, 12_000, 19_500]);
}

#[test]
fn note_off_sorts_before_note_on_at_same_frame() {
    let tempo = tempo();
    let pat = pattern(
        (4, 1),
        vec![note(60, (0, 1), (2, 1), 0.8), note(64, (2, 1), (1, 1), 0.8)],
    );
    let cl = clip((0, 1));
    let sched = Scheduler::compile(&[source(&cl, &pat, 0.0)], &tempo, 7).unwrap();
    let at: Vec<EventPriority> = sched
        .events_in_range(48_000, 49_000)
        .iter()
        .map(|e| e.priority)
        .collect();
    assert_eq!(at, vec![EventPriority::NoteOff, EventPriority::NoteOn]);
}

#[test]
fn same_time_events_keep_stable_sequence_order() {
    let tempo = tempo();
    let pat = pattern((4, 1), vec![note(60, (0, 1), (1, 1), 0.8)]);
    let mut cl2 = clip((0, 1));
    cl2.id = "clip2".to_string();
    cl2.track_id = "track2".to_string();
    let cl1 = clip((0, 1));
    let sched = Scheduler::compile(
        &[source(&cl1, &pat, 0.0), source(&cl2, &pat, 0.0)],
        &tempo,
        7,
    )
    .unwrap();
    let ons: Vec<&str> = sched
        .events_in_range(0, 1)
        .iter()
        .filter(|e| matches!(e.payload, EventPayload::NoteOn { .. }))
        .map(|e| e.track_id.as_str())
        .collect();
    assert_eq!(ons, vec!["track1", "track2"]);
    let mut seqs: Vec<u64> = sched.events().iter().map(|e| e.sequence).collect();
    seqs.sort_unstable();
    seqs.dedup();
    assert_eq!(seqs, vec![0, 1]); // one creation ordinal per scheduled note
}

#[test]
fn events_in_range_covers_blocks_without_gaps() {
    let tempo = tempo();
    let notes: Vec<NoteSpec> = (0..8)
        .map(|i| note(60, (i64::from(i), 1), (1, 2), 0.8))
        .collect();
    let pat = pattern((8, 1), notes);
    let cl = clip((0, 1));
    let sched = Scheduler::compile(&[source(&cl, &pat, 0.0)], &tempo, 7).unwrap();
    let mut stitched: Vec<_> = Vec::new();
    let mut start = 0;
    while start < 8 * FRAMES_PER_BEAT {
        stitched.extend_from_slice(sched.events_in_range(start, start + 6_400));
        start += 6_400;
    }
    assert_eq!(stitched, sched.events());
    // [start, end) semantics at the exact boundary.
    assert_eq!(sched.events_in_range(24_000, 48_000)[0].frame, 24_000);
    assert!(sched
        .events_in_range(0, 24_000)
        .iter()
        .all(|e| e.frame < 24_000));
    assert!(sched.events_in_range(500_000, 600_000).is_empty());
}

#[test]
fn note_and_clip_validation_errors() {
    let tempo = tempo();
    let bad_notes: Vec<NoteSpec> = vec![
        NoteSpec {
            duration: Beat::ZERO,
            ..note(60, (0, 1), (1, 1), 0.8)
        },
        note(200, (0, 1), (1, 1), 0.8),
        note(60, (0, 1), (1, 1), 1.5),
        note(60, (0, 1), (1, 1), f64::NAN),
        note(60, (4, 1), (1, 1), 0.8), // start outside pattern length
    ];
    for n in bad_notes {
        let pat = pattern((4, 1), vec![n]);
        let cl = clip((0, 1));
        let err = Scheduler::compile(&[source(&cl, &pat, 0.0)], &tempo, 7).unwrap_err();
        assert_eq!(err.code, codes::INVALID_PROJECT);
    }

    let pat = pattern((4, 1), vec![note(120, (0, 1), (1, 1), 0.8)]);
    let mut cl = clip((0, 1));
    cl.transpose = Some(12);
    let err = Scheduler::compile(&[source(&cl, &pat, 0.0)], &tempo, 7).unwrap_err();
    assert_eq!(err.code, codes::INVALID_PROJECT);

    let pat = pattern((4, 1), vec![note(60, (0, 1), (1, 1), 0.8)]);
    let mut cl = clip((0, 1));
    cl.velocity_scale = Some(2.5);
    assert!(Scheduler::compile(&[source(&cl, &pat, 0.0)], &tempo, 7).is_err());
    cl.velocity_scale = None;
    cl.probability = Some(1.5);
    assert!(Scheduler::compile(&[source(&cl, &pat, 0.0)], &tempo, 7).is_err());
    cl.probability = None;
    assert!(Scheduler::compile(&[source(&cl, &pat, 1.5)], &tempo, 7).is_err());

    let mut cl = clip((2, 1));
    cl.last_beat = Some(beat(1, 1));
    assert!(Scheduler::compile(&[source(&cl, &pat, 0.0)], &tempo, 7).is_err());
}
