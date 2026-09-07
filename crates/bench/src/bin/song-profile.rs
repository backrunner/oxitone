//! Offline whole-song graph timing. Never creates a device or realtime session.
//! Usage: song-profile SNAPSHOT DRUM_REGISTRATION_JSON [zero-based start bars...]
use oxitone_core::wire::{AllowPlugins, ProjectSnapshot, RegisterPluginOptions};
use oxitone_render::{builtin_registry, RenderGraph, RenderGraphOptions, SampleStore};
use serde_json::json;
use std::time::Instant;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("snapshot path");
    let snapshot: ProjectSnapshot = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let registration: RegisterPluginOptions = serde_json::from_slice(
        &std::fs::read(args.next().expect("local drum registration JSON")).unwrap(),
    )
    .unwrap();
    assert!(
        registration.expected_hash.is_some(),
        "use hash-pinned local drums"
    );
    // The caller explicitly supplies the locally prepared, hash-pinned library.
    let plugin =
        unsafe { oxitone_render::plugins::load_plugin(&registration, AllowPlugins::Any) }.unwrap();
    let mut registry = builtin_registry().unwrap();
    registry.register(plugin.clone()).unwrap();
    let mut graph = RenderGraph::compile(
        &snapshot,
        &registry,
        &SampleStore::new(None),
        &RenderGraphOptions::default(),
    )
    .unwrap();
    let bars: Vec<u64> = args.map(|v| v.parse().unwrap()).collect();
    let bars = if bars.is_empty() { vec![24, 80] } else { bars };
    let block = snapshot.block_size as usize;
    let rate = snapshot.sample_rate as f64;
    let (mut left, mut right) = (vec![0.; block], vec![0.; block]);
    let mut regions = Vec::new();
    for bar in bars {
        let beat = oxitone_core::beat::Beat::new((bar * 4) as i64, 1).unwrap();
        let first = graph.plan().tempo.beat_to_frame(beat);
        graph.seek(first);
        graph.transport_mut().begin_render(first);
        for _ in 0..128 {
            let _ftz = oxitone_dsp::ftz::FtzGuard::new();
            graph.process_block(&mut left, &mut right);
        }
        let mut times = Vec::with_capacity(4000);
        let mut peak = 0f32;
        for _ in 0..4000 {
            let _ftz = oxitone_dsp::ftz::FtzGuard::new();
            let start = Instant::now();
            graph.process_block(&mut left, &mut right);
            times.push(start.elapsed().as_secs_f64() * 1000.);
            for value in left.iter().chain(&right) {
                assert!(value.is_finite());
                peak = peak.max(value.abs());
            }
        }
        assert!(peak > 0.01, "profile region must contain audio");
        times.sort_by(f64::total_cmp);
        regions.push(json!({"startBar": bar, "warmupBlocks": 128, "blocks": times.len(),
            "meanMs": times.iter().sum::<f64>() / times.len() as f64,
            "p95Ms": times[times.len() * 95 / 100], "p99Ms": times[times.len() * 99 / 100],
            "maxMs": times.last(), "peak": peak, "deadlineMs": block as f64 / rate * 1000.,
            "deadlineExceedances": times.iter().filter(|t| **t > block as f64 / rate * 1000.).count()}));
    }
    assert_eq!(plugin.fault_count(), 0);
    println!("{}", serde_json::to_string_pretty(&json!({
        "snapshot": path, "sampleRate": rate, "blockSize": block, "tracks": snapshot.tracks.len(),
        "instrumentChannels": snapshot.channels.len(), "mixerBuses": snapshot.mixer_channels.len(),
        "graphLatencyFrames": graph.graph_latency_frames(), "pluginFaults": plugin.fault_count(),
        "device": "offline memory buffers", "callbackP99Ms": null, "xruns": null,
        "regions": regions
    })).unwrap());
}
