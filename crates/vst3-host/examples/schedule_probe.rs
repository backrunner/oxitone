//! Explicit VestiGain fixed-latency probe. Best-effort paced caller, no audio device or graph.
#[cfg(unix)]
fn main() {
    use oxitone_vst3_host::{
        schedule_wire::Schedule,
        stream::{
            schedule::{Fault, Input, ScheduledPort},
            Session, SessionOptions, Status,
        },
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
        "usage: vst3-schedule-probe HELPER START_JSON REPORT_JSON"
    );
    let start: Start = serde_json::from_slice(&std::fs::read(&args[2]).unwrap()).unwrap();
    assert_eq!(start.options.block_size, 128);
    assert_eq!(start.options.sample_rate, 48000);
    assert!(start.source.expected_hash.is_some());
    assert!(start.options.configuration.is_some());
    let mut calls = Vec::with_capacity(2000);
    let mut intervals = Vec::with_capacity(1000);
    let mut initialization = Vec::new();
    let mut info = serde_json::Value::Null;
    let mut deadline_misses = 0;
    let mut checked_frames = 0;
    let mut failure = None;
    let duration = Duration::from_secs_f64(128. / 48000.);
    for epoch in 0..2 {
        let time = Instant::now();
        let (mut session, port) = Session::spawn(
            Path::new(&args[1]),
            start.clone(),
            SessionOptions::default(),
        )
        .unwrap();
        initialization.push(time.elapsed().as_secs_f64() * 1000.);
        info = serde_json::to_value(session.info()).unwrap();
        assert_eq!(
            session.info().latency_frames,
            0,
            "probe requires zero-latency bypass"
        );
        let mut scheduled = ScheduledPort::prepare(
            port,
            Schedule {
                schedule_version: 1,
                epoch,
                latency_blocks: 2,
            },
        )
        .unwrap();
        assert_eq!(scheduled.latency_frames(), 256);
        let mut previous = Instant::now();
        'blocks: for block in 0..1100 {
            let tick = Instant::now();
            let interval = tick.duration_since(previous).as_secs_f64() * 1000.;
            previous = tick;
            if epoch == 0 && block >= 100 {
                intervals.push(interval);
            }
            // Two irregular segments per caller period exercise block assembly and event rebasing.
            for frames in [17, 111] {
                let frame = scheduled.frame();
                let input_l = std::array::from_fn::<_, 128, _>(|i| signal(frame + i as u64, epoch));
                let input_r = input_l.map(|v| -v);
                let mut left = [1.; 128];
                let mut right = [1.; 128];
                let events = [Event::Parameter {
                    frame: (frames - 1) as u64,
                    parameter_id: 1,
                    value: 1.,
                }];
                let time = Instant::now();
                let result = scheduled.process(
                    Input {
                        payload: &[],
                        epoch,
                        frame,
                        left: &input_l[..frames],
                        right: &input_r[..frames],
                        events: &events,
                    },
                    &mut left[..frames],
                    &mut right[..frames],
                );
                let elapsed = time.elapsed().as_secs_f64() * 1000.;
                if let Err(fault) = result {
                    deadline_misses += u32::from(fault == Fault::DeadlineMissed);
                    assert!(left[..frames]
                        .iter()
                        .chain(&right[..frames])
                        .all(|v| *v == 0.));
                    failure = Some(format!("epoch {epoch} frame {frame}: {fault:?}"));
                    break 'blocks;
                }
                for i in 0..frames {
                    let expected = (frame + i as u64)
                        .checked_sub(256)
                        .map(|f| signal(f, epoch))
                        .unwrap_or(0.);
                    assert!((left[i] - expected).abs() < 1e-6, "left {frame}+{i}");
                    assert!((right[i] + expected).abs() < 1e-6, "right {frame}+{i}");
                }
                checked_frames += frames;
                if epoch == 0 && block >= 100 {
                    calls.push(elapsed);
                }
            }
            // Never catch up with a burst after a delayed wakeup. This is not a HAL deadline test.
            std::thread::sleep((tick + duration).saturating_duration_since(Instant::now()));
        }
        scheduled.invalidate();
        if failure.is_none() {
            assert_eq!(scheduled.fault(), Some(Fault::Invalidated));
            assert_eq!(session.status(), Status::Invalidated);
        }
        session.close();
        assert_eq!(unsafe { libc::kill(session.pid() as i32, 0) }, -1);
        if failure.is_some() {
            break;
        }
    }
    let rust = std::process::Command::new("rustc")
        .arg("-vV")
        .output()
        .unwrap();
    let report = serde_json::json!({
        "scenario":"vst3-fixed-schedule", "profile":if cfg!(debug_assertions) {"debug"} else {"release"},
        "rust":String::from_utf8_lossy(&rust.stdout).trim(), "sampleRate":48000,"blockSize":128,
        "segmentFrames":[17,111],"channels":2,"queueDepth":4,"latencyBlocks":2,"totalLatencyFrames":256,
        "warmup":100,"iterations":1000,"generations":initialization.len(),"checkedFrames":checked_frames,
        "plugin":info,"parameters":{"1":1},"input":"deterministic stereo signal with opposite channels and epoch-dependent values",
        "initializationMs":initialization,"process":timings(calls),"callerInterval":timings(intervals),
        "deadlineMisses":deadline_misses,"failure":failure,
        "device":null,"callbackP95Ms":null,"callbackP99Ms":null,"cpuUtilization":null,"xruns":null,
        "scope":"native scheduled process cost and sample alignment; paced no-device caller; two fresh process epochs; no graph PDC, absolute seek, loop continuity or device deadline guarantee"
    });
    std::fs::write(
        &args[3],
        serde_json::to_string_pretty(&report).unwrap() + "\n",
    )
    .unwrap();
    assert!(failure.is_none(), "{failure:?}");
    println!("VST3 schedule probe passed: {checked_frames} aligned stereo frames, 256-frame latency, two epochs, no device");
}
#[cfg(unix)]
fn signal(frame: u64, epoch: u64) -> f32 {
    ((frame + epoch * 51) % 257) as f32 / 512. - 0.25
}
#[cfg(unix)]
fn timings(mut values: Vec<f64>) -> serde_json::Value {
    if values.is_empty() {
        return serde_json::Value::Null;
    }
    values.sort_by(f64::total_cmp);
    serde_json::json!({"p50Ms":values[values.len()/2],"p95Ms":values[values.len()*95/100],
        "p99Ms":values[values.len()*99/100],"maxMs":values[values.len()-1],"samplesMs":values})
}
#[cfg(not(unix))]
fn main() {
    panic!("macOS VST3 schedule probe only");
}
