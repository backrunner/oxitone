use super::*;
use crate::transport_wire::Transport;

fn journal() -> Journal {
    Journal {
        last_id: 1,
        capture: Some(Capture {
            id: "1".into(),
            status: Status::Recording,
            overflow_count: 4,
            next: 0,
            events: VecDeque::with_capacity(CAPACITY),
            pending: Vec::with_capacity(CAPACITY),
            end: None,
            error: None,
            recording: None,
        }),
    }
}
fn position(sequence: u64, frame: u64, beat: f64, playing: bool) -> Position {
    Position {
        audio_sequence: sequence,
        reset: frame == 128,
        transport: Transport {
            project_frame: frame,
            continuous_frame: sequence * 128,
            project_beat: beat,
            bar_beat: 0.,
            tempo: 120.,
            time_signature: [4, 4],
            playing,
            cycle: Some([0., 4.]),
        },
    }
}
fn ready() -> Ready {
    serde_json::from_value(serde_json::json!({
        "streamProtocolVersion":11,"sampleRate":48000,"blockSize":128,"classId":"1".repeat(32),"sha256":"a".repeat(64),
        "inputChannels":2,"outputChannels":2,"audioBuses":{"inputs":[{"channels":2,"active":true}],"outputs":[{"channels":2,"active":true}]},
        "noteInput":false,"noteOutput":false,"category":"Fx","latencyFrames":0,"tailFrames":0,"helperTimeConstraint":false,
        "parameters":[{"id":7,"writable":true,"automatable":true}]
    })).unwrap()
}
fn value(v: f64) -> ParameterEdit {
    ParameterEdit {
        id: 7,
        kind: ParameterEditKind::ValueChange,
        value: Some(v),
    }
}
#[test]
fn quantum_positions_retry_cursors_and_stop_are_authoritative() {
    let mut journal = journal();
    journal.capture.as_mut().unwrap().collect(
        vec![
            ParameterEdit {
                id: 7,
                kind: ParameterEditKind::BeginGesture,
                value: None,
            },
            value(0.25),
        ],
        Some(4),
        &ready(),
    );
    assert_eq!(journal.read("1", 0, false).unwrap().pending_events, 2);
    journal.before_audio(position(3, 512, 1., false), 128);
    assert!(journal.read("1", 0, false).unwrap().events.is_empty());
    journal.before_audio(position(4, 128, 0.5, true), 128);
    let page = journal.read("1", 0, false).unwrap();
    page.validate(&ready()).unwrap();
    assert_eq!(page.events.len(), 2);
    assert_eq!(page.events[0].kind, Kind::Begin);
    assert_eq!(page.events[1].position, position(4, 128, 0.5, true));
    assert_eq!(journal.read("1", 0, false).unwrap().events, page.events);
    assert!(journal.read("1", 3, false).is_err());
    assert_eq!(journal.read("1", 2, true).unwrap().status, Status::Stopping);
    assert!(journal.read("1", 0, false).is_err());
    journal
        .capture
        .as_mut()
        .unwrap()
        .collect(vec![value(0.9)], Some(4), &ready());
    journal.before_audio(position(5, 256, 0.6, true), 128);
    let stopped = journal.read("1", 2, false).unwrap();
    stopped.validate(&ready()).unwrap();
    assert_eq!(stopped.status, Status::Stopped);
    assert!(stopped.events.is_empty());
    assert_eq!(stopped.end_position, Some(position(5, 256, 0.6, true)));
    assert!(journal.read("2", 2, false).is_err());
}
#[test]
fn queue_loss_invalid_feedback_and_unacknowledged_overflow_fail_the_whole_capture() {
    for case in 0..4 {
        let mut journal = journal();
        let capture = journal.capture.as_mut().unwrap();
        match case {
            0 => capture.collect(vec![value(0.5)], Some(5), &ready()),
            1 => capture.collect(vec![value(f64::NAN)], Some(4), &ready()),
            2 => capture.collect(vec![value(0.5); CAPACITY + 1], Some(4), &ready()),
            _ => {
                capture.collect(vec![value(0.5); CAPACITY], Some(4), &ready());
                capture.position(position(0, 0, 0., true));
                capture.collect(vec![value(0.6)], Some(4), &ready());
                capture.position(position(1, 128, 0.1, true));
            }
        }
        let page = journal.read("1", 0, false).unwrap();
        page.validate(&ready()).unwrap();
        assert_eq!(page.status, Status::Failed);
        assert_eq!(page.pending_events, 0);
        assert!(page.events.is_empty());
        assert!(page.error.is_some());
    }
}
#[test]
fn cursor_acknowledgement_supports_long_captures_with_bounded_storage() {
    let mut journal = journal();
    for block in 0..40 {
        journal
            .capture
            .as_mut()
            .unwrap()
            .collect(vec![value(0.5); PAGE_SIZE], Some(4), &ready());
        journal.before_audio(position(block, block * 128, block as f64 / 100., true), 128);
        let from = block * PAGE_SIZE as u64;
        let page = journal.read("1", from, false).unwrap();
        assert_eq!(page.events.len(), PAGE_SIZE);
        assert_eq!(page.next_sequence, from + PAGE_SIZE as u64);
        assert_eq!(journal.capture.as_ref().unwrap().events.len(), PAGE_SIZE);
        page.validate(&ready()).unwrap();
    }
}

#[test]
fn recording_samples_validate_against_the_prepared_plugin_and_stop_boundary() {
    let mut page = journal().read("1", 0, false).unwrap();
    page.recording = Some(crate::edit_wire::Recording {
        mode: crate::edit_wire::RecordingMode::Write,
        parameter_ids: vec![7],
        sample_rate: 48000,
    });
    page.events.push(Event {
        sequence: 0,
        parameter_id: 7,
        kind: Kind::Sample,
        value: Some(0.5),
        frames: Some(128),
        position: position(4, 128, 0.5, true),
    });
    page.next_sequence = 1;
    page.status = Status::Stopped;
    page.end_position = Some(position(5, 256, 0.6, true));
    page.validate(&ready()).unwrap();
    for case in 0..8 {
        let mut bad = page.clone();
        match case {
            0 => bad.recording = None,
            1 => bad.recording.as_mut().unwrap().sample_rate = 44100,
            2 => bad.recording.as_mut().unwrap().parameter_ids = vec![7, 7],
            3 => bad.recording.as_mut().unwrap().parameter_ids = vec![999],
            4 => bad.events[0].frames = Some(129),
            5 => bad.events[0].frames = Some(0),
            6 => bad.events[0].value = None,
            _ => bad.end_position = Some(bad.events[0].position),
        }
        assert!(bad.validate(&ready()).is_err(), "case {case}");
    }
}
