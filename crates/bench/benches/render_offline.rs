//! `render/offline` — offline render ratio and WAV writer throughput
//! (05-performance-and-benchmarks.md).
//!
//! - `render_wav_8tracks_20s`: full `render_wav` export of a 20 s project
//!   (8 wavetable tracks + FX bus) to float32 WAV. Throughput is audio
//!   frames/sec; render ratio = throughput / 48 000 (budget: >= 20x).
//! - `graph_block_8tracks`: in-memory `RenderGraph::process_block` of the
//!   same project (no file I/O), frames/sec.
//! - `wav_writer/*`: `WavWriter::write_block` in bytes/sec for float32,
//!   24-bit PCM and dithered 16-bit PCM.

mod common;

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};

use oxitone_render::render_wav::BitDepth;
use oxitone_render::wav::WavWriter;
use oxitone_render::{render_wav, RenderGraph, RenderGraphOptions, RenderOptions, SampleStore};

use common::{noise_buffer, typical_snapshot, BLOCK_SIZE, SAMPLE_RATE, SEED};

const BLOCK: usize = BLOCK_SIZE as usize;

fn bench_render_wav(c: &mut Criterion) {
    let snapshot = typical_snapshot(8, 40, 8); // 40 beats @ 120 bpm = 20 s
    let registry = oxitone_render::builtin_registry().unwrap();
    let store = SampleStore::new(None);
    let dir = common::scratch_dir("render_offline");
    let frames = SAMPLE_RATE as u64 * 20;

    let mut group = c.benchmark_group("render/offline");
    group.sample_size(10);
    group.throughput(Throughput::Elements(frames));
    group.bench_function("render_wav_8tracks_20s", |b| {
        b.iter(|| {
            let options = RenderOptions::new(dir.join("bench_master.wav"));
            let report = render_wav(
                black_box(&snapshot),
                black_box(&registry),
                black_box(&store),
                &options,
            )
            .unwrap();
            black_box(report.graph_latency_frames)
        });
    });
    group.finish();
}

fn bench_graph_block(c: &mut Criterion) {
    let snapshot = typical_snapshot(8, 40, 8);
    let registry = oxitone_render::builtin_registry().unwrap();
    let store = SampleStore::new(None);

    let mut group = c.benchmark_group("render/offline");
    group.throughput(Throughput::Elements(BLOCK as u64));
    group.bench_function("graph_block_8tracks", |b| {
        let mut graph =
            RenderGraph::compile(&snapshot, &registry, &store, &RenderGraphOptions::default())
                .unwrap();
        graph.transport_mut().begin_render(0);
        let mut out_l = vec![0.0f32; BLOCK];
        let mut out_r = vec![0.0f32; BLOCK];
        b.iter(|| {
            graph.process_block(black_box(&mut out_l), black_box(&mut out_r));
            black_box(&out_l);
        });
    });
    group.finish();
}

fn bench_wav_writer(c: &mut Criterion) {
    let dir = common::scratch_dir("wav_writer");
    let left = noise_buffer(SEED, BLOCK);
    let right = noise_buffer(SEED ^ 1, BLOCK);

    let mut group = c.benchmark_group("render/offline");
    for (name, depth, dither) in [
        ("wav_writer_float32", BitDepth::Float32, false),
        ("wav_writer_pcm24", BitDepth::Pcm24, false),
        ("wav_writer_pcm16_tpdf", BitDepth::Pcm16, true),
    ] {
        group.throughput(Throughput::Bytes(
            BLOCK as u64 * 2 * depth.bytes_per_sample(),
        ));
        group.bench_function(name, |b| {
            let path = dir.join(format!("{name}.wav"));
            let make = || {
                WavWriter::create(&path, SAMPLE_RATE, depth, dither.then_some(SEED), BLOCK).unwrap()
            };
            let mut writer = make();
            // Recreate the file well before the accumulated iterations hit
            // the RIFF 32-bit size limit enforced by `WavWriter::finish`.
            let mut frames = 0u64;
            b.iter(|| {
                writer
                    .write_block(black_box(&left), black_box(&right))
                    .unwrap();
                frames += BLOCK as u64;
                if frames >= 100_000_000 {
                    writer = make();
                    frames = 0;
                }
            });
            writer.finish().unwrap();
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_render_wav,
    bench_graph_block,
    bench_wav_writer
);
criterion_main!(benches);
