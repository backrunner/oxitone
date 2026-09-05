//! Slicer structured-state parsing and slice-table resolution
//! (04-api-contracts.md §SlicerState). The state is validated and resolved
//! into an immutable frame-interval table at compile time; `process` only
//! reads the resolved table.

use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::Beat;
use serde_json::Value;

use super::onset::ONSET_ALGORITHM_V1;

/// Hard cap on slices (one preallocated voice per slice).
pub const MAX_SLICES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayMode {
    /// Note-off is ignored; the slice plays to its end.
    Oneshot,
    /// Note-off triggers the release stage.
    Gate,
}

/// One slice source marker: frame position or beat position.
#[derive(Debug, Clone, Copy)]
pub(super) enum Position {
    Frames(u64),
    Beat(Beat),
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Override {
    pub level: Option<f64>,
    pub pan: Option<f64>,
    pub rate: Option<f64>,
    pub reverse: Option<bool>,
}

#[derive(Debug, Clone)]
pub(super) struct ExplicitSlice {
    pub start: Position,
    pub end: Option<Position>,
    pub over: Override,
}

#[derive(Debug, Clone)]
pub(super) enum SliceSource {
    Explicit(Vec<ExplicitSlice>),
    Grid(u32),
    Onset { sensitivity: f64 },
}

/// Parsed (not yet sample-resolved) Slicer state.
#[derive(Debug, Clone)]
pub struct SlicerConfig {
    pub sample_id: String,
    pub(super) source: SliceSource,
    pub trigger_note: u8,
    pub play_mode: PlayMode,
}

/// Immutable resolved slice: frame interval plus per-slice overrides.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedSlice {
    pub start_frame: u64,
    pub end_frame: u64,
    pub level: f32,
    pub pan: f32,
    pub rate: f64,
    pub reverse: bool,
}

pub(super) fn err(path: impl Into<String>, message: impl Into<String>) -> OxitoneError {
    OxitoneError::with_path(codes::INVALID_PROJECT, message, path)
}

fn parse_position(value: &Value, path: &str) -> Result<Position, OxitoneError> {
    let object = value
        .as_object()
        .ok_or_else(|| err(path, "slice position must be { frames } or { beat }"))?;
    if let Some(frames) = object.get("frames") {
        let frames = match frames {
            Value::Number(n) => n
                .as_u64()
                .ok_or_else(|| err(path, "frames must be a non-negative integer"))?,
            Value::String(s) => s
                .parse::<u64>()
                .map_err(|_| err(path, "frames string must be a non-negative integer"))?,
            _ => return Err(err(path, "frames must be a number or decimal string")),
        };
        return Ok(Position::Frames(frames));
    }
    if let Some(beat) = object.get("beat") {
        let beat: Beat = serde_json::from_value(beat.clone())
            .map_err(|e| err(path, format!("invalid beat position: {e}")))?;
        return Ok(Position::Beat(beat));
    }
    Err(err(path, "slice position must be { frames } or { beat }"))
}

fn parse_override(value: &Value, path: &str) -> Result<Override, OxitoneError> {
    let number = |key: &str| -> Result<Option<f64>, OxitoneError> {
        match value.get(key) {
            None => Ok(None),
            Some(v) => v
                .as_f64()
                .map(Some)
                .ok_or_else(|| err(format!("{path}.{key}"), "must be a number")),
        }
    };
    let over = Override {
        level: number("level")?,
        pan: number("pan")?,
        rate: number("rate")?,
        reverse: match value.get("reverse") {
            None => None,
            Some(v) => Some(
                v.as_bool()
                    .ok_or_else(|| err(format!("{path}.reverse"), "must be a boolean"))?,
            ),
        },
    };
    if let Some(level) = over.level {
        if !(0.0..=2.0).contains(&level) {
            return Err(err(format!("{path}.level"), "level must be in 0..=2"));
        }
    }
    if let Some(pan) = over.pan {
        if !(-1.0..=1.0).contains(&pan) {
            return Err(err(format!("{path}.pan"), "pan must be in -1..=1"));
        }
    }
    if let Some(rate) = over.rate {
        if !(0.25..=4.0).contains(&rate) {
            return Err(err(format!("{path}.rate"), "rate must be in 0.25..=4"));
        }
    }
    Ok(over)
}

/// Parse and validate an `InstrumentRef.state` value for `oxitone.slicer`.
pub fn parse_state(state: &Value) -> Result<SlicerConfig, OxitoneError> {
    let path = "$.state";
    let object = state
        .as_object()
        .ok_or_else(|| err(path, "slicer state must be an object"))?;
    let sample_id = object
        .get("sampleId")
        .and_then(Value::as_str)
        .ok_or_else(|| err(format!("{path}.sampleId"), "sampleId is required"))?
        .to_string();
    let slices = object
        .get("slices")
        .ok_or_else(|| err(format!("{path}.slices"), "slices is required"))?;
    let source = if let Some(array) = slices.as_array() {
        let mut explicit = Vec::with_capacity(array.len());
        for (i, entry) in array.iter().enumerate() {
            let epath = format!("{path}.slices[{i}]");
            let start = parse_position(
                entry
                    .get("start")
                    .ok_or_else(|| err(&epath, "slice start is required"))?,
                &format!("{epath}.start"),
            )?;
            let end = entry
                .get("end")
                .map(|e| parse_position(e, &format!("{epath}.end")))
                .transpose()?;
            let over = parse_override(entry, &epath)?;
            explicit.push(ExplicitSlice { start, end, over });
        }
        if explicit.is_empty() {
            return Err(err(
                format!("{path}.slices"),
                "slice list must not be empty",
            ));
        }
        SliceSource::Explicit(explicit)
    } else if let Some(grid) = slices.get("grid") {
        let n = grid
            .as_u64()
            .ok_or_else(|| err(format!("{path}.slices.grid"), "grid must be an integer"))?;
        if n == 0 || n > MAX_SLICES as u64 {
            return Err(err(
                format!("{path}.slices.grid"),
                format!("grid must be in 1..={MAX_SLICES}"),
            ));
        }
        SliceSource::Grid(n as u32)
    } else if let Some(onset) = slices.get("onset") {
        let algorithm = onset
            .get("algorithm")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                err(
                    format!("{path}.slices.onset.algorithm"),
                    "algorithm is required",
                )
            })?;
        if algorithm != ONSET_ALGORITHM_V1 {
            return Err(err(
                format!("{path}.slices.onset.algorithm"),
                format!("unknown onset algorithm {algorithm:?}"),
            ));
        }
        let sensitivity = match onset.get("sensitivity") {
            None => 0.5,
            Some(v) => v.as_f64().ok_or_else(|| {
                err(
                    format!("{path}.slices.onset.sensitivity"),
                    "must be a number",
                )
            })?,
        };
        if !(0.0..=1.0).contains(&sensitivity) {
            return Err(err(
                format!("{path}.slices.onset.sensitivity"),
                "sensitivity must be in 0..=1",
            ));
        }
        SliceSource::Onset { sensitivity }
    } else {
        return Err(err(
            format!("{path}.slices"),
            "slices must be an array, { grid }, or { onset }",
        ));
    };
    let trigger_note = match object.get("triggerNote") {
        None => 60,
        Some(v) => {
            let n = v
                .as_u64()
                .ok_or_else(|| err(format!("{path}.triggerNote"), "must be an integer"))?;
            u8::try_from(n).map_err(|_| err(format!("{path}.triggerNote"), "must be in 0..=127"))?
        }
    };
    let play_mode = match object.get("playMode").and_then(Value::as_str) {
        None | Some("oneshot") => PlayMode::Oneshot,
        Some("gate") => PlayMode::Gate,
        Some(other) => {
            return Err(err(
                format!("{path}.playMode"),
                format!("unknown playMode {other:?}"),
            ))
        }
    };
    Ok(SlicerConfig {
        sample_id,
        source,
        trigger_note,
        play_mode,
    })
}
