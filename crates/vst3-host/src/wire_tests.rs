use super::*;
use base64::engine::general_purpose::STANDARD;
use serde_json::{json, Value};

fn request() -> Value {
    json!({"protocolVersion":1,"operation":"render",
        "source":{"bundlePath":"/tmp/Fixture.vst3","classId":"11111111111111111111111111111111","allowPlugins":"any"},
        "options":{"path":"/tmp/out.wav","sampleRate":48000,"blockSize":128,"frames":129,"tailFrames":7,
        "parameters":{},"events":[],"tempo":120,"timeSignature":[4,4]}})
}
fn validate(value: Value) -> Result<()> {
    serde_json::from_value::<Request>(value)
        .map_err(invalid)?
        .validate()
}

#[test]
fn native_boundary_rejects_versions_unknown_fields_and_operation_mismatch() {
    assert!(validate(request()).is_ok());
    let mut value = request();
    value["protocolVersion"] = json!(2);
    assert_eq!(
        validate(value).unwrap_err().code,
        "ProtocolVersionUnsupported"
    );
    let mut value = request();
    value["extra"] = json!(1);
    assert!(validate(value).is_err());
    let mut value = request();
    value["operation"] = json!("inspect");
    assert!(validate(value).is_err());
    let mut value = request();
    value["source"]["bundlePath"] = json!("relative.vst3");
    assert!(validate(value).is_err());
}

#[test]
fn native_frame_budget_and_midi_bounds_do_not_depend_on_typescript() {
    let mut value = request();
    value["options"]["frames"] = json!(u64::MAX);
    assert_eq!(validate(value).unwrap_err().code, "WavTooLarge");
    for event in [
        json!({"type":"noteOn","frame":129,"channel":0,"pitch":60,"velocity":1}),
        json!({"type":"noteOn","frame":0,"channel":16,"pitch":60,"velocity":1}),
        json!({"type":"noteOn","frame":0,"channel":0,"pitch":128,"velocity":1}),
        json!({"type":"parameter","frame":0,"parameterId":0,"value":1.1}),
    ] {
        let mut value = request();
        value["options"]["events"] = json!([event]);
        assert!(validate(value).is_err());
    }
}

#[test]
fn configuration_versions_and_noncanonical_parameter_ids_fail_closed() {
    for id in ["01", "+1", "4294967296", "x"] {
        let mut value = request();
        value["options"]["parameters"] = json!({ id: 0.5 });
        assert!(validate(value).is_err());
    }
    let mut value = request();
    value["options"]["configuration"] = json!({"formatVersion":2,"classId":"1".repeat(32),
        "sha256":"a".repeat(64),"stateBase64":"","parameters":{}});
    assert_eq!(
        validate(value).unwrap_err().code,
        "ProtocolVersionUnsupported"
    );
}

#[test]
fn native_configuration_rejects_malformed_identity_and_opaque_state_before_loading() {
    let mut value = request();
    value["options"]["configuration"] = json!({
        "formatVersion": 1,
        "classId": "1".repeat(32),
        "sha256": "a".repeat(64),
        "stateBase64": STANDARD.encode([1u8, 2, 3]),
        "parameters": {}
    });
    assert!(validate(value.clone()).is_ok());
    for (field, invalid) in [
        ("classId", json!("z".repeat(32))),
        ("sha256", json!("z".repeat(64))),
        ("stateBase64", json!("not-base64")),
    ] {
        let mut candidate = value.clone();
        candidate["options"]["configuration"][field] = invalid;
        assert_eq!(validate(candidate).unwrap_err().code, "PluginConfigInvalid");
    }
    let mut oversized = value;
    oversized["options"]["configuration"]["stateBase64"] =
        json!(STANDARD.encode(vec![0u8; MAX_STATE + 1]));
    assert_eq!(validate(oversized).unwrap_err().code, "BudgetExceeded");
}

#[test]
fn equal_frame_events_keep_order_with_off_before_parameter_before_on() {
    let mut events: Vec<Event> = serde_json::from_value(json!([
        {"type":"noteOn","frame":128,"channel":0,"pitch":60,"velocity":1},
        {"type":"parameter","frame":128,"parameterId":9,"value":0.2},
        {"type":"noteOff","frame":128,"channel":0,"pitch":60,"velocity":0},
        {"type":"parameter","frame":128,"parameterId":9,"value":0.8},
        {"type":"noteOn","frame":0,"channel":0,"pitch":64,"velocity":1}
    ]))
    .unwrap();
    events.sort_by_key(|event| (event.frame(), event.priority()));
    assert_eq!(events[0].frame(), 0);
    assert_eq!(events[1].priority(), 0);
    assert!(matches!(events[2], Event::Parameter { value, .. } if value == 0.2));
    assert!(matches!(events[3], Event::Parameter { value, .. } if value == 0.8));
    assert_eq!(events[4].priority(), 2);
}
