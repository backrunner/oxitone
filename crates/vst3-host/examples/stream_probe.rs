//! Explicit local VestiGain stream/queue probe. Never creates an audio device.
#[cfg(unix)]
fn main() {
    use oxitone_vst3_host::{
        stream::{Session, SessionOptions, Status},
        stream_wire::Start,
        wire::Event,
    };
    use std::{
        path::Path,
        time::{Duration, Instant},
    };
    let args: Vec<String> = std::env::args().collect();
    assert_eq!(
        args.len(),
        4,
        "usage: vst3-stream-probe HELPER START_JSON REPORT_JSON"
    );
    let start: Start = serde_json::from_slice(&std::fs::read(&args[2]).unwrap()).unwrap();
    assert_eq!(start.options.block_size, 128);
    assert_eq!(start.options.sample_rate, 48000);
    assert!(start.source.expected_hash.is_some());
    assert!(start.options.configuration.is_some());
    let mut roundtrip = Vec::with_capacity(1000);
    let mut submits = Vec::with_capacity(1000);
    let mut receives = Vec::with_capacity(1000);
    let mut initialization = Vec::new();
    let mut info = serde_json::Value::Null;
    // Recreate the process to prove recovery starts a new stream, never replays an old queue.
    for generation in 0..2 {
        let time = Instant::now();
        let (mut session, mut port) = Session::spawn(
            Path::new(&args[1]),
            start.clone(),
            SessionOptions::default(),
        )
        .unwrap();
        initialization.push(time.elapsed().as_secs_f64() * 1000.);
        info = serde_json::to_value(session.info()).unwrap();
        let input = [0.25f32; 128];
        let mut left = [0.; 128];
        let mut right = [0.; 128];
        for i in 0..1100u64 {
            let frames = if i < 100 && i % 2 != 0 { 17 } else { 128 };
            let events = [Event::Parameter {
                frame: (frames - 1) as u64,
                parameter_id: 1,
                value: 1.,
            }];
            let time = Instant::now();
            let submit = Instant::now();
            assert_eq!(
                port.submit(&input[..frames], &input[..frames], &events)
                    .unwrap(),
                i
            );
            let submit_ms = submit.elapsed().as_secs_f64() * 1000.;
            let deadline = Instant::now() + Duration::from_secs(2);
            let receive_ms = loop {
                let receive = Instant::now();
                let result = port.receive(&mut left, &mut right).unwrap();
                let elapsed = receive.elapsed().as_secs_f64() * 1000.;
                if let Some(result) = result {
                    assert_eq!(result.sequence, i);
                    assert_eq!(result.frames, frames);
                    break elapsed;
                }
                assert!(Instant::now() < deadline);
                std::thread::yield_now();
            };
            let total_ms = time.elapsed().as_secs_f64() * 1000.;
            assert!(left[..frames].iter().all(|v| (*v - 0.25).abs() < 1e-6));
            assert_eq!(left, right);
            assert!(left[frames..].iter().all(|v| *v == 0.));
            if generation == 0 && i >= 100 {
                roundtrip.push(total_ms);
                submits.push(submit_ms);
                receives.push(receive_ms);
            }
        }
        // Four queued blocks preserve sequence; every buffer is reused, including variable frame counts.
        for i in 1100..1104 {
            assert_eq!(port.submit(&input, &input, &[]).unwrap(), i);
        }
        let mut next = 1100;
        let deadline = Instant::now() + Duration::from_secs(2);
        while next < 1104 {
            if let Some(received) = port.receive(&mut left, &mut right).unwrap() {
                assert_eq!(received.sequence, next);
                assert_eq!(left, input);
                assert_eq!(right, input);
                next += 1;
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(session.status(), Status::Running);
        session.close();
        assert_eq!(port.status(), Status::Closed);
        assert_eq!(unsafe { libc::kill(session.pid() as i32, 0) }, -1);
    }
    let rust = std::process::Command::new("rustc")
        .arg("-vV")
        .output()
        .unwrap();
    let report = serde_json::json!({
        "scenario":"vst3-native-stream", "profile":if cfg!(debug_assertions) { "debug" } else { "release" },
        "rust":String::from_utf8_lossy(&rust.stdout).trim(), "sampleRate":48000,"blockSize":128,
        "channels":2,"queueDepth":4,"warmup":100,"iterations":1000,"generations":2,
        "plugin":info,"parameters":{"1":1},"input":"stereo constant 0.25", "event":"bypass=1 at final block frame",
        "initializationMs":initialization,"roundtrip":timings(roundtrip),"submit":timings(submits),"receive":timings(receives),
        "device":null,"callbackP95Ms":null,"callbackP99Ms":null,"cpuUtilization":null,"xruns":null,
        "scope":"queue + worker socket + isolated plugin processing; busy polling caller; no device, scheduling guarantee, graph PDC, seek, loop or editor validation"
    });
    std::fs::write(
        &args[3],
        serde_json::to_string_pretty(&report).unwrap() + "\n",
    )
    .unwrap();
    println!("VST3 stream probe passed: 2208 ordered blocks, state/parameter initialization, partial blocks, two process generations, bounded close; no device");
}
#[cfg(unix)]
fn timings(mut values: Vec<f64>) -> serde_json::Value {
    values.sort_by(f64::total_cmp);
    serde_json::json!({"p50Ms":values[values.len()/2],"p95Ms":values[values.len()*95/100],
        "p99Ms":values[values.len()*99/100],"maxMs":values[values.len()-1],"samplesMs":values})
}
#[cfg(not(unix))]
fn main() {
    panic!("macOS VST3 stream probe only");
}
