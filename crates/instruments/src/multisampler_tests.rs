use super::*;
use crate::testutil::*;
use std::collections::BTreeMap;

fn configured(transpose: f64) -> SamplerInstance {
    let sample = |hz: f64| {
        Arc::new(prepared(
            vec![(0..48000)
                .map(|i| (std::f64::consts::TAU * hz * i as f64 / 48000.).sin() as f32)
                .collect()],
            None,
        ))
    };
    let samples = MapSamples(BTreeMap::from([
        ("soft".into(), sample(440.)),
        ("loud".into(), sample(660.)),
    ]));
    let resources = BTreeMap::from([("a".into(), "soft".into()), ("b".into(), "loud".into())]);
    let parameters = BTreeMap::from([("transpose".into(), transpose), ("amp.attack".into(), 0.)]);
    let state = serde_json::json!({"version":1,"regions":[
        {"resource":"a","rootKey":60,"keyRange":[48,72],"velocityRange":[1,64]},
        {"resource":"b","rootKey":60,"keyRange":[48,72],"velocityRange":[65,127]}
    ]});
    MultisamplerPlugin
        .create_configured(
            &host(),
            &InstrumentConfig {
                parameters: &parameters,
                resources: Some(&resources),
                state: Some(&state),
            },
            &samples,
        )
        .unwrap()
}

#[test]
fn descriptor_and_shared_parameter_indices() {
    descriptor().validate().unwrap();
    let ours = parameter_specs();
    assert_eq!(ours[TRANSPOSE].id, "transpose");
    assert_eq!(ours[TRANSPOSE].default, 0.);
    assert_eq!(&ours[1..], &crate::sampler::parameter_specs()[1..]);
}

#[test]
fn velocity_layers_key_gaps_pitch_and_transpose() {
    for (key, velocity, transpose, expected) in [
        (60, 0.5, 0., 440.),
        (60, 65. / 127., 0., 660.),
        (72, 0.5, 0., 880.),
        (60, 0.5, 12., 880.),
        (60, 0.5, -12., 220.),
        (47, 1., 0., 0.),
        (73, 1., 0., 0.),
    ] {
        let mut instance = configured(transpose);
        let blocks: Vec<_> = std::iter::once(Block {
            notes: vec![note_on(0, key, velocity)],
            params: vec![],
        })
        .chain(silence(100))
        .collect();
        let (l, _) = run(&mut instance, &blocks);
        if expected == 0. {
            assert_eq!(peak(&l), 0.);
            continue;
        }
        let region = &l[20 * FRAMES..80 * FRAMES];
        let hz = region
            .windows(2)
            .filter(|w| w[0] <= 0. && w[1] > 0.)
            .count() as f64
            * 48000.
            / region.len() as f64;
        assert!(
            (hz - expected).abs() < expected * 0.025,
            "{key} / {velocity} / {transpose}: {hz} != {expected}"
        );
        assert!(rms(region) > 0.1);
        instance.reset();
        assert_eq!(peak(&run(&mut instance, &silence(1)).0), 0.);
    }
}

#[test]
fn missing_prepared_sample_and_unknown_parameter_fail() {
    let state = serde_json::json!({"version":1,"regions":[
        {"resource":"a","rootKey":60,"keyRange":[0,127],"velocityRange":[1,127]}
    ]});
    let resources = BTreeMap::from([("a".into(), "missing".into())]);
    for (params, code) in [
        (BTreeMap::new(), codes::ASSET_UNAVAILABLE),
        (
            BTreeMap::from([("rootKey".into(), 60.)]),
            codes::INVALID_PROJECT,
        ),
    ] {
        let error = MultisamplerPlugin
            .create_configured(
                &host(),
                &InstrumentConfig {
                    parameters: &params,
                    resources: Some(&resources),
                    state: Some(&state),
                },
                &MapSamples(BTreeMap::new()),
            )
            .err()
            .unwrap();
        assert_eq!(error.code, code);
    }
}
