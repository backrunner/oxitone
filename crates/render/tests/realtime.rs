//! Session-level realtime tests on the simulated sink (macOS has no null
//! output device; the real HAL path is exercised by the soak harness in
//! `crates/bench/src/bin/realtime-soak.rs`). Covers transport commands,
//! jitter absorption (03 §线程模型: 注入调度抖动时 0 underrun), underrun
//! behavior, and the latency report.

mod common;

use std::time::Duration;

use common::{base_snapshot, channel, note, pattern, pattern_clip, track};
use oxitone_core::wire::{DeviceRatePolicy, LatencyMode};
use oxitone_render::realtime::{
    JitterConfig, RealtimeConfig, RealtimeSession, SimulatedSinkConfig, TransportCmd,
};
use oxitone_render::{builtin_registry, RenderGraph, RenderGraphOptions, SampleStore};

fn playable_graph() -> Box<RenderGraph> {
    let mut snapshot = base_snapshot();
    snapshot
        .tracks
        .push(track("trk_a", &["chn_a"], &["pcl_a"], &[]));
    snapshot.patterns.push(pattern(
        "pat_a",
        (4, 1),
        vec![
            note(60, (0, 1), (1, 1), 0.9),
            note(64, (1, 1), (1, 1), 0.9),
            note(67, (2, 1), (1, 1), 0.9),
            note(72, (3, 1), (1, 1), 0.9),
        ],
    ));
    snapshot
        .pattern_clips
        .push(pattern_clip("pcl_a", "pat_a", "trk_a", (0, 1), (64, 1)));
    snapshot.channels.push(channel(
        "chn_a",
        "mix_master",
        common::wavetable_ref(&[]),
        vec![],
    ));
    Box::new(
        RenderGraph::compile(
            &snapshot,
            &builtin_registry().unwrap(),
            &SampleStore::new(None),
            &RenderGraphOptions::default(),
        )
        .unwrap(),
    )
}

fn simulated(frames_per_slice: u32) -> SimulatedSinkConfig {
    SimulatedSinkConfig {
        sample_rate: 48_000.0,
        frames_per_slice,
        channels: 2,
        latency_frames: 24,
        safety_offset_frames: 10,
    }
}

fn wait_until(deadline: Duration, mut condition: impl FnMut() -> bool) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < deadline {
        if condition() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    false
}

/// These tests measure realtime scheduling, but `cargo test` runs them on
/// parallel threads. Six engines each spinning a worker and sink thread
/// starve one another on small CI runners (3–4 cores), so every session
/// test takes this lock and owns the machine while it measures.
fn session_guard() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

#[test]
fn session_play_pause_seek_via_simulated_sink() {
    let _guard = session_guard();
    let session = RealtimeSession::start_simulated(
        playable_graph(),
        RealtimeConfig::default(),
        simulated(128),
        None,
    )
    .map_err(|e| e.error)
    .unwrap();

    let (state, _) = session
        .transport(TransportCmd::Play {
            from: None,
            loop_region: None,
        })
        .unwrap();
    assert_eq!(state, oxitone_render::TransportState::Playing);
    assert!(wait_until(Duration::from_secs(3), || session.cursor() > 4800));
    let diag = session.snapshot_diagnostics();
    assert!(diag.blocks > 0);
    assert_eq!(diag.xruns, 0);

    session.transport(TransportCmd::Pause).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let cursor_a = session.cursor();
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(session.cursor(), cursor_a, "paused cursor must not advance");

    session
        .transport(TransportCmd::Seek { frame: 24_000 })
        .unwrap();
    session
        .transport(TransportCmd::Play {
            from: None,
            loop_region: None,
        })
        .unwrap();
    assert!(wait_until(Duration::from_secs(3), || session.cursor() > 26_000));

    session.transport(TransportCmd::Stop).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(session.cursor(), 0);
}

#[test]
fn jitter_within_horizon_causes_no_underrun() {
    let _guard = session_guard();
    // Bursts up to 2 periods at 30% probability sit well inside the
    // 8-block render-ahead horizon; the extra depth also absorbs the
    // occasional worker preemption a shared CI runner cannot avoid.
    let session = RealtimeSession::start_simulated(
        playable_graph(),
        RealtimeConfig {
            render_ahead_blocks: 8,
            ..RealtimeConfig::default()
        },
        simulated(128),
        Some(JitterConfig {
            max_extra_periods: 2.0,
            probability: 0.3,
            seed: 11,
        }),
    )
    .map_err(|e| e.error)
    .unwrap();
    session
        .transport(TransportCmd::Play {
            from: None,
            loop_region: None,
        })
        .unwrap();
    // Wait on rendered frames instead of wall clock: the simulated sink
    // paces by real time, so a loaded CI runner falls behind nominal
    // realtime and a fixed sleep makes the block count flaky. Two
    // seconds of audio is ~750 jittered blocks at 48 kHz/128.
    assert!(
        wait_until(Duration::from_secs(15), || session.cursor() > 96_000),
        "render worker must keep playing through injected jitter"
    );
    let diag = session.snapshot_diagnostics();
    assert_eq!(
        diag.xruns,
        0,
        "ring must absorb injected jitter; load={}, p99_ns={}, misses={}, events={:?}",
        diag.engine_load,
        diag.block_time_p99_ns,
        diag.deadline_misses,
        diag.events
            .iter()
            .map(|event| (event.code, event.frame))
            .collect::<Vec<_>>()
    );
    assert!(diag.blocks > 96_000 / 128);
}

#[test]
fn extreme_jitter_underruns_but_transport_continues() {
    let _guard = session_guard();
    let session = RealtimeSession::start_simulated(
        playable_graph(),
        RealtimeConfig {
            render_ahead_blocks: 2,
            ..RealtimeConfig::default()
        },
        simulated(128),
        Some(JitterConfig {
            max_extra_periods: 12.0,
            probability: 0.9,
            seed: 5,
        }),
    )
    .map_err(|e| e.error)
    .unwrap();
    session
        .transport(TransportCmd::Play {
            from: None,
            loop_region: None,
        })
        .unwrap();
    let mut events = Vec::new();
    let mut underran = false;
    let reported = wait_until(Duration::from_secs(10), || {
        let diag = session.snapshot_diagnostics();
        underran |= diag.xruns > 0;
        events.extend(diag.events);
        // The callback increments xruns before publishing its queue event, and
        // diagnostics drains the queue before loading counters. Wait for both
        // observations instead of requiring an atomic cross-thread snapshot.
        underran && events.iter().any(|event| event.code == "Underrun")
    });
    assert!(underran, "extreme jitter must starve the ring");
    assert!(reported, "underrun diagnostic event must arrive");
    // Underrun recovery: transport keeps playing, cursor advances.
    let cursor = session.cursor();
    assert!(wait_until(Duration::from_secs(3), || {
        session.cursor() > cursor
    }));
    let underrun = events
        .iter()
        .find(|event| event.code == "Underrun")
        .expect("underrun diagnostic event");
    assert!(underrun.message().contains("busiest channel"));
}

#[test]
fn output_latency_breakdown_matches_chain() {
    let _guard = session_guard();
    let session = RealtimeSession::start_simulated(
        playable_graph(),
        RealtimeConfig::default(),
        simulated(128),
        None,
    )
    .map_err(|e| e.error)
    .unwrap();
    let latency = session.output_latency().unwrap();
    // renderAhead 4 * 128 ring, 128 buffer, 10 safety, 24 device latency.
    assert_eq!(latency.ring, 512);
    assert_eq!(latency.resampler, 0);
    assert_eq!(latency.device_buffer, 128);
    assert_eq!(latency.safety_offset, 10);
    assert_eq!(latency.device_latency, 24);
    assert_eq!(latency.frames, 512 + 128 + 10 + 24);
    assert!((latency.seconds - latency.frames as f64 / 48_000.0).abs() < 1e-9);
}

#[test]
fn direct_mode_reports_zero_ring() {
    let _guard = session_guard();
    let session = RealtimeSession::start_simulated(
        playable_graph(),
        RealtimeConfig {
            latency_mode: LatencyMode::Direct,
            ..RealtimeConfig::default()
        },
        simulated(128),
        None,
    )
    .map_err(|e| e.error)
    .unwrap();
    let latency = session.output_latency().unwrap();
    assert_eq!(latency.ring, 0);
    session
        .transport(TransportCmd::Play {
            from: None,
            loop_region: None,
        })
        .unwrap();
    assert!(wait_until(Duration::from_secs(3), || session.cursor() > 4800));
}

#[test]
fn resample_mode_runs_against_lower_device_rate() {
    let _guard = session_guard();
    let session = RealtimeSession::start_simulated(
        playable_graph(),
        RealtimeConfig {
            device_rate_policy: DeviceRatePolicy::Resample,
            ..RealtimeConfig::default()
        },
        SimulatedSinkConfig {
            sample_rate: 44_100.0,
            ..simulated(128)
        },
        None,
    )
    .map_err(|e| e.error)
    .unwrap();
    let latency = session.output_latency().unwrap();
    assert!(latency.resampler > 0, "resampler group delay is reported");
    session
        .transport(TransportCmd::Play {
            from: None,
            loop_region: None,
        })
        .unwrap();
    assert!(wait_until(Duration::from_secs(3), || session.cursor() > 4800));
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(session.snapshot_diagnostics().xruns, 0);
}
