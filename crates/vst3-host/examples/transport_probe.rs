//! Real VST3 fixture observes the processor context and total processed samples. No devices.
use oxitone_vst3_host::{
    stream::{Session, SessionOptions},
    stream_wire::Start,
    transport_wire::Transport,
};
use std::{
    path::Path,
    time::{Duration, Instant},
};

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(
        args.len(),
        4,
        "usage: vst3-transport-probe HELPER START_JSON REPORT_JSON"
    );
    let mut start: Start = serde_json::from_slice(&std::fs::read(&args[2]).unwrap()).unwrap();
    let mut continuation = context_at(0, 0);
    start.options.transport = Some(continuation);
    let (mut session, mut port) =
        Session::spawn(Path::new(&args[1]), start, SessionOptions::default()).unwrap();
    let zeros = [0.0; 128];
    let mut left = [0.0; 128];
    let mut right = [0.0; 128];
    let mut processed = 0u64;
    let mut epoch_frames = 0u64;
    let mut resets = 0;
    let mut timings = Vec::new();
    for index in 0..400u64 {
        let frames = [17, 111, 1, 128, 7, 128][index as usize % 6];
        // Project jumps backwards independently of the monotonically increasing sample clock.
        // Alternate stopped/playing and cycle/non-cycle context without resetting DSP state.
        let explicit = index > 0 && index % 3 == 0;
        let context = if explicit {
            context_at(index, processed)
        } else {
            continuation
        };
        let time = Instant::now();
        if index % 37 == 1 {
            let control = session.controller();
            let state = control
                .request(
                    oxitone_vst3_host::control_wire::Command::Capture {},
                    Duration::from_secs(2),
                )
                .unwrap();
            assert_eq!(state.next_sequence, index);
        }
        let reset = index > 0 && index % 5 == 0;
        let sequence = if reset {
            epoch_frames = 0;
            resets += 1;
            port.submit_reset_at(&zeros[..frames], &zeros[..frames], &[], context)
        } else if explicit {
            port.submit_at(&zeros[..frames], &zeros[..frames], &[], context)
        } else {
            port.submit(&zeros[..frames], &zeros[..frames], &[])
        }
        .unwrap();
        assert_eq!(sequence, index);
        continuation = context.advanced(frames, 48000).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(received) = port.receive(&mut left, &mut right).unwrap() {
                assert_eq!(received.frames, frames);
                assert_eq!(received.sequence, index);
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        timings.push(time.elapsed().as_secs_f64() * 1000.0);
        let cycle = context.cycle.unwrap_or([0.0; 2]);
        let fields = [
            context.project_frame as f32,
            context.continuous_frame as f32,
            context.project_beat as f32,
            context.bar_beat as f32,
            context.tempo as f32,
            7.0,
            8.0,
            context.playing as u8 as f32,
            cycle[0] as f32,
            cycle[1] as f32,
        ];
        for i in 0..frames {
            assert!(
                (left[i] - (epoch_frames + i as u64) as f32 / 1_000_000.0).abs() < 1e-7,
                "processor clock drift at block {index}, sample {i}"
            );
            if i % 11 < 10 {
                assert_eq!(
                    right[i],
                    fields[i % 11],
                    "context mismatch at block {index}, field {}",
                    i % 11
                );
            } else {
                let flags = right[i] as u32;
                // SDK flags: playing 1<<1, cycleActive 1<<2, cycleValid 1<<12.
                assert_eq!(flags & (1 << 1) != 0, context.playing);
                assert_eq!(flags & (1 << 2) != 0, context.cycle.is_some());
                assert_eq!(flags & (1 << 12) != 0, context.cycle.is_some());
            }
        }
        assert!(left[frames..]
            .iter()
            .chain(&right[frames..])
            .all(|v| *v == 0.0));
        processed += frames as u64;
        epoch_frames += frames as u64;
    }
    let diagnostics = session.diagnostics();
    session.close();
    timings.sort_by(f64::total_cmp);
    let report = serde_json::json!({"scenario":"vst3-transport-context", "streamProtocolVersion":11,
        "device":null,"sampleRate":48000,"maxBlockSize":128,"blocks":400,"verifiedFrames":processed,
        "resets":resets,
        "callbackP95Ms":null,"callbackP99Ms":null,"xruns":null,
        "roundtripP95Ms":timings[380],"roundtripP99Ms":timings[396],"diagnostics":diagnostics,
        "checks":["short-block processor clock", "project and continuous frames", "tempo and meter",
            "musical position and bar", "playing and cycle flags", "short output padding",
            "DSP reset at packet boundary with continuous transport and sequence",
            "live state capture preserves processor clock and musical time"]});
    std::fs::write(&args[3], serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    println!("verified {processed} frames through a real VST3 processor");
}

fn context_at(index: u64, processed: u64) -> Transport {
    Transport {
        project_frame: if index % 2 == 0 { 96000 } else { 24000 },
        continuous_frame: 256000 + processed,
        project_beat: if index % 2 == 0 { 5.5 } else { 1.75 },
        bar_beat: if index % 2 == 0 { 3.5 } else { 0.0 },
        tempo: if index % 2 == 0 { 137.0 } else { 89.0 },
        time_signature: [7, 8],
        playing: index % 2 == 0,
        cycle: (index % 2 == 0).then_some([3.5, 10.5]),
    }
}
