//! Isolated helper IPC costs while Write recording suppresses incoming automation.
use super::*;
use oxitone_vst3_host::stream::Controller;

pub fn measure(port: &mut RealtimePort, controller: &Controller) -> serde_json::Value {
    let command = |command| controller.request(command, Duration::from_secs(5)).unwrap();
    let capture = command(Command::StartRecording {
        mode: RecordingMode::Write,
        parameter_ids: vec![0],
    })
    .edits
    .unwrap()
    .capture_id;
    let mut cursor = 0;
    let mut times = Vec::with_capacity(1000);
    for index in 0..1100 {
        let ms = block(
            port,
            15 + index,
            index * 128,
            true,
            false,
            &[(0, if index % 2 == 0 { 0.1 } else { 0.3 })],
            &[0.2; 128],
        );
        if index >= 100 {
            times.push(ms);
        }
        if index % 32 == 31 || index == 1099 {
            let page = command(Command::ReadEdits {
                capture_id: capture.clone(),
                from_sequence: cursor,
            })
            .edits
            .unwrap();
            assert!(page
                .events
                .iter()
                .all(|event| event.kind == Kind::Sample && event.value == Some(0.2)));
            cursor += page.events.len() as u64;
            assert_eq!(cursor, index + 1);
        }
    }
    command(Command::DiscardEdits {
        capture_id: capture,
    });
    let mut sorted = times.clone();
    sorted.sort_by(f64::total_cmp);
    serde_json::json!({
        "warmupBlocks":100,"measuredBlocks":1000,"parameterCount":1,
        "verifiedFrames":140800,"roundtripP95Ms":sorted[949],"roundtripP99Ms":sorted[989],
        "roundtripMaxMs":sorted[999],"roundtripSamplesMs":times,
        "boundary":"submit through receive, polling with yield_now; journal drain every 32 packets excluded; no GUI or device callback"
    })
}
