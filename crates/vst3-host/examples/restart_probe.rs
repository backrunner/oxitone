//! Real helper restart/state handoff conformance. No device or system audio is opened.
use oxitone_vst3_host::{
    control_wire::{Command, RestartReason},
    edit_wire::{RecordingMode, Status as CaptureStatus},
    stream::{BlockContext, RealtimePort, Session, SessionOptions, Status},
    stream_wire::Start,
    transport_wire::Transport,
    wire::Event,
};
use std::{
    path::Path,
    time::{Duration, Instant},
};
const TIMEOUT: Duration = Duration::from_secs(3);
fn audio(port: &mut RealtimePort, events: &[Event], reset: bool) -> (bool, Vec<Vec<f32>>) {
    audio_at(port, events, reset, 96000)
}
fn audio_at(
    port: &mut RealtimePort,
    events: &[Event],
    reset: bool,
    frame: u64,
) -> (bool, Vec<Vec<f32>>) {
    let input = [0.; 128];
    let inputs = vec![[input.as_slice(); 2]; port.info().audio_buses.input_count()];
    let sequence = port
        .submit_buses(
            &inputs,
            events,
            BlockContext {
                payload: &[],
                reset,
                transport: Some(Transport {
                    project_frame: frame,
                    continuous_frame: 100000,
                    project_beat: 4.,
                    bar_beat: 4.,
                    tempo: 120.,
                    time_signature: [4, 4],
                    playing: true,
                    cycle: None,
                }),
            },
        )
        .unwrap();
    let mut channels = vec![vec![1.; 128]; port.info().audio_buses.outputs.len() * 2];
    let end = Instant::now() + TIMEOUT;
    loop {
        let mut outputs: Vec<_> = channels
            .chunks_exact_mut(2)
            .map(|pair| {
                let (left, right) = pair.split_at_mut(1);
                [left[0].as_mut_slice(), right[0].as_mut_slice()]
            })
            .collect();
        if let Some(received) = port.receive_buses(&mut outputs).unwrap_or_else(|error| {
            panic!(
                "audio sequence {sequence} reset {reset}: {error:?}, status {:?}",
                port.status()
            )
        }) {
            assert_eq!(received.sequence, sequence);
            assert_eq!(received.frames, 128);
            return (received.restart_required, channels);
        }
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(
        args.len(),
        4,
        "usage: vst3-restart-probe HELPER START_JSON REPORT_JSON"
    );
    let start: Start = serde_json::from_slice(&std::fs::read(&args[2]).unwrap()).unwrap();
    let before = Instant::now();
    let mut timings = Vec::new();
    let mut frozen = None;
    for trigger in ["automation", "controller", "processor"] {
        let controller_change = trigger == "controller";
        let (mut session, mut port) = Session::spawn(
            Path::new(&args[1]),
            start.clone(),
            SessionOptions::default(),
        )
        .unwrap();
        let control = session.controller();
        assert_eq!(
            audio(&mut port, &[], false),
            (false, vec![vec![0.5; 128]; 2])
        );
        let capture = control
            .request(
                Command::StartRecording {
                    mode: RecordingMode::Write,
                    parameter_ids: vec![0],
                },
                TIMEOUT,
            )
            .unwrap()
            .edits
            .unwrap();
        // Recording would suppress automation, so use raw edits for the process-triggered case.
        if !controller_change {
            control
                .request(
                    Command::DiscardEdits {
                        capture_id: capture.capture_id.clone(),
                    },
                    TIMEOUT,
                )
                .unwrap();
        }
        let started = Instant::now();
        if controller_change {
            let changed = control
                .request(
                    Command::SetParameter {
                        parameter_id: 0,
                        value: 1. / 3.,
                    },
                    TIMEOUT,
                )
                .unwrap();
            assert!(changed.restart.is_some());
        } else if trigger == "processor" {
            let (required, channels) = audio_at(&mut port, &[], false, 96001);
            assert!(required);
            assert!(
                channels.iter().flatten().all(|v| *v == 0.),
                "process-triggered old-layout PCM must be muted"
            );
        } else {
            let (required, channels) = audio(
                &mut port,
                &[Event::Parameter {
                    frame: 0,
                    parameter_id: 0,
                    value: 1. / 3.,
                }],
                false,
            );
            assert!(required);
            assert!(
                channels.iter().flatten().all(|v| *v == 0.),
                "the triggering block must be muted"
            );
        }
        timings.push(started.elapsed().as_secs_f64() * 1000.);
        assert_eq!(session.status(), Status::Running);
        assert!(port.restart_required());
        for reset in [false, true, false] {
            assert_eq!(audio(&mut port, &[], reset), (true, vec![vec![0.; 128]; 2]));
        }
        let state = control.request(Command::Capture {}, TIMEOUT).unwrap();
        let restart = state.restart.unwrap();
        assert_eq!(
            restart.reasons,
            vec![
                RestartReason::Io,
                RestartReason::Latency,
                RestartReason::Parameters
            ]
        );
        assert_eq!(restart.latency_frames, 64);
        assert_eq!(restart.tail_frames, 0);
        let info = state.info.unwrap();
        assert_eq!(info["audioBuses"]["outputs"].as_array().unwrap().len(), 3);
        assert_eq!(info["configuration"]["parameters"]["0"], 1. / 3.);
        assert_eq!(info["configuration"]["parameters"]["7"], 0.5);
        for command in [
            Command::SetParameter {
                parameter_id: 7,
                value: 0.25,
            },
            Command::OpenEditor {},
            Command::StartEdits {},
        ] {
            assert_eq!(
                control.request(command, TIMEOUT).unwrap_err().code,
                "PluginRestartRequired"
            );
        }
        if controller_change {
            let page = control
                .request(
                    Command::ReadEdits {
                        capture_id: capture.capture_id.clone(),
                        from_sequence: 0,
                    },
                    TIMEOUT,
                )
                .unwrap()
                .edits
                .unwrap();
            assert_eq!(page.status, CaptureStatus::Failed);
            assert_eq!(page.error.unwrap().code, "PluginRestartRequired");
            assert!(page.events.is_empty());
            control
                .request(
                    Command::DiscardEdits {
                        capture_id: capture.capture_id,
                    },
                    TIMEOUT,
                )
                .unwrap();
        }
        for _ in 0..3 {
            assert_eq!(
                control
                    .request(Command::Capture {}, TIMEOUT)
                    .unwrap()
                    .info
                    .as_ref(),
                Some(&info)
            );
        }
        let mut replacement = start.clone();
        replacement.options.configuration =
            Some(serde_json::from_value(info["configuration"].clone()).unwrap());
        let (mut next, mut next_port) =
            Session::spawn(Path::new(&args[1]), replacement, SessionOptions::default()).unwrap();
        assert_eq!(next.info().latency_frames, 64);
        assert_eq!(next.info().audio_buses.outputs.len(), 3);
        assert!(next
            .info()
            .parameters
            .iter()
            .any(|p| p.id == 7 && p.writable));
        let state = next
            .controller()
            .request(
                Command::SetParameter {
                    parameter_id: 7,
                    value: 0.25,
                },
                TIMEOUT,
            )
            .unwrap();
        assert!(state.restart.is_none());
        let (required, channels) = audio(&mut next_port, &[], false);
        assert!(!required);
        assert_eq!(channels[0], vec![0.25; 128]);
        assert_eq!(channels[1], vec![0.25; 128]);
        next.close();
        session.close();
        assert_eq!(
            control.request(Command::Poll {}, TIMEOUT).unwrap_err().code,
            "PluginHostCrashed"
        );
        for pid in [next.pid(), session.pid()] {
            assert_eq!(unsafe { libc::kill(pid as i32, 0) }, -1);
        }
        frozen = Some(info);
    }
    let report = serde_json::json!({"streamProtocolVersion":11,"device":null,"sampleRate":48000,"blockSize":128,
        "elapsedMs":before.elapsed().as_secs_f64()*1000.,"freezeMs":timings,"captured":frozen,
        "checks":["controllerAndProcessRestart","triggeringBlockMuted","oldBusShapePreserved","resetDoesNotLoseState",
        "captureNewParametersAndLatency","mutationsRejected","recordingFailedAndDiscarded","stableFrozenState",
        "replacementRestoresCurrentState","newBusShapeAndParameterUsable","helpersReaped"]});
    std::fs::write(&args[3], serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    println!("VST3 runtime restart conformance passed: frozen state survives control/audio changes and restores in a fresh helper");
}
