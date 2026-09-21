//! VestiGain instance handoff and retirement probe; no system audio device.
#[path = "support/load.rs"]
mod load;
#[cfg(unix)]
fn main() {
    use oxitone_vst3_host::{
        manager_wire::ManagerOptions,
        schedule_wire::Schedule,
        stream::{managed::Controller, schedule::Input, SessionOptions},
        stream_wire::Start,
    };
    use std::{
        path::Path,
        time::{Duration, Instant},
    };
    let args: Vec<String> = std::env::args().collect();
    assert_eq!(
        args.len(),
        4,
        "usage: vst3-managed-probe HELPER START_JSON REPORT_JSON"
    );
    let start: Start = serde_json::from_slice(&std::fs::read(&args[2]).unwrap()).unwrap();
    assert_eq!(start.options.sample_rate, 48000);
    assert_eq!(start.options.block_size, 128);
    assert!(start.options.configuration.is_some());
    assert!(start.source.expected_hash.is_some());
    let (cpu_load, load_threads) = load::BusyLoad::start();
    let latency_blocks = std::env::var("OXITONE_VST3_MANAGED_LATENCY_BLOCKS")
        .map(|value| value.parse::<usize>().expect("latency must be an integer"))
        .unwrap_or(2);
    assert!((2..=16).contains(&latency_blocks));
    let latency_frames = (latency_blocks * 128) as u64;
    let queue_depth = latency_blocks.max(4);
    let (mut control, mut audio) = Controller::new(ManagerOptions {
        manager_version: 1,
        sample_rate: 48000,
        block_size: 128,
        capacity: 2,
    })
    .unwrap();
    let mut preparation = Vec::with_capacity(32);
    let mut swaps = Vec::with_capacity(30);
    let mut reclamation = Vec::with_capacity(32);
    let mut processing = Vec::with_capacity(2400);
    let mut intervals = Vec::with_capacity(1200);
    let mut pids = Vec::with_capacity(32);
    let mut diagnostics = Vec::with_capacity(32);
    let period = Duration::from_secs_f64(128. / 48000.);
    let mut checked_frames = 0;
    let mut failure = None;
    let mut reclaimed = 0;
    let mut activated_epochs = 0;
    let mut completed_epochs = 0;
    let mut prepare = |control: &mut Controller, epoch| {
        let before = Instant::now();
        control
            .prepare(
                Path::new(&args[1]),
                start.clone(),
                Schedule {
                    schedule_version: 1,
                    epoch,
                    latency_blocks,
                },
                SessionOptions {
                    queue_depth,
                    ..SessionOptions::default()
                },
            )
            .unwrap();
        preparation.push(before.elapsed().as_secs_f64() * 1000.);
        pids.push(control.pid(epoch).unwrap());
    };
    prepare(&mut control, 0);
    'epochs: for epoch in 0..32 {
        // Preparation occurs on control before the first sample of each newly activated epoch.
        // This probe measures method costs, not simultaneous full-engine render/compile load.
        audio.require_epoch(epoch).unwrap();
        let before = Instant::now();
        let active = audio.activate_next().unwrap().unwrap();
        activated_epochs += 1;
        let elapsed = before.elapsed().as_secs_f64() * 1000.;
        assert_eq!(active.epoch, epoch);
        assert_eq!(
            active.latency_frames as u64, latency_frames,
            "requires zero-latency VestiGain bypass"
        );
        if epoch >= 2 {
            swaps.push(elapsed);
        }
        let before = Instant::now();
        reclaimed += control.reclaim();
        reclamation.push(before.elapsed().as_secs_f64() * 1000.);
        if epoch < 31 {
            prepare(&mut control, epoch + 1);
        }
        assert!(control.live_sessions() <= 2);
        let mut frame = 0;
        let mut previous = Instant::now();
        for block in 0..40 {
            let tick = Instant::now();
            if epoch >= 2 && block > 0 {
                intervals.push(tick.duration_since(previous).as_secs_f64() * 1000.);
            }
            previous = tick;
            for frames in [17, 111] {
                let input_l = std::array::from_fn::<_, 128, _>(|i| signal(frame + i as u64, epoch));
                let input_r = input_l.map(|v| -v);
                let mut left = [1.; 128];
                let mut right = [1.; 128];
                let before = Instant::now();
                let result = audio.process(
                    Input {
                        payload: &[],
                        epoch,
                        frame,
                        left: &input_l[..frames],
                        right: &input_r[..frames],
                        events: &[],
                    },
                    &mut left[..frames],
                    &mut right[..frames],
                );
                let elapsed = before.elapsed().as_secs_f64() * 1000.;
                if let Err(error) = result {
                    assert!(left[..frames]
                        .iter()
                        .chain(&right[..frames])
                        .all(|v| *v == 0.));
                    failure = Some(format!("epoch {epoch}, frame {frame}: {error:?}"));
                    diagnostics.push(
                        serde_json::json!({"epoch":epoch,"timing":control.diagnostics(epoch)}),
                    );
                    break 'epochs;
                }
                for i in 0..frames {
                    let expected = (frame + i as u64)
                        .checked_sub(latency_frames)
                        .map(|f| signal(f, epoch))
                        .unwrap_or(0.);
                    assert!((left[i] - expected).abs() < 1e-6);
                    assert!((right[i] + expected).abs() < 1e-6);
                }
                frame += frames as u64;
                checked_frames += frames;
                if epoch >= 2 {
                    processing.push(elapsed);
                }
            }
            std::thread::sleep((tick + period).saturating_duration_since(Instant::now()));
        }
        // Deliberately leave the last delayed PCM in flight/cached. The next epoch cannot replay it.
        completed_epochs += 1;
        diagnostics.push(serde_json::json!({"epoch":epoch,"timing":control.diagnostics(epoch)}));
    }
    let before = Instant::now();
    control.shutdown();
    let shutdown_ms = before.elapsed().as_secs_f64() * 1000.;
    for pid in &pids {
        assert_eq!(unsafe { libc::kill(*pid as i32, 0) }, -1);
    }
    assert_eq!(control.live_sessions(), 0);
    drop(cpu_load);
    let rust = std::process::Command::new("rustc")
        .arg("-vV")
        .output()
        .unwrap();
    let report = serde_json::json!({
        "scenario":"vst3-managed-lifecycle","profile":if cfg!(debug_assertions) {"debug"} else {"release"},
        "rust":String::from_utf8_lossy(&rust.stdout).trim(),"sampleRate":48000,"blockSize":128,
        "channels":2,"segmentFrames":[17,111],"capacity":2,"queueDepth":queue_depth,"latencyBlocks":latency_blocks,"totalLatencyFrames":latency_frames,
        "warmupEpochs":2,"plannedEpochs":32,"preparedEpochs":pids.len(),"activatedEpochs":activated_epochs,
        "completedEpochs":completed_epochs,"blocksPerEpoch":40,"checkedFrames":checked_frames,
        "reclaimedReplacements":reclaimed,"reapedProcesses":pids.len(),"pluginHash":start.source.expected_hash,
        "parameters":start.options.parameters,"prepare":timings(preparation),"activate":timings(swaps),
        "process":timings(processing),"reclaim":timings(reclamation),"callerInterval":timings(intervals),
        "shutdownMs":shutdown_ms,"failure":failure,"diagnostics":diagnostics,"loadThreads":load_threads,
        "device":null,"callbackP95Ms":null,"callbackP99Ms":null,"cpuUtilization":null,"xruns":null,
        "scope":"native lifecycle methods and PCM epoch isolation; paced no-device caller; control preparation before epoch audio; no graph, absolute seek or device deadline guarantee"
    });
    std::fs::write(
        &args[3],
        serde_json::to_string_pretty(&report).unwrap() + "\n",
    )
    .unwrap();
    assert!(failure.is_none(), "{failure:?}");
    println!("VST3 managed probe passed: {checked_frames} stereo frames, 32 epochs, 31 retirements, all helpers reaped; no device");
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
    panic!("macOS VST3 managed probe only");
}
