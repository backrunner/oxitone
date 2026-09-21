//! Real VST3 callbacks through native control, with explicit frames and no device.
use oxitone_vst3_host::{
    control_wire::Command,
    edit_wire::{Kind, Status},
    stream::{RealtimePort, Session, SessionOptions},
    stream_wire::Start,
    transport_wire::Transport,
};
use std::{
    path::Path,
    time::{Duration, Instant},
};

fn block(port: &mut RealtimePort, sequence: u64, frame: u64, beat: f64, playing: bool) {
    let position = Transport {
        project_frame: frame,
        continuous_frame: sequence * 128,
        project_beat: beat,
        bar_beat: 0.,
        tempo: 120.,
        time_signature: [4, 4],
        playing,
        cycle: Some([0., 4.]),
    };
    let submitted = if sequence == 1 {
        port.submit_reset_at(&[0.; 128], &[0.; 128], &[], position)
    } else {
        port.submit_at(&[0.; 128], &[0.; 128], &[], position)
    };
    assert_eq!(submitted.unwrap(), sequence);
    let (mut left, mut right) = ([0.; 128], [0.; 128]);
    let end = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(received) = port.receive(&mut left, &mut right).unwrap() {
            assert_eq!(received.sequence, sequence);
            break;
        }
        assert!(Instant::now() < end, "audio packet timed out");
        std::thread::sleep(Duration::from_micros(100));
    }
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let start: Start = serde_json::from_slice(&std::fs::read(&args[2]).unwrap()).unwrap();
    let (mut session, mut port) =
        Session::spawn(Path::new(&args[1]), start, SessionOptions::default()).unwrap();
    let controller = session.controller();
    let command = |command| controller.request(command, Duration::from_secs(5)).unwrap();
    let first = command(Command::StartEdits {}).edits.unwrap();
    let id = first.capture_id.clone();
    assert_eq!(first.status, Status::Recording);
    command(Command::SetParameter {
        parameter_id: 99,
        value: 0.125,
    });
    let pending = command(Command::ReadEdits {
        capture_id: id.clone(),
        from_sequence: 0,
    })
    .edits
    .unwrap();
    assert_eq!(pending.pending_events, 4);
    assert!(pending.events.is_empty());
    block(&mut port, 0, 12_000, 0.5, true);
    let first = command(Command::ReadEdits {
        capture_id: id.clone(),
        from_sequence: 0,
    })
    .edits
    .unwrap();
    assert_eq!(
        first
            .events
            .iter()
            .map(|event| event.kind)
            .collect::<Vec<_>>(),
        [Kind::Begin, Kind::Value, Kind::Value, Kind::End]
    );
    assert_eq!(first.events[1].value, Some(0.25));
    assert_eq!(first.events[2].value, Some(0.75));
    for event in &first.events {
        assert_eq!(event.position.audio_sequence, 0);
        assert_eq!(event.position.transport.project_frame, 12_000);
        assert_eq!(event.position.transport.project_beat, 0.5);
    }
    assert_eq!(
        command(Command::ReadEdits {
            capture_id: id.clone(),
            from_sequence: 0
        })
        .edits
        .unwrap()
        .events,
        first.events
    );
    command(Command::SetParameter {
        parameter_id: 99,
        value: 0.25,
    });
    command(Command::SetParameter {
        parameter_id: 99,
        value: 0.375,
    });
    block(&mut port, 1, 240, 0.01, true);
    let moved = command(Command::ReadEdits {
        capture_id: id.clone(),
        from_sequence: 4,
    })
    .edits
    .unwrap();
    assert_eq!(moved.events.len(), 2);
    assert!(moved
        .events
        .iter()
        .all(|e| e.position.transport.project_frame == 240 && e.position.reset));
    assert_eq!(
        controller
            .request(
                Command::ReadEdits {
                    capture_id: id.clone(),
                    from_sequence: 0
                },
                Duration::from_secs(1)
            )
            .unwrap_err()
            .code,
        "PluginTaskConflict"
    );
    let stopping = command(Command::StopEdits {
        capture_id: id.clone(),
        from_sequence: 6,
    })
    .edits
    .unwrap();
    assert_eq!(stopping.status, Status::Stopping);
    block(&mut port, 2, 240, 0.01, false);
    assert_eq!(
        command(Command::ReadEdits {
            capture_id: id.clone(),
            from_sequence: 6
        })
        .edits
        .unwrap()
        .status,
        Status::Stopping
    );
    block(&mut port, 3, 300, 0.0125, true);
    let stopped = command(Command::ReadEdits {
        capture_id: id.clone(),
        from_sequence: 6,
    })
    .edits
    .unwrap();
    assert_eq!(stopped.status, Status::Stopped);
    assert_eq!(stopped.end_position.unwrap().audio_sequence, 3);
    command(Command::DiscardEdits {
        capture_id: id.clone(),
    });
    assert_eq!(
        controller
            .request(
                Command::ReadEdits {
                    capture_id: id,
                    from_sequence: 6
                },
                Duration::from_secs(1)
            )
            .unwrap_err()
            .code,
        "PluginTaskConflict"
    );
    let overflow = command(Command::StartEdits {}).edits.unwrap();
    command(Command::SetParameter {
        parameter_id: 99,
        value: 0.5,
    });
    let failed = command(Command::ReadEdits {
        capture_id: overflow.capture_id,
        from_sequence: 0,
    })
    .edits
    .unwrap();
    assert_eq!(failed.status, Status::Failed);
    assert_eq!(failed.error.unwrap().code, "BudgetExceeded");
    assert!(failed.events.is_empty());
    for value in [0.625, 0.75, 0.875] {
        let id = command(Command::StartEdits {}).edits.unwrap().capture_id;
        command(Command::SetParameter {
            parameter_id: 99,
            value,
        });
        let page = command(Command::ReadEdits {
            capture_id: id,
            from_sequence: 0,
        })
        .edits
        .unwrap();
        assert_eq!(page.status, Status::Failed);
        assert_eq!(page.error.unwrap().code, "PluginConfigInvalid");
        assert!(page.events.is_empty());
    }
    let id = command(Command::StartEdits {}).edits.unwrap().capture_id;
    command(Command::SetParameter {
        parameter_id: 99,
        value: 1.,
    });
    block(&mut port, 4, 428, 0.018, true);
    for from in [0, 256, 512, 768] {
        let page = command(Command::ReadEdits {
            capture_id: id.clone(),
            from_sequence: from,
        })
        .edits
        .unwrap();
        assert_eq!(page.first_sequence, from);
        assert_eq!(page.next_sequence, 1024);
        assert_eq!(page.events.len(), 256);
        for (index, event) in page.events.iter().enumerate() {
            assert_eq!(event.sequence, from + index as u64);
            assert_eq!(event.value, Some((from as f64 + index as f64) / 1024.));
        }
    }
    command(Command::DiscardEdits { capture_id: id });
    assert_eq!(session.status(), oxitone_vst3_host::stream::Status::Running);
    println!(
        "{}",
        serde_json::json!({"checks":["actualComponentHandlerGestures","nextAudioQuantumPositions","orderedBrackets","retryCursor","seekAndResetCoordinates","stoppedClockDoesNotFinishCapture","stopBoundary","staleCursorDoesNotKillAudio","vendorOverflowFailsCapture","invalidVendorFeedbackFailsCapture","paged1024EventsWithoutLoss"],"processedFrames":640,"firstPage":first,"stopped":stopped})
    );
    session.close();
}
