//! Tempo lane baking (03-audio-runtime-spec.md §Tempo automation 烘焙).
//!
//! A tempo lane replaces the static tempo map: the lane output in `[0, 1]`
//! is log-mapped to `20..=999` BPM and baked into a piecewise-`linear`
//! [`TempoSegment`] table on a fixed rational 1/64-beat grid, with every
//! source discontinuity added as an extra segment boundary so jumps land on
//! exact sample frames. The table feeds [`crate::tempo::TempoMap::compile`]
//! directly and is the single source of tempo truth while playing.

use oxitone_core::beat::Beat;
use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::wire::{AutomationLaneSpec, AutomationSourceSpec, TempoCurve, TempoSegment};

use super::{CompiledAutomation, EvalContext};

/// Fixed bake grid: 1/64 beat (exact in binary floating point).
pub const TEMPO_BAKE_GRID_BEAT: f64 = 1.0 / 64.0;
/// Maximum number of baked tempo segments (`TempoMapComplexity`).
pub const TEMPO_BAKE_MAX_SEGMENTS: usize = 65_536;

/// Left-limit probe offset for discontinuity boundaries: the segment ending
/// at a jump is anchored to the source's left-limit value `ε` before the
/// boundary, so the jump itself lands on the boundary's exact frame
/// (03-audio-runtime-spec.md §Tempo automation 烘焙: 保证跳变落在精确
/// frame) instead of ramping across the enclosing grid span. `ε` is
/// 1/1024 of the grid (~1/65536 beat, sub-frame at any BPM).
const DISCONTINUITY_EPSILON_BEAT: f64 = TEMPO_BAKE_GRID_BEAT / 1024.0;

const MIN_BPM: f64 = 20.0;
const MAX_BPM: f64 = 999.0;

/// Log-map a clamped lane output in `[0, 1]` to `20..=999` BPM.
pub fn normalized_to_bpm(value: f64) -> f64 {
    MIN_BPM * (MAX_BPM / MIN_BPM).powf(value.clamp(0.0, 1.0))
}

/// Tempo lane sources must be transport-invariant (02-domain-spec.md):
/// `chance` and any restart semantics are rejected so `bpm(t)` is a pure
/// function of the Project beat.
pub fn ensure_transport_invariant(spec: &AutomationSourceSpec) -> Result<(), OxitoneError> {
    match spec {
        AutomationSourceSpec::Chance(_) => Err(OxitoneError::with_path(
            codes::AUTOMATION_TEMPO_RESTRICTION,
            "tempo lane sources must be transport-invariant; chance is not allowed",
            "$.source",
        )),
        AutomationSourceSpec::Map { input, .. } | AutomationSourceSpec::Unary { input, .. } => {
            ensure_transport_invariant(input)
        }
        AutomationSourceSpec::Binary { left, right, .. } => {
            ensure_transport_invariant(left).and_then(|()| ensure_transport_invariant(right))
        }
        _ => Ok(()),
    }
}

/// Find the single tempo lane bound to `project_id` (`parameterId == "tempo"`),
/// if any. More than one tempo lane is a `TempoAutomationConflict`
/// (02-domain-spec.md §Project 与时间轴).
pub fn find_tempo_lane<'a>(
    lanes: &'a [AutomationLaneSpec],
    project_id: &str,
) -> Result<Option<&'a AutomationLaneSpec>, OxitoneError> {
    let mut tempo_lanes = lanes
        .iter()
        .filter(|lane| lane.target.entity_id == project_id && lane.target.parameter_id == "tempo");
    let first = tempo_lanes.next();
    if first.is_some() && tempo_lanes.next().is_some() {
        return Err(OxitoneError::with_path(
            codes::TEMPO_AUTOMATION_CONFLICT,
            "at most one tempo automation lane is allowed per project",
            "$.automation",
        ));
    }
    Ok(first)
}

/// Bake a transport-invariant tempo source into a piecewise-linear BPM table.
///
/// The bake covers `[0, length_beats + tail]` where the tail is
/// `tail_seconds` converted at the BPM of the bake end. Boundaries are the
/// 1/64-beat grid plus every source discontinuity in range; each
/// discontinuity also contributes a left-limit probe just before it so a
/// jump lands on the boundary's exact frame instead of ramping across the
/// enclosing grid span. Each boundary starts a `linear` segment whose BPM
/// is the exact mapped source value at that beat. The final segment is
/// `step` and holds the end BPM past the bake range.
pub fn bake_tempo_lane(
    source: &AutomationSourceSpec,
    project_seed: u64,
    length_beats: f64,
    tail_seconds: f64,
) -> Result<Vec<TempoSegment>, OxitoneError> {
    if !length_beats.is_finite() || length_beats < 0.0 {
        return Err(OxitoneError::new(
            codes::INVALID_PROJECT,
            format!("bake length must be finite and >= 0, got {length_beats}"),
        ));
    }
    if !tail_seconds.is_finite() || tail_seconds < 0.0 {
        return Err(OxitoneError::new(
            codes::INVALID_PROJECT,
            format!("bake tail must be finite and >= 0, got {tail_seconds}"),
        ));
    }
    ensure_transport_invariant(source)?;
    let evaluator = CompiledAutomation::compile(source, project_seed)?;
    let ctx = EvalContext::default();
    let end_bpm = normalized_to_bpm(evaluator.value_at(length_beats, &ctx));
    let end_beat = length_beats + end_bpm * tail_seconds / 60.0;

    let mut boundaries: Vec<f64> = Vec::new();
    let grid_slots = (end_beat / TEMPO_BAKE_GRID_BEAT).ceil() as u64;
    for slot in 0..=grid_slots {
        boundaries.push(slot as f64 * TEMPO_BAKE_GRID_BEAT);
    }
    // Discontinuities are added twice: the boundary itself (right-continuous
    // value, the post-jump BPM) and, only when the source value materially
    // jumps there, a left-limit probe `ε` before it, so the segment leading
    // into the jump keeps the pre-jump value. Continuous "discontinuities"
    // (curve control points on a sloped source) need no probe: their value
    // delta across `ε` is proportional to the slope, not a jump.
    const JUMP_THRESHOLD: f64 = 1e-3;
    let discontinuities = evaluator.discontinuities(0.0, end_beat, &ctx);
    let mut probes: Vec<f64> = Vec::new();
    for &d in &discontinuities {
        boundaries.push(d);
        let probe = d - DISCONTINUITY_EPSILON_BEAT;
        if probe > 0.0
            && (evaluator.value_at(d, &ctx) - evaluator.value_at(probe, &ctx)).abs()
                > JUMP_THRESHOLD
        {
            boundaries.push(probe);
            probes.push(probe);
        }
    }
    boundaries.sort_by(f64::total_cmp);
    boundaries.dedup();

    if boundaries.len() > TEMPO_BAKE_MAX_SEGMENTS {
        return Err(OxitoneError::with_path(
            codes::TEMPO_MAP_COMPLEXITY,
            format!(
                "tempo bake produced {} segments, exceeding the {} segment budget",
                boundaries.len(),
                TEMPO_BAKE_MAX_SEGMENTS
            ),
            "$.automation",
        ));
    }

    let mut segments = Vec::with_capacity(boundaries.len());
    let mut previous: Option<Beat> = None;
    // Probe boundaries hold their segment at the pre-jump (left-limit) BPM
    // so the jump itself is an exact step at the discontinuity boundary.
    let probes: std::collections::BTreeSet<u64> = probes.iter().map(|p| p.to_bits()).collect();
    for (index, boundary) in boundaries.iter().enumerate() {
        let start_beat = Beat::from_f64(*boundary)?;
        if previous == Some(start_beat) {
            // Distinct f64 boundaries can collapse to the same rational;
            // keep start beats strictly increasing for `TempoMap::compile`.
            continue;
        }
        previous = Some(start_beat);
        segments.push(TempoSegment {
            start_beat,
            bpm: normalized_to_bpm(evaluator.value_at(*boundary, &ctx)),
            curve: if index + 1 < boundaries.len() {
                if probes.contains(&boundary.to_bits()) {
                    Some(TempoCurve::Step)
                } else {
                    Some(TempoCurve::Linear)
                }
            } else {
                None
            },
        });
    }
    if segments.is_empty() {
        return Err(OxitoneError::new(
            codes::TEMPO_MAP_COMPLEXITY,
            "tempo bake produced no segments",
        ));
    }
    Ok(segments)
}
