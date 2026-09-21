//! Actual VST3 output conformance over stream 11. No audio device is opened.
use oxitone_vst3_host::{
    stream::{RealtimePort, Session, SessionOptions, Status},
    stream_wire::Start,
    wire::Event,
};
use std::{
    path::Path,
    time::{Duration, Instant},
};
#[path = "midi/sysex.rs"]
mod sysex;

fn exchange(port: &mut RealtimePort, events: &[Event], reset: bool) -> Vec<Event> {
    let silence = vec![0.; port.info().block_size];
    let sequence = port
        .submit_buses(
            &[[&silence, &silence]],
            events,
            oxitone_vst3_host::stream::BlockContext {
                payload: &[],
                reset,
                transport: None,
            },
        )
        .unwrap();
    let mut storage = (0..port.info().audio_buses.outputs.len())
        .map(|_| [silence.clone(), silence.clone()])
        .collect::<Vec<_>>();
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let mut buses = storage
            .iter_mut()
            .map(|[l, r]| [l.as_mut_slice(), r.as_mut_slice()])
            .collect::<Vec<_>>();
        if let Some(received) = port.receive_buses(&mut buses).unwrap() {
            assert_eq!(received.sequence, sequence);
            assert_eq!(received.frames, silence.len());
            return port.output_events().to_vec();
        }
        assert!(Instant::now() < deadline, "MIDI completion deadline");
        oxitone_vst3_host::stream::wait_for_completion();
    }
}
fn fault(helper: &Path, start: &Start, suffix: &str) {
    let mut start = start.clone();
    start.source.class_id = format!("6E33225254224A00AA69301AF318{suffix}");
    let (_session, mut port) = Session::spawn(helper, start, SessionOptions::default()).unwrap();
    let silence = [0.; 128];
    port.submit(&silence, &silence, &[]).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while port.status() == Status::Running {
        assert!(Instant::now() < deadline);
        oxitone_vst3_host::stream::wait_for_completion();
    }
    assert_eq!(port.status(), Status::PluginFault);
    assert!(port.output_events().is_empty());
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let helper = Path::new(&args[1]);
    let mut start: Start = serde_json::from_slice(&std::fs::read(&args[2]).unwrap()).unwrap();
    start.options.midi_output = true;
    start.source.class_id = "6E33225254224A00AA69301AF3187986".into();
    let (session, mut port) =
        Session::spawn(helper, start.clone(), SessionOptions::default()).unwrap();
    assert!(port.info().note_output);
    let events = [
        Event::Midi {
            frame: 13,
            message: [0x92, 60, 64],
        },
        Event::Midi {
            frame: 109,
            message: [0x82, 60, 23],
        },
    ];
    let mut micros = Vec::new();
    for i in 0..400 {
        let before = Instant::now();
        let output = exchange(&mut port, &events, i % 37 == 0);
        micros.push(before.elapsed().as_secs_f64() * 1e6);
        assert_eq!(output.len(), 2);
        assert!(matches!(
            output[0],
            Event::Midi {
                frame: 13,
                message: [0x9f, 72, 64]
            }
        ));
        assert!(matches!(
            output[1],
            Event::Midi {
                frame: 109,
                message: [0x8f, 72, 23]
            }
        ));
    }
    assert!(exchange(&mut port, &[], true).is_empty());
    assert!(exchange(&mut port, &[], false).is_empty());
    fault(helper, &start, "7988");
    fault(helper, &start, "7989");
    let mut disabled = start.clone();
    disabled.options.midi_output = false;
    let (_disabled_session, mut disabled_port) =
        Session::spawn(helper, disabled, SessionOptions::default()).unwrap();
    assert!(exchange(&mut disabled_port, &events, false).is_empty());
    let mut only = start.clone();
    only.source.class_id = "6E33225254224A00AA69301AF318798A".into();
    only.options.bus_activation = Some(oxitone_vst3_host::bus_wire::BusActivation {
        inputs: vec![],
        outputs: vec![],
    });
    let (_only_session, mut only_port) =
        Session::spawn(helper, only.clone(), SessionOptions::default()).unwrap();
    assert_eq!(only_port.info().output_channels, 0);
    assert!(only_port.info().audio_buses.inputs.is_empty());
    assert!(only_port.info().audio_buses.outputs.is_empty());
    let mut only_micros = Vec::new();
    for i in 0..400 {
        let before = Instant::now();
        let output = exchange(&mut only_port, &events, i % 37 == 0);
        only_micros.push(before.elapsed().as_secs_f64() * 1e6);
        assert!(matches!(
            output.as_slice(),
            [
                Event::Midi {
                    frame: 13,
                    message: [0x9f, 72, 64]
                },
                Event::Midi {
                    frame: 109,
                    message: [0x8f, 72, 23]
                }
            ]
        ));
    }
    assert!(exchange(&mut only_port, &[], true).is_empty());
    assert!(exchange(&mut only_port, &[], false).is_empty());
    only.source.class_id = "6E33225254224A00AA69301AF318798B".into();
    let (_generator_session, mut generator) =
        Session::spawn(helper, only, SessionOptions::default()).unwrap();
    assert!(!generator.info().note_input);
    assert_eq!(
        generator.submit(&[0.; 128], &[0.; 128], &events),
        Err(oxitone_vst3_host::stream::PortError::InvalidBlock)
    );
    assert_eq!(exchange(&mut generator, &[], false).len(), 2);
    assert!(exchange(&mut generator, &[], false).is_empty());
    assert_eq!(exchange(&mut generator, &[], true).len(), 2);
    micros.sort_by(f64::total_cmp);
    only_micros.sort_by(f64::total_cmp);
    let report = serde_json::json!({"streamProtocolVersion":11,"packets":400,"sampleRate":48000,"blockSize":128,"device":null,
        "channelAndOffsets":true,"resetNoStaleEvents":true,"disabledCapture":true,"overflowFault":true,"invalidOutputFault":true,
        "roundtripP95Micros":micros[379],"roundtripP99Micros":micros[395],"callbackP95Ms":null,"callbackP99Ms":null,
        "diagnostics":session.diagnostics()});
    let mut report = report;
    report["midiOnlyThru"] = true.into();
    report["midiOnlyGenerator"] = true.into();
    report["sysex"] = sysex::check(helper, &start);
    report["midiOnlyRoundtripP95Micros"] = only_micros[379].into();
    report["midiOnlyRoundtripP99Micros"] = only_micros[395].into();
    std::fs::write(&args[3], serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    println!("VST3 MIDI probe passed: {}", args[3]);
}
