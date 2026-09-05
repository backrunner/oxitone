//! `transport/schedule` — per-block scheduling cost under tempo changes,
//! clip-loop (boundary) expansion, and automation density
//! (05-performance-and-benchmarks.md).
//!
//! - `events_query/*`: `Scheduler::events_in_range` over one second of
//!   128-frame blocks (375 blocks) for a 512-beat schedule of
//!   sixteenth-note clips — static tempo vs. a 64-segment step/linear
//!   tempo map.
//! - `loop_wrap/*`: full `RenderGraph::process_block` with a playing
//!   transport looping an 8-beat region; each iteration renders exactly
//!   one loop pass, so every measurement batch crosses the loop boundary
//!   once (seek + event redispatch included).
//! - `automation_density/*`: control-rate evaluation of N compiled
//!   automation lanes plus the block event query — the per-block cost the
//!   dispatcher pays as lane count grows.

mod common;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

use oxitone_transport::{ClipSource, CompiledAutomation, EvalContext, Scheduler, TempoMap};

use common::{
    beat, gate_source, pattern_clip, sixteenth_pattern, typical_snapshot, varying_tempo_map,
    wave_source, BLOCK_SIZE, SAMPLE_RATE, SEED,
};
use oxitone_core::wire::WaveKind;

const BLOCK: usize = BLOCK_SIZE as usize;
/// One second of audio in blocks at 48 kHz / 128 frames.
const BLOCKS_PER_SECOND: u64 = SAMPLE_RATE as u64 / BLOCK as u64;

fn scheduler_for(tempo: &oxitone_transport::CompiledTempoMap, beats: i64) -> Scheduler {
    let pattern = sixteenth_pattern("pat_sched", (4, 1), 48);
    let clip = pattern_clip("pcl_sched", "pat_sched", "trk_sched", (0, 1), (beats, 1));
    let channel_id = "chn_sched".to_string();
    let sources = [ClipSource {
        clip: &clip,
        pattern: &pattern,
        channel_id: &channel_id,
        swing: 0.0,
        track_tempo: None,
    }];
    Scheduler::compile(&sources, tempo, SEED).unwrap()
}

/// Sum event payloads over one second of blocks starting at `start_frame`.
fn query_one_second(scheduler: &Scheduler, start_frame: u64) -> usize {
    let mut total = 0usize;
    for block in 0..BLOCKS_PER_SECOND {
        let lo = start_frame + block * BLOCK as u64;
        let events = scheduler.events_in_range(lo, lo + BLOCK as u64);
        total += black_box(events).len();
    }
    total
}

fn bench_events_query(c: &mut Criterion) {
    let static_tempo = TempoMap::compile(&common::base_snapshot().tempo_map, SAMPLE_RATE).unwrap();
    let varying_tempo = TempoMap::compile(&varying_tempo_map(64), SAMPLE_RATE).unwrap();

    let static_sched = scheduler_for(&static_tempo, 512);
    let varying_sched = scheduler_for(&varying_tempo, 512);

    // Middle of the arrangement (beat 128) so clips are fully expanded.
    let start_frame = static_tempo.beat_to_frame(beat(128, 1));
    let varying_start = varying_tempo.beat_to_frame(beat(128, 1));

    let mut group = c.benchmark_group("transport/schedule");
    group.throughput(Throughput::Elements(BLOCKS_PER_SECOND));
    group.bench_function("events_query/static_tempo", |b| {
        b.iter(|| black_box(query_one_second(black_box(&static_sched), start_frame)));
    });
    group.bench_function("events_query/tempo_changes_64seg", |b| {
        b.iter(|| black_box(query_one_second(black_box(&varying_sched), varying_start)));
    });
    group.finish();
}

fn bench_loop_wrap(c: &mut Criterion) {
    // Small but real project: 2 tracks so a wrap re-dispatches note events.
    let snapshot = typical_snapshot(2, 64, 0);
    let registry = oxitone_render::builtin_registry().unwrap();
    let store = oxitone_render::SampleStore::new(None);

    let tempo = TempoMap::compile(&snapshot.tempo_map, SAMPLE_RATE).unwrap();
    let loop_frames = tempo.beat_to_frame(beat(8, 1));
    let blocks_per_loop = loop_frames / BLOCK as u64;

    let mut group = c.benchmark_group("transport/schedule");
    group.throughput(Throughput::Elements(blocks_per_loop));
    group.sample_size(30);
    group.bench_function("loop_wrap/8beats_2tracks", |b| {
        let mut graph = oxitone_render::RenderGraph::compile(
            &snapshot,
            &registry,
            &store,
            &oxitone_render::RenderGraphOptions::default(),
        )
        .unwrap();
        let mut out_l = vec![0.0f32; BLOCK];
        let mut out_r = vec![0.0f32; BLOCK];
        b.iter(|| {
            graph.transport_mut().play_from(0, Some((0, loop_frames)));
            for _ in 0..blocks_per_loop {
                graph.process_block(&mut out_l, &mut out_r);
                black_box(&out_l);
            }
        });
    });
    group.finish();
}

fn bench_automation_density(c: &mut Criterion) {
    let snapshot = common::base_snapshot();
    let tempo = TempoMap::compile(&snapshot.tempo_map, SAMPLE_RATE).unwrap();
    let scheduler = scheduler_for(&tempo, 512);
    let ctx = EvalContext::default();

    let mut group = c.benchmark_group("transport/schedule");
    group.throughput(Throughput::Elements(BLOCKS_PER_SECOND));
    for lanes in [0usize, 8, 32, 128] {
        let compiled: Vec<CompiledAutomation> = (0..lanes)
            .map(|i| {
                let source = match i % 3 {
                    0 => wave_source(WaveKind::Sine, (8, 1)),
                    1 => gate_source((1, 2), 0.5),
                    _ => common::chance_source(0.5, 4.0, i as u64 + 1),
                };
                CompiledAutomation::compile(&source, SEED).unwrap()
            })
            .collect();
        group.bench_with_input(
            BenchmarkId::new("automation_density", format!("{lanes}_lanes")),
            &lanes,
            |b, _| {
                let start = tempo.beat_to_frame(beat(128, 1));
                b.iter(|| {
                    let mut acc = 0.0f32;
                    let mut events = 0usize;
                    let mut f = start;
                    for _ in 0..BLOCKS_PER_SECOND {
                        events += scheduler.events_in_range(f, f + BLOCK as u64).len();
                        for lane in &compiled {
                            acc += black_box(lane.value_at_frame(f, &tempo, &ctx));
                        }
                        f += BLOCK as u64;
                    }
                    black_box((acc, events))
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_events_query,
    bench_loop_wrap,
    bench_automation_density
);
criterion_main!(benches);
