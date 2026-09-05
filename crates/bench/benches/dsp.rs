//! `dsp/*` — per-sample throughput of the DSP primitives
//! (05-performance-and-benchmarks.md), plus the denormal corpus: decaying
//! tails that cross the f32 subnormal boundary must show no performance
//! cliff (FTZ/DAZ + per-sample guards in `oxitone_dsp::ftz`).
//!
//! All inputs are fixed-seed and preallocated; criterion's warmup covers
//! the warmup requirement. Throughput is samples/sec unless noted.

mod common;

use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion, Throughput};

use oxitone_dsp::biquad::{design, Biquad, BiquadF64, BiquadKind};
use oxitone_dsp::envelope::Adsr;
use oxitone_dsp::ftz;
use oxitone_dsp::meter::{self, Meter};
use oxitone_dsp::oscillator::{Oscillator, Waveform, Wavetable, WavetableReader};
use oxitone_dsp::resample::SincResampler;
use oxitone_dsp::wsola::Wsola;

use common::{denormal_tail, noise_buffer, SAMPLE_RATE, SEED};

const FRAMES: usize = 4096;
const SR: f64 = SAMPLE_RATE as f64;

fn bench_oscillator(c: &mut Criterion) {
    let mut group = c.benchmark_group("dsp/oscillator");
    group.throughput(Throughput::Elements(FRAMES as u64));
    let mut out = vec![0.0f32; FRAMES];
    for (name, wf) in [
        ("sine", Waveform::Sine),
        ("saw_polyblep", Waveform::Saw),
        ("square_polyblep", Waveform::Square),
        ("triangle", Waveform::Triangle),
    ] {
        group.bench_function(name, |b| {
            let mut osc = Oscillator::new();
            b.iter(|| osc.render(wf, black_box(440.0), SR, black_box(&mut out)));
        });
    }
    // Mip-mapped wavetable read (the WavetableSynth voice hot path).
    let base: Vec<f32> = (0..2048)
        .map(|i| (core::f64::consts::TAU * i as f64 / 2048.0).sin() as f32)
        .collect();
    let table = Wavetable::new(&base, 6, SR);
    group.bench_function("wavetable_mip", |b| {
        let mut reader = WavetableReader::new();
        reader.prepare(&table, 440.0);
        b.iter(|| reader.render(black_box(&table), black_box(440.0), black_box(&mut out)));
    });
    group.finish();
}

fn bench_envelope(c: &mut Criterion) {
    let mut group = c.benchmark_group("dsp/envelope");
    group.throughput(Throughput::Elements(FRAMES as u64));
    let mut out = vec![0.0f32; FRAMES];

    group.bench_function("adsr_sustain", |b| {
        let mut env = Adsr::new(SR);
        env.set_params(0.001, 0.05, 0.7, 0.2);
        env.note_on();
        for _ in 0..SAMPLE_RATE {
            env.next_sample();
        }
        b.iter(|| env.process(black_box(&mut out)));
    });
    group.bench_function("adsr_attack_decay", |b| {
        b.iter_batched(
            || {
                let mut env = Adsr::new(SR);
                env.set_params(0.02, 0.06, 0.7, 0.2);
                env.note_on();
                env
            },
            |mut env| env.process(black_box(&mut out)),
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

fn bench_biquad(c: &mut Criterion) {
    let mut group = c.benchmark_group("dsp/biquad");
    group.throughput(Throughput::Elements(FRAMES as u64));
    let coeffs = design(BiquadKind::Lowpass, SR, 1_000.0, 0.707, 0.0);
    let input = noise_buffer(SEED, FRAMES);

    group.bench_function("lowpass_f32_state", |b| {
        let mut bq = Biquad::new(coeffs);
        b.iter_batched(
            || input.clone(),
            |mut buf| {
                bq.process(black_box(&mut buf));
                buf
            },
            BatchSize::SmallInput,
        );
    });
    group.bench_function("lowpass_f64_state", |b| {
        let mut bq = BiquadF64::new(coeffs);
        b.iter_batched(
            || input.clone(),
            |mut buf| {
                bq.process(black_box(&mut buf));
                buf
            },
            BatchSize::SmallInput,
        );
    });

    // Denormal corpus: decaying tail crossing the subnormal boundary. The
    // f32-state biquad flushes subnormal state per sample; on targets
    // without hardware FTZ this pair is the cliff detector — the two
    // samples/sec numbers must stay within noise of each other.
    let tail = denormal_tail(FRAMES);
    group.bench_function("denormal_tail_input_f32", |b| {
        let mut bq = Biquad::new(coeffs);
        b.iter_batched(
            || tail.clone(),
            |mut buf| {
                bq.process(black_box(&mut buf));
                buf
            },
            BatchSize::SmallInput,
        );
    });
    group.bench_function("denormal_tail_input_f64", |b| {
        let mut bq = BiquadF64::new(coeffs);
        b.iter_batched(
            || tail.clone(),
            |mut buf| {
                bq.process(black_box(&mut buf));
                buf
            },
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

fn bench_resampler(c: &mut Criterion) {
    let mut group = c.benchmark_group("dsp/resampler");
    let input = noise_buffer(SEED, FRAMES);
    for step in [0.5f64, 1.0, 2.0] {
        let out_len = (FRAMES as f64 * step) as usize + 128;
        // Throughput in produced (output) samples per call.
        group.throughput(Throughput::Elements((FRAMES as f64 * step) as u64));
        group.bench_function(format!("sinc_step_{step}"), |b| {
            let mut rs = SincResampler::new(2.0, FRAMES);
            let mut out = vec![0.0f32; out_len];
            b.iter(|| {
                let done = rs.process(black_box(&input), black_box(&mut out), step);
                black_box(done.produced)
            });
        });
    }
    group.finish();
}

fn bench_wsola(c: &mut Criterion) {
    let mut group = c.benchmark_group("dsp/wsola");
    // Burst-train content keeps the correlation search representative.
    let input = noise_buffer(SEED ^ 0xA5, 1024);
    for ratio in [0.75f64, 1.5] {
        let out_len = (1024.0 * ratio) as usize + 2 * oxitone_dsp::wsola::DEFAULT_WINDOW;
        group.throughput(Throughput::Elements(1024));
        group.bench_function(format!("ratio_{ratio}"), |b| {
            let mut wsola = Wsola::new(oxitone_dsp::wsola::DEFAULT_WINDOW, 1024);
            let mut out = vec![0.0f32; out_len];
            b.iter(|| {
                let done = wsola
                    .process(black_box(&input), black_box(&mut out), ratio)
                    .unwrap();
                black_box(done.produced)
            });
        });
    }
    group.finish();
}

fn bench_meter(c: &mut Criterion) {
    let mut group = c.benchmark_group("dsp/meter");
    group.throughput(Throughput::Elements(FRAMES as u64));
    let buf = noise_buffer(SEED, FRAMES);
    group.bench_function("peak", |b| {
        b.iter(|| black_box(meter::peak(black_box(&buf))))
    });
    group.bench_function("rms", |b| b.iter(|| black_box(meter::rms(black_box(&buf)))));
    group.bench_function("meter_add_block", |b| {
        let mut m = Meter::new();
        b.iter(|| m.add_block(black_box(&buf)));
    });
    group.finish();
}

fn bench_ftz(c: &mut Criterion) {
    let mut group = c.benchmark_group("dsp/ftz");
    group.throughput(Throughput::Elements(FRAMES as u64));
    let tail = denormal_tail(FRAMES);
    group.bench_function("flush_denormal_tail", |b| {
        b.iter_batched(
            || tail.clone(),
            |mut buf| {
                for x in buf.iter_mut() {
                    *x = ftz::flush_denormal(*x);
                }
                buf
            },
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

/// Reverb tail processing with a decaying internal state: after one
/// impulse block the tail decays toward and into the subnormal range; the
/// per-block cost of those tail blocks must match active-input blocks.
fn bench_reverb_denormal(c: &mut Criterion) {
    use oxitone_graph::{HostContext, ProcessContext};

    let plugins = oxitone_mixer::builtin_effect_plugins();
    let reverb = plugins
        .iter()
        .find(|p| p.descriptor().plugin_id == "oxitone.reverb")
        .expect("oxitone.reverb built-in");

    let host = HostContext {
        sample_rate: SR,
        max_block_size: 128,
    };
    let silence_l = vec![0.0f32; 128];
    let silence_r = vec![0.0f32; 128];
    let noise_l = noise_buffer(SEED, 128);
    let noise_r = noise_buffer(SEED ^ 1, 128);

    let make_instance = || {
        let mut inst = reverb.create(&host);
        inst.prepare(SR, 128);
        inst
    };

    let run_block = |inst: &mut dyn oxitone_graph::PluginInstance,
                     in_l: &[f32],
                     in_r: &[f32],
                     out_l: &mut [f32],
                     out_r: &mut [f32]| {
        let inputs: [&[f32]; 2] = [in_l, in_r];
        let mut outputs: [&mut [f32]; 2] = [out_l, out_r];
        let mut ctx = ProcessContext {
            frames: 128,
            sample_rate: SR,
            inputs: &inputs,
            outputs: &mut outputs,
            note_events: &[],
            parameter_events: &[],
            sidechain: None,
        };
        inst.process(&mut ctx);
    };

    let mut group = c.benchmark_group("dsp/denormal");
    group.bench_function("reverb_active_input_block", |b| {
        let mut inst = make_instance();
        let mut out_l = vec![0.0f32; 128];
        let mut out_r = vec![0.0f32; 128];
        b.iter(|| {
            run_block(
                inst.as_mut(),
                black_box(&noise_l),
                black_box(&noise_r),
                &mut out_l,
                &mut out_r,
            );
            black_box(&out_l);
        });
    });
    group.bench_function("reverb_tail_block_subnormal", |b| {
        let mut inst = make_instance();
        let mut out_l = vec![0.0f32; 128];
        let mut out_r = vec![0.0f32; 128];
        // Prime the tail with one impulse, then let it decay for ~2 s so
        // internal state sits deep in the subnormal range.
        let mut impulse_l = vec![0.0f32; 128];
        impulse_l[0] = 1.0;
        run_block(
            inst.as_mut(),
            &impulse_l,
            &silence_r,
            &mut out_l,
            &mut out_r,
        );
        for _ in 0..750 {
            run_block(
                inst.as_mut(),
                &silence_l,
                &silence_r,
                &mut out_l,
                &mut out_r,
            );
        }
        b.iter(|| {
            run_block(
                inst.as_mut(),
                black_box(&silence_l),
                black_box(&silence_r),
                &mut out_l,
                &mut out_r,
            );
            black_box(&out_l);
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_oscillator,
    bench_envelope,
    bench_biquad,
    bench_resampler,
    bench_wsola,
    bench_meter,
    bench_ftz,
    bench_reverb_denormal
);
criterion_main!(benches);
