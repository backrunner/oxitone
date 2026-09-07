use super::*;
use crate::{testutil::*, InstrumentConfig};
use oxitone_graph::{ParameterEvent, PluginInstance};
use std::collections::BTreeMap;

pub(super) fn configured(given: &[(&str, f64)]) -> WavetableSynthInstance {
    let mut values = BTreeMap::from([
        ("oscA.wavetable".into(), 0.),
        ("oscB.wavetable".into(), 0.),
        ("filter.cutoff".into(), 20000.),
        ("amp.attack".into(), 0.),
        ("amp.decay".into(), 0.),
        ("amp.sustain".into(), 1.),
    ]);
    values.extend(given.iter().map(|(k, v)| ((*k).into(), *v)));
    WavetableSynthPlugin
        .create_configured(&host(), &InstrumentConfig::new(&values))
        .unwrap()
}
pub(super) fn notes() -> Vec<Block<'static>> {
    std::iter::once(Block {
        notes: vec![note_on(17, 69, 0.8)],
        params: vec![],
    })
    .chain(silence(374))
    .collect()
}
fn frequency(samples: &[f32]) -> f64 {
    // Interpolate rising zero crossings after the initial filter transient.
    let crossings: Vec<_> = samples[4800..]
        .windows(2)
        .enumerate()
        .filter(|(_, w)| w[0] <= 0. && w[1] > 0.)
        .map(|(i, w)| i as f64 - w[0] as f64 / (w[1] - w[0]) as f64)
        .collect();
    assert!(crossings.len() > 2);
    (crossings.len() - 1) as f64 * 48000. / (crossings.last().unwrap() - crossings[0])
}

#[test]
fn a_b_and_sub_octaves_double_frequency_and_add_to_semitone_pitch() {
    for octave in -4..=4 {
        for osc in ["oscA", "oscB", "sub"] {
            let octave_id = format!("{osc}.octave");
            let mut settings = vec![(octave_id.as_str(), octave as f64)];
            if osc == "oscB" {
                settings.push(("osc.mix", 1.));
            }
            if osc == "sub" {
                settings.extend([("oscA.level", 0.), ("sub.level", 0.7)]);
            }
            let (left, right) = run(&mut configured(&settings), &notes());
            let expected = 440. * 2f64.powi(octave);
            let actual = frequency(&left);
            assert!(
                (actual / expected - 1.).abs() < 0.001,
                "{osc} {octave}: {actual} != {expected}"
            );
            assert_eq!(left, right, "single voices and sub are centered");
        }
    }
    let (pcm, _) = run(
        &mut configured(&[("oscA.octave", -1.), ("oscA.pitch", 12.)]),
        &notes(),
    );
    assert!((frequency(&pcm) - 440.).abs() < 0.1);
}

#[test]
fn sub_waveforms_are_distinct_centered_finite_and_reset_deterministically() {
    let mut previous = Vec::<Vec<f32>>::new();
    for wave in 0..6 {
        let mut synth = configured(&[
            ("oscA.level", 0.),
            ("sub.level", 0.8),
            ("sub.wave", wave as f64),
        ]);
        let (left, right) = run(&mut synth, &notes());
        assert_eq!(left, right);
        assert!(left.iter().all(|v| v.is_finite()));
        assert!(rms(&left) > 0.05);
        assert!(left[..17].iter().all(|v| *v == 0.));
        for other in &previous {
            let delta: Vec<_> = left.iter().zip(other).map(|(a, b)| a - b).collect();
            assert!(rms(&delta) > 0.005, "wave {wave} duplicates another source");
        }
        synth.reset();
        assert_eq!((left.clone(), right), run(&mut synth, &notes()));
        previous.push(left);
    }
}

#[test]
fn sub_is_independent_of_main_tuning_and_source_cycles_have_no_dc() {
    let base = [
        ("oscA.level", 0.),
        ("oscB.level", 0.),
        ("sub.level", 0.7),
        ("sub.wave", 4.),
    ];
    let mut transposed = base.to_vec();
    transposed.extend([
        ("oscA.octave", 4.),
        ("oscB.octave", -4.),
        ("oscA.pitch", 24.),
    ]);
    assert_eq!(
        run(&mut configured(&base), &notes()),
        run(&mut configured(&transposed), &notes())
    );
    for wave in 0..6 {
        let cycle = preview_sub_cycle(wave);
        let dc = cycle[..256].iter().sum::<f32>() / 256.;
        assert!(dc.abs() < 0.002, "wave {wave}: DC {dc}");
    }
    let mut high = configured(&[
        ("oscA.octave", 4.),
        ("oscB.octave", 4.),
        ("sub.octave", 4.),
        ("sub.level", 1.),
    ]);
    let (pcm, _) = run(
        &mut high,
        &[Block {
            notes: vec![note_on(0, 127, 1.)],
            params: vec![],
        }],
    );
    assert!(
        pcm.iter().all(|v| *v == 0.),
        "above-Nyquist oscillators must be suppressed"
    );
}

#[test]
fn pitch_and_wave_events_apply_at_the_requested_frame_and_reject_bad_initial_values() {
    for (id, value) in [
        ("oscA.octave", 1.),
        ("oscB.octave", -1.),
        ("sub.octave", 2.),
        ("sub.wave", 4.),
    ] {
        let settings = [("osc.mix", 0.5), ("sub.level", 0.6)];
        let mut baseline = configured(&settings);
        let mut edited = configured(&settings);
        let initial = &notes()[..2];
        run(&mut baseline, initial);
        run(&mut edited, initial);
        let mut event = silence(1);
        event[0].params.push(ParameterEvent {
            frame_offset: 63,
            parameter_id: id,
            value,
        });
        let (a, _) = run(&mut baseline, &silence(1));
        let (b, _) = run(&mut edited, &event);
        assert_eq!(&a[..63], &b[..63]);
        assert_ne!(&a[64..], &b[64..], "{id}");
    }
    for (id, value) in [
        ("oscA.octave", 4.1),
        ("oscB.octave", -5.),
        ("sub.octave", 0.5),
        ("sub.wave", 6.),
    ] {
        let values = BTreeMap::from([(id.into(), value)]);
        assert!(WavetableSynthPlugin
            .create_configured(&host(), &InstrumentConfig::new(&values))
            .is_err());
    }
}
