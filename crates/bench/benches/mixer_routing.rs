//! `mixer/routing` — mixer engine per-block cost vs. bus count, send
//! count, sidechain detector routing, insert chain length, and the meter
//! floor (05-performance-and-benchmarks.md).
//!
//! Every engine renders one 128-frame stereo block per iteration with one
//! fixed-seed noise input per non-FX bus; throughput is frames/sec. The
//! `meter/*` pair isolates the BusMeter / true-peak cost directly.

mod common;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

use oxitone_mixer::{BusMeter, ChannelInput, MixerEngine, TruePeakMeter};

use common::{effect_ref, mixer_channel, noise_buffer, send, BLOCK_SIZE, SAMPLE_RATE, SEED};
use oxitone_core::wire::MixerChannelSpec;

const BLOCK: usize = BLOCK_SIZE as usize;
const SR: f64 = SAMPLE_RATE as f64;

struct Rig {
    engine: MixerEngine,
    inputs: Vec<(String, Vec<f32>, Vec<f32>)>,
}

impl Rig {
    fn run(&mut self, out_l: &mut [f32], out_r: &mut [f32]) {
        let inputs: Vec<ChannelInput> = self
            .inputs
            .iter()
            .map(|(bus, l, r)| ChannelInput {
                bus_id: bus,
                left: l,
                right: r,
            })
            .collect();
        self.engine
            .process_block(black_box(&inputs), black_box(out_l), black_box(out_r));
    }
}

/// `buses` content buses + one FX bus; each content bus gets `inserts`
/// EQs, `sends` sends to the FX bus (half sidechain when `sidechain`),
/// and a noise input.
fn rig(buses: usize, inserts: usize, sends_per_bus: usize, sidechain: bool, seed: u64) -> Rig {
    let registry = oxitone_render::builtin_registry().unwrap();
    let mut channels: Vec<MixerChannelSpec> = Vec::new();
    for i in 0..buses {
        let inserts = (0..inserts)
            .map(|k| effect_ref("oxitone.eq", &[("band2.freqHz", 400.0 + 100.0 * k as f64)]))
            .collect();
        let sends = (0..sends_per_bus)
            .map(|_| send("mix_fx", 0.2, sidechain))
            .collect();
        channels.push(mixer_channel(&format!("mix_{i:02}"), inserts, sends));
    }
    let fx_inserts = if sidechain {
        vec![effect_ref(
            "oxitone.compressor",
            &[("thresholdDb", -18.0), ("ratio", 4.0)],
        )]
    } else {
        vec![]
    };
    channels.push(mixer_channel("mix_fx", fx_inserts, vec![]));

    let engine = MixerEngine::build(SR, BLOCK_SIZE, &channels, &registry).unwrap();
    let inputs = (0..buses)
        .map(|i| {
            (
                format!("mix_{i:02}"),
                noise_buffer(seed + 2 * i as u64, BLOCK),
                noise_buffer(seed + 2 * i as u64 + 1, BLOCK),
            )
        })
        .collect();
    Rig { engine, inputs }
}

fn bench_routing(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixer/routing");
    group.throughput(Throughput::Elements(BLOCK as u64));
    let mut out_l = vec![0.0f32; BLOCK];
    let mut out_r = vec![0.0f32; BLOCK];

    for buses in [2usize, 8, 16] {
        let mut rig = rig(buses, 0, 0, false, SEED);
        group.bench_with_input(BenchmarkId::new("buses", buses), &buses, |b, _| {
            b.iter(|| rig.run(&mut out_l, &mut out_r))
        });
    }
    for sends in [1usize, 2, 4] {
        let mut rig = rig(8, 0, sends, false, SEED);
        group.bench_with_input(
            BenchmarkId::new("sends_per_bus_8buses", sends),
            &sends,
            |b, _| b.iter(|| rig.run(&mut out_l, &mut out_r)),
        );
    }
    for sidechain in [false, true] {
        let mut rig = rig(8, 0, 1, sidechain, SEED);
        group.bench_with_input(
            BenchmarkId::new("sidechain_detector_8buses", sidechain),
            &sidechain,
            |b, _| b.iter(|| rig.run(&mut out_l, &mut out_r)),
        );
    }
    for inserts in [1usize, 4] {
        let mut rig = rig(8, inserts, 0, false, SEED);
        group.bench_with_input(
            BenchmarkId::new("inserts_per_bus_8buses", inserts),
            &inserts,
            |b, _| b.iter(|| rig.run(&mut out_l, &mut out_r)),
        );
    }
    group.finish();
}

fn bench_meter(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixer/meter");
    group.throughput(Throughput::Elements(BLOCK as u64));
    let left = noise_buffer(SEED, BLOCK);
    let right = noise_buffer(SEED ^ 1, BLOCK);

    group.bench_function("bus_meter_block", |b| {
        let mut m = BusMeter::new();
        b.iter(|| m.add_block(black_box(&left), black_box(&right)));
    });
    group.bench_function("true_peak_block", |b| {
        let mut m = TruePeakMeter::new(BLOCK);
        b.iter(|| m.add_block(black_box(&left), black_box(&right)));
    });
    group.finish();
}

criterion_group!(benches, bench_routing, bench_meter);
criterion_main!(benches);
