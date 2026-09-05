mod common;

use common::*;
use oxitone_core::codes;
use oxitone_core::wire::{EffectRef, ProjectSnapshot, SampleFormat, SampleRef};
use oxitone_graph::validate;
use serde_json::{json, Value};

fn err(
    snapshot: &ProjectSnapshot,
    registry: &oxitone_graph::PluginRegistry,
) -> oxitone_core::OxitoneError {
    validate(snapshot, registry).unwrap_err()
}

fn effect_ref(plugin_id: &str) -> EffectRef {
    EffectRef {
        plugin_id: plugin_id.to_string(),
        plugin_version: PLUGIN_VERSION.to_string(),
        parameters: Default::default(),
        resources: None,
        bypass: None,
        mix: None,
    }
}

#[test]
fn unknown_plugin_and_version_mismatch() {
    let mut s = base_snapshot();
    s.channels[0].instrument.plugin_id = "oxi.unknown".to_string();
    let e = err(&s, &base_registry());
    assert_eq!(e.code, codes::INVALID_PROJECT);
    assert!(e.message.contains("unknown plugin"), "{}", e.message);
    assert_eq!(e.path.as_deref(), Some("$.channels[0].instrument"));

    let mut s = base_snapshot();
    s.channels[0].instrument.plugin_version = "2.0.0".to_string();
    let e = err(&s, &base_registry());
    assert_eq!(e.code, codes::INVALID_PROJECT);
    assert!(e.message.contains("no registered version"), "{}", e.message);
}

#[test]
fn parameter_tables_are_checked_against_descriptors() {
    let registry = base_registry();

    let mut s = base_snapshot();
    s.channels[0]
        .instrument
        .parameters
        .insert("notAParam".to_string(), 1.0);
    let e = err(&s, &registry);
    assert_eq!(e.code, codes::INVALID_PROJECT);
    assert_eq!(
        e.path.as_deref(),
        Some("$.channels[0].instrument.parameters.notAParam")
    );

    let mut s = base_snapshot();
    s.channels[0]
        .instrument
        .parameters
        .insert("gainDb".to_string(), 100.0);
    let e = err(&s, &registry);
    assert!(e.path.as_deref().unwrap().ends_with("parameters.gainDb"));

    let mut s = base_snapshot();
    s.channels[0]
        .instrument
        .parameters
        .insert("gainDb".to_string(), f64::NAN);
    assert_eq!(err(&s, &registry).code, codes::INVALID_PROJECT);

    // Missing parameters fall back to descriptor defaults; in-range values pass.
    let mut s = base_snapshot();
    s.channels[0]
        .instrument
        .parameters
        .insert("gainDb".to_string(), -12.0);
    assert!(validate(&s, &registry).is_ok());
}

#[test]
fn kinds_must_match_the_slot() {
    let registry = base_registry();

    let mut s = base_snapshot();
    s.channels[0].instrument.plugin_id = EFFECT_ID.to_string();
    let e = err(&s, &registry);
    assert!(e.message.contains("not an instrument"), "{}", e.message);

    let mut s = base_snapshot();
    s.channels[0].effect_chain.push(effect_ref(SYNTH_ID));
    let e = err(&s, &registry);
    assert!(e.message.contains("not an effect"), "{}", e.message);

    let mut s = base_snapshot();
    s.channels[0].effect_chain.push(effect_ref(EFFECT_ID));
    assert!(validate(&s, &registry).is_ok());

    let mut s = base_snapshot();
    s.mixer_channels[0].inserts.push(effect_ref("oxi.unknown"));
    let e = err(&s, &registry);
    assert_eq!(e.path.as_deref(), Some("$.mixerChannels[0].inserts[0]"));
}

fn slicer_snapshot(state: Value) -> (ProjectSnapshot, oxitone_graph::PluginRegistry) {
    let mut registry = base_registry();
    registry.register(mock_plugin(slicer_descriptor())).unwrap();
    let mut s = base_snapshot();
    s.samples.push(SampleRef {
        id: "smp_0001".to_string(),
        asset_uri: "assets/a.wav".to_string(),
        sha256: "ab".repeat(32),
        format: SampleFormat::Wav,
        sample_rate: 48_000,
        channels: 2,
        frames: 192_000,
        edits: None,
        musical_length_beats: None,
    });
    s.channels[0].instrument.plugin_id = "oxitone.slicer".to_string();
    s.channels[0].instrument.state = Some(state);
    (s, registry)
}

fn explicit_slices() -> Value {
    json!({
        "sampleId": "smp_0001",
        "playMode": "oneshot",
        "triggerNote": 60,
        "slices": [
            { "start": { "frames": "0" } },
            { "start": { "beat": { "numerator": 1, "denominator": 2 } },
              "end": { "frames": "96000" }, "level": 0.8, "pan": -0.5, "rate": 2.0, "reverse": true }
        ]
    })
}

#[test]
fn state_requires_a_declared_schema() {
    let mut s = base_snapshot();
    s.channels[0].instrument.state = Some(json!({ "anything": true }));
    let e = err(&s, &base_registry());
    assert_eq!(e.code, codes::INVALID_PROJECT);
    assert!(e.message.contains("no state schema"), "{}", e.message);
    assert_eq!(e.path.as_deref(), Some("$.channels[0].instrument.state"));
}

#[test]
fn slicer_state_accepts_all_three_slice_sources() {
    let (s, registry) = slicer_snapshot(explicit_slices());
    assert!(validate(&s, &registry).is_ok());

    let (s, registry) = slicer_snapshot(json!({
        "sampleId": "smp_0001", "playMode": "gate", "slices": { "grid": 8 }
    }));
    assert!(validate(&s, &registry).is_ok());

    let (s, registry) = slicer_snapshot(json!({
        "sampleId": "smp_0001", "playMode": "oneshot",
        "slices": { "onset": { "algorithm": "oxitone.onset.v1", "sensitivity": 0.7 } }
    }));
    assert!(validate(&s, &registry).is_ok());
}

#[test]
fn slicer_slice_sources_are_mutually_exclusive() {
    let (s, registry) = slicer_snapshot(json!({
        "sampleId": "smp_0001", "playMode": "gate",
        "slices": { "grid": 8, "onset": { "algorithm": "oxitone.onset.v1" } }
    }));
    let e = err(&s, &registry);
    assert!(
        e.message.contains("exactly one of grid|onset"),
        "{}",
        e.message
    );

    let (s, registry) = slicer_snapshot(json!({
        "sampleId": "smp_0001", "playMode": "gate", "slices": {}
    }));
    assert_eq!(err(&s, &registry).code, codes::INVALID_PROJECT);

    let (s, registry) = slicer_snapshot(json!({
        "sampleId": "smp_0001", "playMode": "gate", "slices": { "grid": 0 }
    }));
    assert_eq!(err(&s, &registry).code, codes::INVALID_PROJECT);

    let (s, registry) = slicer_snapshot(json!({
        "sampleId": "smp_0001", "playMode": "gate",
        "slices": { "onset": { "sensitivity": 0.5 } }
    }));
    assert_eq!(err(&s, &registry).code, codes::INVALID_PROJECT);

    let (s, registry) = slicer_snapshot(json!({
        "sampleId": "smp_0001", "playMode": "gate",
        "slices": { "onset": { "algorithm": "oxitone.onset.v1", "sensitivity": 1.5 } }
    }));
    assert_eq!(err(&s, &registry).code, codes::INVALID_PROJECT);
}

#[test]
fn slicer_play_mode_and_trigger_note_ranges() {
    let (s, registry) = slicer_snapshot(json!({
        "sampleId": "smp_0001", "playMode": "hold", "slices": { "grid": 4 }
    }));
    assert!(err(&s, &registry).path.unwrap().ends_with(".playMode"));

    let (s, registry) = slicer_snapshot(json!({
        "sampleId": "smp_0001", "playMode": "gate", "triggerNote": 128, "slices": { "grid": 4 }
    }));
    assert!(err(&s, &registry).path.unwrap().ends_with(".triggerNote"));

    let (s, registry) = slicer_snapshot(json!({
        "sampleId": "smp_0001", "playMode": "gate", "triggerNote": 60.5, "slices": { "grid": 4 }
    }));
    assert_eq!(err(&s, &registry).code, codes::INVALID_PROJECT);
}

#[test]
fn slicer_sample_reference_must_exist() {
    let (s, registry) = slicer_snapshot(json!({
        "sampleId": "smp_missing", "playMode": "gate", "slices": { "grid": 4 }
    }));
    let e = err(&s, &registry);
    assert!(e.path.as_deref().unwrap().ends_with(".sampleId"));
}

#[test]
fn slicer_explicit_slice_shape() {
    // missing start
    let (s, registry) = slicer_snapshot(json!({
        "sampleId": "smp_0001", "playMode": "gate", "slices": [{ "level": 1.0 }]
    }));
    assert_eq!(err(&s, &registry).code, codes::INVALID_PROJECT);

    // frames and beat are mutually exclusive on a point
    let (s, registry) = slicer_snapshot(json!({
        "sampleId": "smp_0001", "playMode": "gate",
        "slices": [{ "start": { "frames": "0", "beat": { "numerator": 0, "denominator": 1 } } }]
    }));
    assert_eq!(err(&s, &registry).code, codes::INVALID_PROJECT);

    // empty slice list
    let (s, registry) = slicer_snapshot(json!({
        "sampleId": "smp_0001", "playMode": "gate", "slices": []
    }));
    assert_eq!(err(&s, &registry).code, codes::INVALID_PROJECT);

    // override ranges
    for (field, value) in [("level", 3.0), ("pan", 2.0), ("rate", 10.0)] {
        let mut slice = json!({ "start": { "frames": "0" } });
        slice[field] = json!(value);
        let (s, registry) = slicer_snapshot(json!({
            "sampleId": "smp_0001", "playMode": "gate",
            "slices": [slice]
        }));
        let e = err(&s, &registry);
        assert!(
            e.path.as_deref().unwrap().ends_with(field),
            "{}",
            e.path.unwrap()
        );
    }

    // start beyond the sample length
    let (s, registry) = slicer_snapshot(json!({
        "sampleId": "smp_0001", "playMode": "gate",
        "slices": [{ "start": { "frames": "999999" } }]
    }));
    assert_eq!(err(&s, &registry).code, codes::INVALID_PROJECT);

    // non-boolean reverse
    let (s, registry) = slicer_snapshot(json!({
        "sampleId": "smp_0001", "playMode": "gate",
        "slices": [{ "start": { "frames": "0" }, "reverse": "yes" }]
    }));
    assert_eq!(err(&s, &registry).code, codes::INVALID_PROJECT);
}
