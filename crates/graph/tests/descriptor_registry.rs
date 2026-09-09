mod common;

use common::*;
use oxitone_core::codes;
use oxitone_core::wire::{ParameterMapping, ParameterSmoothing, ParameterSpec, ParameterUnit};
use oxitone_graph::descriptor::validate_parameter_specs;
use oxitone_graph::{ChannelLayout, PluginDescriptor, PluginRegistry};

fn expect_manifest_err(descriptor: &PluginDescriptor) {
    let err = descriptor.validate().unwrap_err();
    assert_eq!(err.code, codes::PLUGIN_MANIFEST_MISMATCH, "{err}");
}

#[test]
fn valid_descriptor_passes_and_registers() {
    let descriptor = synth_descriptor();
    assert!(descriptor.validate().is_ok());
    let mut registry = PluginRegistry::new();
    assert!(registry.register(mock_plugin(descriptor)).is_ok());
    assert_eq!(registry.len(), 1);
    assert!(registry.contains_id(SYNTH_ID));
    assert!(registry.lookup(SYNTH_ID, PLUGIN_VERSION).is_some());
    assert!(registry
        .lookup_descriptor(SYNTH_ID, PLUGIN_VERSION)
        .is_some());
    assert!(registry.lookup(SYNTH_ID, "9.9.9").is_none());
}

#[test]
fn reregistration_is_idempotent_and_keeps_first() {
    let mut registry = PluginRegistry::new();
    let first = mock_plugin(synth_descriptor());
    registry.register(first.clone()).unwrap();
    let second = mock_plugin(synth_descriptor());
    registry.register(second).unwrap();
    assert_eq!(registry.len(), 1);
    assert!(std::sync::Arc::ptr_eq(
        &registry.lookup(SYNTH_ID, PLUGIN_VERSION).unwrap(),
        &first
    ));
}

#[test]
fn invalid_descriptor_is_rejected_at_registration() {
    let mut descriptor = synth_descriptor();
    descriptor.output_layout = ChannelLayout::None;
    let mut registry = PluginRegistry::new();
    let err = registry.register(mock_plugin(descriptor)).unwrap_err();
    assert_eq!(err.code, codes::PLUGIN_MANIFEST_MISMATCH);
    assert!(registry.is_empty());
}

#[test]
fn duplicate_parameter_ids_are_rejected() {
    let mut descriptor = synth_descriptor();
    descriptor.parameters.push(param("gainDb", 0.0, 1.0, 0.5));
    expect_manifest_err(&descriptor);
}

#[test]
fn range_and_default_rules() {
    let mut bad = synth_descriptor();
    bad.parameters = vec![param("x", 2.0, 1.0, 1.5)];
    expect_manifest_err(&bad);

    let mut bad = synth_descriptor();
    bad.parameters = vec![param("x", 0.0, 1.0, 1.5)];
    expect_manifest_err(&bad);

    let mut bad = synth_descriptor();
    bad.parameters = vec![param("x", f64::NAN, 1.0, 0.5)];
    expect_manifest_err(&bad);
}

#[test]
fn unit_and_mapping_combinations() {
    // enum unit requires enum mapping (and vice versa), integer steps, no smoothing.
    let mut enum_spec = param("mode", 0.0, 2.0, 0.0);
    enum_spec.unit = ParameterUnit::Enum;
    expect_manifest_err(&PluginDescriptor {
        parameters: vec![enum_spec.clone()],
        ..synth_descriptor()
    });
    enum_spec.mapping = Some(ParameterMapping::Enum);
    enum_spec.smoothing = ParameterSmoothing::None;
    assert!(validate_parameter_specs(&[enum_spec.clone()]).is_ok());
    let mut fractional = enum_spec.clone();
    fractional.max = 1.5;
    expect_manifest_err(&PluginDescriptor {
        parameters: vec![fractional],
        ..synth_descriptor()
    });
    let mut smoothed = enum_spec;
    smoothed.smoothing = ParameterSmoothing::OnePole;
    expect_manifest_err(&PluginDescriptor {
        parameters: vec![smoothed],
        ..synth_descriptor()
    });

    // log mapping requires a positive domain.
    let mut log_spec = param("rate", 0.0, 4.0, 1.0);
    log_spec.mapping = Some(ParameterMapping::Log);
    expect_manifest_err(&PluginDescriptor {
        parameters: vec![log_spec.clone()],
        ..synth_descriptor()
    });
    log_spec.min = 0.25;
    assert!(validate_parameter_specs(&[log_spec]).is_ok());

    // bipolar mapping requires a range straddling zero.
    let mut bipolar_spec = param("pan", 0.0, 1.0, 0.5);
    bipolar_spec.mapping = Some(ParameterMapping::Bipolar);
    expect_manifest_err(&PluginDescriptor {
        parameters: vec![bipolar_spec.clone()],
        ..synth_descriptor()
    });
    bipolar_spec.min = -1.0;
    assert!(validate_parameter_specs(&[bipolar_spec]).is_ok());
}

#[test]
fn layout_and_polyphony_rules() {
    let mut bad = synth_descriptor();
    bad.input_layout = ChannelLayout::Mono;
    expect_manifest_err(&bad);

    let mut bad = effect_descriptor(EFFECT_ID, PLUGIN_VERSION, vec![]);
    bad.input_layout = ChannelLayout::None;
    expect_manifest_err(&bad);

    let mut bad = effect_descriptor(EFFECT_ID, PLUGIN_VERSION, vec![]);
    bad.max_polyphony = Some(8);
    expect_manifest_err(&bad);

    let mut bad = synth_descriptor();
    bad.plugin_version = "".into();
    expect_manifest_err(&bad);
}

#[test]
fn parameter_spec_struct_fields_match_wire() {
    let spec: ParameterSpec = serde_json::from_str(
        r#"{"id":"cutoff","label":"Cutoff","unit":"hz","min":20,"max":20000,
            "default":1000,"smoothing":"one-pole","rate":"audio","automation":true,"mapping":"log"}"#,
    )
    .unwrap();
    assert!(validate_parameter_specs(&[spec]).is_ok());
}
