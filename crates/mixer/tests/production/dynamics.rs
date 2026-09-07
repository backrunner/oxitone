use super::common::*;

#[test]
fn density_processors_boost_quiet_detail_control_hot_signals_and_preserve_silence() {
    for id in ["oxitone.compactor", "oxitone.multiband-dynamics"] {
        let input = sine(1000., 0.003, 48000);
        let mut instance = configured(id, &[("upwardDb", 24.)]);
        let out = render(instance.as_mut(), &input, &input, 128);
        assert!(rms(&out[0][24000..]) > rms(&input[24000..]) * 1.5, "{id}");
        instance.reset();
        assert_eq!(
            render(instance.as_mut(), &vec![0.; 4096], &vec![0.; 4096], 128),
            [vec![0.; 4096], vec![0.; 4096]]
        );
    }
    let input = sine(1000., 0.7, 48000);
    let mut instance = configured(
        "oxitone.multiband-dynamics",
        &[("depth", 1.), ("upwardDb", 0.)],
    );
    let out = render(instance.as_mut(), &input, &input, 128);
    assert!(rms(&out[0][24000..]) < rms(&input[24000..]) * 0.4);
}

#[test]
fn lr4_crossover_sum_has_flat_magnitude_at_zero_depth() {
    for hz in [40., 180., 1000., 2800., 8000.] {
        let input = sine(hz, 0.2, 24000);
        let mut instance = configured("oxitone.multiband-dynamics", &[("depth", 0.)]);
        let out = render(instance.as_mut(), &input, &input, 127);
        let ratio = rms(&out[0][12000..]) / rms(&input[12000..]);
        assert!((ratio - 1.).abs() < 0.005, "{hz}: {ratio}");
    }
}

#[test]
fn mastering_limiter_holds_lookahead_peaks_and_bounds_reconstructed_output() {
    for release in [5., 1000.] {
        let mut input = sine(11000., 2., 24000);
        input[12001] = 8.;
        input[12177] = -6.;
        let right: Vec<_> = input.iter().map(|v| *v * 0.3).collect();
        let mut instance = configured(
            "oxitone.limiter",
            &[("ceilingDb", -1.), ("releaseMs", release)],
        );
        let out = render(instance.as_mut(), &input, &right, 127);
        let mut meter = oxitone_mixer::TruePeakMeter::new(256);
        for (left, right) in out[0].chunks(256).zip(out[1].chunks(256)) {
            meter.add_block(left, right);
        }
        assert!(
            meter.true_peak() <= 10f32.powf(-1. / 20.) + 1e-4,
            "{release}: true peak {}",
            meter.true_peak()
        );
        assert!(out[0]
            .iter()
            .zip(&out[1])
            .all(|(a, b)| (*a * 0.3 - *b).abs() < 2e-6));
        assert!(rms(&out[0][4000..8000]) > 0.1);
    }
}

#[test]
fn gate_range_and_hysteresis_hold_a_quiet_signal_without_chatter() {
    let mut instance = configured(
        "oxitone.gate",
        &[
            ("thresholdDb", -20.),
            ("hysteresisDb", 12.),
            ("holdMs", 0.),
            ("releaseMs", 1.),
            ("rangeDb", -12.),
        ],
    );
    let loud = vec![0.2; 2048];
    let quiet = vec![0.05; 4096];
    let closed = vec![0.001; 8192];
    render(instance.as_mut(), &loud, &loud, 128);
    let held = render(instance.as_mut(), &quiet, &quiet, 128);
    assert!(held[0][4000] > 0.049);
    let shut = render(instance.as_mut(), &closed, &closed, 128);
    assert!((shut[0][8000] - 0.001 * 10f32.powf(-12. / 20.)).abs() < 1e-7);
}
