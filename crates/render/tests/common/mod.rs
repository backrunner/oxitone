//! Shared fixtures for oxitone-render integration tests.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use oxitone_core::beat::Beat;
use oxitone_core::wire::{
    ChannelSpec, EffectRef, InstrumentRef, MixerChannelSpec, NoteSpec, PatternClipSpec,
    PatternSpec, ProjectSnapshot, SampleClipSpec, SampleFormat, SampleRef, SendSpec, TempoSegment,
    TimeSignatureSegment, TrackSpec,
};
use oxitone_core::PROTOCOL_VERSION;
use oxitone_render::{RenderGraph, RenderGraphOptions, SampleStore};

pub fn beat(n: i64, d: u32) -> Beat {
    Beat::new(n, d).unwrap()
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

pub fn base_snapshot() -> ProjectSnapshot {
    ProjectSnapshot {
        protocol_version: PROTOCOL_VERSION.to_string(),
        revision: 1,
        id: "prj_render".into(),
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

pub fn track(
    id: &str,
    channel_ids: &[&str],
    pattern_clips: &[&str],
    sample_clips: &[&str],
) -> TrackSpec {
    TrackSpec {
        id: id.into(),
        name: None,
        channel_ids: channel_ids.iter().map(|s| s.to_string()).collect(),
        tempo: None,
        pattern_clip_ids: pattern_clips.iter().map(|s| s.to_string()).collect(),
        sample_clip_ids: sample_clips.iter().map(|s| s.to_string()).collect(),
        enabled: None,
        midi_channel: None,
    }
}

pub fn wavetable_ref(parameters: &[(&str, f64)]) -> InstrumentRef {
    InstrumentRef {
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

pub fn pattern(id: &str, length: (i64, u32), notes: Vec<NoteSpec>) -> PatternSpec {
    PatternSpec {
        id: id.into(),
        name: None,
        length_beats: beat(length.0, length.1),
        notes,
    }
}

pub fn sample_clip(
    id: &str,
    sample: &str,
    track: &str,
    duration: Option<(i64, u32)>,
) -> SampleClipSpec {
    SampleClipSpec {
        id: id.into(),
        sample_id: sample.into(),
        track_id: track.into(),
        start_beat: Beat::ZERO,
        duration_beats: duration.map(|(n, d)| beat(n, d)),
        gain: None,
        pan: None,
        rate: None,
        loop_spec: None,
        tempo_sync: None,
        stretch_algorithm: None,
        enabled: None,
    }
}

/// Output scratch directory under `target/tmp` (git-ignored).
pub fn out_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/tmp/render-tests")
        .join(name);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Write a mono 16-bit PCM WAV; returns the lowercase hex SHA-256.
pub fn write_mono_wav(path: &Path, sample_rate: u32, samples: &[f32]) -> String {
    let data_bytes = (samples.len() * 2) as u32;
    let mut bytes = Vec::with_capacity(44 + data_bytes as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_bytes).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_bytes.to_le_bytes());
    for &x in samples {
        let v = (x.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    std::fs::write(path, &bytes).unwrap();
    oxitone_samples::sha256_hex(&bytes)
}

/// Burst-train content: one 10 ms sine burst every `every` frames (better
/// conditioned for WSOLA correlation than isolated impulses).
pub fn burst_train(frames: usize, every: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; frames];
    let mut pos = 0;
    while pos < frames {
        for i in 0..480 {
            if pos + i < frames {
                let t = i as f32 / 48_000.0;
                out[pos + i] += 0.8
                    * (2.0 * std::f32::consts::PI * 440.0 * t).sin()
                    * (std::f32::consts::PI * i as f32 / 480.0).sin();
            }
        }
        pos += every;
    }
    out
}

/// Click-train content: one decaying impulse every `every` frames.
pub fn click_train(frames: usize, every: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; frames];
    let mut pos = 0;
    while pos < frames {
        for i in 0..200 {
            if pos + i < frames {
                out[pos + i] += 0.9 * (-(i as f32) / 40.0).exp();
            }
        }
        pos += every;
    }
    out
}

/// Register the content as a `SampleRef` + written asset.
pub fn sample_asset(dir: &Path, id: &str, samples: &[f32], musical_beats: (i64, u32)) -> SampleRef {
    let path = dir.join(format!("{id}.wav"));
    let sha = write_mono_wav(&path, 48_000, samples);
    SampleRef {
        id: id.into(),
        asset_uri: path.display().to_string(),
        sha256: sha,
        format: SampleFormat::Wav,
        sample_rate: 48_000,
        channels: 1,
        frames: samples.len() as u64,
        edits: None,
        musical_length_beats: Some(beat(musical_beats.0, musical_beats.1)),
    }
}

/// Compile a graph and render `frames` into memory (no file I/O).
/// Returns `(left, right, graph_latency_frames)`; the limiter's lookahead
/// is part of the latency, so onsets land `latency` frames late.
pub fn render_frames(snapshot: &ProjectSnapshot, frames: usize) -> (Vec<f32>, Vec<f32>, u64) {
    let registry = oxitone_render::builtin_registry().unwrap();
    let store = SampleStore::new(None);
    let mut graph =
        RenderGraph::compile(snapshot, &registry, &store, &RenderGraphOptions::default()).unwrap();
    graph.transport_mut().begin_render(0);
    let block = snapshot.block_size as usize;
    let mut left = Vec::with_capacity(frames);
    let mut right = Vec::with_capacity(frames);
    let mut out_l = vec![0.0f32; block];
    let mut out_r = vec![0.0f32; block];
    let mut done = 0;
    while done < frames {
        graph.process_block(&mut out_l, &mut out_r);
        let n = (frames - done).min(block);
        left.extend_from_slice(&out_l[..n]);
        right.extend_from_slice(&out_r[..n]);
        done += n;
    }
    assert!(!graph.faulted(), "render faulted (NaN guard)");
    let latency = graph.graph_latency_frames();
    (left, right, latency)
}

/// Onset frames: first index of each above-`threshold` region, merging
/// regions closer than `min_gap` frames.
pub fn onset_frames(buf: &[f32], threshold: f32, min_gap: usize) -> Vec<usize> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < buf.len() {
        if buf[i].abs() > threshold {
            out.push(i);
            i += min_gap;
        } else {
            i += 1;
        }
    }
    out
}
