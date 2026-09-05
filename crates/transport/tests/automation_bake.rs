//! Golden vectors for tempo lane baking (03-audio-runtime-spec.md §Tempo
//! automation 烘焙, 07-automation-spec.md §6).

use oxitone_core::beat::Beat;
use oxitone_core::error::codes;
use oxitone_core::wire::{
    AutomationLaneSpec, AutomationPoint, AutomationSourceSpec, AutomationTarget, ChanceSpec,
    CurveKind, TempoCurve, WaveKind,
};
use oxitone_transport::{
    bake_tempo_lane, find_tempo_lane, normalized_to_bpm, TempoMap, TEMPO_BAKE_MAX_SEGMENTS,
};

const SEED: u64 = 0;

fn beat(n: i64, d: u32) -> Beat {
    Beat::new(n, d).unwrap()
}

fn point(n: i64, d: u32, value: f64) -> AutomationPoint {
    AutomationPoint {
        beat: beat(n, d),
        value,
        curve: None,
    }
}

fn ramp() -> AutomationSourceSpec {
    AutomationSourceSpec::Curve {
        interpolation: CurveKind::Linear,
        points: vec![point(0, 1, 0.0), point(8, 1, 1.0)],
    }
}

fn lane(id: &str, entity: &str, parameter: &str) -> AutomationLaneSpec {
    AutomationLaneSpec {
        id: id.to_owned(),
        target: AutomationTarget {
            entity_id: entity.to_owned(),
            parameter_id: parameter.to_owned(),
        },
        source: AutomationSourceSpec::Constant { value: 0.5 },
        combine: None,
        loop_spec: None,
        last_beat: None,
    }
}

#[test]
fn log_mapping_endpoints() {
    assert_eq!(normalized_to_bpm(0.0), 20.0);
    assert_eq!(normalized_to_bpm(1.0), 999.0);
    assert!((normalized_to_bpm(0.5) - (20.0 * 999.0_f64).sqrt()).abs() < 1e-9);
}

#[test]
fn ramp_bakes_on_grid_with_exact_endpoints() {
    let segments = bake_tempo_lane(&ramp(), SEED, 8.0, 0.0).unwrap();
    // 8 beats * 64 slots + 1 final boundary; curve points 0 and 8 are
    // already on the grid.
    assert_eq!(segments.len(), 8 * 64 + 1);
    assert_eq!(segments[0].start_beat, Beat::ZERO);
    assert_eq!(segments[0].bpm, 20.0);
    assert_eq!(segments[0].curve, Some(TempoCurve::Linear));
    let last = segments.last().unwrap();
    assert_eq!(last.start_beat, beat(8, 1));
    assert_eq!(last.bpm, 999.0);
    assert_eq!(last.curve, None, "the final segment is a step tail");
    // Every segment BPM is the exact log-mapped source value at its beat.
    for segment in &segments {
        let expected = normalized_to_bpm(segment.start_beat.to_f64() / 8.0);
        assert!(
            (segment.bpm - expected).abs() < 1e-9,
            "bpm at {:?}: expected {expected}, got {}",
            segment.start_beat,
            segment.bpm
        );
    }
    // All start beats are on the 1/64 grid and strictly increasing.
    for (index, segment) in segments.iter().enumerate() {
        assert_eq!(segment.start_beat, beat(index as i64, 64));
    }
    // The baked table feeds TempoMap::compile directly.
    let map = TempoMap::compile(&segments, 48_000).unwrap();
    assert_eq!(map.beat_to_frame(Beat::ZERO), 0);
    // Beat 8 at ~the ramped tempo must be later than 4 s at any constant
    // 20 BPM would imply, and the map is monotonic.
    let mid = map.beat_to_frame(beat(4, 1));
    let end = map.beat_to_frame(beat(8, 1));
    assert!(mid > 0 && end > mid);
}

#[test]
fn gate_boundaries_become_exact_segment_edges() {
    let source = AutomationSourceSpec::Gate {
        period_beats: beat(2, 1),
        duty: 0.25,
        phase: None,
        on: None,
        off: None,
    };
    let segments = bake_tempo_lane(&source, SEED, 4.0, 0.0).unwrap();
    let starts: Vec<Beat> = segments.iter().map(|segment| segment.start_beat).collect();
    // Duty edges at 0.5 and 2.5 (off-grid) plus period starts at 2 and 4.
    assert!(starts.contains(&beat(1, 2)), "duty edge at beat 1/2");
    assert!(starts.contains(&beat(5, 2)), "duty edge at beat 5/2");
    assert!(starts.contains(&beat(2, 1)), "period start at beat 2");
    // BPM at the edges is exact: on -> 999, off -> 20.
    let at = |b: Beat| segments.iter().find(|s| s.start_beat == b).unwrap().bpm;
    assert_eq!(at(Beat::ZERO), 999.0);
    assert_eq!(at(beat(1, 2)), 20.0);
    assert_eq!(at(beat(2, 1)), 999.0);
    assert_eq!(at(beat(5, 2)), 20.0);
    // Grid segments are still present around the discontinuities.
    assert!(starts.contains(&beat(31, 64)));
    assert!(starts.contains(&beat(33, 64)));
    TempoMap::compile(&segments, 48_000).unwrap();
}

#[test]
fn wave_sources_bake_smoothly() {
    let source = AutomationSourceSpec::Wave {
        wave: WaveKind::Sine,
        period_beats: beat(4, 1),
        phase: None,
        min: None,
        max: None,
        pulse_width: None,
    };
    let segments = bake_tempo_lane(&source, SEED, 4.0, 0.0).unwrap();
    assert_eq!(segments.len(), 4 * 64 + 1);
    let map = TempoMap::compile(&segments, 48_000).unwrap();
    assert_eq!(map.sample_rate(), 48_000);
}

#[test]
fn tail_extends_the_bake_range() {
    // Tail seconds convert at the end BPM (999 for the ramp end): 1 second
    // of tail = 999/60 = 16.65 beats.
    let segments = bake_tempo_lane(&ramp(), SEED, 8.0, 1.0).unwrap();
    let last = segments.last().unwrap();
    let expected_end = 8.0 + 999.0 / 60.0;
    assert!((last.start_beat.to_f64() - expected_end).abs() < 1.0 / 64.0 + 1e-9);
    assert_eq!(last.bpm, 999.0);
}

#[test]
fn chance_sources_are_rejected() {
    let source = AutomationSourceSpec::Chance(ChanceSpec {
        probability: 0.5,
        seed: 1,
        smooth_beats: None,
        random_phase: None,
        rate: Some(1.0),
        interval_beats: None,
    });
    let error = bake_tempo_lane(&source, SEED, 8.0, 0.0).unwrap_err();
    assert_eq!(error.code, codes::AUTOMATION_TEMPO_RESTRICTION);
    let nested = AutomationSourceSpec::Unary {
        op: oxitone_core::wire::UnaryOp::Invert,
        input: Box::new(source),
        steps: None,
        amount: None,
        min: None,
        max: None,
    };
    assert_eq!(
        bake_tempo_lane(&nested, SEED, 8.0, 0.0).unwrap_err().code,
        codes::AUTOMATION_TEMPO_RESTRICTION
    );
}

#[test]
fn segment_budget_is_enforced() {
    // 2000 beats * 64 slots + 1 > 65_536.
    let error = bake_tempo_lane(&ramp(), SEED, 2_000.0, 0.0).unwrap_err();
    assert_eq!(error.code, codes::TEMPO_MAP_COMPLEXITY);
    // Just under the budget bakes fine: 1024 beats -> 65_537 > limit? 1023
    // beats * 64 + 1 = 65_473 <= 65_536.
    let ok_length = (TEMPO_BAKE_MAX_SEGMENTS - 1) as f64 / 64.0;
    assert!(bake_tempo_lane(&ramp(), SEED, ok_length, 0.0).is_ok());
}

#[test]
fn at_most_one_tempo_lane() {
    let lanes = [
        lane("auto_1", "prj_1", "tempo"),
        lane("auto_2", "chn_1", "level"),
    ];
    let found = find_tempo_lane(&lanes, "prj_1").unwrap();
    assert_eq!(found.map(|lane| lane.id.as_str()), Some("auto_1"));
    assert!(find_tempo_lane(&lanes[..1], "prj_2").unwrap().is_none());

    let conflict = [
        lane("auto_1", "prj_1", "tempo"),
        lane("auto_2", "prj_1", "tempo"),
    ];
    let error = find_tempo_lane(&conflict, "prj_1").unwrap_err();
    assert_eq!(error.code, codes::TEMPO_AUTOMATION_CONFLICT);
}
