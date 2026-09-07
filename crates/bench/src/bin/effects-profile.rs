//! Silent per-effect process cost sampler. Timing surrounds DSP, never enters it.
use oxitone_graph::{HostContext, PluginInstance, ProcessContext};
use serde_json::json;
use std::time::Instant;

fn profile(id: &str, mut instance: Box<dyn PluginInstance>) -> serde_json::Value {
    let input = std::array::from_fn::<_, 128, _>(|i| 0.3 * (i as f32 * 0.1309).sin());
    let (mut left, mut right) = ([0.; 128], [0.; 128]);
    let mut times = Vec::with_capacity(3000);
    let mut peak = 0f32;
    for block in 0..3128 {
        let _ftz = oxitone_dsp::ftz::FtzGuard::new();
        let start = Instant::now();
        instance.process(&mut ProcessContext {
            frames: 128,
            sample_rate: 48000.,
            inputs: &[&input, &input],
            outputs: &mut [&mut left, &mut right],
            note_events: &[],
            parameter_events: &[],
            sidechain: None,
        });
        let elapsed = start.elapsed().as_secs_f64() * 1e6;
        if block >= 128 {
            times.push(elapsed);
        }
        for x in left.iter().chain(&right) {
            assert!(x.is_finite());
            peak = peak.max(x.abs());
        }
    }
    times.sort_by(f64::total_cmp);
    json!({ "id": id, "sampleRate": 48000, "blockSize": 128, "blocks": times.len(),
        "meanUs": times.iter().sum::<f64>() / times.len() as f64,
        "p95Us": times[times.len() * 95 / 100], "p99Us": times[times.len() * 99 / 100],
        "maxUs": times.last(), "deadlineUs": 128. / 48000. * 1e6,
        "deadlineExceedances": times.iter().filter(|t| **t > 128. / 48000. * 1e6).count(),
        "latencyFrames": instance.latency_frames(), "peak": peak })
}
fn main() {
    let host = HostContext {
        sample_rate: 48000.,
        max_block_size: 128,
    };
    let mut results = Vec::new();
    for plugin in oxitone_mixer::builtin_effect_plugins() {
        results.push(profile(
            &plugin.descriptor().plugin_id,
            plugin.create(&host),
        ));
    }
    let length = oxitone_mixer::effects::convolver::MAX_IMPULSE_FRAMES;
    let impulse: Vec<_> = (0..length)
        .map(|i| (i as f32 * 0.183).sin() * (-(i as f32) / 24000.).exp() * 0.002)
        .collect();
    let instance = oxitone_mixer::effects::convolver::from_impulse(
        oxitone_mixer::effects::convolver::Impulse {
            channels: [impulse.clone(), impulse],
            sample_rate: 48000.,
        },
        48000.,
    )
    .unwrap();
    results.push(profile("oxitone.convolver/262144-frame-ir", instance));
    println!("{}", serde_json::to_string_pretty(&json!({
        "mode": "offline DSP timing; no output device", "arch": std::env::consts::ARCH,
        "os": std::env::consts::OS, "xrunCount": serde_json::Value::Null,
        "note": "Deadline exceedances are timed DSP calls, not device xruns. Includes FFT partition bursts. Whole-project acceptance is separate.",
        "results": results,
    })).unwrap());
}
