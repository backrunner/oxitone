//! `InstrumentRef.state` validation (04-api-contracts.md). Only plugins whose
//! descriptor declares a state schema may carry state; Phase 1 shapes the
//! Slicer slice table (slices 三来源互斥、playMode、triggerNote 范围). Other
//! declared schemas pass through opaquely — the plugin owns their shape.

use std::collections::HashSet;

use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::wire::ProjectSnapshot;
use oxitone_core::Beat;
use serde_json::Value;

use crate::descriptor::SLICER_STATE_SCHEMA_ID;
use crate::registry::PluginRegistry;

fn invalid(path: &str, message: impl Into<String>) -> OxitoneError {
    OxitoneError::with_path(codes::INVALID_PROJECT, message, path)
}

fn as_u64(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|s| s.parse::<u64>().ok()))
}

fn check_slice_point(path: &str, value: &Value) -> Result<(), OxitoneError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid(path, "slice point must be an object"))?;
    let has_frames = object.contains_key("frames");
    let has_beat = object.contains_key("beat");
    if has_frames == has_beat {
        return Err(invalid(
            path,
            "slice point needs exactly one of frames|beat",
        ));
    }
    if has_frames {
        as_u64(&object["frames"])
            .ok_or_else(|| invalid(path, "frames must be a non-negative integer"))?;
    } else {
        let beat = &object["beat"];
        let num = beat.get("numerator").and_then(as_u64);
        let den = beat.get("denominator").and_then(as_u64);
        match (num, den) {
            (Some(num), Some(den))
                if num <= i64::MAX as u64 && (1..=u32::MAX as u64).contains(&den) =>
            {
                Beat::new(num as i64, den as u32)
                    .map_err(|_| invalid(path, "invalid beat point"))?
            }
            _ => {
                return Err(invalid(
                    path,
                    "beat point needs numerator/denominator in range",
                ))
            }
        };
    }
    Ok(())
}

fn check_number_field(
    slice: &serde_json::Map<String, Value>,
    path: &str,
    field: &str,
    min: f64,
    max: f64,
) -> Result<(), OxitoneError> {
    if let Some(value) = slice.get(field) {
        let number = value
            .as_f64()
            .ok_or_else(|| invalid(path, format!("{field} must be a number")))?;
        if !number.is_finite() || number < min || number > max {
            return Err(invalid(
                path,
                format!("{field} must be within {min}..={max}"),
            ));
        }
    }
    Ok(())
}

fn check_explicit_slices(
    path: &str,
    slices: &[Value],
    sample_frames: Option<u64>,
) -> Result<(), OxitoneError> {
    if slices.is_empty() {
        return Err(invalid(path, "slices must not be empty"));
    }
    for (i, slice) in slices.iter().enumerate() {
        let slice_path = format!("{path}[{i}]");
        let object = slice
            .as_object()
            .ok_or_else(|| invalid(&slice_path, "slice must be an object"))?;
        let start = object
            .get("start")
            .ok_or_else(|| invalid(&slice_path, "slice needs a start"))?;
        check_slice_point(&format!("{slice_path}.start"), start)?;
        if let Some(end) = object.get("end") {
            check_slice_point(&format!("{slice_path}.end"), end)?;
        }
        if let (Some(frames), Some(start_frames)) = (
            sample_frames,
            object
                .get("start")
                .and_then(|s| s.get("frames"))
                .and_then(as_u64),
        ) {
            if start_frames >= frames {
                return Err(invalid(
                    &format!("{slice_path}.start"),
                    "slice start is beyond the sample length",
                ));
            }
        }
        check_number_field(object, &format!("{slice_path}.level"), "level", 0.0, 2.0)?;
        check_number_field(object, &format!("{slice_path}.pan"), "pan", -1.0, 1.0)?;
        check_number_field(object, &format!("{slice_path}.rate"), "rate", 0.25, 4.0)?;
        if let Some(reverse) = object.get("reverse") {
            if !reverse.is_boolean() {
                return Err(invalid(
                    &format!("{slice_path}.reverse"),
                    "reverse must be a boolean",
                ));
            }
        }
    }
    Ok(())
}

/// Slicer slice table (04-api-contracts.md §InstrumentRef.state).
fn check_slicer_state(
    path: &str,
    state: &Value,
    sample_ids: &HashSet<&str>,
    sample_frames: impl Fn(&str) -> Option<u64>,
) -> Result<(), OxitoneError> {
    let object = state
        .as_object()
        .ok_or_else(|| invalid(path, "slicer state must be an object"))?;
    let sample_id = object
        .get("sampleId")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(path, "slicer state needs a sampleId string"))?;
    if !sample_ids.contains(sample_id) {
        return Err(invalid(
            &format!("{path}.sampleId"),
            format!("reference to unknown sample {sample_id:?}"),
        ));
    }
    match object.get("playMode").and_then(Value::as_str) {
        Some("oneshot" | "gate") => {}
        _ => {
            return Err(invalid(
                &format!("{path}.playMode"),
                "playMode must be oneshot|gate",
            ))
        }
    }
    if let Some(trigger) = object.get("triggerNote") {
        let note = trigger.as_u64().ok_or_else(|| {
            invalid(
                &format!("{path}.triggerNote"),
                "triggerNote must be an integer",
            )
        })?;
        if note > 127 {
            return Err(invalid(
                &format!("{path}.triggerNote"),
                "triggerNote must be within 0..=127",
            ));
        }
    }
    let slices_path = format!("{path}.slices");
    let slices = object
        .get("slices")
        .ok_or_else(|| invalid(&slices_path, "slicer state needs slices"))?;
    if let Some(list) = slices.as_array() {
        return check_explicit_slices(&slices_path, list, sample_frames(sample_id));
    }
    let source = slices
        .as_object()
        .ok_or_else(|| invalid(&slices_path, "slices must be a list, grid, or onset spec"))?;
    match (source.get("grid"), source.get("onset")) {
        (Some(grid), None) => {
            let n = grid
                .as_u64()
                .ok_or_else(|| invalid(&slices_path, "grid must be a positive integer"))?;
            if n == 0 {
                return Err(invalid(&slices_path, "grid must be a positive integer"));
            }
        }
        (None, Some(onset)) => {
            let onset = onset
                .as_object()
                .ok_or_else(|| invalid(&slices_path, "onset must be an object"))?;
            match onset.get("algorithm").and_then(Value::as_str) {
                Some(algorithm) if !algorithm.is_empty() => {}
                _ => {
                    return Err(invalid(
                        &slices_path,
                        "onset needs a versioned algorithm id",
                    ))
                }
            }
            if let Some(sensitivity) = onset.get("sensitivity") {
                let value = sensitivity
                    .as_f64()
                    .ok_or_else(|| invalid(&slices_path, "sensitivity must be a number"))?;
                if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                    return Err(invalid(&slices_path, "sensitivity must be within 0..=1"));
                }
            }
        }
        _ => {
            return Err(invalid(
                &slices_path,
                "slices object needs exactly one of grid|onset",
            ))
        }
    }
    Ok(())
}

pub(super) fn validate_states(
    snapshot: &ProjectSnapshot,
    registry: &PluginRegistry,
) -> Result<(), OxitoneError> {
    let sample_ids: HashSet<&str> = snapshot.samples.iter().map(|s| s.id.as_str()).collect();
    let sample_frames = |id: &str| {
        snapshot
            .samples
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.frames)
    };
    for (i, channel) in snapshot.channels.iter().enumerate() {
        let Some(state) = &channel.instrument.state else {
            continue;
        };
        let path = format!("$.channels[{i}].instrument.state");
        let descriptor = registry
            .lookup_descriptor(
                &channel.instrument.plugin_id,
                &channel.instrument.plugin_version,
            )
            .expect("instrument references are validated before state");
        let Some(schema) = descriptor.state_schema else {
            return Err(invalid(
                &path,
                format!(
                    "plugin {:?} declares no state schema and cannot carry state",
                    channel.instrument.plugin_id
                ),
            ));
        };
        if schema == SLICER_STATE_SCHEMA_ID {
            check_slicer_state(&path, state, &sample_ids, sample_frames)?;
        }
    }
    Ok(())
}
