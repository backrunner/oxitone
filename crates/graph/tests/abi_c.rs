use std::ffi::CString;

use oxitone_graph::abi_c::{descriptor_from_c, OxiParamSpecV1, OxiPluginDescriptorV1, ABI_MAJOR};

#[test]
fn valid_c_descriptor_converts_and_validates() {
    let id = CString::new("gain").unwrap().into_raw();
    let label = CString::new("Gain").unwrap().into_raw();
    let plugin_id = CString::new("example.effect").unwrap().into_raw();
    let version = CString::new("1.0.0").unwrap().into_raw();
    let param = OxiParamSpecV1 {
        id,
        label,
        unit: 0,
        min: 0.0,
        max: 1.0,
        default_value: 1.0,
        smoothing: 1,
        rate: 0,
        automation: 1,
        mapping: 0,
    };
    let raw = OxiPluginDescriptorV1 {
        abi_major: ABI_MAJOR,
        abi_minor: 0,
        plugin_id,
        plugin_version: version,
        kind: 1,
        input_layout: 2,
        output_layout: 2,
        capabilities: 0,
        params: &param,
        param_count: 1,
        max_polyphony: 0,
    };
    let descriptor = descriptor_from_c(&raw).unwrap();
    assert_eq!(descriptor.plugin_id, "example.effect");
    assert_eq!(descriptor.parameters[0].id, "gain");
}

#[test]
fn incompatible_major_is_rejected_before_pointer_access() {
    let raw = OxiPluginDescriptorV1 {
        abi_major: ABI_MAJOR + 1,
        abi_minor: 0,
        plugin_id: std::ptr::null(),
        plugin_version: std::ptr::null(),
        kind: 9,
        input_layout: 9,
        output_layout: 9,
        capabilities: 0,
        params: std::ptr::null(),
        param_count: 0,
        max_polyphony: 0,
    };
    let error = descriptor_from_c(&raw).unwrap_err();
    assert_eq!(error.code, oxitone_core::error::codes::PLUGIN_ABI_MISMATCH);
}
