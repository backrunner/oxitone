use super::tuning_tests::{configured, notes};
use crate::testutil::*;
use oxitone_graph::PluginInstance;

#[test]
fn banks_warps_fm_ring_curves_and_matrix_change_pcm_and_reset() {
    let reference = run(&mut configured(&[]), &notes()).0;
    for values in [
        vec![("oscA.bank", 1.), ("oscA.position", 0.6)],
        vec![("oscA.bank", 2.), ("oscA.position", 0.4)],
        vec![("oscA.bank", 3.), ("oscA.position", 0.7)],
        vec![("oscA.warpMode", 1.), ("oscA.warp", 0.8)],
        vec![("oscA.warpMode", 2.), ("oscA.warp", 0.8)],
        vec![("oscA.warpMode", 3.), ("oscA.warp", 0.8)],
        vec![("fm", 0.7), ("oscB.octave", 1.)],
        vec![("ring", 0.7)],
        vec![("amp.attack", 0.1), ("amp.attackCurve", 0.8)],
        vec![
            ("mod.0.source", 2.),
            ("mod.0.target", 0.),
            ("mod.0.amount", 0.3),
        ],
        vec![
            ("mod.0.source", 5.),
            ("mod.0.target", 10.),
            ("mod.0.amount", -0.8),
            ("modEnv.decay", 0.3),
        ],
        vec![
            ("mod.0.source", 9.),
            ("mod.0.target", 7.),
            ("mod.0.amount", 0.8),
            ("macro1", 0.7),
        ],
        vec![
            ("mod.0.source", 8.),
            ("mod.0.target", 9.),
            ("mod.0.amount", 0.7),
        ],
    ] {
        let mut synth = configured(&values);
        let (left, right) = run(&mut synth, &notes());
        assert!(left.iter().chain(&right).all(|v| v.is_finite()));
        let delta: Vec<_> = left
            .iter()
            .chain(&right)
            .zip(reference.iter().chain(&reference))
            .map(|(a, b)| a - b)
            .collect();
        assert!(rms(&delta) > 0.001, "ineffective {values:?}");
        synth.reset();
        assert_eq!((left, right), run(&mut synth, &notes()), "reset {values:?}");
    }
}

#[test]
fn new_neutral_controls_preserve_omitted_defaults() {
    assert_eq!(
        run(&mut configured(&[]), &notes()),
        run(
            &mut configured(&[
                ("oscA.octave", 0.),
                ("oscB.octave", 0.),
                ("oscA.level", 1.),
                ("oscB.level", 1.),
                ("sub.wave", 0.),
                ("sub.octave", -1.),
                ("oscA.bank", 0.),
                ("oscB.bank", 0.),
                ("oscA.warpMode", 0.),
                ("oscA.warp", 0.),
                ("fm", 0.),
                ("ring", 0.),
                ("amp.attackCurve", 0.),
                ("amp.decayCurve", 0.),
                ("amp.releaseCurve", 0.),
            ]),
            &notes()
        )
    );
}
