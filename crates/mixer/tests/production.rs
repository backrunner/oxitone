#[path = "../../render/tests/common/allocations.rs"]
mod allocations;
#[path = "production/common.rs"]
mod common;
#[path = "production/dynamics.rs"]
mod dynamics;
#[path = "production/spatial.rs"]
mod spatial;
use common::*;
use oxitone_graph::{HostContext, ParameterEvent, ProcessContext};

#[test]
fn production_effects_are_partition_independent_during_parameter_motion() {
    let input = sine(731., 0.2, 4096);
    for id in NEW {
        let plugin = oxitone_mixer::builtin_effect_plugins()
            .into_iter()
            .find(|p| p.descriptor().plugin_id == id)
            .unwrap();
        let params: Vec<_> = plugin
            .descriptor()
            .parameters
            .iter()
            .map(|p| (p.id.as_str(), p.min + (p.max - p.min) * 0.7))
            .collect();
        let mut results = Vec::new();
        for block in [64, 127, 256] {
            let mut instance = configured(id, &[]);
            let mut out = render(instance.as_mut(), &input[..333], &input[..333], block);
            set(instance.as_mut(), &params);
            let tail = render(instance.as_mut(), &input[333..], &input[333..], block);
            for ch in 0..2 {
                out[ch].extend(&tail[ch]);
            }
            results.push(out);
        }
        for other in &results[1..] {
            let delta = results[0]
                .iter()
                .flatten()
                .zip(other.iter().flatten())
                .map(|(a, b)| (a - b).abs())
                .fold(0f32, f32::max);
            assert!(delta < 2e-5, "{id}: block delta {delta}");
        }
    }
}

#[test]
fn every_effect_parameter_extreme_process_and_reset_remain_finite_and_allocation_free() {
    let input = std::array::from_fn::<_, 256, _>(|n| (n as f32 * 0.123).sin() * 1.5);
    for plugin in oxitone_mixer::builtin_effect_plugins() {
        let id = &plugin.descriptor().plugin_id;
        let mut instance = plugin.create(&HostContext {
            sample_rate: RATE,
            max_block_size: 256,
        });
        for high in [false, true] {
            let params: Vec<_> = plugin
                .descriptor()
                .parameters
                .iter()
                .map(|p| ParameterEvent {
                    frame_offset: 0,
                    parameter_id: &p.id,
                    value: if high { p.max } else { p.min },
                })
                .collect();
            let (mut left, mut right) = ([0.; 256], [0.; 256]);
            let count = allocations::count(|| {
                for block in 0..8 {
                    instance.process(&mut ProcessContext {
                        frames: 256,
                        sample_rate: RATE,
                        inputs: &[&input, &input],
                        outputs: &mut [&mut left, &mut right],
                        note_events: &[],
                        parameter_events: if block == 0 { &params } else { &[] },
                        sidechain: None,
                    });
                }
                instance.reset();
            });
            assert_eq!(count, (0, 0), "{id} allocations/frees");
            assert!(
                left.iter().chain(&right).all(|v| v.is_finite()),
                "{id} extreme finite"
            );
        }
    }
}

#[test]
fn frequency_shift_is_additive_with_suppressed_opposite_sideband() {
    let input = sine(1000., 0.4, 48000);
    for shift in [-200., 200.] {
        let mut instance = configured("oxitone.frequency-shifter", &[("shiftHz", shift)]);
        let out = render(instance.as_mut(), &input, &input, 128);
        let desired = amplitude(&out[0][24000..], 1000. + shift);
        let image = amplitude(&out[0][24000..], 1000. - shift);
        assert!(
            desired > 0.35 && image < desired * 0.01,
            "{shift}: desired {desired}, image {image}"
        );
    }
}

#[test]
fn pitch_shift_changes_ratio_and_unison_is_a_delayed_identity() {
    let input = sine(1000., 0.4, 48000);
    for (semitones, frequency) in [(12., 2000.), (-12., 500.)] {
        let mut instance = configured("oxitone.pitch-shifter", &[("semitones", semitones)]);
        let out = render(instance.as_mut(), &input, &input, 127);
        let desired = amplitude(&out[0][24000..], frequency);
        let original = amplitude(&out[0][24000..], 1000.);
        assert!(
            desired > 0.2 && original < desired * 0.02,
            "{semitones}: {desired} original {original}"
        );
    }
    let mut instance = configured("oxitone.pitch-shifter", &[("semitones", 0.)]);
    let latency = instance.latency_frames() as usize;
    let out = render(instance.as_mut(), &input, &input, 64);
    assert_eq!(&out[0][latency..], &input[..input.len() - latency]);
}

#[test]
fn nonlinear_filter_rejects_stopband_and_drive_changes_spectrum() {
    let low = sine(100., 0.1, 12000);
    let high = sine(8000., 0.1, 12000);
    let settings = [("cutoffHz", 700.), ("driveDb", 0.), ("resonance", 0.)];
    let mut instance = configured("oxitone.nonlinear-filter", &settings);
    let pass = render(instance.as_mut(), &low, &low, 128);
    instance.reset();
    let stop = render(instance.as_mut(), &high, &high, 128);
    assert!(rms(&stop[0][6000..]) < rms(&pass[0][6000..]) * 0.02);
    let mut driven = configured(
        "oxitone.nonlinear-filter",
        &[("cutoffHz", 10000.), ("driveDb", 24.)],
    );
    let input = sine(500., 0.4, 24000);
    let out = render(driven.as_mut(), &input, &input, 128);
    assert!(amplitude(&out[0][12000..], 1500.) > 0.005);
}

#[test]
fn bitcrush_quantizes_holds_and_resets_its_clock() {
    let input = sine(731., 0.5, 8192);
    let mut instance = configured("oxitone.bitcrush", &[("bits", 4.), ("rateHz", 12000.)]);
    let out = render(instance.as_mut(), &input, &input, 127);
    assert!(out[0].iter().all(|v| (*v * 8.).fract() == 0.));
    assert!(out[0].chunks_exact(4).all(|c| c.iter().all(|v| *v == c[0])));
    instance.reset();
    assert_eq!(out, render(instance.as_mut(), &input, &input, 256));
}
