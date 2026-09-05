//! `render/realtime` soak harness (M4 exit criteria, 00-roadmap.md;
//! 05-performance-and-benchmarks.md §Profiling 和验收): realtime playback
//! on the real CoreAudio device (default) or the simulated sink, with
//! optional worker-preemption jitter. Writes a JSON record to
//! `benchmarks/results/` with device, rate, block size, block-time
//! percentiles, engineLoad, deadline misses and xruns.
//!
//! Usage:
//!   cargo run -p oxitone-bench --release --bin realtime-soak -- \
//!       [--seconds 600] [--tracks 8] [--simulated] \
//!       [--jitter <probability:max-extra-periods:seed>] [--out PATH]
//!
//! The default run plays on the system default output device. The project
//! is audible (quiet synth patterns); use --simulated for a silent harness
//! (macOS has no null output device).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use oxitone_bench::common::{typical_snapshot, BLOCK_SIZE, SAMPLE_RATE, SEED};
use oxitone_render::realtime::{
    JitterConfig, RealtimeConfig, RealtimeSession, SessionStartError, SimulatedSinkConfig,
    TransportCmd,
};
use oxitone_render::{builtin_registry, RenderGraph, RenderGraphOptions, SampleStore};
use serde_json::json;

struct Args {
    seconds: u64,
    tracks: usize,
    simulated: bool,
    jitter: Option<JitterConfig>,
    out: Option<PathBuf>,
}

fn parse_args() -> Args {
    let mut args = Args {
        seconds: 600,
        tracks: 8,
        simulated: false,
        jitter: None,
        out: None,
    };
    let mut argv = std::env::args().skip(1);
    while let Some(arg) = argv.next() {
        match arg.as_str() {
            "--seconds" => args.seconds = argv.next().expect("--seconds value").parse().unwrap(),
            "--tracks" => args.tracks = argv.next().expect("--tracks value").parse().unwrap(),
            "--simulated" => args.simulated = true,
            "--jitter" => {
                let spec = argv.next().expect("--jitter p:max:seed");
                let parts: Vec<&str> = spec.split(':').collect();
                assert!(
                    parts.len() == 3,
                    "--jitter expects probability:max_extra:seed"
                );
                args.jitter = Some(JitterConfig {
                    probability: parts[0].parse().unwrap(),
                    max_extra_periods: parts[1].parse().unwrap(),
                    seed: parts[2].parse().unwrap(),
                });
            }
            "--out" => args.out = Some(PathBuf::from(argv.next().expect("--out value"))),
            other => panic!("unknown argument {other}"),
        }
    }
    args
}

fn sysctl(key: &str) -> String {
    std::process::Command::new("sysctl")
        .args(["-n", key])
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into())
}

fn machine_info() -> serde_json::Value {
    json!({
        "cpu": sysctl("machdep.cpu.brand_string"),
        "os": format!("macOS {}", sysctl("kern.osproductversion")),
        "arch": std::env::consts::ARCH,
        "rustc": std::process::Command::new("rustc")
            .arg("--version")
            .output()
            .ok()
            .and_then(|out| String::from_utf8(out.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "unknown".into()),
    })
}

fn start_session(
    graph: Box<RenderGraph>,
    config: RealtimeConfig,
    args: &Args,
) -> Result<RealtimeSession, SessionStartError> {
    if args.simulated {
        let sink = SimulatedSinkConfig {
            sample_rate: f64::from(SAMPLE_RATE),
            frames_per_slice: BLOCK_SIZE,
            channels: 2,
            latency_frames: 24,
            safety_offset_frames: 10,
        };
        RealtimeSession::start_simulated(graph, config, sink, args.jitter)
    } else {
        RealtimeSession::start(graph, config)
    }
}

fn main() {
    let args = parse_args();
    let mut snapshot = typical_snapshot(args.tracks, 32, 8);
    // Keep the real-device run unobtrusive: the workload is audible on the
    // default output, so play it quietly (gain-only; CPU cost unchanged).
    for channel in &mut snapshot.channels {
        channel.level = 0.05;
    }
    let registry = builtin_registry().unwrap();
    let store = SampleStore::new(None);
    let graph = Box::new(
        RenderGraph::compile(&snapshot, &registry, &store, &RenderGraphOptions::default())
            .expect("workload compiles"),
    );

    let session = start_session(graph, RealtimeConfig::default(), &args)
        .map_err(|failure| failure.error)
        .unwrap();
    let latency = session.output_latency().unwrap();
    let (state, _) = session.transport(TransportCmd::Play { from: None });
    assert_eq!(state, oxitone_render::TransportState::Playing);

    let started = Instant::now();
    let mut event_log: Vec<serde_json::Value> = Vec::new();
    let mut engine_load_max = 0.0f64;
    let mut occupancy_max = 0usize;
    while started.elapsed() < Duration::from_secs(args.seconds) {
        std::thread::sleep(Duration::from_secs(5));
        let snapshot = session.snapshot_diagnostics();
        engine_load_max = engine_load_max.max(snapshot.engine_load);
        occupancy_max = occupancy_max.max(snapshot.ring_occupancy_frames);
        for event in &snapshot.events {
            event_log.push(json!({
                "code": event.code,
                "severity": event.severity.as_str(),
                "frame": event.frame.to_string(),
                "message": event.message(),
            }));
        }
    }
    let diagnostics = session.snapshot_diagnostics();
    session.transport(TransportCmd::Stop);

    let devices = oxitone_io_macos::list_output_devices().unwrap_or_default();
    let default_device = devices.iter().find(|d| d.is_default);
    let date = std::process::Command::new("date")
        .arg("+%Y-%m-%d")
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown-date".into());
    let host = sysctl("kern.hostname");
    let report = json!({
        "benchmark": "render/realtime soak (M4 exit)",
        "date": date,
        "machine": machine_info(),
        "sink": if args.simulated { "simulated" } else { "coreaudio" },
        "device": default_device.map(|d| json!({
            "id": d.id,
            "name": d.name,
            "nominalSampleRates": d.nominal_sample_rates,
            "bufferFrameSizeRange": [d.buffer_frame_size_range.0, d.buffer_frame_size_range.1],
        })),
        "config": {
            "sampleRate": SAMPLE_RATE,
            "blockSize": BLOCK_SIZE,
            "renderAheadBlocks": 4,
            "latencyMode": "buffered",
            "tracks": args.tracks,
            "seed": SEED,
            "seconds": args.seconds,
            "workerJitter": args.jitter.map(|j| json!({
                "probability": j.probability,
                "maxExtraPeriods": j.max_extra_periods,
                "seed": j.seed,
            })),
        },
        "latency": {
            "frames": latency.frames,
            "seconds": latency.seconds,
            "breakdown": {
                "ring": latency.ring,
                "resampler": latency.resampler,
                "deviceBuffer": latency.device_buffer,
                "safetyOffset": latency.safety_offset,
                "deviceLatency": latency.device_latency,
            },
        },
        "results": {
            "blocks": diagnostics.blocks,
            "xruns": diagnostics.xruns,
            "deadlineMisses": diagnostics.deadline_misses,
            "nanBlocks": diagnostics.nan_blocks,
            "queueDrops": diagnostics.queue_drops,
            "performanceWarnings": diagnostics.performance_warnings,
            "engineLoadEma": diagnostics.engine_load,
            "engineLoadMax": engine_load_max,
            "ringOccupancyMaxFrames": occupancy_max,
            "blockTimeNs": {
                "p50": diagnostics.block_time_p50_ns,
                "p95": diagnostics.block_time_p95_ns,
                "p99": diagnostics.block_time_p99_ns,
                "max": diagnostics.block_time_max_ns,
                "deadline": (BLOCK_SIZE as u64 * 1_000_000_000) / SAMPLE_RATE as u64,
            },
            "events": event_log,
        },
    });

    let out = args.out.unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../benchmarks/results")
            .join(format!("{date}-{host}-m4-soak.json"))
    });
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&out, serde_json::to_string_pretty(&report).unwrap()).unwrap();
    println!("wrote {}", out.display());
    println!(
        "xruns={} deadlineMisses={} p99={}ns engineLoad={:.3}",
        diagnostics.xruns,
        diagnostics.deadline_misses,
        diagnostics.block_time_p99_ns,
        diagnostics.engine_load
    );
    std::process::exit(if diagnostics.xruns == 0 { 0 } else { 1 });
}
