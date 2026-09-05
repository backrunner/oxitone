//! Denormal corpus smoke tests: decaying filter tails and subnormal input
//! must not produce NaN and must not cause a performance cliff with FTZ/DAZ
//! enabled (05-performance-and-benchmarks.md §必备 benchmark denormal 语料).

use std::time::Instant;

use oxitone_dsp::biquad::{design, Biquad, BiquadF64, BiquadKind};
use oxitone_dsp::ftz::FtzGuard;
use oxitone_dsp::resample::SincResampler;

fn timed<F: FnMut()>(mut f: F) -> std::time::Duration {
    let t = Instant::now();
    f();
    t.elapsed()
}

#[test]
fn filter_tail_decays_through_denormal_range_without_nan() {
    let _ftz = FtzGuard::new();
    let mut b = Biquad::new(design(BiquadKind::Lowpass, 48_000.0, 200.0, 4.0, 0.0));
    let mut buf = vec![0.0f32; 1_000_000];
    buf[0] = 1.0;
    b.process(&mut buf);
    assert!(buf.iter().all(|x| x.is_finite()));
    assert!(buf[900_000..].iter().all(|&x| x == 0.0));
}

#[test]
fn subnormal_corpus_has_no_performance_cliff() {
    let _ftz = FtzGuard::new();
    let normal = vec![0.25f32; 1 << 20];
    let mut denormal = vec![0.0f32; 1 << 20];
    // Subnormal f32 input (1e-42) plus a decaying tail into a resonant LP.
    for (i, x) in denormal.iter_mut().enumerate() {
        *x = if i % 2 == 0 { 1e-42 } else { -1e-42 };
    }
    denormal[0] = 1.0;

    let run = |buf: &mut Vec<f32>| {
        let mut b = BiquadF64::new(design(BiquadKind::Lowpass, 48_000.0, 50.0, 8.0, 0.0));
        b.process(buf);
        buf.iter().all(|x| x.is_finite())
    };

    // Warm both paths, then measure.
    let mut warm = normal.clone();
    assert!(run(&mut warm));
    let t_normal = timed(|| {
        let mut buf = normal.clone();
        assert!(run(&mut buf));
    });
    let t_denormal = timed(|| {
        let mut buf = denormal.clone();
        assert!(run(&mut buf));
    });
    // Very loose bound: a real cliff is 10x-100x; allow noise.
    assert!(
        t_denormal < t_normal * 25 + std::time::Duration::from_millis(50),
        "denormal {t_denormal:?} vs normal {t_normal:?}"
    );
}

#[test]
fn resampler_handles_subnormal_input() {
    let _ftz = FtzGuard::new();
    let input = vec![1e-42f32; 65_536];
    let mut rs = SincResampler::new(2.0, 8192);
    let mut out = vec![0.0f32; 8192];
    let mut produced = 0;
    for block in input.chunks(4096) {
        let p = rs.process(block, &mut out, 1.5);
        assert!(out[..p.produced].iter().all(|x| x.is_finite()));
        produced += p.produced;
    }
    assert!(produced > 30_000, "produced {produced}");
}
