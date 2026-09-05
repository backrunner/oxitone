use oxitone_core::beat::Beat;
use oxitone_core::error::codes;
use oxitone_core::wire::{TempoCurve, TempoSegment};
use oxitone_transport::TempoMap;

const SR: u32 = 48_000;

#[test]
fn seconds_timecode_rounds_at_project_rate_and_rejects_unrepresentable_positions() {
    let tempo = TempoMap::compile(&[seg(0, 1, 120.0, None)], 24000).unwrap();
    assert_eq!(tempo.seconds_to_frame(1.25).unwrap(), 30000);
    assert_eq!(tempo.seconds_to_frame(0.5 / 24000.0).unwrap(), 1);
    for value in [f64::NAN, f64::INFINITY, -1.0, 1e30] {
        assert_eq!(
            tempo.seconds_to_frame(value).unwrap_err().code,
            codes::INVALID_PROJECT
        );
    }
}

fn beat(n: i64, d: u32) -> Beat {
    Beat::new(n, d).unwrap()
}

fn seg(n: i64, d: u32, bpm: f64, curve: Option<TempoCurve>) -> TempoSegment {
    TempoSegment {
        start_beat: beat(n, d),
        bpm,
        curve,
    }
}

#[test]
fn step_curve_exact_frames_and_inverse() {
    let map = TempoMap::compile(&[seg(0, 1, 120.0, None)], SR).unwrap();
    assert_eq!(map.beat_to_frame(beat(1, 1)), 24_000);
    assert_eq!(map.beat_to_frame(beat(1, 2)), 12_000);
    assert_eq!(map.beat_to_seconds(beat(3, 2)), 0.75);
    assert_eq!(map.seconds_to_beat(0.75), beat(3, 2));
    assert_eq!(map.frame_to_beat(24_000), beat(1, 1));
    assert_eq!(map.beat_to_seconds(Beat::ZERO), 0.0);
    assert_eq!(map.sample_rate(), SR);
}

#[test]
fn frames_round_half_up() {
    let map = TempoMap::compile(&[seg(0, 1, 120.0, None)], SR).unwrap();
    // 1/9600 beat = 2.5 frames exactly -> rounds up to 3.
    assert_eq!(map.beat_to_frame(beat(1, 9_600)), 3);
    // 1/19200 beat = 1.25 frames -> 1.
    assert_eq!(map.beat_to_frame(beat(1, 19_200)), 1);
}

#[test]
fn linear_curve_closed_form() {
    let map = TempoMap::compile(
        &[
            seg(0, 1, 120.0, Some(TempoCurve::Linear)),
            seg(4, 1, 240.0, None),
        ],
        SR,
    )
    .unwrap();
    // seconds(4) = 60*4/(240-120) * ln(2) = 2 ln 2.
    let end = map.beat_to_seconds(beat(4, 1));
    assert!((end - 2.0 * 2.0_f64.ln()).abs() < 1e-12);
    // midpoint beat 2: bpm = 180, seconds = 2 ln 1.5.
    let mid = map.beat_to_seconds(beat(2, 1));
    assert!((mid - 2.0 * 1.5_f64.ln()).abs() < 1e-12);
    let back = map.seconds_to_beat(mid);
    assert!((back.to_f64() - 2.0).abs() < 1e-9);
}

#[test]
fn exponential_curve_closed_form() {
    let map = TempoMap::compile(
        &[
            seg(0, 1, 120.0, Some(TempoCurve::Exponential)),
            seg(4, 1, 240.0, None),
        ],
        SR,
    )
    .unwrap();
    // seconds(4) = 60*4/(120 ln 2) * (1 - 120/240) = 1/ln 2.
    let end = map.beat_to_seconds(beat(4, 1));
    assert!((end - 1.0 / 2.0_f64.ln()).abs() < 1e-12);
    // midpoint beat 2: bpm = 120*sqrt(2), seconds = 2/ln2 * (1 - 1/sqrt(2)).
    let mid = map.beat_to_seconds(beat(2, 1));
    let expected = 2.0 / 2.0_f64.ln() * (1.0 - 1.0 / 2.0_f64.sqrt());
    assert!((mid - expected).abs() < 1e-12);
    let back = map.seconds_to_beat(mid);
    assert!((back.to_f64() - 2.0).abs() < 1e-9);
}

#[test]
fn degenerate_equal_bpm_matches_step() {
    for curve in [TempoCurve::Linear, TempoCurve::Exponential] {
        let map = TempoMap::compile(&[seg(0, 1, 120.0, Some(curve)), seg(4, 1, 120.0, None)], SR)
            .unwrap();
        assert_eq!(map.beat_to_seconds(beat(4, 1)), 2.0, "{curve:?}");
        assert_eq!(map.beat_to_frame(beat(4, 1)), 96_000, "{curve:?}");
        assert_eq!(map.seconds_to_beat(2.0), beat(4, 1), "{curve:?}");
    }
}

#[test]
fn segment_boundary_frames_are_exact() {
    let map = TempoMap::compile(&[seg(0, 1, 120.0, None), seg(2, 1, 60.0, None)], SR).unwrap();
    assert_eq!(map.beat_to_frame(beat(2, 1)), 48_000);
    assert_eq!(map.beat_to_frame(beat(3, 1)), 96_000);
    assert_eq!(map.beat_to_frame(beat(4, 1)), 144_000);
    assert_eq!(map.frame_to_beat(48_000), beat(2, 1));
    // Continuity: approaching the boundary from below stays below it.
    let before = map.beat_to_seconds(beat(65_535, 32_768));
    assert!(before < 1.0);
}

#[test]
fn round_trip_is_monotonic_and_invertible_for_all_curves() {
    let cases: [Vec<TempoSegment>; 4] = [
        vec![seg(0, 1, 90.0, None), seg(4, 1, 150.0, None)],
        vec![
            seg(0, 1, 120.0, Some(TempoCurve::Linear)),
            seg(4, 1, 60.0, Some(TempoCurve::Exponential)),
            seg(8, 1, 180.0, None),
        ],
        vec![
            seg(0, 1, 200.0, Some(TempoCurve::Exponential)),
            seg(2, 1, 80.0, None),
        ],
        vec![
            seg(0, 1, 120.0, Some(TempoCurve::Linear)),
            seg(4, 1, 120.0, Some(TempoCurve::Exponential)),
            seg(8, 1, 120.0, None),
        ],
    ];
    for (case, segments) in cases.iter().enumerate() {
        let map = TempoMap::compile(segments, SR).unwrap();
        let mut prev_frame = None;
        for sixteenth in 0..=160 {
            let b = beat(sixteenth, 16);
            let frame = map.beat_to_frame(b);
            if let Some(prev) = prev_frame {
                assert!(frame > prev, "case {case} beat {}", b.to_f64());
            }
            prev_frame = Some(frame);
            let rt = map.seconds_to_beat(map.beat_to_seconds(b));
            assert!((rt.to_f64() - b.to_f64()).abs() < 1e-9, "case {case}");
            // Frame round-trip stays within half a frame of the beat.
            let fb = map.frame_to_beat(frame);
            let tol = 1.1 / (2.0 * f64::from(SR)) * 200.0 / 60.0;
            assert!((fb.to_f64() - b.to_f64()).abs() < tol, "case {case}");
            // All query paths for one beat agree on the same frame.
            assert_eq!(map.beat_to_frame(rt), frame, "case {case}");
        }
    }
}

#[test]
fn validation_errors() {
    let range = |bpm: f64| {
        let err = TempoMap::compile(&[seg(0, 1, bpm, None)], SR).unwrap_err();
        assert_eq!(err.code, codes::TEMPO_RANGE);
    };
    range(19.9);
    range(1_000.0);
    range(f64::NAN);
    range(f64::INFINITY);

    let err = TempoMap::compile(&[], SR).unwrap_err();
    assert_eq!(err.code, codes::TEMPO_MAP_ORDER);

    let err = TempoMap::compile(&[seg(1, 1, 120.0, None)], SR).unwrap_err();
    assert_eq!(err.code, codes::TEMPO_MAP_ORDER);
    assert_eq!(err.path.as_deref(), Some("$.tempoMap[0].startBeat"));

    let err = TempoMap::compile(
        &[
            seg(0, 1, 120.0, None),
            seg(2, 1, 130.0, None),
            seg(2, 1, 140.0, None),
        ],
        SR,
    )
    .unwrap_err();
    assert_eq!(err.code, codes::TEMPO_MAP_ORDER);
}
