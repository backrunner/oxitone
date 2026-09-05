//! `automation/evaluate` — per-source evaluator cost at control rate (one
//! value per 128-frame block) and audio rate (`fill_segment` over the
//! block), per 05-performance-and-benchmarks.md.
//!
//! Sources: gate, polyline (32-point linear curve), sine, cos, triangle,
//! saw, chance (rate 4/beat). `composite_16nodes` stacks 16 sources in a
//! `Binary::Add` chain to show per-node scaling. Chance PRNG cost is the
//! hash-seeded O(1) jumpable draw baked into each evaluation; the
//! `chance_rate_*` pair shows the draw-rate effect.
//!
//! Throughput is reported in evaluations (control: 375/block-second,
//! audio: 375 × 128 frames = 48 000 samples).

mod common;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

use oxitone_transport::{CompiledAutomation, EvalContext, TempoMap};

use common::{
    chance_source, gate_source, polyline_source, wave_source, BLOCK_SIZE, SAMPLE_RATE, SEED,
};
use oxitone_core::wire::{AutomationSourceSpec, BinaryOp, WaveKind};

const BLOCK: usize = BLOCK_SIZE as usize;
const BLOCKS_PER_SECOND: u64 = SAMPLE_RATE as u64 / BLOCK as u64;

fn sources() -> Vec<(&'static str, AutomationSourceSpec)> {
    vec![
        ("gate", gate_source((1, 4), 0.5)),
        ("polyline_32pt", polyline_source(32, (8, 1))),
        ("sine", wave_source(WaveKind::Sine, (4, 1))),
        ("cos", wave_source(WaveKind::Cos, (4, 1))),
        ("triangle", wave_source(WaveKind::Triangle, (4, 1))),
        ("saw", wave_source(WaveKind::Saw, (4, 1))),
        ("chance_rate4", chance_source(0.5, 4.0, 3)),
        ("chance_rate16", chance_source(0.5, 16.0, 3)),
        ("composite_16nodes", composite(16)),
    ]
}

/// Left-leaning `Binary::Add` chain of `n` mixed sources.
fn composite(n: usize) -> AutomationSourceSpec {
    let mut acc = wave_source(WaveKind::Sine, (4, 1));
    for i in 1..n {
        let next = match i % 3 {
            0 => gate_source((1, 8), 0.5),
            1 => wave_source(WaveKind::Triangle, (2, 1)),
            _ => chance_source(0.5, 2.0, i as u64),
        };
        acc = AutomationSourceSpec::Binary {
            op: BinaryOp::Add,
            left: Box::new(acc),
            right: Box::new(next),
            amount: None,
        };
    }
    acc
}

fn bench_evaluate(c: &mut Criterion) {
    let tempo = TempoMap::compile(&common::base_snapshot().tempo_map, SAMPLE_RATE).unwrap();
    let ctx = EvalContext::default();
    let start = tempo.beat_to_frame(common::beat(32, 1));

    let mut control = c.benchmark_group("automation/evaluate/control_rate");
    control.throughput(Throughput::Elements(BLOCKS_PER_SECOND));
    for (name, spec) in sources() {
        let lane = CompiledAutomation::compile(&spec, SEED).unwrap();
        control.bench_with_input(BenchmarkId::from_parameter(name), &lane, |b, lane| {
            b.iter(|| {
                let mut acc = 0.0f32;
                let mut f = start;
                for _ in 0..BLOCKS_PER_SECOND {
                    acc += black_box(lane.value_at_frame(f, &tempo, &ctx));
                    f += BLOCK as u64;
                }
                black_box(acc)
            });
        });
    }
    control.finish();

    let mut audio = c.benchmark_group("automation/evaluate/audio_rate");
    audio.throughput(Throughput::Elements(BLOCKS_PER_SECOND * BLOCK as u64));
    for (name, spec) in sources() {
        let lane = CompiledAutomation::compile(&spec, SEED).unwrap();
        audio.bench_with_input(BenchmarkId::from_parameter(name), &lane, |b, lane| {
            let mut out = vec![0.0f32; BLOCK];
            b.iter(|| {
                let mut acc = 0.0f32;
                let mut f = start;
                for _ in 0..BLOCKS_PER_SECOND {
                    lane.fill_segment(f, BLOCK, &tempo, &ctx, &mut out);
                    acc += black_box(out[0]);
                    f += BLOCK as u64;
                }
                black_box(acc)
            });
        });
    }
    audio.finish();
}

criterion_group!(benches, bench_evaluate);
criterion_main!(benches);
