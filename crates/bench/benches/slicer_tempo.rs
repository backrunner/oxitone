//! Audible Slicer graph under a changing tempo, including restart cost.
mod common;
use common::*;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use oxitone_core::wire::{SampleFormat, SampleRef};
use oxitone_render::{RenderGraph, RenderGraphOptions, SampleStore};
use oxitone_samples::{ChannelLayoutAction, PreparedSample, SampleMetadata};
use std::sync::Arc;

fn slicer(c: &mut Criterion) {
    let signal: Vec<f32> = (0..48000)
        .map(|i| (std::f64::consts::TAU * 440.0 * i as f64 / 48000.).sin() as f32 * 0.5)
        .collect();
    let store = SampleStore::from_prepared(
        vec![(
            "smp_bench".into(),
            Arc::new(PreparedSample {
                channels: vec![signal],
                sample_rate: 48000,
                loop_points: None,
                musical_length_beats: Some(beat(2, 1)),
                metadata: SampleMetadata {
                    sha256: "00".repeat(32),
                    format: SampleFormat::Wav,
                    source_sample_rate: 48000,
                    source_channels: 1,
                    source_bit_depth: Some(32),
                    channel_layout_action: ChannelLayoutAction::Kept,
                    decoder: None,
                },
            }),
        )],
        48000,
    );
    let mut s = base_snapshot();
    s.samples.push(SampleRef {
        id: "smp_bench".into(),
        asset_uri: "in-memory.wav".into(),
        sha256: "00".repeat(32),
        format: SampleFormat::Wav,
        sample_rate: 48000,
        channels: 1,
        frames: 48000,
        edits: None,
        musical_length_beats: Some(beat(2, 1)),
        provenance: None,
    });
    s.tempo_map = varying_tempo_map(2);
    let mut reference = wavetable_ref(&[]);
    reference.plugin_id = "oxitone.slicer".into();
    reference.state = Some(
        serde_json::json!({ "sampleId": "smp_bench", "slices": {"grid": 1},
        "playMode": "oneshot", "tempoSync": "repitch" }),
    );
    s.channels
        .push(channel("chn_bench", "mix_master", reference, vec![]));
    s.tracks
        .push(track("trk_bench", &["chn_bench"], &["pcl_bench"]));
    let mut pattern = sixteenth_pattern("pat_bench", (8, 1), 60);
    pattern.notes = vec![note(60, (0, 1), (8, 1), 1.)];
    s.patterns.push(pattern);
    s.pattern_clips.push(pattern_clip(
        "pcl_bench",
        "pat_bench",
        "trk_bench",
        (0, 1),
        (8, 1),
    ));
    // Constant region followed by a ramp inside the sounding slice.
    s.tempo_map[0].curve = Some(oxitone_core::wire::TempoCurve::Linear);
    s.tempo_map[1].start_beat = beat(1, 2);
    let registry = oxitone_render::builtin_registry().unwrap();
    let options = RenderGraphOptions {
        master_limiter: false,
        ..Default::default()
    };
    let mut graph = RenderGraph::compile(&s, &registry, &store, &options).unwrap();
    graph.transport_mut().begin_render(0);
    let (mut l, mut r) = ([0.; 128], [0.; 128]);
    let mut group = c.benchmark_group("instruments/slicer_tempo");
    group.bench_function("reset_first_128", |b| {
        b.iter(|| {
            graph.seek(0);
            graph.process_block(&mut l, &mut r);
            black_box((&l, &r));
        })
    });
    // Restart every 1/2 second: the measured path always has a sounding voice.
    let mut blocks = 0;
    group.bench_function("sounding_128", |b| {
        b.iter(|| {
            if blocks % 180 == 0 {
                graph.seek(0);
            }
            graph.process_block(&mut l, &mut r);
            blocks += 1;
            black_box((&l, &r));
        })
    });
    assert!(l.iter().any(|v| v.abs() > 0.01));
    group.finish();
}
criterion_group!(benches, slicer);
criterion_main!(benches);
