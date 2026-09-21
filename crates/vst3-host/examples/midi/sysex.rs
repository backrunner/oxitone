//! Real plugin SysEx echo, payload ownership, processing budgets and disabled capture.
use oxitone_core::midi_bytes::{append_sysex, MAX_MIDI_PAYLOAD_BYTES};
use oxitone_vst3_host::{
    stream::{BlockContext, RealtimePort, Session, SessionOptions, Status},
    stream_wire::Start,
    wire::Event,
};
use std::{
    path::Path,
    time::{Duration, Instant},
};

fn exchange(
    port: &mut RealtimePort,
    events: &[Event],
    payload: &[u8],
    reset: bool,
) -> (Vec<Event>, Vec<u8>) {
    let silence = [0.; 128];
    let sequence = port
        .submit_buses(
            &[[&silence, &silence]],
            events,
            BlockContext {
                payload,
                reset,
                transport: None,
            },
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(received) = port.receive_buses(&mut []).unwrap() {
            assert_eq!(received.sequence, sequence);
            assert_eq!(received.frames, 128);
            return (
                port.output_events().to_vec(),
                port.output_payload().to_vec(),
            );
        }
        assert!(Instant::now() < deadline, "SysEx deadline");
        oxitone_vst3_host::stream::wait_for_completion();
    }
}
pub(super) fn check(helper: &Path, start: &Start) -> serde_json::Value {
    let mut start = start.clone();
    start.source.class_id = "6E33225254224A00AA69301AF318798A".into();
    let (session, mut port) =
        Session::spawn(helper, start.clone(), SessionOptions::default()).unwrap();
    let mut bytes = [0x7d; 4096];
    bytes[0] = 0xf0;
    bytes[4095] = 0xf7;
    let mut payload = Vec::with_capacity(MAX_MIDI_PAYLOAD_BYTES);
    let events: [Event; 4] = std::array::from_fn(|i| Event::SysEx {
        frame: [0, 13, 63, 127][i],
        data: append_sysex(&mut payload, &bytes).unwrap(),
    });
    let mut times = Vec::new();
    for i in 0..400 {
        let before = Instant::now();
        let (output, data) = exchange(&mut port, &events, &payload, i % 37 == 0);
        times.push(before.elapsed().as_secs_f64() * 1e6);
        assert_eq!(output.len(), 4);
        for (i, event) in output.iter().enumerate() {
            let Event::SysEx { frame, data: range } = event else {
                panic!("lost SysEx")
            };
            assert_eq!(*frame, events[i].frame());
            assert_eq!(range.get(&data).unwrap(), bytes);
        }
    }
    let (output, data) = exchange(&mut port, &[], &[], true);
    assert!(output.is_empty() && data.is_empty());
    assert!(exchange(&mut port, &[], &[], false).0.is_empty());
    for suffix in ["798D", "798E", "798F"] {
        let mut faulty = start.clone();
        faulty.source.class_id = format!("6E33225254224A00AA69301AF318{suffix}");
        let (_session, mut faulty) =
            Session::spawn(helper, faulty, SessionOptions::default()).unwrap();
        faulty.submit(&[0.; 128], &[0.; 128], &[]).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while faulty.status() == Status::Running {
            assert!(Instant::now() < deadline);
            oxitone_vst3_host::stream::wait_for_completion();
        }
        assert_eq!(faulty.status(), Status::PluginFault);
        assert!(faulty.output_events().is_empty() && faulty.output_payload().is_empty());
    }
    start.options.midi_output = false;
    start.source.class_id = "6E33225254224A00AA69301AF318798F".into();
    let (_session, mut disabled) =
        Session::spawn(helper, start, SessionOptions::default()).unwrap();
    let (output, data) = exchange(&mut disabled, &[], &[], false);
    assert!(output.is_empty() && data.is_empty());
    times.sort_by(f64::total_cmp);
    serde_json::json!({
        "packets":400,"messagesPerPacket":4,"bytesPerPacket":16384,
        "roundtripP95Micros":times[379],"roundtripP99Micros":times[395],
        "sampleOffsets":true,"resetClearsPayload":true,"invalidAndOversizedFaults":true,
        "disabledCapture":true,"diagnostics":session.diagnostics(),
    })
}
