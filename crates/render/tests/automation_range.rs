#[path = "common/allocations.rs"]
mod allocations;
use oxitone_core::{
    beat::Beat,
    wire::{AutomationSourceSpec as Source, ChanceSpec, RandomPhase, TempoSegment, UnaryOp},
};
use oxitone_transport::{CompiledAutomation, EvalContext, TempoMap};

fn beat(value: i64) -> Beat {
    Beat::new(value, 1).unwrap()
}
fn overlay(base: Source, replacement: Source, start: i64, end: i64, fade: Option<Beat>) -> Source {
    Source::ReplaceRange {
        base: Box::new(base),
        replacement: Box::new(replacement),
        start_beat: beat(start),
        end_beat: beat(end),
        fade_beats: fade,
    }
}
fn compile(source: &Source) -> CompiledAutomation {
    CompiledAutomation::compile(source, 42).unwrap()
}
fn chance(phase: RandomPhase) -> Source {
    Source::Chance(ChanceSpec {
        probability: 0.5,
        seed: 7,
        rate: Some(4.),
        interval_beats: None,
        smooth_beats: None,
        random_phase: Some(phase),
    })
}

#[test]
fn range_shared_golden_and_round_trip() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../schemas/fixtures/automation-range.json"
    ))
    .unwrap();
    let beats = fixture["beats"].as_array().unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let source: Source = serde_json::from_value(case["source"].clone()).unwrap();
        assert_eq!(serde_json::to_value(&source).unwrap(), case["source"]);
        let compiled = compile(&source);
        for (beat, expected) in beats.iter().zip(case["expected"].as_array().unwrap()) {
            let actual = compiled.value_at(beat.as_f64().unwrap(), &EvalContext::default());
            assert!(
                (actual - expected.as_f64().unwrap()).abs()
                    < if case["name"] == "bezier-and-step-control-points" {
                        1e-8
                    } else {
                        1e-12
                    },
                "{case}: {beat}: {actual}"
            );
        }
    }
}

#[test]
fn overlays_preserve_base_chance_draws_and_the_clock_outside_selection() {
    for phase in [RandomPhase::Absolute, RandomPhase::Restart] {
        let base = chance(phase);
        let reference = compile(&base);
        let edited = compile(&overlay(base, Source::Constant { value: 0.37 }, 2, 4, None));
        for iteration in 0..4 {
            let ctx = EvalContext {
                origin_beat: 0.75,
                loop_iteration: iteration,
            };
            // Seek in reverse order too: no sequential random state or range reset.
            for step in (0..256).rev() {
                let t = f64::from(step) / 8.;
                let expected = if (2.0..4.0).contains(&t) {
                    0.37
                } else {
                    reference.value_at(t, &ctx)
                };
                assert_eq!(edited.value_at(t, &ctx), expected);
            }
        }
    }
}

#[test]
fn boundaries_ordering_and_fade_do_not_change_unselected_time() {
    let first = overlay(
        Source::Constant { value: 0.25 },
        Source::Constant { value: 1. },
        1,
        5,
        None,
    );
    let second = compile(&overlay(
        first,
        Source::Constant { value: 0.5 },
        2,
        4,
        Some(Beat::new(1, 2).unwrap()),
    ));
    let ctx = EvalContext::default();
    for (t, expected) in [
        (0., 0.25),
        (1., 1.),
        (2., 1.),
        (2.25, 0.75),
        (2.5, 0.5),
        (3.75, 0.75),
        (4., 1.),
        (5., 0.25),
    ] {
        assert_eq!(second.value_at(t, &ctx), expected);
    }
    assert_eq!(
        second.discontinuities(0., 6., &ctx),
        [1., 2., 2.5, 3.5, 4., 5.]
    );
    assert!(second.has_edge(1.9, 2., &ctx));
    assert!(!second.has_edge(2.6, 2.7, &ctx));
    let overflow = Source::Unary {
        op: UnaryOp::Scale,
        input: Box::new(Source::Constant { value: f64::MAX }),
        amount: Some(f64::MAX),
        steps: None,
        min: None,
        max: None,
    };
    let bounded = compile(&overlay(
        Source::Constant { value: 0.25 },
        overflow,
        2,
        4,
        Some(beat(1)),
    ));
    assert_eq!(bounded.value_at(2., &ctx), 0.25); // Zero blend weight must not multiply an infinite replacement.
}

#[test]
fn range_evaluation_is_allocation_free_and_matches_across_audio_block_sizes() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../schemas/fixtures/automation-range.json"
    ))
    .unwrap();
    let source: Source = serde_json::from_value(fixture["cases"][0]["source"].clone()).unwrap();
    let source = compile(&source);
    let tempo = TempoMap::compile(
        &[TempoSegment {
            start_beat: beat(0),
            bpm: 120.,
            curve: None,
        }],
        48_000,
    )
    .unwrap();
    let ctx = EvalContext::default();
    let start = 47_000;
    let count = 50_000;
    let mut reference = vec![0.; count];
    source.fill_segment(start, count, &tempo, &ctx, &mut reference);
    for block in [64, 128, 256] {
        let mut rendered = vec![0.; count];
        assert_eq!(
            allocations::count(|| {
                for (index, output) in rendered.chunks_mut(block).enumerate() {
                    source.fill_segment(
                        start + (index * block) as u64,
                        output.len(),
                        &tempo,
                        &ctx,
                        output,
                    );
                    std::hint::black_box(source.has_edge(1.99, 2., &ctx));
                }
            }),
            (0, 0)
        );
        assert_eq!(rendered, reference);
    }
    assert_eq!(reference[1000], 0.2);
    assert_eq!(reference[49_000], 0.25);
}

#[test]
fn invalid_ranges_and_hidden_tempo_chance_are_rejected() {
    let constant = Source::Constant { value: 0.5 };
    for source in [
        overlay(constant.clone(), constant.clone(), 2, 2, None),
        overlay(constant.clone(), constant.clone(), 2, 1, None),
        overlay(constant.clone(), constant.clone(), 2, 3, Some(beat(1))),
    ] {
        assert_eq!(
            CompiledAutomation::compile(&source, 0).unwrap_err().code,
            "AutomationRange"
        );
    }
    for source in [
        overlay(chance(RandomPhase::Absolute), constant.clone(), 0, 1, None),
        overlay(constant, chance(RandomPhase::Restart), 0, 1, None),
    ] {
        assert_eq!(
            oxitone_transport::automation::ensure_transport_invariant(&source)
                .unwrap_err()
                .code,
            "AutomationTempoRestriction"
        );
    }
}
