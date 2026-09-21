use super::*;

fn metadata(category: &str, inputs: usize, outputs: usize) -> Metadata {
    serde_json::from_value(serde_json::json!({
        "protocolVersion":1,"classId":"1".repeat(32),"sha256":"a".repeat(64),
        "name":"Fixture","vendor":"Fixture","version":"1","category":category,
        "inputChannels":if inputs == 0 { 0 } else { 2 }, "outputChannels":2,"noteInput":true,"noteOutput":false,
        "audioBuses":{"inputs":vec![serde_json::json!({"channels":2,"active":true}); inputs],
            "outputs":vec![serde_json::json!({"channels":2,"active":true}); outputs]},
        "parameters":[],"configuration":null
    }))
    .unwrap()
}

#[test]
fn instruments_with_audio_inputs_keep_their_kind_and_effects_expose_the_detector() {
    let instrument = metadata(" Instrument |Synth", 1, 1).descriptor().unwrap();
    assert_eq!(instrument.kind, PluginKind::Instrument);
    assert_eq!(instrument.input_layout, ChannelLayout::None);
    let effect = metadata("Fx|Dynamics", 2, 1).descriptor().unwrap();
    assert_eq!(effect.kind, PluginKind::Effect);
    assert!(effect.capabilities.sidechain_input);
    assert!(
        !metadata("Fx", 1, 1)
            .descriptor()
            .unwrap()
            .capabilities
            .sidechain_input
    );
    assert!(metadata("Fx", 3, 1).descriptor().is_ok());
    assert!(metadata("Fx", 1, 2).descriptor().is_ok());
    assert_eq!(
        metadata("Fx", 0, 1).descriptor().unwrap_err().code,
        "PluginCapabilityUnsupported"
    );
}

#[test]
fn midi_only_classes_are_channel_generators_even_when_the_vendor_labels_them_fx() {
    let mut meta = metadata("Fx|Tools", 0, 0);
    meta.output_channels = 0;
    meta.note_output = true;
    meta.note_input = false;
    let source = Source {
        bundle_path: "/tmp/Midi.vst3".into(),
        class_id: meta.class_id.clone(),
        expected_hash: None,
        allow_plugins: oxitone_vst3_host::wire::Policy::Any,
    };
    meta.validate(&source).unwrap();
    assert_eq!(meta.descriptor().unwrap().kind, PluginKind::Instrument);
    meta.note_input = true;
    meta.validate(&source).unwrap();
    assert_eq!(meta.descriptor().unwrap().kind, PluginKind::Instrument);
    meta.note_output = false;
    assert!(meta.validate(&source).is_err());
}

#[test]
fn fresh_handshake_must_match_physical_bus_shape_and_category() {
    let meta = metadata("Fx", 2, 1);
    let mut ready = Ready {
        stream_protocol_version: oxitone_vst3_host::stream_wire::STREAM_VERSION,
        sample_rate: 48000,
        block_size: 128,
        class_id: meta.class_id.clone(),
        sha256: meta.sha256.clone(),
        input_channels: 2,
        output_channels: 2,
        audio_buses: meta.audio_buses.clone(),
        category: "Fx".into(),
        note_input: true,
        note_output: false,
        latency_frames: 0,
        tail_frames: 0,
        helper_time_constraint: false,
        parameters: vec![],
    };
    meta.check_ready(&ready).unwrap();
    ready.category = "Instrument".into();
    assert!(meta.check_ready(&ready).is_err());
    ready.category = "Fx".into();
    ready.audio_buses.inputs[1].channels = 1;
    assert!(meta.check_ready(&ready).is_err());
    ready.audio_buses.inputs[1].channels = 2;
    ready.audio_buses.inputs.pop();
    assert!(meta.check_ready(&ready).is_err());
}
