//! Resampler quality and determinism tests: ≥ 100 dB SNR for 48k→44.1k and
//! varispeed 2x (03-audio-runtime-spec.md §数值精度).

use core::f64::consts::TAU;

use oxitone_dsp::resample::{SincResampler, VarispeedReader};

fn sine(freq: f64, sr: f64, len: usize, amp: f64) -> Vec<f32> {
    (0..len)
        .map(|i| (amp * (TAU * freq * i as f64 / sr).sin()) as f32)
        .collect()
}

/// Least-squares fit of a pure tone (exact 2x2 solve, robust for windows
/// that do not contain an integer number of periods). Returns SNR of the
/// fit vs the residual in dB; absorbs group delay, phase, and passband
/// gain, so the residual is the resampler noise floor.
fn sine_fit_snr_db(y: &[f32], freq_hz: f64, sr: f64) -> f64 {
    let (mut cc, mut ss, mut cs, mut pc, mut ps) = (0.0f64, 0.0, 0.0, 0.0, 0.0);
    for (i, &v) in y.iter().enumerate() {
        let ph = TAU * freq_hz * i as f64 / sr;
        let (s, c) = ph.sin_cos();
        cc += c * c;
        ss += s * s;
        cs += c * s;
        pc += v as f64 * c;
        ps += v as f64 * s;
    }
    let det = cc * ss - cs * cs;
    let a = (pc * ss - ps * cs) / det;
    let b = (ps * cc - pc * cs) / det;
    let (mut sig, mut err) = (0.0f64, 0.0f64);
    for (i, &v) in y.iter().enumerate() {
        let ph = TAU * freq_hz * i as f64 / sr;
        let r = a * ph.cos() + b * ph.sin();
        sig += r * r;
        let e = v as f64 - r;
        err += e * e;
    }
    10.0 * (sig / err).log10()
}

/// Resample `input` at fixed step; returns (all output, frames produced
/// while real input was being fed). Output beyond the second value covers
/// the zero-flushed tail and must be excluded from SNR measurement.
fn run_fixed_ratio(input: &[f32], step: f64, chunk: usize) -> (Vec<f32>, usize) {
    let mut rs = SincResampler::new(step, chunk);
    let mut out = Vec::new();
    let mut buf = vec![0.0f32; chunk * 2];
    for block in input.chunks(chunk) {
        let p = rs.process(block, &mut buf, step);
        out.extend_from_slice(&buf[..p.produced]);
    }
    let tone_frames = out.len();
    // Flush the tail with silence.
    let zeros = vec![0.0f32; chunk];
    for _ in 0..8 {
        let p = rs.process(&zeros, &mut buf, step);
        if p.produced == 0 {
            break;
        }
        out.extend_from_slice(&buf[..p.produced]);
    }
    (out, tone_frames)
}

#[test]
fn resample_48k_to_44k1_snr_above_100db() {
    let step = 48_000.0 / 44_100.0;
    for freq in [1_000.0, 8_000.0, 15_000.0] {
        let input = sine(freq, 48_000.0, 96_000, 0.7);
        let (out, tone_frames) = run_fixed_ratio(&input, step, 4096);
        assert!(tone_frames > 80_000, "produced {tone_frames}");
        let body = &out[4096..tone_frames];
        let snr = sine_fit_snr_db(body, freq, 44_100.0);
        assert!(snr >= 100.0, "{freq} Hz SNR {snr:.1} dB");
    }
}

#[test]
fn varispeed_2x_snr_above_100db() {
    let input = sine(1_000.0, 48_000.0, 96_000, 0.7);
    let mut vr = VarispeedReader::new(4.0, 8192, 48_000.0, 1.0);
    vr.set_rate(2.0);
    let mut out = Vec::new();
    let mut buf = vec![0.0f32; 8192];
    for block in input.chunks(4096) {
        let p = vr.process(block, &mut buf);
        out.extend_from_slice(&buf[..p.produced]);
    }
    assert!(out.len() > 40_000, "produced {}", out.len());
    let body = &out[8192..out.len() - 1024];
    let snr = sine_fit_snr_db(body, 2_000.0, 48_000.0);
    assert!(snr >= 100.0, "varispeed 2x SNR {snr:.1} dB");
}

#[test]
fn unity_ratio_is_transparent_with_known_delay() {
    let mut input = vec![0.0f32; 4096];
    input[100] = 1.0;
    let (out, _) = run_fixed_ratio(&input, 1.0, 1024);
    let peak = out
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
        .map(|(i, _)| i)
        .unwrap();
    // taps/2 - 1 = 31 input frames of latency.
    assert!((peak as i64 - 69).abs() <= 1, "peak at {peak}");
    assert!((out[peak] - 1.0).abs() < 0.1, "peak {}", out[peak]);
}

#[test]
fn fixed_ratio_is_bit_deterministic() {
    let input = sine(2_500.0, 48_000.0, 32_000, 0.6);
    let (a, _) = run_fixed_ratio(&input, 48_000.0 / 44_100.0, 1024);
    let (b, _) = run_fixed_ratio(&input, 48_000.0 / 44_100.0, 1024);
    assert_eq!(a, b);
}

#[test]
fn output_is_chunk_size_invariant() {
    let input = sine(3_000.0, 48_000.0, 16_000, 0.6);
    let (a, _) = run_fixed_ratio(&input, 1.4, 777);
    let (b, _) = run_fixed_ratio(&input, 1.4, 4096);
    let n = a.len().min(b.len());
    assert_eq!(&a[..n], &b[..n]);
}
