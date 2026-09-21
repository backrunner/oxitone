//! Actual processor PCM proves recording priority and restoration. No output devices.
use oxitone_vst3_host::{
    control_wire::Command,
    edit_wire::{Kind, RecordingMode, Status},
    stream::{RealtimePort, Session, SessionOptions},
    stream_wire::Start,
    transport_wire::Transport,
    wire::Event,
};
use std::{
    path::Path,
    time::{Duration, Instant},
};
#[path = "recording/profile.rs"]
mod profile;
fn block(
    port: &mut RealtimePort,
    sequence: u64,
    frame: u64,
    playing: bool,
    reset: bool,
    values: &[(u64, f64)],
    expected: &[f32; 128],
) -> f64 {
    let context = Transport {
        project_frame: frame,
        continuous_frame: sequence * 128,
        project_beat: frame as f64 / 24000.,
        bar_beat: 0.,
        tempo: 120.,
        time_signature: [4, 4],
        playing,
        cycle: Some([0., 4.]),
    };
    let events: Vec<_> = values
        .iter()
        .map(|(frame, value)| Event::Parameter {
            frame: *frame,
            parameter_id: 0,
            value: *value,
        })
        .collect();
    let started = Instant::now();
    let result = if reset {
        port.submit_reset_at(&[1.; 128], &[1.; 128], &events, context)
    } else {
        port.submit_at(&[1.; 128], &[1.; 128], &events, context)
    };
    assert_eq!(result.unwrap(), sequence);
    let (mut left, mut right) = ([0.; 128], [0.; 128]);
    let end = Instant::now() + Duration::from_secs(5);
    while port.receive(&mut left, &mut right).unwrap().is_none() {
        assert!(Instant::now() < end);
        std::thread::yield_now();
    }
    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.;
    for (index, value) in expected.iter().enumerate() {
        assert!(
            (left[index] - value).abs() < 1e-6 && (right[index] - value).abs() < 1e-6,
            "block {sequence}, frame {index}: {} != {value}",
            left[index]
        );
    }
    elapsed_ms
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let mut start: Start = serde_json::from_slice(&std::fs::read(&args[2]).unwrap()).unwrap();
    start.source.class_id = "6E33225254224A00AA69301AF3187983".into();
    let (mut session, mut port) =
        Session::spawn(Path::new(&args[1]), start, SessionOptions::default()).unwrap();
    let controller = session.controller();
    let command = |command| controller.request(command, Duration::from_secs(5)).unwrap();
    let trigger = |value| {
        command(Command::SetParameter {
            parameter_id: 99,
            value,
        });
    };
    let arm = |mode| {
        command(Command::StartRecording {
            mode,
            parameter_ids: vec![0],
        })
        .edits
        .unwrap()
        .capture_id
    };
    let read = |id: &String| {
        command(Command::ReadEdits {
            capture_id: id.clone(),
            from_sequence: 0,
        })
        .edits
        .unwrap()
    };
    let id = arm(RecordingMode::Touch);
    let mut ramp = [0.1; 128];
    ramp[64..].fill(0.2);
    block(&mut port, 0, 0, true, false, &[(0, 0.1), (64, 0.2)], &ramp);
    assert!(read(&id).events.is_empty());
    trigger(0.125);
    block(
        &mut port,
        1,
        128,
        true,
        false,
        &[(0, 0.3), (64, 0.4)],
        &[0.75; 128],
    );
    block(&mut port, 2, 256, true, false, &[], &[0.4; 128]);
    trigger(0.25);
    block(&mut port, 3, 384, true, false, &[(0, 0.6)], &[0.4; 128]);
    block(&mut port, 4, 0, true, true, &[(0, 0.2)], &[0.4; 128]);
    trigger(0.375);
    block(&mut port, 5, 128, true, false, &[(0, 0.3)], &[0.4; 128]);
    block(&mut port, 6, 256, true, false, &[], &[0.3; 128]);
    command(Command::StopEdits {
        capture_id: id.clone(),
        from_sequence: 0,
    });
    block(&mut port, 7, 384, true, false, &[], &[0.3; 128]);
    let touch = read(&id);
    assert_eq!(touch.status, Status::Stopped);
    let samples: Vec<_> = touch
        .events
        .iter()
        .filter(|e| e.kind == Kind::Sample)
        .collect();
    assert_eq!(samples.len(), 4);
    assert!(samples.iter().all(|e| e.frames == Some(128)));
    assert!(samples
        .iter()
        .any(|e| e.position.reset && e.position.transport.project_frame == 0));
    let id = arm(RecordingMode::Write);
    block(&mut port, 8, 512, true, false, &[(0, 0.1)], &[0.3; 128]);
    trigger(0.125);
    block(&mut port, 9, 640, true, false, &[(0, 0.2)], &[0.75; 128]);
    let write = read(&id);
    assert_eq!(
        write
            .events
            .iter()
            .filter(|e| e.kind == Kind::Sample)
            .count(),
        2
    );
    command(Command::DiscardEdits { capture_id: id });
    block(&mut port, 10, 768, true, false, &[], &[0.2; 128]);
    let id = arm(RecordingMode::Touch);
    trigger(0.375);
    let failed = read(&id);
    assert_eq!(failed.status, Status::Failed);
    assert!(failed.events.is_empty());
    block(&mut port, 11, 896, true, false, &[(0, 0.6)], &[0.6; 128]);
    let id = arm(RecordingMode::Touch);
    trigger(0.125);
    block(&mut port, 12, 1024, false, false, &[(0, 0.5)], &[0.5; 128]);
    assert!(read(&id).events.is_empty());
    block(&mut port, 13, 1024, true, false, &[(0, 0.2)], &[0.75; 128]);
    block(&mut port, 14, 1152, true, false, &[], &[0.2; 128]);
    assert_eq!(
        read(&id)
            .events
            .iter()
            .filter(|e| e.kind == Kind::Sample)
            .count(),
        1
    );
    command(Command::DiscardEdits { capture_id: id });
    assert_eq!(session.status(), oxitone_vst3_host::stream::Status::Running);
    let performance = args
        .iter()
        .any(|arg| arg == "--benchmark")
        .then(|| profile::measure(&mut port, &controller));
    session.close();
    println!(
        "{}",
        serde_json::json!({"processedFrames":1920,"performance":performance,"touch":touch,"write":write,"checks":["idleTouchLeavesSampleOffsets","touchOverridesEntireQuantum","releaseRestoresSuppressedAutomation","heldTouchAcrossSeekReset","writeStartsWithoutGesture","discardRestoresAutomation","invalidTouchFailsCapture","pausedGestureRetained","sampleSpansMatchPcm"]})
    );
}
