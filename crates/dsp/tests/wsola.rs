//! WSOLA tests: ratio bounds (SampleStretchRange), tone period preservation,
//! ratio-1 identity, and realtime/offline chunking parity.

use oxitone_core::{codes, Pcg32};
use oxitone_dsp::wsola::{Wsola, MAX_RATIO, MIN_RATIO};

fn sine(freq: f64, sr: f64, len: usize) -> Vec<f32> {
    (0..len)
        .map(|i| (0.8 * (2.0 * core::f64::consts::PI * freq * i as f64 / sr).sin()) as f32)
        .collect()
}

fn noise(len: usize, seed: u64) -> Vec<f32> {
    let mut rng = Pcg32::new(seed);
    (0..len)
        .map(|_| (rng.next_f64() * 2.0 - 1.0) as f32)
        .collect()
}

fn stretch(input: &[f32], ratio: f64, in_chunk: usize, out_chunk: usize) -> Vec<f32> {
    let mut w = Wsola::new(1024, in_chunk);
    // Caller contract: output space per call must cover input·ratio plus a
    // window, or the internal fifo (fixed at prepare) would back up.
    let out_chunk = out_chunk.max((in_chunk as f64 * ratio) as usize + 2 * 1024 + 16);
    let mut out = Vec::new();
    let mut buf = vec![0.0f32; out_chunk];
    let mut blocks: Vec<&[f32]> = input.chunks(in_chunk).collect();
    let zeros = [0.0f32; 64];
    for _ in 0..32 {
        blocks.push(&zeros);
    }
    for block in blocks {
        let mut todo = block;
        loop {
            let p = w.process(todo, &mut buf, ratio).unwrap();
            out.extend_from_slice(&buf[..p.produced]);
            todo = &todo[p.consumed..];
            if todo.is_empty() || (p.consumed == 0 && p.produced == 0) {
                break;
            }
        }
    }
    out
}

#[test]
fn ratio_out_of_range_is_sample_stretch_range() {
    let mut w = Wsola::new(1024, 128);
    let buf_in = [0.0f32; 128];
    let mut buf_out = [0.0f32; 128];
    for bad in [0.0, 0.2, 4.01, f64::NAN, f64::INFINITY] {
        let err = w.process(&buf_in, &mut buf_out, bad).unwrap_err();
        assert_eq!(err.code, codes::SAMPLE_STRETCH_RANGE);
    }
    assert!(w.process(&buf_in, &mut buf_out, MIN_RATIO).is_ok());
    assert!(w.process(&buf_in, &mut buf_out, MAX_RATIO).is_ok());
}

#[test]
fn tone_keeps_its_period_when_stretched() {
    let sr = 48_000.0;
    let freq = 440.0;
    let input = sine(freq, sr, 48_000);
    let out = stretch(&input, 2.0, 4096, 8192);
    // Expected output length ≈ ratio × input length.
    let expect = 2.0 * 48_000.0;
    assert!(
        (out.len() as f64 - expect).abs() < 0.1 * expect,
        "len {} vs {expect}",
        out.len()
    );
    // Autocorrelation peak of a steady segment must sit at the tone period.
    let seg = &out[20_000..36_000];
    let period = sr / freq;
    let mut best_lag = 0usize;
    let mut best = f64::NEG_INFINITY;
    for lag in 60..160usize {
        let c: f64 = seg[..seg.len() - lag]
            .iter()
            .zip(&seg[lag..])
            .map(|(&a, &b)| a as f64 * b as f64)
            .sum();
        if c > best {
            best = c;
            best_lag = lag;
        }
    }
    assert!(
        (best_lag as f64 - period).abs() <= 2.0,
        "period {best_lag} vs {period}"
    );
    let amp = seg.iter().map(|x| x.abs()).sum::<f32>() / seg.len() as f32;
    assert!(amp > 0.3, "mean |x| {amp}");
}

#[test]
fn ratio_one_is_identity() {
    let input = noise(32_768, 99);
    let out = stretch(&input, 1.0, 2048, 4096);
    assert!(out.len() >= 30_000, "len {}", out.len());
    // Frame 0 sits on a Hann zero crossing (norm 0) and stays silent.
    let max_diff = input[1..30_000]
        .iter()
        .zip(&out[1..30_000])
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    assert!(max_diff < 1e-3, "max diff {max_diff}");
}

#[test]
fn output_is_chunk_size_invariant() {
    let input = sine(330.0, 48_000.0, 24_000);
    let a = stretch(&input, 1.5, 128, 256);
    let b = stretch(&input, 1.5, 4096, 8192);
    let n = a.len().min(b.len());
    assert!(n > 20_000, "len {n}");
    assert_eq!(&a[..n], &b[..n]);
}

#[test]
fn boundary_ratios_produce_audio() {
    let input = sine(220.0, 48_000.0, 24_000);
    for ratio in [MIN_RATIO, MAX_RATIO] {
        let out = stretch(&input, ratio, 2048, 4096);
        assert!(!out.is_empty());
        assert!(out.iter().all(|x| x.is_finite()));
        let expect = ratio * 24_000.0;
        assert!(
            (out.len() as f64 - expect).abs() < 0.2 * expect + 4096.0,
            "ratio {ratio}: len {} vs {expect}",
            out.len()
        );
    }
}
