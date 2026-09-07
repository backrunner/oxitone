use super::*;
use crate::{testutil::*, InstrumentConfig};
use oxitone_graph::{ParameterEvent, PluginInstance};
use std::collections::BTreeMap;

fn configured(given: &[(&str, f64)]) -> WavetableSynthInstance {
    let mut values = BTreeMap::from([("oscA.unison".into(), 3.), ("osc.mix".into(), 0.2)]);
    values.extend(given.iter().map(|(k, v)| ((*k).into(), *v)));
    WavetableSynthPlugin
        .create_configured(&host(), &InstrumentConfig::new(&values))
        .unwrap()
}
fn notes() -> Vec<Block<'static>> {
    std::iter::once(Block {
        notes: vec![note_on(17, 60, 0.8)],
        params: vec![],
    })
    .chain(silence(80))
    .collect()
}
#[test]
fn neutral_timbre_controls_preserve_default_samples() {
    let mut base = configured(&[]);
    let mut extended = configured(&[
        ("oscA.morphTo", 5.),
        ("oscB.morphTo", 4.),
        ("oscA.position", 0.),
        ("sub.level", 0.),
        ("noise.level", 0.),
        ("lfo.pitch", 0.),
        ("lfo.cutoff", 0.),
        ("lfo.positionA", 0.),
        ("lfo.positionB", 0.),
        ("lfo.level", 0.),
    ]);
    assert_eq!(run(&mut base, &notes()), run(&mut extended, &notes()));
}
#[test]
fn each_timbre_layer_and_modulation_destination_changes_audible_output() {
    let baseline = run(&mut configured(&[]), &notes()).0;
    for parameters in [
        vec![("oscA.morphTo", 5.), ("oscA.position", 0.65)],
        vec![("oscA.phaseSpread", 0.7)],
        vec![("sub.level", 0.4), ("sub.octave", -2.)],
        vec![("noise.level", 0.15)],
        vec![("lfo.rateHz", 8.), ("lfo.pitch", 1.)],
        vec![("lfo.rateHz", 8.), ("lfo.cutoff", -24.)],
        vec![("lfo.rateHz", 8.), ("lfo.positionA", 0.8)],
        vec![("lfo.rateHz", 8.), ("lfo.positionB", 0.8)],
        vec![("lfo.rateHz", 8.), ("lfo.level", 0.8)],
    ] {
        let (a, b) = run(&mut configured(&parameters), &notes());
        assert!(a.iter().chain(&b).all(|v| v.is_finite()));
        let diff: Vec<_> = a.iter().zip(&baseline).map(|(a, b)| a - b).collect();
        assert!(rms(&diff) > 0.001, "inaudible {parameters:?}");
    }
}
#[test]
fn motion_noise_reset_and_note_offsets_are_deterministic() {
    let mut synth = configured(&[
        ("noise.level", 0.2),
        ("sub.level", 0.2),
        ("lfo.shape", 1.),
        ("lfo.rateHz", 5.),
        ("lfo.cutoff", 12.),
        ("lfo.positionA", 0.4),
        ("oscA.phaseSpread", 0.6),
    ]);
    let a = run(&mut synth, &notes());
    assert!(a.0[..17].iter().all(|v| *v == 0.));
    synth.reset();
    assert_eq!(a, run(&mut synth, &notes()));
    let _ = run(
        &mut synth,
        &[Block {
            notes: vec![note_off(31, 60)],
            params: vec![ParameterEvent {
                frame_offset: 64,
                parameter_id: "lfo.positionA",
                value: 0.9,
            }],
        }],
    );
    let (tail, _) = run(&mut synth, &silence(300));
    assert!(tail[tail.len() - 128..].iter().all(|v| v.abs() < 1e-8));
}
#[test]
fn lfo_shapes_and_added_parameter_indices_are_stable() {
    use super::params as p;
    assert_eq!(parameter_specs().len(), 46);
    for (i, id) in [
        (p::OSC_A_MORPH_TO, "oscA.morphTo"),
        (p::OSC_B_POSITION, "oscB.position"),
        (p::SUB_LEVEL, "sub.level"),
        (p::LFO_SHAPE, "lfo.shape"),
        (p::LFO_LEVEL, "lfo.level"),
    ] {
        assert_eq!(parameter_specs()[i].id, id);
    }
    assert_eq!(lfo_value(1, 0.), -1.);
    assert_eq!(lfo_value(1, 0.5), 1.);
    assert_eq!(lfo_value(2, 0.), 1.);
    assert_eq!(lfo_value(3, 0.5), -1.);
    for shape in 0..4 {
        for i in 0..1024 {
            assert!((-1.0..=1.).contains(&lfo_value(shape, i as f64 / 1024.)));
        }
    }
}
