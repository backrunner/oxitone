mod common;

use common::*;
use oxitone_core::codes;
use oxitone_core::wire::{
    AutomationLaneSpec, AutomationSourceSpec, AutomationTarget, EffectRef, FadeCurve, FadeSpec,
    LoopSpec, MixerChannelSpec, NormalizeSpec, NoteSpec, PatternClipSpec, PatternSpec,
    SampleClipSpec, SampleEditSpec, SampleRef, SendSpec, TempoSegment, TimeSignatureSegment,
};
use oxitone_core::wire::{ProjectSnapshot, SampleFormat};
use oxitone_graph::validate;

fn err(snapshot: &ProjectSnapshot) -> oxitone_core::OxitoneError {
    validate(snapshot, &base_registry()).unwrap_err()
}

fn note(pitch: u8, velocity: f64, duration: (i64, u32)) -> NoteSpec {
    NoteSpec {
        id: None,
        pitch,
        start: beat(0, 1),
        duration: beat(duration.0, duration.1),
        velocity,
        off_velocity: None,
        chance: None,
        voice: None,
        tags: None,
    }
}

fn pattern_with(note: NoteSpec) -> PatternSpec {
    PatternSpec {
        id: "pat_0001".to_string(),
        name: None,
        length_beats: beat(4, 1),
        notes: vec![note],
    }
}

fn pattern_clip() -> PatternClipSpec {
    PatternClipSpec {
        id: "pcl_0001".to_string(),
        pattern_id: "pat_0001".to_string(),
        track_id: "trk_0001".to_string(),
        start_beat: beat(0, 1),
        duration_beats: Some(beat(4, 1)),
        loop_count: None,
        last_beat: None,
        transpose: None,
        velocity_scale: None,
        probability: None,
        enabled: None,
    }
}

fn sample() -> SampleRef {
    SampleRef {
        id: "smp_0001".to_string(),
        asset_uri: "assets/a.wav".to_string(),
        sha256: "ab".repeat(32),
        format: SampleFormat::Wav,
        sample_rate: 48_000,
        channels: 2,
        frames: 192_000,
        edits: None,
        musical_length_beats: None,
        provenance: None,
    }
}

fn sample_clip() -> SampleClipSpec {
    SampleClipSpec {
        id: "scl_0001".to_string(),
        sample_id: "smp_0001".to_string(),
        track_id: "trk_0001".to_string(),
        start_beat: beat(0, 1),
        duration_beats: Some(beat(8, 1)),
        gain: None,
        pan: None,
        rate: None,
        loop_spec: None,
        tempo_sync: None,
        stretch_algorithm: None,
        enabled: None,
    }
}

fn lane(parameter_id: &str) -> AutomationLaneSpec {
    AutomationLaneSpec {
        id: "auto_0001".to_string(),
        target: AutomationTarget {
            entity_id: "chn_0001".to_string(),
            parameter_id: parameter_id.to_string(),
        },
        source: AutomationSourceSpec::Constant { value: 0.5 },
        combine: None,
        loop_spec: None,
        last_beat: None,
    }
}

#[test]
fn note_domains() {
    for velocity in [1.5, -0.1, f64::NAN] {
        let mut s = base_snapshot();
        s.patterns.push(pattern_with(note(60, velocity, (1, 4))));
        let e = err(&s);
        assert_eq!(e.code, codes::INVALID_PROJECT);
        assert!(e.path.unwrap().ends_with(".velocity"));
    }
    let mut s = base_snapshot();
    s.patterns.push(pattern_with(note(128, 0.5, (1, 4))));
    assert!(err(&s).path.unwrap().ends_with(".pitch"));

    let mut s = base_snapshot();
    s.patterns.push(pattern_with(note(60, 0.5, (0, 1))));
    assert!(err(&s).path.unwrap().ends_with(".duration"));

    let mut s = base_snapshot();
    let mut n = note(60, 0.5, (1, 4));
    n.off_velocity = Some(2.0);
    s.patterns.push(pattern_with(n));
    assert!(err(&s).path.unwrap().ends_with(".offVelocity"));

    let mut s = base_snapshot();
    s.patterns.push(PatternSpec {
        length_beats: beat(0, 1),
        ..pattern_with(note(60, 0.5, (1, 4)))
    });
    assert!(err(&s).path.unwrap().contains("lengthBeats"));
}

#[test]
fn pattern_clip_length_forms_are_mutually_exclusive() {
    let valid = |clip: PatternClipSpec, s: &mut ProjectSnapshot| {
        s.patterns.push(PatternSpec {
            notes: vec![],
            ..pattern_with(note(60, 0.5, (1, 4)))
        });
        s.pattern_clips.push(clip);
        validate(s, &base_registry()).is_ok()
    };

    let mut s = base_snapshot();
    assert!(valid(pattern_clip(), &mut s));

    let mut s = base_snapshot();
    let mut clip = pattern_clip();
    clip.loop_count = Some(4);
    clip.last_beat = Some(beat(16, 1));
    clip.duration_beats = None;
    assert!(!valid(clip, &mut s));

    let mut s = base_snapshot();
    let mut clip = pattern_clip();
    clip.loop_count = Some(4);
    assert!(!valid(clip, &mut s)); // durationBeats + loopCount

    let mut s = base_snapshot();
    let mut clip = pattern_clip();
    clip.duration_beats = None;
    clip.last_beat = Some(beat(0, 1)); // not after startBeat
    assert!(!valid(clip, &mut s));

    let mut s = base_snapshot();
    let mut clip = pattern_clip();
    clip.duration_beats = Some(beat(0, 1));
    assert!(!valid(clip, &mut s));

    let mut s = base_snapshot();
    let mut clip = pattern_clip();
    clip.velocity_scale = Some(2.5);
    assert!(!valid(clip, &mut s));

    let mut s = base_snapshot();
    let mut clip = pattern_clip();
    clip.probability = Some(1.5);
    assert!(!valid(clip, &mut s));
}

#[test]
fn sample_clip_domains() {
    let mut s = base_snapshot();
    s.samples.push(sample());
    s.sample_clips.push(SampleClipSpec {
        gain: Some(2.5),
        ..sample_clip()
    });
    assert!(err(&s).path.unwrap().ends_with(".gain"));

    let mut s = base_snapshot();
    s.samples.push(sample());
    s.sample_clips.push(SampleClipSpec {
        pan: Some(-1.5),
        ..sample_clip()
    });
    assert!(err(&s).path.unwrap().ends_with(".pan"));

    let mut s = base_snapshot();
    s.samples.push(sample());
    s.sample_clips.push(SampleClipSpec {
        rate: Some(0.1),
        ..sample_clip()
    });
    assert!(err(&s).path.unwrap().ends_with(".rate"));

    let mut s = base_snapshot();
    s.samples.push(sample());
    s.sample_clips.push(SampleClipSpec {
        loop_spec: Some(LoopSpec {
            start_beat: None,
            length_beats: beat(4, 1),
            count: Some(2),
            last_beat: Some(beat(8, 1)),
        }),
        ..sample_clip()
    });
    assert!(err(&s).path.unwrap().contains("loop"));

    let mut s = base_snapshot();
    s.samples.push(sample());
    s.sample_clips.push(SampleClipSpec {
        loop_spec: Some(LoopSpec {
            start_beat: None,
            length_beats: beat(4, 1),
            count: Some(2),
            last_beat: None,
        }),
        ..sample_clip()
    });
    assert!(validate(&s, &base_registry()).is_ok());
}

#[test]
fn sample_edit_domains() {
    let edits = |start: Option<u64>, end: Option<u64>| SampleEditSpec {
        start_frame: start,
        end_frame: end,
        level: None,
        tone: None,
        normalize: None,
        fade_in: None,
        fade_out: None,
        crossfade: None,
    };

    let mut s = base_snapshot();
    s.samples.push(SampleRef {
        edits: Some(edits(Some(100), Some(100))),
        ..sample()
    });
    assert!(err(&s).path.unwrap().contains("edits"));

    let mut s = base_snapshot();
    s.samples.push(SampleRef {
        edits: Some(edits(None, Some(192_001))),
        ..sample()
    });
    assert_eq!(err(&s).code, codes::INVALID_PROJECT);

    let mut s = base_snapshot();
    s.samples.push(SampleRef {
        edits: Some(SampleEditSpec {
            normalize: Some(NormalizeSpec { peak_db: 0.5 }),
            ..edits(None, None)
        }),
        ..sample()
    });
    assert!(err(&s).path.unwrap().contains("peakDb"));

    let mut s = base_snapshot();
    s.samples.push(SampleRef {
        edits: Some(SampleEditSpec {
            tone: Some(1.5),
            ..edits(None, None)
        }),
        ..sample()
    });
    assert_eq!(err(&s).code, codes::INVALID_PROJECT);

    let mut s = base_snapshot();
    s.samples.push(SampleRef {
        edits: Some(SampleEditSpec {
            start_frame: Some(96_000),
            end_frame: Some(192_000),
            level: Some(1.0),
            normalize: Some(NormalizeSpec { peak_db: -1.0 }),
            fade_in: Some(FadeSpec {
                length_frames: 256,
                curve: Some(FadeCurve::EqualPower),
            }),
            ..edits(None, None)
        }),
        ..sample()
    });
    assert!(validate(&s, &base_registry()).is_ok());

    let mut s = base_snapshot();
    s.samples.push(SampleRef {
        frames: 0,
        ..sample()
    });
    assert_eq!(err(&s).code, codes::INVALID_PROJECT);
}

#[test]
fn channel_and_mixer_domains() {
    let mut s = base_snapshot();
    s.channels[0].level = 2.5;
    assert!(err(&s).path.unwrap().ends_with("channels[0].level"));

    let mut s = base_snapshot();
    s.channels[0].pan = -1.5;
    assert_eq!(err(&s).code, codes::INVALID_PROJECT);

    let mut s = base_snapshot();
    s.channels[0].swing = Some(1.5);
    assert_eq!(err(&s).code, codes::INVALID_PROJECT);

    let mut s = base_snapshot();
    s.channels[0].effect_chain.push(EffectRef {
        plugin_id: EFFECT_ID.to_string(),
        plugin_version: PLUGIN_VERSION.to_string(),
        parameters: Default::default(),
        resources: None,
        bypass: None,
        mix: Some(1.5),
    });
    assert!(err(&s).path.unwrap().ends_with("effectChain[0].mix"));

    let mut s = base_snapshot();
    s.mixer_channels[0].balance = 2.0;
    assert_eq!(err(&s).code, codes::INVALID_PROJECT);

    let mut s = base_snapshot();
    s.mixer_channels[0].master_send_ratio = Some(1.5);
    assert_eq!(err(&s).code, codes::INVALID_PROJECT);

    let mut s = base_snapshot();
    s.mixer_channels.push(MixerChannelSpec {
        id: "mix_0002".to_string(),
        ..s.mixer_channels[0].clone()
    });
    s.mixer_channels[0].sends.push(SendSpec {
        destination_id: "mix_0002".to_string(),
        ratio: 1.5,
        pre_fader: None,
        sidechain: None,
    });
    assert!(err(&s).path.unwrap().ends_with("sends[0].ratio"));
}

#[test]
fn tempo_map_rules_use_tempo_codes() {
    let mut s = base_snapshot();
    s.tempo_map.clear();
    assert_eq!(err(&s).code, codes::TEMPO_MAP_ORDER);

    let mut s = base_snapshot();
    s.tempo_map[0].start_beat = beat(4, 1);
    assert_eq!(err(&s).code, codes::TEMPO_MAP_ORDER);

    let mut s = base_snapshot();
    s.tempo_map.push(TempoSegment {
        start_beat: beat(0, 1),
        bpm: 90.0,
        curve: None,
    });
    assert_eq!(err(&s).code, codes::TEMPO_MAP_ORDER);

    let mut s = base_snapshot();
    s.tempo_map[0].bpm = 10.0;
    assert_eq!(err(&s).code, codes::TEMPO_RANGE);

    let mut s = base_snapshot();
    s.tempo_map[0].bpm = 1_000.0;
    assert_eq!(err(&s).code, codes::TEMPO_RANGE);

    let mut s = base_snapshot();
    s.tracks[0].tempo = Some(f64::NAN);
    assert_eq!(err(&s).code, codes::TEMPO_RANGE);

    let mut s = base_snapshot();
    s.tempo_map.push(TempoSegment {
        start_beat: beat(16, 1),
        bpm: 140.0,
        curve: None,
    });
    assert!(validate(&s, &base_registry()).is_ok());
}

#[test]
fn time_signature_rules() {
    let mut s = base_snapshot();
    s.time_signature_map[0].denominator = 3;
    assert_eq!(err(&s).code, codes::INVALID_PROJECT);

    let mut s = base_snapshot();
    s.time_signature_map[0].start_bar = 0;
    assert_eq!(err(&s).code, codes::INVALID_PROJECT);

    let mut s = base_snapshot();
    s.time_signature_map.push(TimeSignatureSegment {
        start_bar: 1,
        numerator: 3,
        denominator: 4,
    });
    assert_eq!(err(&s).code, codes::INVALID_PROJECT);

    let mut s = base_snapshot();
    s.time_signature_map.push(TimeSignatureSegment {
        start_bar: 9,
        numerator: 3,
        denominator: 8,
    });
    assert!(validate(&s, &base_registry()).is_ok());
}

#[test]
fn midi_channel_range() {
    for channel in [0, 17] {
        let mut s = base_snapshot();
        s.tracks[0].midi_channel = Some(channel);
        assert_eq!(err(&s).code, codes::INVALID_PROJECT);
    }
    let mut s = base_snapshot();
    s.tracks[0].midi_channel = Some(16);
    assert!(validate(&s, &base_registry()).is_ok());
}

#[test]
fn automation_lane_loop_rules() {
    let mut s = base_snapshot();
    s.automation.push(AutomationLaneSpec {
        loop_spec: Some(LoopSpec {
            start_beat: None,
            length_beats: beat(4, 1),
            count: Some(2),
            last_beat: None,
        }),
        last_beat: Some(beat(16, 1)),
        ..lane("level")
    });
    assert_eq!(err(&s).code, codes::INVALID_PROJECT);

    let mut s = base_snapshot();
    s.automation.push(AutomationLaneSpec {
        last_beat: Some(beat(16, 1)),
        ..lane("level")
    });
    assert!(validate(&s, &base_registry()).is_ok());
}
