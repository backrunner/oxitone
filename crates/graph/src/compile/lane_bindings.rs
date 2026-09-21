//! Priority ordering and compilation of lanes after target validation.
use super::bindings::{resolve, AutomationBinding, CompiledLane};
use crate::registry::PluginRegistry;
use oxitone_core::{
    error::OxitoneError,
    wire::{AutomationCombine, ProjectSnapshot},
};
use oxitone_transport::automation::CompiledAutomation;
use std::collections::BTreeMap;

/// Compile every non-tempo lane and group lanes by target. Bindings sort by
/// (entity ID, parameter ID); lanes inside a binding sort by priority, then lane ID.
pub(crate) fn compile_bindings(
    snapshot: &ProjectSnapshot,
    registry: &PluginRegistry,
    channel_index: &BTreeMap<&str, usize>,
    clip_index: &BTreeMap<&str, usize>,
    seed: u64,
    solo_active: bool,
) -> Result<Vec<AutomationBinding>, OxitoneError> {
    let mut lanes: Vec<&_> = snapshot.automation.iter().collect();
    lanes.sort_by(|a, b| {
        a.priority
            .unwrap_or(0)
            .cmp(&b.priority.unwrap_or(0))
            .then(a.id.cmp(&b.id))
    });

    let mut grouped = BTreeMap::<_, AutomationBinding>::new();
    for lane in lanes {
        let resolved = if let Some(scope) = lane.target.scope {
            Some(crate::instance_targets::resolve(
                snapshot,
                registry,
                &lane.target.entity_id,
                scope,
                &lane.target.parameter_id,
            )?)
        } else {
            resolve(
                snapshot,
                registry,
                channel_index,
                clip_index,
                &lane.target.entity_id,
                &lane.target.parameter_id,
            )?
        };
        let Some((target, spec)) = resolved else {
            continue;
        };
        let automation = CompiledAutomation::compile(&lane.source, seed).map_err(|mut err| {
            err.path = Some(format!("$.automation[{}].source", lane.id));
            err
        })?;
        let (loop_start, loop_length, loop_end) = match &lane.loop_spec {
            Some(loop_spec) => {
                let start = loop_spec
                    .start_beat
                    .unwrap_or(oxitone_core::beat::Beat::ZERO)
                    .to_f64();
                let length = loop_spec.length_beats.to_f64();
                let end = match (loop_spec.count, loop_spec.last_beat) {
                    (Some(count), None) => Some(start + length * f64::from(count)),
                    (None, Some(last)) => Some(last.to_f64()),
                    _ => None,
                };
                (start, Some(length), end)
            }
            None => (0.0, None, None),
        };
        let compiled = CompiledLane {
            placements: super::placement::compile(snapshot, lane, solo_active),
            automation,
            combine: lane.combine.unwrap_or(AutomationCombine::Replace),
            loop_start,
            loop_length,
            loop_end,
            last_beat: lane.last_beat.map(|b| b.to_f64()),
        };
        let key = target.clone();
        grouped
            .entry(key)
            .or_insert_with(|| AutomationBinding {
                fallback: super::initial_value::normalized(snapshot, &target, &spec),
                target,
                spec,
                lanes: Vec::new(),
            })
            .lanes
            .push(compiled);
    }
    Ok(grouped.into_values().collect())
}
