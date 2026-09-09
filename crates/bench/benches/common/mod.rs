//! Shared fixtures for oxitone-bench criterion targets.
//!
//! Everything is deterministic: fixed seed, fixed buffer contents, fixed
//! snapshot shape. The "typical project" mirrors the render integration
//! fixture style (`crates/render/tests/common`) but is generated
//! programmatically so snapshot size scales with a few knobs.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use oxitone_core::pcg32::Pcg32;
use oxitone_core::wire::{
    AutomationLaneSpec, AutomationPoint, AutomationSourceSpec, AutomationTarget, ChanceSpec,
    ChannelSpec, CurveKind, EffectRef, InstrumentRef, MixerChannelSpec, NoteSpec, PatternClipSpec,
    PatternSpec, ProjectSnapshot, SendSpec, TempoCurve, TempoSegment, TimeSignatureSegment,
    TrackSpec, WaveKind,
};
use oxitone_core::{Beat, PROTOCOL_VERSION};

pub const SAMPLE_RATE: u32 = 48_000;
pub const BLOCK_SIZE: u32 = 128;

/// Project seed used by every fixture (kept as a plain constant so it
/// shows up in benchmark reports).
pub const SEED: u64 = 7;

pub fn beat(n: i64, d: u32) -> Beat {
    Beat::new(n, d).unwrap()
}

/// Deterministic noise in [-1, 1) from the project PRNG.
pub fn noise_buffer(seed: u64, len: usize) -> Vec<f32> {
    let mut rng = Pcg32::new(seed);
    (0..len)
        .map(|_| (rng.next_f64() * 2.0 - 1.0) as f32)
        .collect()
}

/// Decaying filter/reverb tail corpus: an impulse followed by an
/// exponential decay that crosses the f32 subnormal boundary (~1.18e-38)
/// and ends deep inside the subnormal range. Used to prove FTZ/DAZ and the
/// per-sample denormal guards keep throughput flat on decaying tails.
pub fn denormal_tail(len: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; len];
    for (i, slot) in out.iter_mut().enumerate() {
        // -600 dB over `len` samples: crosses into subnormals mid-buffer.
        let db = -600.0 * i as f64 / len as f64;
        *slot = 10f64.powf(db / 20.0) as f32;
    }
    out
}

/// Scratch directory for file-writing benchmarks (git-ignored).
pub fn scratch_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/tmp/oxitone-bench")
        .join(name);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

pub fn note(pitch: u8, start: (i64, u32), duration: (i64, u32), velocity: f64) -> NoteSpec {
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

/// Sixteenth-note pattern over `length` beats, pitches walking a minor
/// pentatonic — a representative melodic pattern.
pub fn sixteenth_pattern(id: &str, length: (i64, u32), base_pitch: u8) -> PatternSpec {
    let (n, d) = length;
    let steps = (4 * n / d as i64).max(1) as usize; // 4 sixteenths per beat
    let intervals = [0i8, 3, 5, 7, 10, 12, 7, 3];
    let notes = (0..steps)
        .map(|i| {
            let pitch = (base_pitch as i16 + intervals[i % intervals.len()] as i16) as u8;
            let velocity = 0.55 + 0.4 * ((i % 4) as f64 / 3.0);
            note(pitch, (i as i64, 4), (1, 4), velocity)
        })
        .collect();
    PatternSpec {
        id: id.into(),
        name: None,
        length_beats: beat(n, d),
        notes,
        parts: None,
    }
}

pub fn track(id: &str, channel_ids: &[&str], pattern_clips: &[&str]) -> TrackSpec {
    TrackSpec {
        id: id.into(),
        name: None,
        channel_ids: channel_ids.iter().map(|s| s.to_string()).collect(),
        tempo: None,
        pattern_clip_ids: pattern_clips.iter().map(|s| s.to_string()).collect(),
        sample_clip_ids: vec![],
        enabled: None,
        mute: None,
        solo: None,
        midi_channel: None,
    }
}

pub fn wavetable_ref(parameters: &[(&str, f64)]) -> InstrumentRef {
    InstrumentRef {
        instance_id: None,
        plugin_id: "oxitone.wavetable".into(),
        plugin_version: "1.0.0".into(),
        parameters: parameters
            .iter()
            .map(|(k, v)| (k.to_string(), *v))
            .collect::<BTreeMap<_, _>>(),
        resources: None,
        state: None,
    }
}

pub fn effect_ref(plugin_id: &str, parameters: &[(&str, f64)]) -> EffectRef {
    EffectRef {
        instance_id: None,
        plugin_id: plugin_id.into(),
        plugin_version: "1.0.0".into(),
        parameters: parameters
            .iter()
            .map(|(k, v)| (k.to_string(), *v))
            .collect::<BTreeMap<_, _>>(),
        resources: None,
        bypass: None,
        mix: None,
    }
}

pub fn channel(
    id: &str,
    bus: &str,
    instrument: InstrumentRef,
    effects: Vec<EffectRef>,
) -> ChannelSpec {
    ChannelSpec {
        id: id.into(),
        name: None,
        instrument,
        effect_chain: effects,
        level: 1.0,
        pan: 0.0,
        swing: None,
        mixer_channel_id: bus.into(),
        mute: None,
        solo: None,
    }
}

pub fn mixer_channel(id: &str, inserts: Vec<EffectRef>, sends: Vec<SendSpec>) -> MixerChannelSpec {
    MixerChannelSpec {
        id: id.into(),
        name: None,
        level: 1.0,
        balance: 0.0,
        master_send_ratio: None,
        inserts,
        sends,
        mute: None,
        solo: None,
    }
}

pub fn send(destination: &str, ratio: f64, sidechain: bool) -> SendSpec {
    SendSpec {
        destination_id: destination.into(),
        ratio,
        pre_fader: None,
        sidechain: sidechain.then_some(true),
    }
}

pub fn pattern_clip(
    id: &str,
    pattern: &str,
    track: &str,
    start: (i64, u32),
    duration: (i64, u32),
) -> PatternClipSpec {
    PatternClipSpec {
        id: id.into(),
        pattern_id: pattern.into(),
        track_id: track.into(),
        start_beat: beat(start.0, start.1),
        duration_beats: Some(beat(duration.0, duration.1)),
        loop_count: None,
        last_beat: None,
        transpose: None,
        velocity_scale: None,
        probability: None,
        enabled: None,
    }
}

pub fn lane(
    id: &str,
    entity: &str,
    parameter: &str,
    source: AutomationSourceSpec,
) -> AutomationLaneSpec {
    AutomationLaneSpec {
        playback: None,
        id: id.into(),
        target: AutomationTarget {
            scope: None,
            entity_id: entity.into(),
            parameter_id: parameter.into(),
        },
        source,
        combine: None,
        loop_spec: None,
        last_beat: None,
    }
}

pub fn gate_source(period: (i64, u32), duty: f64) -> AutomationSourceSpec {
    AutomationSourceSpec::Gate {
        period_beats: beat(period.0, period.1),
        duty,
        phase: None,
        on: None,
        off: None,
    }
}

pub fn wave_source(wave: WaveKind, period: (i64, u32)) -> AutomationSourceSpec {
    AutomationSourceSpec::Wave {
        wave,
        period_beats: beat(period.0, period.1),
        phase: None,
        min: None,
        max: None,
        pulse_width: None,
    }
}

pub fn chance_source(probability: f64, rate: f64, seed: u64) -> AutomationSourceSpec {
    AutomationSourceSpec::Chance(ChanceSpec {
        probability,
        seed,
        smooth_beats: Some(beat(1, 32)),
        random_phase: None,
        rate: Some(rate),
        interval_beats: None,
    })
}

/// Piecewise-linear ("polyline") curve with `points` breakpoints over
/// `span` beats.
pub fn polyline_source(points: usize, span: (i64, u32)) -> AutomationSourceSpec {
    let (n, d) = span;
    let total = n as f64 / d as f64;
    let mut rng = Pcg32::new(0xBEEF);
    let points = (0..points)
        .map(|i| AutomationPoint {
            beat: Beat::from_f64(total * i as f64 / (points - 1).max(1) as f64).unwrap(),
            value: rng.next_f64(),
            curve: None,
        })
        .collect();
    AutomationSourceSpec::Curve {
        interpolation: CurveKind::Linear,
        points,
    }
}

pub fn base_snapshot() -> ProjectSnapshot {
    ProjectSnapshot {
        protocol_version: PROTOCOL_VERSION.to_string(),
        revision: 1,
        id: "prj_bench".into(),
        name: None,
        sample_rate: SAMPLE_RATE,
        block_size: BLOCK_SIZE,
        seed: SEED,
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
        automation_clips: None,
    }
}

/// Tempo map with `segments` segments (step and linear alternating) over
/// 4 beats each — the "tempo changes" transport scenario.
pub fn varying_tempo_map(segments: usize) -> Vec<TempoSegment> {
    (0..segments)
        .map(|i| TempoSegment {
            start_beat: beat(4 * i as i64, 1),
            bpm: 90.0 + 90.0 * ((i * 7) % 10) as f64 / 10.0,
            curve: Some(if i % 2 == 0 {
                TempoCurve::Step
            } else {
                TempoCurve::Linear
            }),
        })
        .collect()
}

/// The "typical project" for compile/validate and render benchmarks:
/// `tracks` instrument tracks (wavetable + EQ insert per channel, one
/// sixteenth-note pattern clip each over `beats` beats), a shared FX bus
/// (reverb + delay), per-track sends, and `lanes` automation lanes cycling
/// through gate / wave / chance / polyline sources on channel and mixer
/// parameters.
pub fn typical_snapshot(tracks: usize, beats: i64, lanes: usize) -> ProjectSnapshot {
    let mut snapshot = base_snapshot();
    for t in 0..tracks {
        let trk = format!("trk_{t:02}");
        let chn = format!("chn_{t:02}");
        let mix = format!("mix_{t:02}");
        let pat = format!("pat_{t:02}");
        let pcl = format!("pcl_{t:02}");
        snapshot.tracks.push(track(&trk, &[&chn], &[&pcl]));
        snapshot
            .patterns
            .push(sixteenth_pattern(&pat, (4, 1), 48 + (t % 24) as u8));
        snapshot
            .pattern_clips
            .push(pattern_clip(&pcl, &pat, &trk, (0, 1), (beats, 1)));
        snapshot.channels.push(channel(
            &chn,
            &mix,
            wavetable_ref(&[
                ("filter.cutoff", 800.0 + 150.0 * t as f64),
                ("oscA.pitch", (t % 3) as f64 * -12.0),
            ]),
            vec![effect_ref("oxitone.eq", &[("band1.gainDb", 1.5)])],
        ));
        snapshot.mixer_channels.push(mixer_channel(
            &mix,
            vec![],
            vec![send("mix_fx", 0.15 + 0.02 * (t % 4) as f64, false)],
        ));
    }
    snapshot.mixer_channels.push(mixer_channel(
        "mix_fx",
        vec![
            effect_ref("oxitone.reverb", &[("decaySeconds", 1.4)]),
            effect_ref("oxitone.delay", &[("timeBeats", 0.75), ("feedback", 0.25)]),
        ],
        vec![],
    ));

    let sources = [
        gate_source((1, 2), 0.6),
        wave_source(WaveKind::Sine, (8, 1)),
        chance_source(0.65, 2.0, 11),
        polyline_source(16, (8, 1)),
        wave_source(WaveKind::Triangle, (4, 1)),
    ];
    // Parameter fan-out is by lane block, so every (channel, parameter)
    // target is unique and no lane needs a `combine` mode.
    let parameters = ["level", "pan", "filter.cutoff", "swing"];
    for i in 0..lanes {
        let chn = format!("chn_{:02}", i % tracks);
        let parameter = parameters[(i / tracks) % parameters.len()];
        let source = sources[i % sources.len()].clone();
        snapshot
            .automation
            .push(lane(&format!("auto_{i:03}"), &chn, parameter, source));
    }
    snapshot
}
