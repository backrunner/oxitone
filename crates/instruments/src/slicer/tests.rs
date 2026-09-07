//! Slicer tests: state schema validation, marker/grid/onset resolution
//! goldens, trigger mapping, oneshot/gate semantics, per-slice overrides
//! (level/pan/rate/reverse), and determinism.

use std::collections::BTreeMap;
use std::sync::Arc;

use oxitone_graph::abi::PluginInstance;

use super::{resolve_slices, SlicerPlugin, MAX_SLICES};
use crate::testutil::*;
use crate::InstrumentConfig;

fn impulse_sample(positions: &[usize], frames: usize) -> Arc<oxitone_samples::PreparedSample> {
    let mut channel = vec![0.0f32; frames];
    for &p in positions {
        for k in 0..64 {
            if p + k < frames {
                channel[p + k] = if k % 2 == 0 { 0.9 } else { -0.9 };
            }
        }
    }
    Arc::new(prepared(vec![channel], None))
}

fn configured(
    state: serde_json::Value,
    sample: Arc<oxitone_samples::PreparedSample>,
) -> Box<dyn PluginInstance> {
    let plugin = SlicerPlugin;
    let params = BTreeMap::new();
    let config = InstrumentConfig {
        parameters: &params,
        resources: None,
        state: Some(&state),
    };
    let mut map = BTreeMap::new();
    map.insert("s1".to_string(), sample);
    let samples = MapSamples(map);
    let mut instance = Box::new(
        plugin
            .create_configured(&host(), &config, &samples, None)
            .expect("valid config"),
    );
    instance.prepare(48_000.0, 128);
    instance
}

#[test]
fn descriptor_validates_with_state_schema() {
    let descriptor = super::descriptor();
    descriptor.validate().expect("descriptor is valid");
    assert_eq!(
        descriptor.state_schema,
        Some(oxitone_graph::SLICER_STATE_SCHEMA_ID)
    );
    assert_eq!(descriptor.plugin_id, crate::SLICER_PLUGIN_ID);
    assert_eq!(descriptor.max_polyphony, Some(MAX_SLICES as u32));
    let ids: Vec<&str> = super::parameter_specs()
        .iter()
        .map(|s| s.id.as_str())
        .collect();
    assert_eq!(ids, ["level", "pan", "tempoFactor"]);
}

#[test]
fn explicit_marker_slices_resolve_to_frame_intervals() {
    let sample = impulse_sample(&[], 4800);
    let state = serde_json::json!({
        "sampleId": "s1",
        "slices": [
            { "start": { "frames": 1200 }, "pan": -0.5 },
            { "start": { "frames": 0 }, "end": { "frames": 1000 }, "level": 0.5 },
            { "start": { "frames": "2400" }, "rate": 2.0, "reverse": true }
        ],
        "triggerNote": 62,
        "playMode": "gate"
    });
    let plugin = SlicerPlugin;
    let params = BTreeMap::new();
    let config = InstrumentConfig {
        parameters: &params,
        resources: None,
        state: Some(&state),
    };
    let mut map = BTreeMap::new();
    map.insert("s1".to_string(), sample.clone());
    let samples = MapSamples(map);
    let instance = plugin
        .create_configured(&host(), &config, &samples, None)
        .expect("valid state");
    let _ = instance;

    // Golden check on the resolved table itself.
    let parsed = super::parse_state(&state).unwrap();
    let slices = resolve_slices(&parsed, &sample, None).unwrap();
    assert_eq!(
        slices,
        vec![
            super::ResolvedSlice {
                start_frame: 0,
                end_frame: 1000,
                level: 0.5,
                pan: 0.0,
                rate: 1.0,
                reverse: false,
            },
            super::ResolvedSlice {
                start_frame: 1200,
                end_frame: 2400,
                level: 1.0,
                pan: -0.5,
                rate: 1.0,
                reverse: false,
            },
            super::ResolvedSlice {
                start_frame: 2400,
                end_frame: 4800,
                level: 1.0,
                pan: 0.0,
                rate: 2.0,
                reverse: true,
            },
        ]
    );
}

#[test]
fn grid_slices_split_the_sample_equally() {
    let sample = impulse_sample(&[], 4800);
    let state = serde_json::json!({
        "sampleId": "s1",
        "slices": { "grid": 4 },
        "playMode": "oneshot"
    });
    let parsed = super::parse_state(&state).unwrap();
    let slices = resolve_slices(&parsed, &sample, None).unwrap();
    let bounds: Vec<(u64, u64)> = slices
        .iter()
        .map(|s| (s.start_frame, s.end_frame))
        .collect();
    assert_eq!(
        bounds,
        [(0, 1200), (1200, 2400), (2400, 3600), (3600, 4800)]
    );
}

#[test]
fn onset_v1_detects_transients_deterministically() {
    let spikes = [4800usize, 9600, 14400];
    let sample = impulse_sample(&spikes, 24_000);
    let state = serde_json::json!({
        "sampleId": "s1",
        "slices": { "onset": { "algorithm": "onset-v1", "sensitivity": 0.8 } },
        "playMode": "oneshot"
    });
    let parsed = super::parse_state(&state).unwrap();
    let slices = resolve_slices(&parsed, &sample, None).unwrap();
    let slices2 = resolve_slices(&parsed, &sample, None).unwrap();
    assert_eq!(slices, slices2, "onset detection must be deterministic");
    let starts: Vec<u64> = slices.iter().map(|s| s.start_frame).collect();
    assert_eq!(starts.len(), 4, "frame 0 plus three spikes: {starts:?}");
    assert_eq!(starts[0], 0);
    for (detected, &spike) in starts[1..].iter().zip(spikes.iter()) {
        let window = super::detect_onsets_v1; // silence unused warning path
        let _ = window;
        assert!(
            detected.abs_diff(spike as u64) <= 512,
            "onset {detected} must land within one window of spike {spike}"
        );
    }
}

#[test]
fn trigger_note_maps_semitones_to_slices_out_of_range_is_silent() {
    let sample = impulse_sample(&[1200, 2400], 4800);
    let state = serde_json::json!({
        "sampleId": "s1",
        "slices": { "grid": 4 },
        "playMode": "oneshot"
    });
    let mut instance = configured(state, sample);
    // Slice 0 (note 60) and slice 2 (note 62) sound; note 65 (beyond the
    // 4 slices) must be silent; note 59 (below trigger) must be silent.
    let blocks: Vec<Block> = vec![
        Block {
            notes: vec![note_on(0, 65, 1.0), note_on(0, 59, 1.0)],
            params: vec![],
        },
        Block {
            notes: vec![note_on(0, 62, 1.0)],
            params: vec![],
        },
    ]
    .into_iter()
    .chain(silence(10))
    .collect();
    let (l, _) = run(instance.as_mut(), &blocks);
    assert!(
        l[..FRAMES].iter().all(|&x| x == 0.0),
        "out-of-range notes must not sound"
    );
    assert!(peak(&l[FRAMES..2 * FRAMES]) > 0.1, "slice 2 must sound");
}

#[test]
fn oneshot_ignores_note_off_gate_releases() {
    let channel: Vec<f32> = (0..24_000)
        .map(|i| (core::f64::consts::TAU * 440.0 * i as f64 / 48_000.0).sin() as f32)
        .collect();
    let sample = Arc::new(prepared(vec![channel], None));
    for (mode, expect_sounding) in [("oneshot", true), ("gate", false)] {
        let state = serde_json::json!({
            "sampleId": "s1",
            "slices": { "grid": 2 },
            "playMode": mode
        });
        let mut instance = configured(state, sample.clone());
        let blocks: Vec<Block> = vec![Block {
            notes: vec![note_on(0, 60, 1.0), note_off(32, 60)],
            params: vec![],
        }]
        .into_iter()
        .chain(silence(20))
        .collect();
        let (l, _) = run(instance.as_mut(), &blocks);
        let late = rms(&l[10 * FRAMES..]);
        if expect_sounding {
            assert!(late > 1e-3, "{mode}: slice must keep playing");
        } else {
            assert!(
                late < 1e-6,
                "{mode}: release must reach silence, rms {late}"
            );
        }
    }
}

#[test]
fn reverse_plays_slice_backwards() {
    // Ramp content so direction is observable.
    let frames = 2400usize;
    let channel: Vec<f32> = (0..frames).map(|i| i as f32 / frames as f32).collect();
    let sample = Arc::new(prepared(vec![channel], None));
    let state = serde_json::json!({
        "sampleId": "s1",
        "slices": [
            { "start": { "frames": 0 }, "end": { "frames": 2400 }, "reverse": true }
        ],
        "playMode": "oneshot"
    });
    let mut instance = configured(state, sample);
    let blocks: Vec<Block> = std::iter::once(Block {
        notes: vec![note_on(0, 60, 1.0)],
        params: vec![],
    })
    .chain(silence(10))
    .collect();
    let (l, _) = run(instance.as_mut(), &blocks);
    // After the 1 ms attack, output frame k plays sample[2399 - k]; the
    // center voice pan applies the equal-power factor (1/sqrt(2)).
    let k = 2 * FRAMES;
    let expected = (1.0 - k as f32 / frames as f32) * core::f32::consts::FRAC_1_SQRT_2;
    assert!(
        (l[k] - expected).abs() < 0.05,
        "reversed playback at {k}: expected ≈{expected}, got {}",
        l[k]
    );
    assert!(l[k] > l[k + FRAMES], "reversed ramp must decrease");
}

#[test]
fn rate_override_plays_slice_faster() {
    let channel: Vec<f32> = (0..4800)
        .map(|i| (core::f64::consts::TAU * 440.0 * i as f64 / 48_000.0).sin() as f32)
        .collect();
    let sample = Arc::new(prepared(vec![channel], None));
    let state = serde_json::json!({
        "sampleId": "s1",
        "slices": [
            { "start": { "frames": 0 }, "end": { "frames": 4800 }, "rate": 2.0 }
        ],
        "playMode": "oneshot"
    });
    let mut instance = configured(state, sample);
    let blocks: Vec<Block> = std::iter::once(Block {
        notes: vec![note_on(0, 60, 1.0)],
        params: vec![],
    })
    .chain(silence(60))
    .collect();
    let (l, _) = run(instance.as_mut(), &blocks);
    // At rate 2 the 4800-frame slice ends after ~2400 output frames.
    assert!(peak(&l[FRAMES..10 * FRAMES]) > 0.1);
    assert!(
        rms(&l[40 * FRAMES..]) < 1e-6,
        "rate 2 slice must end after ~2400 frames"
    );
}

#[test]
fn invalid_state_is_rejected() {
    let plugin = SlicerPlugin;
    let params = BTreeMap::new();
    let sample = impulse_sample(&[], 4800);
    let mut map = BTreeMap::new();
    map.insert("s1".to_string(), sample);
    let samples = MapSamples(map);
    let attempt = |state: serde_json::Value| {
        let config = InstrumentConfig {
            parameters: &params,
            resources: None,
            state: Some(&state),
        };
        plugin.create_configured(&host(), &config, &samples, None)
    };
    for state in [
        serde_json::json!({ "slices": { "grid": 4 }, "playMode": "oneshot" }), // no sampleId
        serde_json::json!({ "sampleId": "s1", "slices": { "grid": 0 } }),
        serde_json::json!({ "sampleId": "s1", "slices": { "grid": 65 } }),
        serde_json::json!({ "sampleId": "s1", "slices": { "onset": { "algorithm": "onset-v2" } } }),
        serde_json::json!({ "sampleId": "s1", "slices": [], "playMode": "oneshot" }),
        serde_json::json!({ "sampleId": "s1", "slices": { "grid": 2 }, "playMode": "hold" }),
        serde_json::json!({ "sampleId": "s1", "slices": { "grid": 2 }, "playMode": 1 }),
        serde_json::json!({ "sampleId": "s1", "slices": { "grid": 2 }, "triggerNote": 128 }),
        serde_json::json!({ "sampleId": "s1", "slices": { "grid": 2 }, "tempoSync": "stretch" }),
        serde_json::json!({ "sampleId": "ghost", "slices": { "grid": 2 } }),
        serde_json::json!({
            "sampleId": "s1",
            "slices": [{ "start": { "beat": { "numerator": 1, "denominator": 1 } } }]
        }), // beat markers without a beat→frame map
    ] {
        assert!(attempt(state).is_err());
    }
}

#[test]
fn beat_markers_resolve_through_the_compilers_map() {
    let sample = impulse_sample(&[], 9600);
    let state = serde_json::json!({
        "sampleId": "s1",
        "slices": [
            { "start": { "frames": 0 } },
            { "start": { "beat": { "numerator": 2, "denominator": 1 } } }
        ],
        "playMode": "oneshot"
    });
    let parsed = super::parse_state(&state).unwrap();
    // The graph compiler's baked tempo table; here 1 beat = 2400 frames.
    let beat_to_frame = |beat: oxitone_core::Beat| Some((beat.to_f64() * 2400.0).round() as u64);
    let slices = resolve_slices(&parsed, &sample, Some(&beat_to_frame)).unwrap();
    assert_eq!(slices[1].start_frame, 4800);
    assert_eq!(slices[1].end_frame, 9600);
}

#[test]
fn synthesis_is_byte_deterministic() {
    let sample = impulse_sample(&[1200, 2400], 4800);
    let state = serde_json::json!({
        "sampleId": "s1",
        "slices": { "grid": 4 },
        "playMode": "gate"
    });
    let events: Vec<Block> = vec![
        Block {
            notes: vec![note_on(0, 60, 1.0), note_on(16, 62, 0.8)],
            params: vec![],
        },
        Block {
            notes: vec![note_off(32, 60), note_off(48, 62)],
            params: vec![],
        },
    ]
    .into_iter()
    .chain(silence(20))
    .collect();
    let mut a = configured(state.clone(), sample.clone());
    let mut b = configured(state, sample);
    let (al, ar) = run(a.as_mut(), &events);
    let (bl, br) = run(b.as_mut(), &events);
    assert_eq!(al, bl);
    assert_eq!(ar, br);
    assert!(rms(&al) > 1e-3 && al.iter().all(|x| x.is_finite()));
}
