use oxitone_core::beat::Beat;
use oxitone_core::error::codes;
use oxitone_core::wire::TimeSignatureSegment;
use oxitone_transport::TimeSignatureMap;

fn beat(n: i64, d: u32) -> Beat {
    Beat::new(n, d).unwrap()
}

fn sig(start_bar: u32, numerator: u32, denominator: u32) -> TimeSignatureSegment {
    TimeSignatureSegment {
        start_bar,
        numerator,
        denominator,
    }
}

#[test]
fn simple_4_4() {
    let map = TimeSignatureMap::compile(&[sig(1, 4, 4)]).unwrap();
    assert_eq!(map.beat_to_bar_beat(Beat::ZERO), (1, Beat::ZERO));
    assert_eq!(map.beat_to_bar_beat(beat(7, 2)), (1, beat(7, 2)));
    assert_eq!(map.beat_to_bar_beat(beat(4, 1)), (2, Beat::ZERO));
    assert_eq!(map.beat_to_bar_beat(beat(9, 2)), (2, beat(1, 2)));
    assert_eq!(map.bar_beat_to_beat(1, Beat::ZERO).unwrap(), Beat::ZERO);
    assert_eq!(map.bar_beat_to_beat(2, Beat::ZERO).unwrap(), beat(4, 1));
    assert_eq!(map.bar_beat_to_beat(3, beat(3, 2)).unwrap(), beat(19, 2));
}

#[test]
fn change_at_bar_boundary_with_eighth_meter() {
    let map = TimeSignatureMap::compile(&[sig(1, 4, 4), sig(3, 3, 8)]).unwrap();
    // 3/8 bar = 1.5 beats; bar 3 starts at beat 8.
    assert_eq!(map.beat_to_bar_beat(beat(8, 1)), (3, Beat::ZERO));
    assert_eq!(map.beat_to_bar_beat(beat(9, 1)), (3, beat(1, 1)));
    assert_eq!(map.beat_to_bar_beat(beat(19, 2)), (4, Beat::ZERO));
    assert_eq!(map.beat_to_bar_beat(beat(21, 2)), (4, beat(1, 1)));
    assert_eq!(map.beat_to_bar_beat(beat(11, 1)), (5, Beat::ZERO));
    assert_eq!(map.bar_beat_to_beat(3, Beat::ZERO).unwrap(), beat(8, 1));
    assert_eq!(map.bar_beat_to_beat(4, beat(1, 1)).unwrap(), beat(21, 2));
    // Before the change, 4/4 still rules.
    assert_eq!(map.beat_to_bar_beat(beat(15, 2)), (2, beat(7, 2)));
    assert_eq!(map.bar_beat_to_beat(2, beat(3, 4)).unwrap(), beat(19, 4));
}

#[test]
fn fractional_bars_accumulate_exactly() {
    let map = TimeSignatureMap::compile(&[sig(1, 7, 8), sig(4, 5, 16)]).unwrap();
    // 3 bars of 7/8 = 10.5 beats; bar 4 (5/16 = 1.25 beats) starts there.
    assert_eq!(map.beat_to_bar_beat(beat(21, 2)), (4, Beat::ZERO));
    assert_eq!(map.bar_beat_to_beat(4, Beat::ZERO).unwrap(), beat(21, 2));
    assert_eq!(map.bar_beat_to_beat(5, beat(1, 8)).unwrap(), beat(95, 8));
    assert_eq!(map.beat_to_bar_beat(beat(95, 8)), (5, beat(1, 8)));
}

#[test]
fn validation_errors() {
    let err = TimeSignatureMap::compile(&[]).unwrap_err();
    assert_eq!(err.code, codes::INVALID_PROJECT);

    let err = TimeSignatureMap::compile(&[sig(2, 4, 4)]).unwrap_err();
    assert_eq!(err.code, codes::INVALID_PROJECT);
    assert_eq!(err.path.as_deref(), Some("$.timeSignatureMap[0].startBar"));

    let err = TimeSignatureMap::compile(&[sig(1, 3, 3)]).unwrap_err();
    assert_eq!(err.code, codes::INVALID_PROJECT);

    let err = TimeSignatureMap::compile(&[sig(1, 0, 4)]).unwrap_err();
    assert_eq!(err.code, codes::INVALID_PROJECT);

    let err = TimeSignatureMap::compile(&[sig(1, 4, 4), sig(1, 3, 4)]).unwrap_err();
    assert_eq!(err.code, codes::INVALID_PROJECT);

    let err = TimeSignatureMap::compile(&[sig(1, 4, 4), sig(3, 3, 4), sig(2, 2, 4)]).unwrap_err();
    assert_eq!(err.code, codes::INVALID_PROJECT);
}

#[test]
fn query_errors() {
    let map = TimeSignatureMap::compile(&[sig(1, 4, 4)]).unwrap();
    assert!(map.bar_beat_to_beat(0, Beat::ZERO).is_err());
    assert!(map.bar_beat_to_beat(1, beat(4, 1)).is_err());
    assert!(map.bar_beat_to_beat(1, beat(5, 1)).is_err());
    assert!(map.bar_beat_to_beat(1, beat(7, 2)).is_ok());
}
