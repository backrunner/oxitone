use super::*;

#[test]
fn sine_matches_analytic() {
    let mut osc = Oscillator::new();
    for i in 0..8 {
        let v = osc.next(Waveform::Sine, 1.0, 8.0);
        assert!((v as f64 - (TAU * i as f64 / 8.0).sin()).abs() < 1e-6);
    }
}

#[test]
fn geometric_waveforms_stay_bounded() {
    for wf in [Waveform::Saw, Waveform::Square, Waveform::Triangle] {
        let mut osc = Oscillator::new();
        let mut buf = [0.0f32; 4096];
        osc.render(wf, 997.0, 48_000.0, &mut buf);
        for &x in &buf {
            assert!((-1.2..=1.2).contains(&x), "{wf:?} out of range: {x}");
        }
    }
}

#[test]
fn polyblep_spreads_the_saw_edge() {
    // A naive saw falls ~2.0 in one sample at the wrap; PolyBLEP spreads
    // the transition over two samples, halving the per-sample step.
    let mut osc = Oscillator::new();
    let mut prev = osc.next(Waveform::Saw, 1000.0, 48_000.0);
    let mut max_step = 0.0f32;
    for _ in 0..4800 {
        let v = osc.next(Waveform::Saw, 1000.0, 48_000.0);
        max_step = max_step.max((v - prev).abs());
        prev = v;
    }
    assert!(max_step < 1.3, "edge step {max_step}");
}

#[test]
fn wavetable_recovers_sine() {
    let n = 64;
    let base: Vec<f32> = (0..n)
        .map(|i| (TAU * i as f64 / n as f64).sin() as f32)
        .collect();
    let table = Wavetable::new(&base, 4, 128.0);
    let mut reader = WavetableReader::new();
    reader.prepare(&table, 1.0);
    assert_eq!(reader.level, 0);
    for i in 0..n {
        let v = reader.next(&table, 2.0);
        assert!(
            (v as f64 - (TAU * i as f64 / n as f64).sin()).abs() < 1e-4,
            "sample {i}: {v}"
        );
    }
}

#[test]
fn mip_level_rises_with_frequency() {
    let base: Vec<f32> = (0..128)
        .map(|i| (TAU * i as f64 / 128.0).sin() as f32)
        .collect();
    let table = Wavetable::new(&base, 5, 48_000.0);
    assert_eq!(table.select_level(10.0), 0);
    assert!(table.select_level(1_000.0) > table.select_level(10.0));
    assert_eq!(table.select_level(20_000.0), table.level_count() - 1);
}

#[test]
fn morph_reads_both_cycles_at_one_phase_and_preserves_zero_position() {
    let a = Wavetable::new(
        &(0..64)
            .map(|i| (TAU * i as f64 / 64.).sin() as f32)
            .collect::<Vec<_>>(),
        4,
        48000.,
    );
    let b = Wavetable::new(
        &(0..64)
            .map(|i| (TAU * 3. * i as f64 / 64.).sin() as f32)
            .collect::<Vec<_>>(),
        4,
        48000.,
    );
    for position in [0., 0.5, 1.] {
        let (mut ar, mut br, mut blend) = (
            WavetableReader::new(),
            WavetableReader::new(),
            WavetableReader::new(),
        );
        ar.prepare(&a, 440.);
        br.prepare(&b, 440.);
        blend.prepare(&a, 440.);
        for _ in 0..4096 {
            let x = ar.next(&a, 440.);
            let y = br.next(&b, 440.);
            let actual = blend.next_blend(&a, &b, position, 440.);
            if position == 0. {
                assert_eq!(actual, x);
            }
            assert!((actual - (x + (y - x) * position)).abs() < 1e-6);
        }
    }
}
