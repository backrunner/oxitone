//! Shared `ParameterSpec` constructors and compile-time initial-parameter
//! validation for the built-in instruments (04-api-contracts.md
//! §ParameterSpec). Unknown, non-finite, or out-of-range initial values are
//! rejected with `InvalidProject`; runtime parameter events are clamped to
//! the declared range (the host has already validated them).

use std::collections::BTreeMap;

use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::wire::{
    ParameterMapping, ParameterRate, ParameterSmoothing, ParameterSpec, ParameterUnit,
};

/// Fully specified control-rate parameter with automation enabled.
#[allow(clippy::too_many_arguments)]
pub fn spec(
    id: &str,
    label: &str,
    unit: ParameterUnit,
    min: f64,
    max: f64,
    default: f64,
    smoothing: ParameterSmoothing,
    mapping: ParameterMapping,
) -> ParameterSpec {
    ParameterSpec {
        id: id.to_string(),
        label: label.to_string(),
        unit,
        min,
        max,
        default,
        smoothing,
        rate: ParameterRate::Control,
        automation: Some(true),
        mapping: Some(mapping),
    }
}

/// Normalized linear parameter with per-sample one-pole smoothing.
pub fn smoothed(id: &str, label: &str, min: f64, max: f64, default: f64) -> ParameterSpec {
    spec(
        id,
        label,
        ParameterUnit::Normalized,
        min,
        max,
        default,
        ParameterSmoothing::OnePole,
        ParameterMapping::Linear,
    )
}

/// Normalized linear parameter updated at control rate without smoothing.
pub fn stepped_continuous(
    id: &str,
    label: &str,
    min: f64,
    max: f64,
    default: f64,
) -> ParameterSpec {
    spec(
        id,
        label,
        ParameterUnit::Normalized,
        min,
        max,
        default,
        ParameterSmoothing::None,
        ParameterMapping::Linear,
    )
}

/// Bipolar -1..1 parameter with per-sample one-pole smoothing.
pub fn bipolar(id: &str, label: &str, default: f64) -> ParameterSpec {
    spec(
        id,
        label,
        ParameterUnit::Normalized,
        -1.0,
        1.0,
        default,
        ParameterSmoothing::OnePole,
        ParameterMapping::Bipolar,
    )
}

/// Seconds parameter (0..8 s, control rate).
pub fn seconds(id: &str, label: &str, default: f64) -> ParameterSpec {
    spec(
        id,
        label,
        ParameterUnit::Seconds,
        0.0,
        8.0,
        default,
        ParameterSmoothing::None,
        ParameterMapping::Linear,
    )
}

/// Integer-valued enum parameter (no smoothing, `enum` unit + mapping).
pub fn enum_spec(id: &str, label: &str, min: f64, max: f64, default: f64) -> ParameterSpec {
    spec(
        id,
        label,
        ParameterUnit::Enum,
        min,
        max,
        default,
        ParameterSmoothing::None,
        ParameterMapping::Enum,
    )
}

/// Validate `given` against `specs` and return the dense value table in spec
/// order (defaults overridden by `given`). Compile-time only.
pub fn initial_values(
    specs: &[ParameterSpec],
    given: &BTreeMap<String, f64>,
) -> Result<Vec<f64>, OxitoneError> {
    let mut values: Vec<f64> = specs.iter().map(|s| s.default).collect();
    for (id, value) in given {
        let Some(index) = specs.iter().position(|s| &s.id == id) else {
            return Err(OxitoneError::with_path(
                codes::INVALID_PROJECT,
                format!("unknown parameter {id:?}"),
                format!("$.parameters.{id}"),
            ));
        };
        let spec = &specs[index];
        if !value.is_finite() || *value < spec.min || *value > spec.max {
            return Err(OxitoneError::with_path(
                codes::INVALID_PROJECT,
                format!(
                    "parameter {id:?} value {value} is outside {}..={}",
                    spec.min, spec.max
                ),
                format!("$.parameters.{id}"),
            ));
        }
        if spec.unit == ParameterUnit::Enum && value.fract() != 0.0 {
            return Err(OxitoneError::with_path(
                codes::INVALID_PROJECT,
                format!("enum parameter {id:?} requires an integer value, got {value}"),
                format!("$.parameters.{id}"),
            ));
        }
        values[index] = *value;
    }
    Ok(values)
}

/// Index of `id` in `specs`, for runtime parameter-event dispatch.
pub fn index_of(specs: &[ParameterSpec], id: &str) -> Option<usize> {
    specs.iter().position(|s| s.id == id)
}

/// Sanitize a runtime parameter-event value for `spec`: clamp to range and
/// round enum values. Unknown IDs are dropped by the caller (the host has
/// already validated events against the descriptor).
pub fn sanitize_event_value(spec: &ParameterSpec, value: f64) -> f64 {
    let clamped = value.clamp(spec.min, spec.max);
    if spec.unit == ParameterUnit::Enum {
        clamped.round()
    } else {
        clamped
    }
}
