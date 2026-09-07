use oxitone_graph::{HostContext, ParameterEvent, PluginInstance, ProcessContext};
use oxitone_mixer::builtin_effect_plugins;
const RATE: f64 = 48000.;

fn process(
    instance: &mut dyn PluginInstance,
    input: &[f32],
    params: &[(&str, f64)],
) -> (Vec<f32>, Vec<f32>) {
    let events: Vec<_> = params
        .iter()
        .map(|(id, value)| ParameterEvent {
            frame_offset: 0,
            parameter_id: id,
            value: *value,
        })
        .collect();
    let (mut left, mut right) = (vec![0.; input.len()], vec![0.; input.len()]);
    for (i, chunk) in input.chunks(128).enumerate() {
        let start = i * 128;
        let end = start + chunk.len();
        instance.process(&mut ProcessContext {
            frames: chunk.len(),
            sample_rate: RATE,
            inputs: &[chunk, chunk],
            outputs: &mut [&mut left[start..end], &mut right[start..end]],
            note_events: &[],
            parameter_events: if i == 0 { &events } else { &[] },
            sidechain: None,
        });
    }
    (left, right)
}
fn configured(name: &str, params: &[(&str, f64)]) -> Box<dyn PluginInstance> {
    let p = builtin_effect_plugins()
        .into_iter()
        .find(|p| p.descriptor().plugin_id == name)
        .unwrap();
    let mut effect = p.create(&HostContext {
        sample_rate: RATE,
        max_block_size: 128,
    });
    process(effect.as_mut(), &[0.; 128], params);
    effect.reset();
    effect
}
fn rms(pcm: &[f32]) -> f64 {
    (pcm.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / pcm.len() as f64).sqrt()
}
fn sine(amp: f32) -> Vec<f32> {
    (0..24000)
        .map(|i| amp * (std::f64::consts::TAU * i as f64 * 1000. / RATE).sin() as f32)
        .collect()
}

#[test]
fn distortion_modes_add_different_harmonics_without_dc_or_reset_drift() {
    let input = sine(0.5);
    let mut results = Vec::<Vec<f32>>::new();
    for mode in 0..4 {
        let mut effect = configured(
            "oxitone.distortion",
            &[("mode", mode as f64), ("driveDb", 18.), ("outputDb", -8.)],
        );
        assert_eq!(effect.latency_frames(), 36);
        let (left, right) = process(effect.as_mut(), &input, &[]);
        assert_eq!(left, right);
        assert!(left.iter().all(|v| v.is_finite() && v.abs() < 1.));
        assert!(rms(&left[12000..]) > 0.03);
        let dc = left[12000..].iter().map(|v| *v as f64).sum::<f64>() / 12000.;
        assert!(dc.abs() < 0.005, "mode {mode}: DC {dc}");
        for other in &results {
            assert!(
                rms(&left
                    .iter()
                    .zip(other)
                    .map(|(a, b)| a - b)
                    .collect::<Vec<_>>())
                    > 0.001
            );
        }
        effect.reset();
        assert_eq!((left.clone(), right), process(effect.as_mut(), &input, &[]));
        results.push(left);
    }
}

#[test]
fn multiband_reconstructs_at_zero_depth_compresses_boosts_and_does_not_raise_silence() {
    let input = sine(0.5);
    let mut dry = configured("oxitone.multiband", &[("depth", 0.)]);
    let (left, right) = process(dry.as_mut(), &input, &[]);
    assert_eq!(left, right);
    assert!(left.iter().zip(&input).all(|(a, b)| (a - b).abs() < 1e-6));
    let mut active = configured(
        "oxitone.multiband",
        &[
            ("depth", 1.),
            ("upperThresholdDb", -30.),
            ("downwardRatio", 12.),
            ("upwardDb", 0.),
        ],
    );
    let (hot, _) = process(active.as_mut(), &input, &[]);
    assert!(rms(&hot[12000..]) < rms(&input[12000..]) * 0.6);
    let quiet = sine(0.004);
    let mut up = configured(
        "oxitone.multiband",
        &[("depth", 1.), ("upwardDb", 18.), ("lowerThresholdDb", -30.)],
    );
    let (boosted, _) = process(up.as_mut(), &quiet, &[]);
    assert!(rms(&boosted[12000..]) > rms(&quiet[12000..]) * 1.5);
    up.reset();
    assert_eq!(
        process(up.as_mut(), &vec![0.; 24000], &[]),
        (vec![0.; 24000], vec![0.; 24000])
    );
}

#[test]
fn delay_is_stereo_synchronous_ping_pongs_and_ducks_the_wet_signal() {
    let settings = [("timeSeconds", 0.002), ("feedback", 0.6)];
    let mut impulse = vec![0.; 1024];
    impulse[0] = 1.;
    let mut plain = configured("oxitone.delay", &settings);
    let (left, right) = process(plain.as_mut(), &impulse, &[]);
    assert_eq!(left, right);
    assert_eq!(left[96], 1.);
    assert!(left[..96].iter().all(|v| *v == 0.));
    let mut ping = configured(
        "oxitone.delay",
        &[settings[0], settings[1], ("pingPong", 1.)],
    );
    let (left, right) = process(ping.as_mut(), &impulse, &[]);
    assert_eq!(left[96], 1.);
    assert_eq!(right[96], 0.);
    assert!(right[192] > 0.3);
    assert_eq!(left[192], 0.);
    ping.reset();
    assert_eq!((left, right), process(ping.as_mut(), &impulse, &[]));
    let mut duck = configured(
        "oxitone.delay",
        &[settings[0], settings[1], ("ducking", 1.)],
    );
    let (ducked, _) = process(duck.as_mut(), &impulse, &[]);
    assert!(ducked[96] < 0.2);
    let input = vec![0.1; 24000];
    let mut filtered = configured(
        "oxitone.delay",
        &[settings[0], settings[1], ("highpassHz", 800.)],
    );
    let (hp, _) = process(filtered.as_mut(), &input, &[]);
    plain.reset();
    let (lp, _) = process(plain.as_mut(), &input, &[]);
    assert!(rms(&hp[12000..]) < rms(&lp[12000..]) * 0.6);
}
