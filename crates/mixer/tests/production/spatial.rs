use super::common::*;

#[test]
fn spreader_creates_high_band_stereo_while_preserving_mono_sum_and_centered_bass() {
    for hz in [40., 5000.] {
        let input = sine(hz, 0.3, 24000);
        let mut instance = configured("oxitone.spreader", &[("amount", 1.), ("bassMonoHz", 300.)]);
        let out = render(instance.as_mut(), &input, &input, 128);
        assert!(out[0]
            .iter()
            .zip(&out[1])
            .zip(&input)
            .all(|((l, r), x)| ((l + r) * 0.5 - x).abs() < 1e-6));
        let side: Vec<_> = out[0][12000..]
            .iter()
            .zip(&out[1][12000..])
            .map(|(a, b)| (a - b) * 0.5)
            .collect();
        if hz < 100. {
            assert!(rms(&side) < 0.001);
        } else {
            assert!(rms(&side) > 0.01);
        }
    }
}

#[test]
fn convolution_impulse_matches_custom_stereo_resource_and_rejects_bad_resources() {
    use oxitone_mixer::effects::convolver::{from_impulse, Impulse, MAX_IMPULSE_FRAMES};
    let mut left = vec![0.; 777];
    left[0] = 0.5;
    left[521] = -0.25;
    let mut right = vec![0.; 777];
    right[79] = 0.2;
    right[776] = 0.1;
    let mut instance = from_impulse(
        Impulse {
            channels: [left.clone(), right.clone()],
            sample_rate: RATE,
        },
        RATE,
    )
    .unwrap();
    let mut input = vec![0.; 2048];
    input[0] = 1.;
    let out = render(instance.as_mut(), &input, &input, 127);
    for (ch, ir) in [left, right].iter().enumerate() {
        for (i, value) in out[ch].iter().enumerate() {
            let expected = i
                .checked_sub(256)
                .and_then(|i| ir.get(i))
                .copied()
                .unwrap_or(0.);
            assert!((value - expected).abs() < 1e-6);
        }
    }
    for ir in [vec![], vec![f32::NAN], vec![0.; MAX_IMPULSE_FRAMES + 1]] {
        assert!(from_impulse(
            Impulse {
                channels: [ir.clone(), ir],
                sample_rate: RATE
            },
            RATE
        )
        .is_err());
    }
}

#[test]
fn resonator_rings_at_tuning_and_spatial_processors_flush_tails_on_reset() {
    let mut impulse = vec![0.; 48000];
    impulse[0] = 1.;
    let mut instance = configured(
        "oxitone.resonator",
        &[("frequencyHz", 440.), ("spread", 0.), ("decaySeconds", 0.2)],
    );
    let out = render(instance.as_mut(), &impulse, &impulse, 128);
    assert!(amplitude(&out[0][..12000], 440.) > amplitude(&out[0][..12000], 600.) * 8.);
    assert!(rms(&out[0][24000..]) < rms(&out[0][..12000]) * 0.01);
    for id in [
        "oxitone.resonator",
        "oxitone.convolver",
        "oxitone.flanger",
        "oxitone.pitch-shifter",
        "oxitone.tape",
    ] {
        let mut instance = configured(id, &[]);
        render(instance.as_mut(), &impulse[..4096], &impulse[..4096], 128);
        instance.reset();
        let out = render(instance.as_mut(), &vec![0.; 8192], &vec![0.; 8192], 128);
        assert!(out.iter().flatten().all(|v| *v == 0.), "{id} reset silence");
        assert_eq!(instance.tail_frames(), 0);
    }
}

#[test]
fn tape_motion_changes_timing_and_flanger_feedback_creates_a_comb() {
    let input = sine(1000., 0.2, 24000);
    let mut plain = configured("oxitone.tape", &[("wow", 0.), ("flutter", 0.)]);
    let clean = render(plain.as_mut(), &input, &input, 128);
    let mut motion = configured("oxitone.tape", &[("wow", 1.), ("flutter", 1.)]);
    let moving = render(motion.as_mut(), &input, &input, 128);
    let diff: Vec<_> = clean[0]
        .iter()
        .zip(&moving[0])
        .map(|(a, b)| a - b)
        .collect();
    assert!(rms(&diff[12000..]) > 0.05);
    let mut flanger = configured(
        "oxitone.flanger",
        &[("delayMs", 2.), ("depthMs", 0.), ("feedback", 0.5)],
    );
    let mut impulse = vec![0.; 1024];
    impulse[0] = 1.;
    let out = render(flanger.as_mut(), &impulse, &impulse, 128);
    assert_eq!(out[0][0], 0.5);
    assert_eq!(out[0][96], 0.5);
    assert_eq!(out[0][192], 0.25);
}
