//! Golden vectors for the automation evaluator (07-automation-spec.md §7).

use oxitone_core::beat::Beat;
use oxitone_core::error::codes;
use oxitone_core::wire::{
    AutomationPoint, AutomationSourceSpec, BinaryOp, ChanceSpec, Curve, CurveKind, RandomPhase,
    TempoCurve, TempoSegment, UnaryOp, WaveKind,
};
use oxitone_transport::{CompiledAutomation, EvalContext, TempoMap};

const SEED: u64 = 0;
const EPS: f64 = 1e-9;

fn beat(n: i64, d: u32) -> Beat {
    Beat::new(n, d).unwrap()
}

fn point(n: i64, d: u32, value: f64, curve: Option<Curve>) -> AutomationPoint {
    AutomationPoint {
        beat: beat(n, d),
        value,
        curve,
    }
}

fn compile(spec: &AutomationSourceSpec) -> CompiledAutomation {
    CompiledAutomation::compile(spec, SEED).unwrap()
}

fn value(spec: &AutomationSourceSpec, t: f64) -> f64 {
    compile(spec).value_at(t, &EvalContext::default())
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < EPS,
        "expected {expected}, got {actual}"
    );
}

fn assert_close_e6(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-6,
        "expected {expected}, got {actual}"
    );
}

fn gate(duty: f64) -> AutomationSourceSpec {
    AutomationSourceSpec::Gate {
        period_beats: beat(1, 1),
        duty,
        phase: None,
        on: None,
        off: None,
    }
}

#[test]
fn gate_duty_boundaries() {
    // duty = 0: always off; duty = 1: always on.
    for t in [0.0, 0.25, 0.5, 0.999, 1.0, 1.5] {
        assert_eq!(value(&gate(0.0), t), 0.0, "duty 0 at {t}");
        assert_eq!(value(&gate(1.0), t), 1.0, "duty 1 at {t}");
    }
    // duty = 0.5: u < duty -> on, left-closed/right-open at the edge.
    let spec = gate(0.5);
    assert_eq!(value(&spec, 0.0), 1.0, "u = 0");
    assert_eq!(value(&spec, 0.499), 1.0, "just before the edge");
    assert_eq!(value(&spec, 0.5), 0.0, "edge belongs to the right interval");
    assert_eq!(value(&spec, 0.999), 0.0, "end of period");
    assert_eq!(value(&spec, 1.0), 1.0, "next period start");
    assert_eq!(value(&spec, 1.5), 0.0, "next period edge");
}

fn wave(wave: WaveKind) -> AutomationSourceSpec {
    AutomationSourceSpec::Wave {
        wave,
        period_beats: beat(1, 1),
        phase: None,
        min: None,
        max: None,
        pulse_width: None,
    }
}

#[test]
fn wave_phase_points() {
    let us = [0.0, 0.25, 0.5, 0.75, 0.999];
    let expected: [(WaveKind, [f64; 5]); 6] = [
        (WaveKind::Sine, [0.5, 1.0, 0.5, 0.0, 0.496858]),
        (WaveKind::Cos, [1.0, 0.5, 0.0, 0.5, 0.99999013]),
        (WaveKind::Triangle, [0.0, 0.5, 1.0, 0.5, 0.002]),
        (WaveKind::Saw, [0.0, 0.25, 0.5, 0.75, 0.999]),
        (WaveKind::Ramp, [1.0, 0.75, 0.5, 0.25, 0.001]),
        (WaveKind::Square, [1.0, 1.0, 0.0, 0.0, 0.0]),
    ];
    for (kind, values) in expected {
        let spec = wave(kind);
        for (u, expected) in us.iter().zip(values) {
            assert_close_e6(value(&spec, *u), expected);
        }
    }
    // sine starts at the midpoint rising, cos starts at 1 (fixed convention).
    let sine = wave(WaveKind::Sine);
    assert!(value(&sine, 0.001) > 0.5, "sine rises from the midpoint");
    assert_eq!(value(&wave(WaveKind::Cos), 0.0), 1.0);
}

#[test]
fn curve_before_at_after_and_interpolations() {
    let make = |kind: CurveKind| AutomationSourceSpec::Curve {
        interpolation: kind,
        points: vec![
            point(0, 1, 0.25, None),
            point(1, 1, 1.0, None),
            point(2, 1, 0.5, None),
        ],
    };
    for kind in [
        CurveKind::Step,
        CurveKind::Linear,
        CurveKind::Smooth,
        CurveKind::Exponential,
    ] {
        let spec = make(kind);
        assert_eq!(value(&spec, -0.5), 0.25, "{kind:?} before the first point");
        assert_eq!(value(&spec, 0.0), 0.25, "{kind:?} at the first point");
        assert_eq!(value(&spec, 1.0), 1.0, "{kind:?} at a middle point");
        assert_eq!(value(&spec, 2.0), 0.5, "{kind:?} at the last point");
        assert_eq!(value(&spec, 3.0), 0.5, "{kind:?} after the last point");
    }
    assert_eq!(value(&make(CurveKind::Step), 0.999), 0.25);
    assert_eq!(value(&make(CurveKind::Step), 1.0), 1.0);
    assert_eq!(value(&make(CurveKind::Linear), 0.5), 0.625);
    assert_eq!(value(&make(CurveKind::Smooth), 0.5), 0.625);
    assert_close(value(&make(CurveKind::Smooth), 0.25), 0.3671875);
    // exponential: 0.25 * (1/0.25)^x -> 0.5 at x = 0.5.
    assert_close(value(&make(CurveKind::Exponential), 0.5), 0.5);
}

#[test]
fn curve_bezier_monotone_time_axis() {
    // Fixture shape (schemas/fixtures/automation/curve.canonical.json):
    // out [0.3, 0], in [0.7, 1] over beats 0..2, values 0..1; symmetric S.
    let spec = AutomationSourceSpec::Curve {
        interpolation: CurveKind::Smooth,
        points: vec![
            point(
                0,
                1,
                0.0,
                Some(Curve::Bezier {
                    out: [0.3, 0.0],
                    r#in: [0.7, 1.0],
                }),
            ),
            point(2, 1, 1.0, None),
        ],
    };
    assert_close(value(&spec, 0.0), 0.0);
    assert_eq!(value(&spec, 2.0), 1.0);
    assert_close(value(&spec, 1.0), 0.5);
    let mut previous = -1.0;
    for step in 0..=64 {
        let v = value(&spec, 2.0 * step as f64 / 64.0);
        assert!(v >= previous, "bezier must be monotonic at step {step}");
        previous = v;
    }
}

#[test]
fn curve_continuity_across_points_and_tempo_boundaries() {
    let spec = AutomationSourceSpec::Curve {
        interpolation: CurveKind::Linear,
        points: vec![point(0, 1, 0.0, None), point(8, 1, 1.0, None)],
    };
    let evaluator = compile(&spec);
    let ctx = EvalContext::default();
    // Continuous across the interior of the segment and across a tempo
    // segment boundary at beat 4 (tempo only remaps beats to frames).
    let tempo = TempoMap::compile(
        &[
            TempoSegment {
                start_beat: beat(0, 1),
                bpm: 120.0,
                curve: None,
            },
            TempoSegment {
                start_beat: beat(4, 1),
                bpm: 90.0,
                curve: Some(TempoCurve::Step),
            },
        ],
        48_000,
    )
    .unwrap();
    let boundary_frame = tempo.beat_to_frame(beat(4, 1));
    let at = evaluator.value_at(tempo.frame_to_beat(boundary_frame).to_f64(), &ctx);
    assert_close(at, 0.5);
    let before = evaluator.value_at_frame(boundary_frame - 1, &tempo, &ctx) as f64;
    assert!(
        (at - before).abs() < 1e-3,
        "continuous across tempo boundary"
    );
}

fn chance_spec(probability: f64, random_phase: Option<RandomPhase>) -> AutomationSourceSpec {
    AutomationSourceSpec::Chance(ChanceSpec {
        probability,
        seed: 17,
        smooth_beats: None,
        random_phase,
        rate: Some(2.0),
        interval_beats: None,
    })
}

#[test]
fn chance_probability_extremes_skip_the_prng() {
    for t in [0.0, 0.5, 0.75, 3.25, 100.0] {
        assert_eq!(value(&chance_spec(0.0, None), t), 0.0);
        assert_eq!(value(&chance_spec(1.0, None), t), 1.0);
    }
}

#[test]
fn chance_golden_first_sixteen_decisions() {
    // pcg32-v1/hash64-v1 golden: rate 2 (interval 1/2), probability 0.72,
    // lane seed 17, project seed 0, canonical source path "0".
    let evaluator = compile(&chance_spec(0.72, None));
    let ctx = EvalContext::default();
    let decisions: Vec<u8> = (0..16)
        .map(|index| evaluator.value_at(index as f64 * 0.5, &ctx) as u8)
        .collect();
    let expected: [u8; 16] = [1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 0, 1, 1, 1];
    assert_eq!(decisions, expected);
}

#[test]
fn chance_absolute_seek_is_order_independent() {
    let evaluator = compile(&chance_spec(0.72, None));
    let ctx = EvalContext::default();
    let first = evaluator.value_at(5.5, &ctx);
    let _ = evaluator.value_at(0.5, &ctx);
    let _ = evaluator.value_at(3.0, &ctx);
    assert_eq!(evaluator.value_at(5.5, &ctx), first, "seek round trip");
}

#[test]
fn chance_restart_iterations_differ_but_reproduce() {
    let evaluator = compile(&chance_spec(0.72, Some(RandomPhase::Restart)));
    let sequence = |iteration: u64| {
        let ctx = EvalContext {
            origin_beat: 4.0,
            loop_iteration: iteration,
        };
        (0..16)
            .map(|index| evaluator.value_at(4.0 + index as f64 * 0.5, &ctx) as u8)
            .collect::<Vec<u8>>()
    };
    let iteration0 = sequence(0);
    let iteration1 = sequence(1);
    assert_eq!(sequence(0), iteration0, "same iteration reproduces");
    assert_eq!(sequence(1), iteration1, "same iteration reproduces");
    assert_ne!(iteration0, iteration1, "loop iteration feeds the seed");
    // Restart origin: t = origin maps to decision index 0 of the iteration.
    let at_origin = evaluator.value_at(
        4.0,
        &EvalContext {
            origin_beat: 4.0,
            loop_iteration: 0,
        },
    );
    assert_eq!(at_origin, f64::from(iteration0[0]));
}

#[test]
fn chance_smooth_transitions_between_decisions() {
    let spec = AutomationSourceSpec::Chance(ChanceSpec {
        probability: 0.5,
        seed: 5,
        smooth_beats: Some(beat(1, 2)),
        random_phase: None,
        rate: Some(1.0),
        interval_beats: None,
    });
    let evaluator = compile(&spec);
    let ctx = EvalContext::default();
    // smoothBeats = 1/2 < interval = 1: at a decision point the value is the
    // OLD decision (dt = 0 starts the ramp), transitions over half a beat,
    // then holds the new decision.
    for boundary in 1..32 {
        let at = |t: f64| evaluator.value_at(t, &ctx);
        let old = at(boundary as f64);
        let new = at(boundary as f64 + 1.0);
        if old != new {
            assert_close(at(boundary as f64 + 0.25), 0.5);
            assert_eq!(at(boundary as f64 + 0.5), new);
            return;
        }
    }
    panic!("expected at least one decision transition in 32 indices");
}

fn constant(value: f64) -> AutomationSourceSpec {
    AutomationSourceSpec::Constant { value }
}

#[test]
fn nested_composition_and_final_clamp() {
    // mix(map(invert(0.8), 0.2..0.9), saw(4 beats), 0.25) at t = 0:
    // invert -> 0.2; map -> 0.2 + 0.2 * 0.7 = 0.34; saw(0) = 0;
    // mix -> 0.75 * 0.34 + 0.25 * 0 = 0.255.
    let nested = AutomationSourceSpec::Binary {
        op: BinaryOp::Mix,
        left: Box::new(AutomationSourceSpec::Map {
            input: Box::new(AutomationSourceSpec::Unary {
                op: UnaryOp::Invert,
                input: Box::new(constant(0.8)),
                steps: None,
                amount: None,
                min: None,
                max: None,
            }),
            min: 0.2,
            max: 0.9,
        }),
        right: Box::new(AutomationSourceSpec::Wave {
            wave: WaveKind::Saw,
            period_beats: beat(4, 1),
            phase: None,
            min: None,
            max: None,
            pulse_width: None,
        }),
        amount: Some(0.25),
    };
    assert_close(value(&nested, 0.0), 0.255);
    assert_close(value(&nested, 2.0), 0.75 * 0.34 + 0.25 * 0.5);

    // Intermediate values may exceed 0..1; the lane clamps once at the end.
    let add = AutomationSourceSpec::Binary {
        op: BinaryOp::Add,
        left: Box::new(constant(0.8)),
        right: Box::new(constant(0.8)),
        amount: None,
    };
    assert_eq!(value(&add, 0.0), 1.0);
    let offset = AutomationSourceSpec::Unary {
        op: UnaryOp::Offset,
        input: Box::new(constant(0.2)),
        steps: None,
        amount: Some(-0.5),
        min: None,
        max: None,
    };
    assert_eq!(value(&offset, 0.0), 0.0);
    // multiply/add/min/max and quantize/scale spot checks.
    let multiply = AutomationSourceSpec::Binary {
        op: BinaryOp::Multiply,
        left: Box::new(constant(0.5)),
        right: Box::new(constant(0.5)),
        amount: None,
    };
    assert_eq!(value(&multiply, 0.0), 0.25);
    let quantize = AutomationSourceSpec::Unary {
        op: UnaryOp::Quantize,
        input: Box::new(constant(0.4)),
        steps: Some(5),
        amount: None,
        min: None,
        max: None,
    };
    assert_close(value(&quantize, 0.0), 0.5);
}

fn nest_unary(depth: usize) -> AutomationSourceSpec {
    let mut spec = constant(0.5);
    for _ in 1..depth {
        spec = AutomationSourceSpec::Unary {
            op: UnaryOp::Invert,
            input: Box::new(spec),
            steps: None,
            amount: None,
            min: None,
            max: None,
        };
    }
    spec
}

fn nest_binary(depth: usize) -> AutomationSourceSpec {
    if depth == 1 {
        return constant(0.5);
    }
    let child = nest_binary(depth - 1);
    AutomationSourceSpec::Binary {
        op: BinaryOp::Add,
        left: Box::new(child.clone()),
        right: Box::new(child),
        amount: None,
    }
}

#[test]
fn depth_and_node_limits() {
    let ok = nest_unary(64);
    assert!(CompiledAutomation::compile(&ok, SEED).is_ok());
    let too_deep = nest_unary(65);
    let error = CompiledAutomation::compile(&too_deep, SEED).unwrap_err();
    assert_eq!(error.code, codes::AUTOMATION_DEPTH_LIMIT);

    let wide_ok = nest_binary(8); // 255 nodes
    assert!(CompiledAutomation::compile(&wide_ok, SEED).is_ok());
    let too_wide = nest_binary(9); // 511 nodes
    let error = CompiledAutomation::compile(&too_wide, SEED).unwrap_err();
    assert_eq!(error.code, codes::AUTOMATION_NODE_LIMIT);
}

#[test]
fn validation_error_codes() {
    let bad_wave = AutomationSourceSpec::Wave {
        wave: WaveKind::Sine,
        period_beats: beat(0, 1),
        phase: None,
        min: None,
        max: None,
        pulse_width: None,
    };
    assert_eq!(
        CompiledAutomation::compile(&bad_wave, SEED)
            .unwrap_err()
            .code,
        codes::AUTOMATION_PERIOD
    );
    let bad_points = AutomationSourceSpec::Curve {
        interpolation: CurveKind::Linear,
        points: vec![point(1, 1, 0.0, None), point(1, 1, 1.0, None)],
    };
    assert_eq!(
        CompiledAutomation::compile(&bad_points, SEED)
            .unwrap_err()
            .code,
        codes::AUTOMATION_POINTS
    );
    let bad_exp = AutomationSourceSpec::Curve {
        interpolation: CurveKind::Exponential,
        points: vec![point(0, 1, 0.0, None), point(1, 1, 1.0, None)],
    };
    assert_eq!(
        CompiledAutomation::compile(&bad_exp, SEED)
            .unwrap_err()
            .code,
        codes::AUTOMATION_EXPONENTIAL_ZERO
    );
    let bad_chance = AutomationSourceSpec::Chance(ChanceSpec {
        probability: 0.5,
        seed: 1,
        smooth_beats: None,
        random_phase: None,
        rate: None,
        interval_beats: None,
    });
    assert_eq!(
        CompiledAutomation::compile(&bad_chance, SEED)
            .unwrap_err()
            .code,
        codes::AUTOMATION_CHANCE_FREQUENCY
    );
}

#[test]
fn parity_across_block_sizes() {
    // Same serialized source, same tempo map: block sizes 64/128/256 must
    // produce identical values at identical sample frames.
    let spec = AutomationSourceSpec::Binary {
        op: BinaryOp::Add,
        left: Box::new(AutomationSourceSpec::Wave {
            wave: WaveKind::Sine,
            period_beats: beat(3, 1),
            phase: None,
            min: Some(0.2),
            max: Some(0.9),
            pulse_width: None,
        }),
        right: Box::new(AutomationSourceSpec::Curve {
            interpolation: CurveKind::Smooth,
            points: vec![point(0, 1, 0.0, None), point(8, 1, 0.5, None)],
        }),
        amount: None,
    };
    let evaluator = compile(&spec);
    let ctx = EvalContext::default();
    let tempo = TempoMap::compile(
        &[
            TempoSegment {
                start_beat: beat(0, 1),
                bpm: 120.0,
                curve: None,
            },
            TempoSegment {
                start_beat: beat(4, 1),
                bpm: 90.0,
                curve: Some(TempoCurve::Step),
            },
        ],
        48_000,
    )
    .unwrap();
    let total_frames = tempo.beat_to_frame(beat(8, 1)) as usize;
    let render = |block: usize| {
        let mut out = vec![0.0_f32; total_frames];
        let mut start = 0;
        while start < total_frames {
            let frames = block.min(total_frames - start);
            evaluator.fill_segment(
                start as u64,
                frames,
                &tempo,
                &ctx,
                &mut out[start..start + frames],
            );
            start += frames;
        }
        out
    };
    let b64 = render(64);
    let b128 = render(128);
    let b256 = render(256);
    assert_eq!(b64, b128, "64 vs 128 frame parity");
    assert_eq!(b128, b256, "128 vs 256 frame parity");
}

#[test]
fn discontinuities_cover_gate_square_curve_chance() {
    let spec = AutomationSourceSpec::Binary {
        op: BinaryOp::Add,
        left: Box::new(AutomationSourceSpec::Gate {
            period_beats: beat(1, 1),
            duty: 0.25,
            phase: None,
            on: None,
            off: None,
        }),
        right: Box::new(AutomationSourceSpec::Curve {
            interpolation: CurveKind::Step,
            points: vec![point(0, 1, 0.0, None), point(3, 2, 1.0, None)],
        }),
        amount: None,
    };
    let evaluator = compile(&spec);
    let edges = evaluator.discontinuities(0.0, 2.0, &EvalContext::default());
    assert_eq!(edges, vec![0.0, 0.25, 1.0, 1.25, 1.5]);

    let chance = compile(&chance_spec(0.5, None));
    let edges = chance.discontinuities(0.0, 2.0, &EvalContext::default());
    assert_eq!(edges, vec![0.0, 0.5, 1.0, 1.5]);
}
