use super::*;
use oxitone_transport::{ClipSource, EventPayload, Scheduler, TempoMap};

fn seconds_at(tempos: &[(u64, u32)], tick: u64, ppq: u16) -> f64 {
    let (mut cursor, mut micros, mut seconds) = (0, 500_000, 0.0);
    for &(next, value) in tempos.iter().filter(|(t, _)| *t <= tick) {
        seconds += (next - cursor) as f64 * f64::from(micros) / (1e6 * f64::from(ppq));
        cursor = next;
        micros = value;
    }
    seconds + (tick - cursor) as f64 * f64::from(micros) / (1e6 * f64::from(ppq))
}

#[test]
fn composite_midi_uses_the_root_period_for_shorter_parts() {
    let mut s = base_snapshot();
    s.patterns = vec![
        pattern("pat_short", (4, 1), vec![note(60, (1, 1), (1, 1), 0.7)]),
        pattern("pat_root", (8, 1), vec![]),
    ];
    s.patterns[1].parts = Some(vec![oxitone_core::wire::PatternPartSpec {
        channel_id: "chn_a".into(),
        pattern_id: "pat_short".into(),
    }]);
    let mut c = clip("pcl_a", "pat_root", "trk_a");
    c.loop_count = Some(2);
    s.pattern_clips.push(c);
    s.tracks.push(track("trk_a", &["pcl_a"], None));
    let result = export_midi(&s, &MidiExportOptions::default()).unwrap();
    let parsed = parse_smf(&result.bytes);
    let ons: Vec<_> = note_events(&parsed.tracks[1])
        .into_iter()
        .filter(|e| e.data[0] & 0xF0 == 0x90 && e.data[2] > 0)
        .map(|e| e.tick)
        .collect();
    assert_eq!(
        ons,
        vec![u64::from(DEFAULT_PPQ), u64::from(DEFAULT_PPQ) * 9]
    );
}

#[test]
fn independent_track_audio_and_midi_match_across_effective_clocks() {
    for mode in 0..4 {
        for bounded in [false, true] {
            let mut s = base_snapshot();
            s.tempo_map[0].curve = Some(match mode {
                1 => TempoCurve::Linear,
                2 => TempoCurve::Exponential,
                _ => TempoCurve::Step,
            });
            s.tempo_map.push(TempoSegment {
                start_beat: beat(4, 1),
                bpm: 240.0,
                curve: None,
            });
            if mode == 3 {
                s.automation.push(AutomationLaneSpec {
                    playback: None,
                    id: "auto_tempo".into(),
                    target: AutomationTarget {
                        scope: None,
                        entity_id: s.id.clone(),
                        parameter_id: "tempo".into(),
                    },
                    source: AutomationSourceSpec::Gate {
                        period_beats: beat(2, 1),
                        duty: 0.5,
                        phase: None,
                        on: Some((240.0_f64 / 20.0).ln() / (999.0_f64 / 20.0).ln()),
                        off: Some((120.0_f64 / 20.0).ln() / (999.0_f64 / 20.0).ln()),
                    },
                    combine: None,
                    loop_spec: None,
                    last_beat: None,
                });
            }
            s.patterns.push(pattern(
                "pat_a",
                (2, 1),
                vec![note(60, (0, 1), (3, 2), 1.0), note(64, (1, 1), (2, 1), 0.5)],
            ));
            for id in ["a", "b"] {
                let mut c = clip(&format!("clip_{id}"), "pat_a", &format!("trk_{id}"));
                c.start_beat = beat(1, 1);
                if bounded {
                    c.last_beat = Some(beat(9, 2));
                } else {
                    c.loop_count = Some(2);
                }
                s.tracks.push(track(&c.track_id, &[&c.id], None));
                s.pattern_clips.push(c);
            }
            s.tracks[0].tempo = Some(120.0);
            let table =
                oxitone_graph::compile::effective_tempo_table(&s, 48000, s.seed, 0.0).unwrap();
            let tempo = TempoMap::compile(&table, 48000).unwrap();
            let channel = "chn_test".to_string();
            let sources: Vec<_> = s
                .pattern_clips
                .iter()
                .zip(&s.tracks)
                .map(|(c, t)| ClipSource {
                    clip: c,
                    pattern: &s.patterns[0],
                    period: s.patterns[0].length_beats,
                    channel_id: &channel,
                    swing: 0.0,
                    track_tempo: t.tempo,
                })
                .collect();
            let scheduler = Scheduler::compile(&sources, &tempo, s.seed).unwrap();
            let local_ons: Vec<_> = scheduler
                .events()
                .iter()
                .filter(|e| {
                    e.track_id == "trk_a" && matches!(e.payload, EventPayload::NoteOn { .. })
                })
                .map(|e| e.frame)
                .collect();
            assert_eq!(local_ons, vec![24000, 48000, 72000, 96000]);
            let global_ons: Vec<_> = scheduler
                .events()
                .iter()
                .filter(|e| {
                    e.track_id == "trk_b" && matches!(e.payload, EventPayload::NoteOn { .. })
                })
                .map(|e| e.frame)
                .collect();
            assert_eq!(
                global_ons,
                (1..=4)
                    .map(|b| tempo.beat_to_frame(beat(b, 1)))
                    .collect::<Vec<_>>()
            );
            let options = MidiExportOptions {
                ppq: Some(9600),
                tempo_event_resolution_ticks: Some(1),
                path: None,
            };
            let exported = export_midi(&s, &options).unwrap();
            assert_eq!(exported.bytes, export_midi(&s, &options).unwrap().bytes);
            let parsed = parse_smf(&exported.bytes);
            let tempos = tempo_events(&parsed.tracks[0]);
            for (i, track) in s.tracks.iter().enumerate() {
                let audio: Vec<_> = scheduler
                    .events()
                    .iter()
                    .filter(|e| e.track_id == track.id)
                    .collect();
                let midi = note_events(&parsed.tracks[i + 1]);
                assert_eq!(audio.len(), midi.len());
                for (a, m) in audio.iter().zip(midi) {
                    let seconds = seconds_at(&tempos, m.tick, parsed.division);
                    assert!(
                        (seconds * 48000.0 - a.frame as f64).abs() <= 12.0,
                        "mode {mode}, bounded {bounded}, MIDI {seconds}s vs frame {}",
                        a.frame
                    );
                }
            }
        }
    }
}
