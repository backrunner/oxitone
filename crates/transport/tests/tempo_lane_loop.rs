use oxitone_core::{
    wire::{
        AutomationLaneSpec, AutomationPoint, AutomationSourceSpec, AutomationTarget, CurveKind,
        LoopSpec,
    },
    Beat,
};
use oxitone_transport::{bake_tempo_lane_spec, TempoMap};

fn b(value: f64) -> Beat {
    Beat::from_f64(value).unwrap()
}
fn lane() -> AutomationLaneSpec {
    let value = |bpm: f64| (bpm / 20.0).ln() / (999.0_f64 / 20.0).ln();
    AutomationLaneSpec {
        id: "auto_tempo".into(),
        target: AutomationTarget {
            entity_id: "prj_test".into(),
            parameter_id: "tempo".into(),
        },
        source: AutomationSourceSpec::Curve {
            interpolation: CurveKind::Step,
            points: vec![
                AutomationPoint {
                    beat: b(0.0),
                    value: value(120.0),
                    curve: None,
                },
                AutomationPoint {
                    beat: b(0.5),
                    value: value(240.0),
                    curve: None,
                },
                AutomationPoint {
                    beat: b(1.0),
                    value: value(60.0),
                    curve: None,
                },
            ],
        },
        combine: None,
        last_beat: None,
        loop_spec: Some(LoopSpec {
            start_beat: None,
            length_beats: b(1.0),
            count: Some(2),
            last_beat: None,
        }),
    }
}

#[test]
fn tempo_loop_repeats_source_edges_and_holds_final_phase() {
    let segments = bake_tempo_lane_spec(&lane(), 0, 4.0, 0.0).unwrap();
    let map = TempoMap::compile(&segments, 48000).unwrap();
    for (beat, bpm) in [
        (0.0, 120.0),
        (0.5, 240.0),
        (1.0, 120.0),
        (1.5, 240.0),
        (2.0, 60.0),
        (3.0, 60.0),
    ] {
        assert!((map.bpm_at_beat(beat) - bpm).abs() < 1e-9, "beat {beat}");
    }
    for (beat, seconds) in [
        (0.5, 0.25),
        (1.0, 0.375),
        (1.5, 0.625),
        (2.0, 0.75),
        (3.0, 1.75),
    ] {
        assert!(
            (map.beat_to_seconds(b(beat)) - seconds).abs() < 1e-5,
            "beat {beat}"
        );
    }
}

#[test]
fn tempo_lane_last_beat_and_partial_final_loop_hold_the_boundary_value() {
    let mut source = lane();
    source.loop_spec = None;
    source.last_beat = Some(b(0.5));
    let table = bake_tempo_lane_spec(&source, 0, 4.0, 0.0).unwrap();
    let map = TempoMap::compile(&table, 48000).unwrap();
    assert!((map.beat_to_seconds(b(2.0)) - 0.625).abs() < 1e-5);
    source = lane();
    let region = source.loop_spec.as_mut().unwrap();
    region.count = None;
    region.last_beat = Some(b(1.75));
    let table = bake_tempo_lane_spec(&source, 0, 4.0, 0.0).unwrap();
    let map = TempoMap::compile(&table, 48000).unwrap();
    assert!((map.bpm_at_beat(3.0) - 240.0).abs() < 1e-9);
}

#[test]
fn enormous_tempo_horizon_is_rejected_before_allocating_the_grid() {
    let error = bake_tempo_lane_spec(&lane(), 0, 1e20, 0.0).unwrap_err();
    assert_eq!(error.code, oxitone_core::codes::TEMPO_MAP_COMPLEXITY);
}
