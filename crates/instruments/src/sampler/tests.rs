//! Sampler tests: descriptor validation, sample injection, rootKey varispeed
//! pitch ratio, loop forward, velocity sensitivity, and determinism.

use std::collections::BTreeMap;
use std::sync::Arc;

use oxitone_graph::abi::PluginInstance;

use super::SamplerPlugin;
use crate::testutil::*;
use crate::InstrumentConfig;

fn sine_sample(freq: f64, frames: usize) -> Arc<oxitone_samples::PreparedSample> {
    let channel: Vec<f32> = (0..frames)
        .map(|i| (core::f64::consts::TAU * freq * i as f64 / 48_000.0).sin() as f32)
        .collect();
    Arc::new(prepared(vec![channel], None))
}

fn samples_with(id: &str, sample: Arc<oxitone_samples::PreparedSample>) -> MapSamples {
    let mut map = BTreeMap::new();
    map.insert(id.to_string(), sample);
    MapSamples(map)
}

fn configured(
    params: &BTreeMap<String, f64>,
    sample: Arc<oxitone_samples::PreparedSample>,
) -> Box<dyn PluginInstance> {
    let plugin = SamplerPlugin;
    let mut resources = BTreeMap::new();
    resources.insert("sample".to_string(), "s1".to_string());
    let config = InstrumentConfig {
        parameters: params,
        resources: Some(&resources),
        state: None,
    };
    let samples = samples_with("s1", sample);
    let mut instance = Box::new(
        plugin
            .create_configured(&host(), &config, &samples)
            .expect("valid config"),
    );
    instance.prepare(48_000.0, 128);
    instance
}

#[test]
fn descriptor_validates() {
    super::descriptor().validate().expect("descriptor is valid");
    assert_eq!(super::descriptor().max_polyphony, Some(32));
    let ids: Vec<&str> = super::parameter_specs()
        .iter()
        .map(|s| s.id.as_str())
        .collect();
    for expected in [
        "rootKey",
        "velocitySensitivity",
        "amp.attack",
        "amp.decay",
        "amp.sustain",
        "amp.release",
        "loop",
        "start",
        "level",
        "pan",
    ] {
        assert!(ids.contains(&expected), "missing parameter {expected}");
    }
}

#[test]
fn missing_sample_is_asset_unavailable_unknown_param_rejected() {
    let plugin = SamplerPlugin;
    let params = BTreeMap::new();
    let mut resources = BTreeMap::new();
    resources.insert("sample".to_string(), "ghost".to_string());
    let config = InstrumentConfig {
        parameters: &params,
        resources: Some(&resources),
        state: None,
    };
    let empty = MapSamples(BTreeMap::new());
    let err = plugin
        .create_configured(&host(), &config, &empty)
        .err()
        .unwrap();
    assert_eq!(err.code, oxitone_core::codes::ASSET_UNAVAILABLE);

    let err = plugin
        .create_configured(&host(), &InstrumentConfig::new(&params), &empty)
        .err()
        .unwrap();
    assert_eq!(err.code, oxitone_core::codes::INVALID_PROJECT);

    let mut bad = BTreeMap::new();
    bad.insert("rootKey".to_string(), 200.0);
    let config = InstrumentConfig {
        parameters: &bad,
        resources: Some(&resources),
        state: None,
    };
    let err = plugin
        .create_configured(&host(), &config, &empty)
        .err()
        .unwrap();
    assert_eq!(err.code, oxitone_core::codes::INVALID_PROJECT);
}

/// Dominant frequency estimate via upward zero crossings.
fn measured_freq(buf: &[f32]) -> f64 {
    let crossings = buf.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count();
    crossings as f64 / (buf.len() as f64 / 48_000.0)
}

#[test]
fn root_key_varispeed_pitch_ratio() {
    let params = BTreeMap::new();
    for (pitch, expected) in [(60u8, 440.0), (72, 880.0), (48, 220.0)] {
        let mut instance = configured(&params, sine_sample(440.0, 48_000));
        let blocks: Vec<Block> = std::iter::once(Block {
            notes: vec![note_on(0, pitch, 1.0)],
            params: vec![],
        })
        .chain(silence(100))
        .collect();
        let (l, _) = run(instance.as_mut(), &blocks);
        // Steady state, away from the attack and the rate-smoother settle.
        let region = &l[20 * FRAMES..80 * FRAMES];
        let freq = measured_freq(region);
        assert!(
            (freq - expected).abs() < expected * 0.02,
            "pitch {pitch}: expected {expected} Hz, measured {freq} Hz"
        );
    }
}

#[test]
fn velocity_sensitivity_scales_level() {
    let full = configured(&BTreeMap::new(), sine_sample(440.0, 48_000));
    let half = configured(&BTreeMap::new(), sine_sample(440.0, 48_000));
    let render = |mut instance: Box<dyn PluginInstance>, velocity: f32| {
        let blocks: Vec<Block> = std::iter::once(Block {
            notes: vec![note_on(0, 60, velocity)],
            params: vec![],
        })
        .chain(silence(40))
        .collect();
        run(instance.as_mut(), &blocks).0
    };
    let a = render(full, 1.0);
    let b = render(half, 0.5);
    let region = 20 * FRAMES..40 * FRAMES;
    let ratio = rms(&b[region.clone()]) / rms(&a[region]);
    assert!(
        (ratio - 0.5).abs() < 0.02,
        "velocity 0.5 at sensitivity 1 must halve the level, ratio {ratio}"
    );
}

#[test]
fn loop_forward_sustains_loop_off_ends() {
    let frames = 1000usize;
    let channel: Vec<f32> = (0..frames)
        .map(|i| (core::f64::consts::TAU * 3.0 * i as f64 / frames as f64).sin() as f32)
        .collect();

    let mut params = BTreeMap::new();
    params.insert("loop".to_string(), 1.0);
    let sample = Arc::new(prepared(vec![channel.clone()], Some((200, 1000))));
    let mut instance = configured(&params, sample);
    let blocks: Vec<Block> = std::iter::once(Block {
        notes: vec![note_on(0, 60, 1.0)],
        params: vec![],
    })
    .chain(silence(40)) // > 5000 frames, well past the sample end
    .collect();
    let (l, _) = run(instance.as_mut(), &blocks);
    assert!(
        rms(&l[30 * FRAMES..]) > 1e-3,
        "forward loop must keep sounding past the sample end"
    );

    let params = BTreeMap::new(); // loop off
    let sample = Arc::new(prepared(vec![channel], Some((200, 1000))));
    let mut instance = configured(&params, sample);
    let (l, _) = run(instance.as_mut(), &blocks);
    assert!(
        rms(&l[30 * FRAMES..]) < 1e-6,
        "loop off must end at the sample end"
    );
}

#[test]
fn start_offset_skips_sample_head() {
    let frames = 4800usize;
    let mut channel = vec![0.0f32; frames];
    for (i, x) in channel.iter_mut().enumerate().skip(2400) {
        *x = (core::f64::consts::TAU * 440.0 * i as f64 / 48_000.0).sin() as f32;
    }
    let mut params = BTreeMap::new();
    params.insert("start".to_string(), 0.05); // 2400 frames at 48 kHz
    let mut instance = configured(&params, Arc::new(prepared(vec![channel], None)));
    let blocks: Vec<Block> = std::iter::once(Block {
        notes: vec![note_on(0, 60, 1.0)],
        params: vec![],
    })
    .chain(silence(5))
    .collect();
    let (l, _) = run(instance.as_mut(), &blocks);
    assert!(
        peak(&l[FRAMES..3 * FRAMES]) > 0.1,
        "playback must start at the sliced offset, not the silent head"
    );
}

#[test]
fn synthesis_is_byte_deterministic() {
    let events: Vec<Block> = vec![
        Block {
            notes: vec![note_on(0, 60, 0.9), note_on(32, 67, 0.6)],
            params: vec![],
        },
        Block {
            notes: vec![note_off(64, 60)],
            params: vec![],
        },
    ]
    .into_iter()
    .chain(silence(30))
    .collect();
    let mut a = configured(&BTreeMap::new(), sine_sample(440.0, 48_000));
    let mut b = configured(&BTreeMap::new(), sine_sample(440.0, 48_000));
    let (al, ar) = run(a.as_mut(), &events);
    let (bl, br) = run(b.as_mut(), &events);
    assert_eq!(al, bl);
    assert_eq!(ar, br);
    assert!(rms(&al) > 1e-3 && al.iter().all(|x| x.is_finite()));
}

#[test]
fn sample_accurate_note_on_offset() {
    let mut instance = configured(&BTreeMap::new(), sine_sample(440.0, 48_000));
    let (l, _) = run(
        instance.as_mut(),
        &[Block {
            notes: vec![note_on(64, 60, 1.0)],
            params: vec![],
        }],
    );
    assert!(l[..64].iter().all(|&x| x == 0.0));
    assert!(peak(&l[64..]) > 0.0);
}
