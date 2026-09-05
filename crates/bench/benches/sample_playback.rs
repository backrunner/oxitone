//! First block after clip reset: varispeed initialization and WSOLA buffer reuse.
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use oxitone_core::{
    wire::{SampleFormat, TempoSync},
    Beat,
};
use oxitone_graph::compile::SampleClipPlan;
use oxitone_render::player::SampleClipPlayer;
use oxitone_samples::{ChannelLayoutAction, PreparedSample, SampleMetadata};
use std::sync::Arc;

fn playback(c: &mut Criterion) {
    let signal: Vec<f32> = (0..48000)
        .map(|i| (std::f64::consts::TAU * 440.0 * i as f64 / 48000.0).sin() as f32 * 0.5)
        .collect();
    let sample = Arc::new(PreparedSample {
        channels: vec![signal.clone(), signal],
        sample_rate: 48000,
        loop_points: None,
        musical_length_beats: Some(Beat::new(2, 1).unwrap()),
        metadata: SampleMetadata {
            sha256: "00".repeat(32),
            format: SampleFormat::Wav,
            source_sample_rate: 48000,
            source_channels: 2,
            source_bit_depth: Some(32),
            channel_layout_action: ChannelLayoutAction::Kept,
            decoder: None,
        },
    });
    let mut group = c.benchmark_group("samples/playback");
    for (name, tempo_sync) in [
        ("repitch_reset_128", TempoSync::Repitch),
        ("stretch_reset_128", TempoSync::Stretch),
    ] {
        let plan = SampleClipPlan {
            id: "scl_bench".into(),
            channels: vec![0],
            sample: sample.clone(),
            start_beat: Beat::ZERO,
            duration_beats: Beat::new(1, 1).unwrap(),
            start_frame: 0,
            end_frame: 24000,
            content_beats: 2.0,
            gain: 1.0,
            pan: 0.0,
            rate: 1.0,
            tempo_sync,
            loop_region: None,
            enabled: true,
        };
        let mut player = SampleClipPlayer::new(&plan, 48000.0, 128);
        let mut left = [0.0; 128];
        let mut right = [0.0; 128];
        group.bench_function(name, |b| {
            b.iter(|| {
                player.reset();
                player.set_rate(black_box(2.0));
                player.set_ratio(black_box(0.5));
                player.render(128, &mut left, &mut right);
                black_box((&left, &right));
            })
        });
    }
    group.finish();
}

criterion_group!(benches, playback);
criterion_main!(benches);
