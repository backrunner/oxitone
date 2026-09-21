use super::*;
use serde_json::json;
fn start() -> Start {
    serde_json::from_value(json!({"streamProtocolVersion":11,
        "source":{"bundlePath":"/local/Gain.vst3","classId":"1".repeat(32),"allowPlugins":"signed-only"},
        "options":{"sampleRate":48000,"blockSize":128,"parameters":{},"tempo":120,"timeSignature":[4,4]}})).unwrap()
}
#[test]
fn stream_settings_reject_unknown_versions_fields_and_invalid_configuration_without_native_code() {
    let value = serde_json::to_value(start()).unwrap();
    let mut unknown = value.clone();
    unknown["pcm"] = json!([0, 0]);
    assert!(serde_json::from_value::<Start>(unknown).is_err());
    let mut options = value.clone();
    options["options"]["frames"] = json!(128);
    assert!(serde_json::from_value::<Start>(options).is_err());
    for (key, bad) in [
        ("sampleRate", json!(384000)),
        ("blockSize", json!(0)),
        ("tempo", json!(0)),
        ("timeSignature", json!([4, 3])),
        ("parameters", json!({"01":0.5})),
    ] {
        let mut candidate = value.clone();
        candidate["options"][key] = bad;
        assert!(serde_json::from_value::<Start>(candidate)
            .unwrap()
            .validate()
            .is_err());
    }
    for version in [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 12] {
        let mut unsupported = start();
        unsupported.stream_protocol_version = version;
        assert_eq!(
            unsupported.validate().unwrap_err().code,
            "ProtocolVersionUnsupported"
        );
    }
    let mut future = value;
    future["options"]["configuration"] = json!({"formatVersion":2,"classId":"1".repeat(32),"sha256":"a".repeat(64),"stateBase64":"","parameters":{}});
    assert_eq!(
        serde_json::from_value::<Start>(future)
            .unwrap()
            .validate()
            .unwrap_err()
            .code,
        "ProtocolVersionUnsupported"
    );
}
#[test]
fn readiness_checks_identity_duplicates_capabilities_and_event_offsets() {
    let mut ready = Ready {
        stream_protocol_version: STREAM_VERSION,
        sample_rate: 48000,
        block_size: 128,
        class_id: "1".repeat(32),
        sha256: "a".repeat(64),
        input_channels: 2,
        output_channels: 2,
        audio_buses: crate::bus_wire::AudioBuses::stereo(2, 2),
        category: "Fx".into(),
        note_input: false,
        note_output: false,
        latency_frames: 0,
        tail_frames: 0,
        helper_time_constraint: false,
        parameters: vec![
            Parameter {
                id: 9,
                writable: true,
                automatable: true,
            },
            Parameter {
                id: 2,
                writable: false,
                automatable: false,
            },
        ],
    };
    ready.validate(&start()).unwrap();
    let mut activation = start();
    activation.options.bus_activation = Some(crate::bus_wire::BusActivation {
        inputs: vec![true, true],
        outputs: vec![true],
    });
    assert!(ready.validate(&activation).is_err());
    activation.options.bus_activation.as_mut().unwrap().inputs = vec![false];
    assert!(ready.validate(&activation).is_err());
    activation.options.bus_activation.as_mut().unwrap().inputs = vec![true];
    ready.validate(&activation).unwrap();
    let parameter = Event::Parameter {
        frame: 127,
        parameter_id: 9,
        value: 0.5,
    };
    assert!(ready.valid_event(&parameter, 128));
    assert!(!ready.valid_event(&parameter, 127));
    assert!(!ready.valid_event(
        &Event::NoteOn {
            frame: 0,
            channel: 0,
            pitch: 60,
            velocity: 1.
        },
        128
    ));
    assert!(!ready.valid_event(
        &Event::Parameter {
            frame: 0,
            parameter_id: 2,
            value: 0.5
        },
        128
    ));
    let mut pinned = start();
    pinned.source.expected_hash = Some("b".repeat(64));
    assert!(ready.validate(&pinned).is_err());
    ready.parameters.push(ready.parameters[0].clone());
    assert!(ready.validate(&start()).is_err());
    ready.parameters.dedup_by_key(|parameter| parameter.id);
    ready.audio_buses.inputs.clear();
    ready.audio_buses.outputs.clear();
    ready.input_channels = 0;
    ready.output_channels = 0;
    assert!(ready.validate(&start()).is_err());
    ready.note_output = true;
    ready.validate(&start()).unwrap();
    let mut midi = start();
    midi.options.midi_output = true;
    midi.options.bus_activation = Some(crate::bus_wire::BusActivation {
        inputs: vec![],
        outputs: vec![],
    });
    midi.validate().unwrap();
    ready.validate(&midi).unwrap();
}
