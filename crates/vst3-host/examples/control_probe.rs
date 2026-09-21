//! Live VestiGain controls and PCM in the same helper. No audio device is opened.
use oxitone_vst3_host::{
    control_wire::Command,
    stream::{Session, SessionOptions, Status},
    stream_wire::Start,
};
use std::{
    path::Path,
    time::{Duration, Instant},
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert!(
        args.len() == 4 || args.len() == 5,
        "usage: vst3-control-probe HELPER START REPORT [--editor]"
    );
    let editor = args.get(4).is_some_and(|s| s == "--editor");
    let start: Start = serde_json::from_slice(&std::fs::read(&args[2]).unwrap()).unwrap();
    let (mut session, mut port) = Session::spawn(
        Path::new(&args[1]),
        start.clone(),
        SessionOptions {
            block_timeout: Duration::from_secs(2),
            ..SessionOptions::default()
        },
    )
    .unwrap();
    let pid = session.pid();
    let control = session.controller();
    let timeout = Duration::from_secs(30);
    let mut controls = Vec::new();
    let mut audio = Vec::new();
    let mut captured = None;
    let mut sequence = 0;
    let input = [0.25; 128];
    let mut left = [0.; 128];
    let mut right = left;
    for cycle in 0..30 {
        if editor && cycle % 10 == 0 {
            assert!(
                control
                    .request(Command::OpenEditor {}, timeout)
                    .unwrap()
                    .editor_open
            );
        }
        let normalized = if cycle % 2 == 0 { 0.75 } else { 0.5 };
        let expected = 0.25 * 10f32.powf((normalized as f32 * 72. - 60.) / 20.);
        for (parameter_id, value) in [(0, normalized), (1, 0.)] {
            let before = Instant::now();
            assert_eq!(
                control
                    .request(
                        Command::SetParameter {
                            parameter_id,
                            value
                        },
                        timeout
                    )
                    .unwrap()
                    .next_sequence,
                sequence
            );
            if cycle >= 3 {
                controls.push(before.elapsed().as_secs_f64() * 1000.);
            }
        }
        for block in 0..40 {
            let before = Instant::now();
            assert_eq!(port.submit(&input, &input, &[]).unwrap(), sequence);
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                if let Some(reply) = port.receive(&mut left, &mut right).unwrap() {
                    assert_eq!(reply.sequence, sequence);
                    break;
                }
                assert!(Instant::now() < deadline);
                oxitone_vst3_host::stream::wait_for_completion();
            }
            if cycle >= 3 {
                audio.push(before.elapsed().as_secs_f64() * 1000.);
            }
            if block > 20 {
                assert!(
                    left.iter()
                        .chain(&right)
                        .all(|v| (*v - expected).abs() < 1e-5),
                    "gain did not reach the live DSP: {:?} != {expected}",
                    &left[..4]
                );
            }
            sequence += 1;
        }
        let state = control.request(Command::Capture {}, timeout).unwrap();
        assert_eq!(state.next_sequence, sequence);
        let info = state.info.unwrap();
        assert_eq!(info["configuration"]["parameters"]["0"], normalized);
        captured = Some(info);
        if editor && cycle % 10 == 9 {
            assert!(
                !control
                    .request(Command::CloseEditor {}, timeout)
                    .unwrap()
                    .editor_open
            );
        }
        assert_eq!(session.pid(), pid);
    }
    let diagnostics = session.diagnostics();
    assert_eq!(session.status(), Status::Running);
    if editor {
        assert!(
            control
                .request(Command::OpenEditor {}, timeout)
                .unwrap()
                .editor_open
        );
    }
    session.close(); // Also exercise helper retirement with a live window attached.
    assert_eq!(unsafe { libc::kill(pid as i32, 0) }, -1);
    assert!(control.request(Command::Poll {}, timeout).is_err());
    let mut restored = start;
    restored.options.configuration =
        Some(serde_json::from_value(captured.as_ref().unwrap()["configuration"].clone()).unwrap());
    restored.options.parameters.clear();
    let (mut session, mut port) =
        Session::spawn(Path::new(&args[1]), restored, SessionOptions::default()).unwrap();
    assert_ne!(session.pid(), pid);
    port.submit(&input, &input, &[]).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while port.receive(&mut left, &mut right).unwrap().is_none() {
        assert!(Instant::now() < deadline);
        oxitone_vst3_host::stream::wait_for_completion();
    }
    assert!(left
        .iter()
        .chain(&right)
        .all(|v| (*v - 0.25 * 10f32.powf(-24. / 20.)).abs() < 1e-5));
    session.close();
    let report = serde_json::json!({"scenario":"vst3-live-control", "streamProtocolVersion":11,"controlProtocolVersion":1,"sampleRate":48000,"blockSize":128,"device":null,"callbackP95Ms":null,"callbackP99Ms":null,"xruns":null,"blocks":sequence,"warmupCycles":3,"control":timing(controls),"audioRoundtrip":timing(audio),"diagnostics":diagnostics,"nativeEditorAttachDetach":editor,"manualGuiAcceptance":false,"stateRestorePcm":true,"captured":captured});
    std::fs::write(&args[3], serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    println!("Live VST3 controls passed: 1200 PCM blocks, 30 state captures, restored PCM, stale handle rejection and process reaping (editor={editor})");
}
fn timing(mut values: Vec<f64>) -> serde_json::Value {
    values.sort_by(f64::total_cmp);
    serde_json::json!({"p95Ms":values[values.len()*95/100],"p99Ms":values[values.len()*99/100],"samplesMs":values})
}
