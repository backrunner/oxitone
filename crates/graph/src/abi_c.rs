//! C ABI v1 boundary types and descriptor validation.
//!
//! This module deliberately contains only POD types and control-thread
//! descriptor conversion. Loading a dynamic library and constructing an
//! instance must happen outside the audio callback; the callback-facing
//! `ProcessContext` remains the Rust-owned contract.

use std::ffi::CStr;
use std::os::raw::c_char;

use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::wire::{ParameterRate, ParameterSmoothing, ParameterSpec, ParameterUnit};

use crate::descriptor::{ChannelLayout, PluginCapabilities, PluginDescriptor, PluginKind};

pub const ABI_MAJOR: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct OxiParamSpecV1 {
    pub id: *const c_char,
    pub label: *const c_char,
    pub unit: u32,
    pub min: f64,
    pub max: f64,
    pub default_value: f64,
    pub smoothing: u32,
    pub rate: u32,
    pub automation: u8,
    pub mapping: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct OxiPluginDescriptorV1 {
    pub abi_major: u32,
    pub abi_minor: u32,
    pub plugin_id: *const c_char,
    pub plugin_version: *const c_char,
    pub kind: u32,
    pub input_layout: u32,
    pub output_layout: u32,
    pub capabilities: u32,
    pub params: *const OxiParamSpecV1,
    pub param_count: u32,
    pub max_polyphony: u32,
}

fn cstr(ptr: *const c_char, field: &str) -> Result<&'static str, OxitoneError> {
    if ptr.is_null() {
        return Err(OxitoneError::new(
            codes::PLUGIN_MANIFEST_MISMATCH,
            format!("{field} is null"),
        ));
    }
    // SAFETY: plugin descriptors must point to static NUL-terminated data.
    let value = unsafe { CStr::from_ptr(ptr) }.to_str().map_err(|_| {
        OxitoneError::new(
            codes::PLUGIN_MANIFEST_MISMATCH,
            format!("{field} is not UTF-8"),
        )
    })?;
    Ok(Box::leak(value.to_owned().into_boxed_str()))
}

fn layout(value: u32) -> Result<ChannelLayout, OxitoneError> {
    match value {
        0 => Ok(ChannelLayout::None),
        1 => Ok(ChannelLayout::Mono),
        2 => Ok(ChannelLayout::Stereo),
        _ => Err(OxitoneError::new(
            codes::PLUGIN_MANIFEST_MISMATCH,
            "invalid channel layout",
        )),
    }
}

/// Convert and validate a C descriptor on the control thread.
pub fn descriptor_from_c(raw: &OxiPluginDescriptorV1) -> Result<PluginDescriptor, OxitoneError> {
    if raw.abi_major != ABI_MAJOR {
        return Err(OxitoneError::new(
            codes::PLUGIN_ABI_MISMATCH,
            format!(
                "plugin ABI major {} is incompatible with host {ABI_MAJOR}",
                raw.abi_major
            ),
        ));
    }
    let kind = match raw.kind {
        0 => PluginKind::Instrument,
        1 => PluginKind::Effect,
        _ => {
            return Err(OxitoneError::new(
                codes::PLUGIN_MANIFEST_MISMATCH,
                "invalid plugin kind",
            ))
        }
    };
    let input_layout = layout(raw.input_layout)?;
    let output_layout = layout(raw.output_layout)?;
    if raw.param_count > 4096 || (raw.param_count != 0 && raw.params.is_null()) {
        return Err(OxitoneError::new(
            codes::PLUGIN_MANIFEST_MISMATCH,
            "invalid parameter table",
        ));
    }
    let mut parameters = Vec::with_capacity(raw.param_count as usize);
    for i in 0..raw.param_count as usize {
        // SAFETY: the table is owned by the plugin and param_count bounds it.
        let p = unsafe { *raw.params.add(i) };
        parameters.push(ParameterSpec {
            id: cstr(p.id, "parameter id")?.to_owned(),
            label: cstr(p.label, "parameter label")?.to_owned(),
            unit: match p.unit {
                0 => ParameterUnit::Normalized,
                1 => ParameterUnit::Db,
                2 => ParameterUnit::Hz,
                3 => ParameterUnit::Semitones,
                4 => ParameterUnit::Seconds,
                5 => ParameterUnit::Beats,
                6 => ParameterUnit::Enum,
                _ => {
                    return Err(OxitoneError::new(
                        codes::PLUGIN_MANIFEST_MISMATCH,
                        "invalid parameter unit",
                    ))
                }
            },
            min: p.min,
            max: p.max,
            default: p.default_value,
            smoothing: match p.smoothing {
                0 => ParameterSmoothing::None,
                1 => ParameterSmoothing::Linear,
                2 => ParameterSmoothing::OnePole,
                _ => {
                    return Err(OxitoneError::new(
                        codes::PLUGIN_MANIFEST_MISMATCH,
                        "invalid parameter smoothing",
                    ))
                }
            },
            rate: match p.rate {
                0 => ParameterRate::Control,
                1 => ParameterRate::Audio,
                _ => {
                    return Err(OxitoneError::new(
                        codes::PLUGIN_MANIFEST_MISMATCH,
                        "invalid parameter rate",
                    ))
                }
            },
            automation: (p.automation != 0).then_some(true),
            mapping: match p.mapping {
                0 => None,
                1 => Some(oxitone_core::wire::ParameterMapping::Linear),
                2 => Some(oxitone_core::wire::ParameterMapping::Log),
                3 => Some(oxitone_core::wire::ParameterMapping::Bipolar),
                4 => Some(oxitone_core::wire::ParameterMapping::Enum),
                _ => {
                    return Err(OxitoneError::new(
                        codes::PLUGIN_MANIFEST_MISMATCH,
                        "invalid parameter mapping",
                    ))
                }
            },
        });
    }
    let descriptor = PluginDescriptor {
        plugin_id: cstr(raw.plugin_id, "plugin id")?,
        plugin_version: cstr(raw.plugin_version, "plugin version")?,
        kind,
        input_layout,
        output_layout,
        parameters,
        capabilities: PluginCapabilities {
            sidechain_input: raw.capabilities & 1 != 0,
            reports_tail: raw.capabilities & 2 != 0,
        },
        state_schema: None,
        max_polyphony: (kind == PluginKind::Instrument && raw.max_polyphony != 0)
            .then_some(raw.max_polyphony),
    };
    descriptor.validate()?;
    Ok(descriptor)
}
