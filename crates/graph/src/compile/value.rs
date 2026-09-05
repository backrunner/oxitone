//! Lane value evaluation helpers: 0..1 → physical mapping, lane combine
//! rules, loop/lastBeat beat mapping, and content-end estimation.

use std::collections::BTreeMap;

use oxitone_core::beat::Beat;
use oxitone_core::error::OxitoneError;
use oxitone_core::wire::{AutomationCombine, ParameterSpec, ProjectSnapshot};
use oxitone_transport::EvalContext;

use super::bindings::{AutomationBinding, CompiledLane};

/// Maximum beat reached by pattern clips (beat-domain only).
pub(crate) fn pattern_content_end(
    snapshot: &ProjectSnapshot,
    tempo: &oxitone_transport::CompiledTempoMap,
) -> Result<f64, OxitoneError> {
    let patterns: BTreeMap<&str, Beat> = snapshot
        .patterns
        .iter()
        .map(|p| (p.id.as_str(), p.length_beats))
        .collect();
    let track_enabled: BTreeMap<&str, bool> = snapshot
        .tracks
        .iter()
        .map(|t| (t.id.as_str(), t.enabled != Some(false)))
        .collect();
    let mut end = 0.0f64;
    for clip in &snapshot.pattern_clips {
        if clip.enabled == Some(false)
            || !track_enabled
                .get(clip.track_id.as_str())
                .copied()
                .unwrap_or(false)
        {
            continue;
        }
        let pattern_len = patterns
            .get(clip.pattern_id.as_str())
            .copied()
            .unwrap_or(Beat::ZERO);
        let clip_end = match (clip.duration_beats, clip.loop_count, clip.last_beat) {
            (Some(d), None, None) => clip.start_beat.checked_add(d)?,
            (None, Some(n), None) => clip
                .start_beat
                .checked_add(pattern_len.checked_mul(Beat::new(i64::from(n), 1)?)?)?,
            (None, None, Some(last)) => last,
            _ => clip.start_beat.checked_add(pattern_len)?,
        };
        let bpm = snapshot
            .tracks
            .iter()
            .find(|t| t.id == clip.track_id)
            .and_then(|t| t.tempo);
        end = end.max(
            oxitone_transport::TrackClock::new(tempo, bpm)?
                .project_beat(clip_end)
                .to_f64(),
        );
    }
    Ok(end)
}

/// Physical mapping of a clamped 0..1 lane output through a parameter spec
/// (04-api-contracts.md §ParameterSpec).
pub fn map_normalized(spec: &ParameterSpec, value: f64) -> f64 {
    use oxitone_core::wire::ParameterMapping;
    let v = value.clamp(0.0, 1.0);
    let mapped = match spec.mapping.unwrap_or(ParameterMapping::Linear) {
        ParameterMapping::Log => spec.min * (spec.max / spec.min).powf(v),
        // Linear and bipolar both interpolate the declared range; enum
        // rounds onto integer steps.
        ParameterMapping::Linear | ParameterMapping::Bipolar | ParameterMapping::Enum => {
            spec.min + v * (spec.max - spec.min)
        }
    };
    let clamped = mapped.clamp(spec.min, spec.max);
    if spec.unit == oxitone_core::wire::ParameterUnit::Enum {
        clamped.round()
    } else {
        clamped
    }
}

/// Combine rule across lanes bound to one target (02-domain-spec.md
/// §Automation): lanes apply in lane-ID order onto the running value.
pub fn combine_value(acc: Option<f64>, combine: AutomationCombine, value: f64) -> f64 {
    match (acc, combine) {
        (None, _) => value,
        (Some(_), AutomationCombine::Replace) => value,
        (Some(a), AutomationCombine::Add) => a + value,
        (Some(a), AutomationCombine::Multiply) => a * value,
        (Some(a), AutomationCombine::Max) => a.max(value),
    }
}

/// Beat-domain wrapper for lane `loop`/`lastBeat` (02-domain-spec.md
/// §Automation). Inside the loop region the beat maps onto the loop phase;
/// past the loop end (or `lastBeat`) the value holds at the boundary beat.
pub fn lane_beat(lane: &CompiledLane, beat: f64) -> f64 {
    if let Some(length) = lane.loop_length {
        let end = lane.loop_end.unwrap_or(f64::INFINITY);
        if beat < lane.loop_start {
            return beat;
        }
        let b = beat.min(end);
        let offset = b - lane.loop_start;
        let phase = if length > 0.0 {
            offset.rem_euclid(length)
        } else {
            0.0
        };
        // `end` landing exactly on a wrap boundary holds the loop's final
        // phase rather than jumping back to the loop start.
        if b == end && phase == 0.0 && offset > 0.0 {
            return lane.loop_start + length;
        }
        lane.loop_start + phase
    } else if let Some(last) = lane.last_beat {
        beat.min(last)
    } else {
        beat
    }
}

/// Effective value of one binding at a Project beat: lanes combined in
/// order, clamped to 0..1.
pub fn binding_value_at(binding: &AutomationBinding, beat: f64, ctx: &EvalContext) -> f64 {
    let mut acc = None;
    for lane in &binding.lanes {
        let value = lane.automation.value_at(lane_beat(lane, beat), ctx);
        acc = Some(combine_value(acc, lane.combine, value));
    }
    acc.unwrap_or(0.0).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxitone_core::wire::{ParameterMapping, ParameterRate, ParameterSmoothing, ParameterUnit};
    use oxitone_transport::automation::CompiledAutomation;

    fn spec(min: f64, max: f64, mapping: ParameterMapping) -> ParameterSpec {
        ParameterSpec {
            id: "p".into(),
            label: "p".into(),
            unit: ParameterUnit::Normalized,
            min,
            max,
            default: min,
            smoothing: ParameterSmoothing::Linear,
            rate: ParameterRate::Control,
            automation: Some(true),
            mapping: Some(mapping),
        }
    }

    #[test]
    fn normalized_mapping_linear_log_bipolar() {
        let linear = spec(0.0, 2.0, ParameterMapping::Linear);
        assert_eq!(map_normalized(&linear, 0.5), 1.0);
        let log = spec(20.0, 999.0, ParameterMapping::Log);
        let mid = map_normalized(&log, 0.5);
        assert!((mid - (20.0f64 * 999.0).sqrt()).abs() < 1e-9);
        let bipolar = spec(-1.0, 1.0, ParameterMapping::Bipolar);
        assert_eq!(map_normalized(&bipolar, 0.75), 0.5);
        // Out-of-range lane outputs clamp first.
        assert_eq!(map_normalized(&linear, 1.5), 2.0);
    }

    #[test]
    fn combine_rules_apply_in_order() {
        assert_eq!(combine_value(None, AutomationCombine::Add, 0.5), 0.5);
        assert_eq!(
            combine_value(Some(0.4), AutomationCombine::Replace, 0.7),
            0.7
        );
        assert_eq!(combine_value(Some(0.4), AutomationCombine::Add, 0.7), 1.1);
        assert_eq!(
            combine_value(Some(0.4), AutomationCombine::Multiply, 0.5),
            0.2
        );
        assert_eq!(combine_value(Some(0.4), AutomationCombine::Max, 0.7), 0.7);
    }

    #[test]
    fn lane_beat_loop_wraps_and_holds() {
        let lane = |loop_start, loop_length, loop_end, last_beat| CompiledLane {
            automation: CompiledAutomation::compile(
                &oxitone_core::wire::AutomationSourceSpec::Constant { value: 1.0 },
                1,
            )
            .unwrap(),
            combine: AutomationCombine::Replace,
            loop_start,
            loop_length,
            loop_end,
            last_beat,
        };
        let looping = lane(2.0, Some(4.0), Some(10.0), None);
        assert_eq!(lane_beat(&looping, 0.5), 0.5); // before the loop
        assert_eq!(lane_beat(&looping, 3.0), 3.0);
        assert_eq!(lane_beat(&looping, 6.5), 2.5); // wraps into the region
        assert_eq!(lane_beat(&looping, 9.75), 5.75);
        assert_eq!(lane_beat(&looping, 12.0), 6.0); // past the end: holds
        let bounded = lane(0.0, None, None, Some(3.0));
        assert_eq!(lane_beat(&bounded, 5.0), 3.0);
        let free = lane(0.0, None, None, None);
        assert_eq!(lane_beat(&free, 42.0), 42.0);
    }
}
