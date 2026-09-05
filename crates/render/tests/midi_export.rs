//! Golden tests for SMF Type 1 export (`.agents/docs/06-format-and-export.md`
//! §MIDI 导出): determinism, tempo resampling, channel allocation, same-tick
//! ordering, and probability parity with the transport scheduler's seed
//! derivation.

use oxitone_core::beat::Beat;
use oxitone_core::pcg32::{hash64, Hash64Part, Pcg32};
use oxitone_core::wire::{
    AutomationLaneSpec, AutomationSourceSpec, AutomationTarget, MarkerSpec, NoteSpec,
    PatternClipSpec, PatternSpec, ProjectSnapshot, TempoCurve, TempoSegment, TimeSignatureSegment,
    TrackSpec,
};
use oxitone_core::{codes, PROTOCOL_VERSION};
use oxitone_render::midi::{export_midi, ChannelSource, MidiExportOptions, DEFAULT_PPQ};
use sha2::{Digest, Sha256};

#[path = "midi_export/track_tempo.rs"]
mod track_tempo;

fn beat(n: i64, d: u32) -> Beat {
    Beat::new(n, d).unwrap()
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

fn base_snapshot() -> ProjectSnapshot {
    ProjectSnapshot {
        protocol_version: PROTOCOL_VERSION.to_string(),
        revision: 1,
        id: "prj_midi".into(),
        name: None,
        sample_rate: 48_000,
        block_size: 128,
        seed: 7,
        tempo_map: vec![TempoSegment {
            start_beat: Beat::ZERO,
            bpm: 120.0,
            curve: None,
        }],
        time_signature_map: vec![TimeSignatureSegment {
            start_bar: 1,
            numerator: 4,
            denominator: 4,
        }],
        markers: vec![],
        tracks: vec![],
        patterns: vec![],
        pattern_clips: vec![],
        sample_clips: vec![],
        samples: vec![],
        channels: vec![],
        mixer_channels: vec![],
        automation: vec![],
    }
}

fn track(id: &str, clip_ids: &[&str], midi_channel: Option<u8>) -> TrackSpec {
    TrackSpec {
        id: id.into(),
        name: None,
        channel_ids: vec![],
        tempo: None,
        pattern_clip_ids: clip_ids.iter().map(|s| s.to_string()).collect(),
        sample_clip_ids: vec![],
        enabled: None,
        midi_channel,
    }
}

fn clip(id: &str, pattern_id: &str, track_id: &str) -> PatternClipSpec {
    PatternClipSpec {
        id: id.into(),
        pattern_id: pattern_id.into(),
        track_id: track_id.into(),
        start_beat: Beat::ZERO,
        duration_beats: None,
        loop_count: None,
        last_beat: None,
        transpose: None,
        velocity_scale: None,
        probability: None,
        enabled: None,
    }
}

fn pattern(id: &str, length: (i64, u32), notes: Vec<NoteSpec>) -> PatternSpec {
    PatternSpec {
        id: id.into(),
        name: None,
        length_beats: beat(length.0, length.1),
        notes,
    }
}

// --- Minimal SMF parser for assertions -------------------------------------

struct ParsedEvent {
    tick: u64,
    data: Vec<u8>,
}

struct ParsedSmf {
    format: u16,
    track_count: u16,
    division: u16,
    tracks: Vec<Vec<ParsedEvent>>,
}

fn read_varlen(bytes: &[u8], pos: &mut usize) -> u64 {
    let mut value = 0_u64;
    loop {
        let b = bytes[*pos];
        *pos += 1;
        value = (value << 7) | u64::from(b & 0x7F);
        if b & 0x80 == 0 {
            return value;
        }
    }
}

fn parse_smf(bytes: &[u8]) -> ParsedSmf {
    assert_eq!(&bytes[0..4], b"MThd");
    assert_eq!(u32::from_be_bytes(bytes[4..8].try_into().unwrap()), 6);
    let format = u16::from_be_bytes(bytes[8..10].try_into().unwrap());
    let track_count = u16::from_be_bytes(bytes[10..12].try_into().unwrap());
    let division = u16::from_be_bytes(bytes[12..14].try_into().unwrap());
    let mut pos = 14_usize;
    let mut tracks = Vec::new();
    for _ in 0..track_count {
        assert_eq!(&bytes[pos..pos + 4], b"MTrk");
        let len = u32::from_be_bytes(bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
        pos += 8;
        let end = pos + len;
        let mut events = Vec::new();
        let mut tick = 0_u64;
        while pos < end {
            tick += read_varlen(bytes, &mut pos);
            let status = bytes[pos];
            let data = if status == 0xFF {
                let kind = bytes[pos + 1];
                let mut p = pos + 2;
                let len = read_varlen(bytes, &mut p) as usize;
                let payload = bytes[p..p + len].to_vec();
                pos = p + len;
                let mut d = vec![0xFF, kind];
                d.extend_from_slice(&payload);
                d
            } else {
                let n = if matches!(status & 0xF0, 0xC0 | 0xD0) {
                    1
                } else {
                    2
                };
                let d = bytes[pos..pos + 1 + n].to_vec();
                pos += 1 + n;
                d
            };
            events.push(ParsedEvent { tick, data });
        }
        tracks.push(events);
    }
    ParsedSmf {
        format,
        track_count,
        division,
        tracks,
    }
}

fn tempo_events(track: &[ParsedEvent]) -> Vec<(u64, u32)> {
    track
        .iter()
        .filter(|e| e.data.len() >= 5 && e.data[0] == 0xFF && e.data[1] == 0x51)
        .map(|e| {
            (
                e.tick,
                (u32::from(e.data[2]) << 16) | (u32::from(e.data[3]) << 8) | u32::from(e.data[4]),
            )
        })
        .collect()
}

fn note_events(track: &[ParsedEvent]) -> Vec<&ParsedEvent> {
    track
        .iter()
        .filter(|e| matches!(e.data[0] & 0xF0, 0x80 | 0x90))
        .collect()
}

fn demo_snapshot() -> ProjectSnapshot {
    let mut s = base_snapshot();
    s.tempo_map = vec![
        TempoSegment {
            start_beat: Beat::ZERO,
            bpm: 120.0,
            curve: Some(TempoCurve::Linear),
        },
        TempoSegment {
            start_beat: beat(4, 1),
            bpm: 180.0,
            curve: None,
        },
    ];
    s.patterns = vec![pattern(
        "pat_a",
        (4, 1),
        vec![
            note(60, (0, 1), (1, 1), 1.0),
            note(62, (1, 1), (1, 1), 0.5),
            note(64, (7, 2), (1, 2), 0.75),
        ],
    )];
    s.pattern_clips = vec![PatternClipSpec {
        loop_count: Some(2),
        probability: Some(0.75),
        ..clip("clip_a", "pat_a", "trk_a")
    }];
    s.tracks = vec![track("trk_a", &["clip_a"], None)];
    s
}

// --- Tests ------------------------------------------------------------------

#[test]
fn export_is_deterministic() {
    let snapshot = demo_snapshot();
    let options = MidiExportOptions::default();
    let first = export_midi(&snapshot, &options).unwrap();
    let second = export_midi(&snapshot, &options).unwrap();
    assert_eq!(first.bytes, second.bytes);
    let hash = Sha256::digest(&first.bytes);
    assert_eq!(
        hash.iter().map(|b| format!("{b:02x}")).collect::<String>(),
        "f4d56369ba95658493e4c77c244c3140fd1969ebb4ce0779a8d4e0a546ce9c95"
    );
}

#[test]
fn header_and_step_tempo_golden() {
    let mut snapshot = base_snapshot();
    snapshot.patterns = vec![pattern(
        "pat_a",
        (4, 1),
        vec![note(60, (0, 1), (1, 1), 1.0)],
    )];
    snapshot.pattern_clips = vec![clip("clip_a", "pat_a", "trk_a")];
    snapshot.tracks = vec![track("trk_a", &["clip_a"], None)];
    let result = export_midi(&snapshot, &MidiExportOptions::default()).unwrap();
    let parsed = parse_smf(&result.bytes);
    assert_eq!(parsed.format, 1);
    assert_eq!(parsed.track_count, 2);
    assert_eq!(parsed.division, DEFAULT_PPQ);
    // Conductor: track name, then time signature 4/4 at tick 0, then tempo
    // 120 BPM = 500000 us at tick 0, then end of track.
    let conductor = &parsed.tracks[0];
    assert_eq!(
        conductor[0].data,
        [vec![0xFF, 0x03], b"conductor".to_vec()].concat()
    );
    let ts = conductor
        .iter()
        .find(|e| e.data.len() >= 2 && e.data[1] == 0x58)
        .unwrap();
    assert_eq!(ts.tick, 0);
    assert_eq!(ts.data, vec![0xFF, 0x58, 4, 2, 24, 8]);
    assert_eq!(tempo_events(conductor), vec![(0, 500_000)]);
    assert_eq!(result.diagnostics.tempo_event_count, 1);
}

#[test]
fn linear_tempo_resampling_golden() {
    let snapshot = demo_snapshot();
    let result = export_midi(&snapshot, &MidiExportOptions::default()).unwrap();
    assert_eq!(result.diagnostics.tempo_event_resolution_ticks, 120);
    let parsed = parse_smf(&result.bytes);
    let tempos = tempo_events(&parsed.tracks[0]);
    // Linear 120 -> 180 BPM over beats 0..4 resampled every 120 ticks:
    // ticks 0, 120, ..., 3720, then the step to 180 BPM at tick 3840.
    assert_eq!(tempos.len(), 33);
    for (i, (tick, _)) in tempos.iter().enumerate().take(32) {
        assert_eq!(*tick, (i as u64) * 120);
    }
    assert_eq!(tempos[0].1, 500_000); // 120 BPM
    assert_eq!(tempos[16], (1920, 400_000)); // beat 2 -> 150 BPM
    assert_eq!(tempos[31].1, 336_842); // beat 3.875 -> 178.125 BPM
    assert_eq!(tempos[32], (3840, 333_333)); // step to 180 BPM
    let micros: Vec<u32> = tempos.iter().map(|(_, m)| *m).collect();
    assert!(micros.windows(2).all(|w| w[0] > w[1]));
}

#[test]
fn exponential_tempo_resampling_golden() {
    let mut snapshot = base_snapshot();
    snapshot.tempo_map = vec![
        TempoSegment {
            start_beat: Beat::ZERO,
            bpm: 120.0,
            curve: Some(TempoCurve::Exponential),
        },
        TempoSegment {
            start_beat: beat(4, 1),
            bpm: 240.0,
            curve: None,
        },
    ];
    let result = export_midi(&snapshot, &MidiExportOptions::default()).unwrap();
    let parsed = parse_smf(&result.bytes);
    let tempos = tempo_events(&parsed.tracks[0]);
    assert_eq!(tempos.len(), 33);
    // Beat 2: bpm = 120 * exp(0.5 * ln 2) = 120 * sqrt(2).
    let bpm = 120.0 * 2.0_f64.sqrt();
    let expect = ((60_000_000.0 / bpm) + 0.5).floor() as u32;
    assert_eq!(tempos[16], (1920, expect));
    assert_eq!(tempos[32], (3840, 250_000)); // 240 BPM
}

#[test]
fn tempo_resolution_override_is_recorded() {
    let snapshot = demo_snapshot();
    let options = MidiExportOptions {
        path: None,
        ppq: Some(480),
        tempo_event_resolution_ticks: Some(240),
    };
    let result = export_midi(&snapshot, &options).unwrap();
    assert_eq!(result.diagnostics.ppq, 480);
    assert_eq!(result.diagnostics.tempo_event_resolution_ticks, 240);
    let parsed = parse_smf(&result.bytes);
    assert_eq!(parsed.division, 480);
    let tempos = tempo_events(&parsed.tracks[0]);
    // Beats 0..4 at 480 PPQ -> end tick 1920; sampled at 0, 240, ..., 1680,
    // then the step event at 1920.
    assert_eq!(tempos.len(), 9);
    assert_eq!(tempos[8], (1920, 333_333));
}

#[test]
fn auto_channel_assignment_sorted_by_track_id() {
    let mut snapshot = base_snapshot();
    snapshot.patterns = vec![pattern(
        "pat_a",
        (4, 1),
        vec![note(60, (0, 1), (1, 1), 1.0)],
    )];
    snapshot.pattern_clips = vec![
        clip("clip_a", "pat_a", "trk_b"),
        clip("clip_b", "pat_a", "trk_a"),
    ];
    snapshot.tracks = vec![
        track("trk_b", &["clip_a"], None),
        track("trk_a", &["clip_b"], None),
    ];
    let result = export_midi(&snapshot, &MidiExportOptions::default()).unwrap();
    let assignments = &result.diagnostics.channel_assignments;
    assert_eq!(assignments.len(), 2);
    assert_eq!(assignments[0].track_id, "trk_a");
    assert_eq!(assignments[0].channel, 1);
    assert_eq!(assignments[0].source, ChannelSource::Auto);
    assert_eq!(assignments[1].track_id, "trk_b");
    assert_eq!(assignments[1].channel, 2);
    // SMF tracks follow the same stable order; trk_a events use channel 0.
    let parsed = parse_smf(&result.bytes);
    assert_eq!(parsed.track_count, 3);
    let on = note_events(&parsed.tracks[1])
        .into_iter()
        .find(|e| e.data[0] & 0xF0 == 0x90)
        .unwrap();
    assert_eq!(on.data[0], 0x90);
}

#[test]
fn explicit_channel_wins_and_auto_skips_it() {
    let mut snapshot = base_snapshot();
    snapshot.patterns = vec![pattern(
        "pat_a",
        (4, 1),
        vec![note(60, (0, 1), (1, 1), 1.0)],
    )];
    snapshot.pattern_clips = vec![
        clip("clip_a", "pat_a", "trk_a"),
        clip("clip_b", "pat_a", "trk_b"),
    ];
    snapshot.tracks = vec![
        track("trk_a", &["clip_a"], Some(5)),
        track("trk_b", &["clip_b"], None),
    ];
    let result = export_midi(&snapshot, &MidiExportOptions::default()).unwrap();
    let assignments = &result.diagnostics.channel_assignments;
    assert_eq!(assignments[0].channel, 5);
    assert_eq!(assignments[0].source, ChannelSource::Explicit);
    assert_eq!(assignments[1].channel, 1);
    assert_eq!(assignments[1].source, ChannelSource::Auto);
}

#[test]
fn channel_limit_lists_all_unassigned_tracks() {
    let mut snapshot = base_snapshot();
    snapshot.patterns = vec![pattern(
        "pat_a",
        (4, 1),
        vec![note(60, (0, 1), (1, 1), 1.0)],
    )];
    let ids: Vec<String> = (0..17).map(|i| format!("trk_{i:02}")).collect();
    // Leave trk_03 without an explicit channel among otherwise explicit
    // tracks: it alone must be reported.
    for (i, id) in ids.iter().enumerate() {
        snapshot.pattern_clips.push(PatternClipSpec {
            id: format!("clip_{i:02}"),
            ..clip(&format!("clip_{i:02}"), "pat_a", id)
        });
        snapshot.tracks.push(track(
            id,
            &[&format!("clip_{i:02}")],
            if i == 3 {
                None
            } else {
                Some(((i % 8) + 1) as u8)
            },
        ));
    }
    let err = export_midi(&snapshot, &MidiExportOptions::default()).unwrap_err();
    assert_eq!(err.error.code, codes::MIDI_CHANNEL_LIMIT);
    assert_eq!(err.unassigned_track_ids, vec!["trk_03".to_string()]);

    // All 17 without explicit channels: every track ID is listed, sorted.
    for t in &mut snapshot.tracks {
        t.midi_channel = None;
    }
    let err = export_midi(&snapshot, &MidiExportOptions::default()).unwrap_err();
    assert_eq!(err.error.code, codes::MIDI_CHANNEL_LIMIT);
    assert_eq!(err.unassigned_track_ids, ids);
}

#[test]
fn explicit_shared_channels_allow_more_than_16_tracks() {
    let mut snapshot = base_snapshot();
    snapshot.patterns = vec![pattern(
        "pat_a",
        (4, 1),
        vec![note(60, (0, 1), (1, 1), 1.0)],
    )];
    for i in 0..17 {
        let id = format!("trk_{i:02}");
        let clip_id = format!("clip_{i:02}");
        snapshot.pattern_clips.push(clip(&clip_id, "pat_a", &id));
        snapshot.tracks.push(track(&id, &[&clip_id], Some(1)));
    }
    let result = export_midi(&snapshot, &MidiExportOptions::default()).unwrap();
    assert_eq!(result.diagnostics.note_track_count, 17);
    let parsed = parse_smf(&result.bytes);
    assert_eq!(parsed.track_count, 18);
}

#[test]
fn same_tick_note_off_precedes_note_on() {
    let mut snapshot = base_snapshot();
    snapshot.patterns = vec![pattern(
        "pat_a",
        (4, 1),
        vec![note(60, (0, 1), (1, 1), 1.0), note(64, (1, 1), (1, 1), 1.0)],
    )];
    snapshot.pattern_clips = vec![clip("clip_a", "pat_a", "trk_a")];
    snapshot.tracks = vec![track("trk_a", &["clip_a"], None)];
    let result = export_midi(&snapshot, &MidiExportOptions::default()).unwrap();
    let parsed = parse_smf(&result.bytes);
    let notes = note_events(&parsed.tracks[1]);
    // Off of the first note and on of the second both land on tick 960;
    // the off must come first.
    let at_tick: Vec<&ParsedEvent> = notes.iter().filter(|e| e.tick == 960).copied().collect();
    assert_eq!(at_tick.len(), 2);
    assert_eq!(at_tick[0].data[0], 0x80);
    assert_eq!(at_tick[0].data[1], 60);
    assert_eq!(at_tick[1].data[0], 0x90);
    assert_eq!(at_tick[1].data[1], 64);
}

#[test]
fn velocity_maps_into_1_to_127() {
    let mut snapshot = base_snapshot();
    let mut quiet = note(60, (0, 1), (1, 1), 0.0);
    quiet.off_velocity = Some(0.0);
    let mut half = note(62, (1, 1), (1, 1), 0.5);
    half.off_velocity = Some(1.0);
    snapshot.patterns = vec![pattern(
        "pat_a",
        (4, 1),
        vec![quiet, half, note(64, (2, 1), (1, 1), 1.0)],
    )];
    snapshot.pattern_clips = vec![clip("clip_a", "pat_a", "trk_a")];
    snapshot.tracks = vec![track("trk_a", &["clip_a"], None)];
    let result = export_midi(&snapshot, &MidiExportOptions::default()).unwrap();
    let parsed = parse_smf(&result.bytes);
    let notes = note_events(&parsed.tracks[1]);
    let on_at = |tick: u64| {
        notes
            .iter()
            .find(|e| e.tick == tick && e.data[0] == 0x90)
            .unwrap()
            .data[2]
    };
    let off_at = |tick: u64, pitch: u8| {
        notes
            .iter()
            .find(|e| e.tick == tick && e.data[0] == 0x80 && e.data[1] == pitch)
            .unwrap()
            .data[2]
    };
    assert_eq!(on_at(0), 1); // 0.0 -> 1
    assert_eq!(on_at(960), 64); // 0.5 -> 1 + round(63) = 64
    assert_eq!(on_at(1920), 127); // 1.0 -> 127
    assert_eq!(off_at(960, 60), 1);
    assert_eq!(off_at(1920, 62), 127);
}

#[test]
fn probability_matches_scheduler_seed_derivation() {
    let seed = 7_u64;
    let mut snapshot = base_snapshot();
    snapshot.seed = seed;
    let notes: Vec<NoteSpec> = (0..4)
        .map(|i| note(60 + i as u8, (i, 1), (1, 2), 1.0))
        .collect();
    snapshot.patterns = vec![pattern("pat_a", (4, 1), notes.clone())];
    snapshot.pattern_clips = vec![PatternClipSpec {
        loop_count: Some(3),
        probability: Some(0.5),
        ..clip("clip_a", "pat_a", "trk_a")
    }];
    snapshot.tracks = vec![track("trk_a", &["clip_a"], None)];

    let mut expected_on_ticks = Vec::new();
    for iteration in 0..3_u64 {
        for (ni, note) in notes.iter().enumerate() {
            let draw_seed = hash64(&[
                Hash64Part::Int(seed),
                Hash64Part::Str("clip_a".to_string()),
                Hash64Part::Int(iteration),
                Hash64Part::Int(ni as u64),
            ]);
            if Pcg32::new(draw_seed).next_f64() < 0.5 {
                expected_on_ticks.push((iteration * 4 + note.start.numerator() as u64) * 960);
            }
        }
    }
    expected_on_ticks.sort_unstable();

    let result = export_midi(&snapshot, &MidiExportOptions::default()).unwrap();
    let parsed = parse_smf(&result.bytes);
    let mut on_ticks: Vec<u64> = note_events(&parsed.tracks[1])
        .iter()
        .filter(|e| e.data[0] == 0x90)
        .map(|e| e.tick)
        .collect();
    on_ticks.sort_unstable();
    assert_eq!(on_ticks, expected_on_ticks);
    assert!(!on_ticks.is_empty());
    assert!(on_ticks.len() < 12);
}

#[test]
fn loop_count_and_last_beat_expansion() {
    let mut snapshot = base_snapshot();
    snapshot.patterns = vec![pattern(
        "pat_a",
        (4, 1),
        vec![note(60, (0, 1), (1, 1), 1.0), note(62, (2, 1), (1, 1), 1.0)],
    )];
    snapshot.pattern_clips = vec![
        // loopCount 2 -> notes at beats 0, 2, 4, 6.
        PatternClipSpec {
            loop_count: Some(2),
            ..clip("clip_a", "pat_a", "trk_a")
        },
        // lastBeat 5 (exclusive) on a second track: iteration at beat 4 is
        // inside, but its note at beat 6 is clipped.
        PatternClipSpec {
            last_beat: Some(beat(5, 1)),
            ..clip("clip_b", "pat_a", "trk_b")
        },
    ];
    snapshot.tracks = vec![
        track("trk_a", &["clip_a"], None),
        track("trk_b", &["clip_b"], None),
    ];
    let result = export_midi(&snapshot, &MidiExportOptions::default()).unwrap();
    let parsed = parse_smf(&result.bytes);
    let on_ticks = |track: usize| -> Vec<u64> {
        note_events(&parsed.tracks[track])
            .iter()
            .filter(|e| e.data[0] & 0xF0 == 0x90)
            .map(|e| e.tick)
            .collect()
    };
    assert_eq!(on_ticks(1), vec![0, 1920, 3840, 5760]);
    assert_eq!(on_ticks(2), vec![0, 1920, 3840]);
}

#[test]
fn skipped_automation_and_markers_are_reported() {
    let mut snapshot = demo_snapshot();
    snapshot.markers = vec![MarkerSpec {
        id: "mrk_a".into(),
        name: Some("Verse".into()),
        start_beat: beat(4, 1),
    }];
    snapshot.automation = vec![AutomationLaneSpec {
        id: "lane_a".into(),
        target: AutomationTarget {
            entity_id: "ch_a".into(),
            parameter_id: "level".into(),
        },
        source: AutomationSourceSpec::Constant { value: 0.5 },
        combine: None,
        loop_spec: None,
        last_beat: None,
    }];
    let result = export_midi(&snapshot, &MidiExportOptions::default()).unwrap();
    assert_eq!(result.diagnostics.skipped_automation.len(), 1);
    let skipped = &result.diagnostics.skipped_automation[0];
    assert_eq!(skipped.lane_id, "lane_a");
    assert_eq!(skipped.target_parameter_id, "level");
    let parsed = parse_smf(&result.bytes);
    let marker = parsed.tracks[0]
        .iter()
        .find(|e| e.data.len() >= 2 && e.data[1] == 0x06)
        .unwrap();
    assert_eq!(marker.tick, 3840);
    assert_eq!(&marker.data[2..], b"Verse");
}

#[test]
fn invalid_ppq_is_rejected() {
    let snapshot = demo_snapshot();
    let options = MidiExportOptions {
        path: None,
        ppq: Some(0),
        tempo_event_resolution_ticks: None,
    };
    let err = export_midi(&snapshot, &options).unwrap_err();
    assert_eq!(err.error.code, codes::INVALID_PROJECT);
}
