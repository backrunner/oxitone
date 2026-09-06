//! Automation lane compilation and target binding (02-domain-spec.md
//! §Automation, 04-api-contracts.md §内建参数全集). Targets are resolved to
//! plan indices/IDs at compile time — no runtime ID lookups on the audio
//! path. The validator has already accepted every lane; resolution failures
//! here indicate an internal inconsistency and reuse the same error codes.

use std::collections::BTreeMap;

use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::wire::{AutomationCombine, EntityId, ParameterSpec, ProjectSnapshot};
use oxitone_transport::automation::CompiledAutomation;

use crate::builtin_params::{find_parameter, parse_send_ratio_parameter, BuiltinEntityKind};
use crate::registry::PluginRegistry;

/// One compiled lane: fixed evaluator plus combine rule and loop mapping.
#[derive(Debug, Clone)]
pub struct CompiledLane {
    pub automation: CompiledAutomation,
    pub combine: AutomationCombine,
    /// Loop region start in beats (default 0 when `loop_length` is set).
    pub loop_start: f64,
    /// Loop region length in beats; `None` when the lane does not loop.
    pub loop_length: Option<f64>,
    /// Exclusive loop end (from `count` or `lastBeat`); `None` = unbounded.
    pub loop_end: Option<f64>,
    /// Exclusive value end without looping; the value holds past it.
    pub last_beat: Option<f64>,
}

/// Resolved automation target. Channel/sample-clip targets are plan indices;
/// mixer targets carry bus IDs (the mixer engine resolves them).
#[derive(Debug, Clone, PartialEq)]
pub enum BindingTarget {
    EffectInsert {
        entity_id: String,
        parameter_id: String,
    },
    ChannelLevel(usize),
    ChannelPan(usize),
    ChannelMute(usize),
    ChannelSwing(usize),
    /// Instrument plugin parameter on a channel (physical value via `spec`).
    InstrumentParam {
        channel: usize,
        parameter_id: String,
    },
    /// Built-in per-insert `mix` on a channel's effect chain.
    InsertMix {
        channel: usize,
        insert: usize,
    },
    /// Built-in per-insert `bypass` on a channel's effect chain.
    InsertBypass {
        channel: usize,
        insert: usize,
    },
    MixerLevel(EntityId),
    MixerBalance(EntityId),
    MixerMute(EntityId),
    MixerMasterSendRatio(EntityId),
    MixerSendRatio {
        bus: EntityId,
        destination: EntityId,
    },
    SampleClipLevel(usize),
    SampleClipTone(usize),
    SampleClipGain(usize),
    SampleClipPan(usize),
    SampleClipRate(usize),
}

/// All lanes bound to one target, in lane-ID order.
#[derive(Debug, Clone)]
pub struct AutomationBinding {
    pub target: BindingTarget,
    /// Target parameter spec for the 0..1 → physical mapping.
    pub spec: ParameterSpec,
    pub lanes: Vec<CompiledLane>,
}

fn target_invalid(message: impl Into<String>) -> OxitoneError {
    OxitoneError::new(codes::AUTOMATION_TARGET_INVALID, message)
}

/// Resolve one lane target to a plan-level binding target plus its spec.
fn resolve(
    snapshot: &ProjectSnapshot,
    registry: &PluginRegistry,
    channel_index: &BTreeMap<&str, usize>,
    clip_index: &BTreeMap<&str, usize>,
    entity_id: &str,
    parameter_id: &str,
) -> Result<Option<(BindingTarget, ParameterSpec)>, OxitoneError> {
    if entity_id == snapshot.id {
        // The tempo lane was consumed by the bake; no runtime binding.
        return Ok(None);
    }
    if let Some(&channel) = channel_index.get(entity_id) {
        if let Some(spec) = find_parameter(BuiltinEntityKind::Channel, parameter_id) {
            let target = match parameter_id {
                "level" => BindingTarget::ChannelLevel(channel),
                "pan" => BindingTarget::ChannelPan(channel),
                "mute" => BindingTarget::ChannelMute(channel),
                "swing" => BindingTarget::ChannelSwing(channel),
                other => {
                    return Err(target_invalid(format!(
                        "channel has no parameter {other:?}"
                    )))
                }
            };
            return Ok(Some((target, spec)));
        }
        let channel_spec = snapshot
            .channels
            .iter()
            .find(|c| c.id == entity_id)
            .expect("channel index covers the snapshot");
        if parameter_id.starts_with("insert.") {
            return resolve_insert(
                &channel_spec.effect_chain,
                registry,
                entity_id,
                parameter_id,
            );
        }
        let descriptor = registry
            .lookup_descriptor(
                &channel_spec.instrument.plugin_id,
                &channel_spec.instrument.plugin_version,
            )
            .expect("instrument references are validated before compile");
        let spec = descriptor
            .parameters
            .iter()
            .find(|s| s.id == parameter_id)
            .cloned()
            .ok_or_else(|| {
                target_invalid(format!(
                    "channel instrument declares no parameter {parameter_id:?}"
                ))
            })?;
        return Ok(Some((
            BindingTarget::InstrumentParam {
                channel,
                parameter_id: parameter_id.to_string(),
            },
            spec,
        )));
    }
    if let Some(bus) = snapshot.mixer_channels.iter().find(|m| m.id == entity_id) {
        if parameter_id.starts_with("insert.") {
            return resolve_insert(&bus.inserts, registry, entity_id, parameter_id);
        }
        let spec =
            find_parameter(BuiltinEntityKind::MixerChannel, parameter_id).ok_or_else(|| {
                target_invalid(format!("mixer channel has no parameter {parameter_id:?}"))
            })?;
        let target = match parameter_id {
            "level" => BindingTarget::MixerLevel(entity_id.to_string()),
            "balance" => BindingTarget::MixerBalance(entity_id.to_string()),
            "mute" => BindingTarget::MixerMute(entity_id.to_string()),
            "masterSendRatio" => BindingTarget::MixerMasterSendRatio(entity_id.to_string()),
            other => {
                let destination = parse_send_ratio_parameter(other).ok_or_else(|| {
                    target_invalid(format!("mixer channel has no parameter {other:?}"))
                })?;
                BindingTarget::MixerSendRatio {
                    bus: entity_id.to_string(),
                    destination: destination.to_string(),
                }
            }
        };
        return Ok(Some((target, spec)));
    }
    if let Some(&clip) = clip_index.get(entity_id) {
        let spec =
            find_parameter(BuiltinEntityKind::SampleClip, parameter_id).ok_or_else(|| {
                target_invalid(format!("sample clip has no parameter {parameter_id:?}"))
            })?;
        let target = match parameter_id {
            "level" => BindingTarget::SampleClipLevel(clip),
            "tone" => BindingTarget::SampleClipTone(clip),
            "gain" => BindingTarget::SampleClipGain(clip),
            "pan" => BindingTarget::SampleClipPan(clip),
            "rate" => BindingTarget::SampleClipRate(clip),
            other => {
                return Err(target_invalid(format!(
                    "sample clip has no parameter {other:?}"
                )))
            }
        };
        return Ok(Some((target, spec)));
    }
    Err(target_invalid(format!(
        "unknown automation target entity {entity_id:?}"
    )))
}

fn resolve_insert(
    effects: &[oxitone_core::wire::EffectRef],
    registry: &PluginRegistry,
    entity_id: &str,
    parameter_id: &str,
) -> Result<Option<(BindingTarget, ParameterSpec)>, OxitoneError> {
    let spec = crate::insert_params::resolve(effects, registry, parameter_id)
        .ok_or_else(|| target_invalid(format!("invalid insert target {parameter_id:?}")))?;
    Ok(Some((
        BindingTarget::EffectInsert {
            entity_id: entity_id.to_string(),
            parameter_id: parameter_id.to_string(),
        },
        spec,
    )))
}

/// Compile every non-tempo lane and group lanes by target. Bindings sort by
/// (entity ID, parameter ID); lanes inside a binding sort by lane ID.
pub(crate) fn compile_bindings(
    snapshot: &ProjectSnapshot,
    registry: &PluginRegistry,
    channel_index: &BTreeMap<&str, usize>,
    clip_index: &BTreeMap<&str, usize>,
    seed: u64,
) -> Result<Vec<AutomationBinding>, OxitoneError> {
    let mut lanes: Vec<&_> = snapshot.automation.iter().collect();
    lanes.sort_by(|a, b| a.id.cmp(&b.id));

    let mut grouped: BTreeMap<(String, String), AutomationBinding> = BTreeMap::new();
    for lane in lanes {
        let Some((target, spec)) = resolve(
            snapshot,
            registry,
            channel_index,
            clip_index,
            &lane.target.entity_id,
            &lane.target.parameter_id,
        )?
        else {
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
            automation,
            combine: lane.combine.unwrap_or(AutomationCombine::Replace),
            loop_start,
            loop_length,
            loop_end,
            last_beat: lane.last_beat.map(|b| b.to_f64()),
        };
        let key = (
            lane.target.entity_id.clone(),
            lane.target.parameter_id.clone(),
        );
        grouped
            .entry(key)
            .or_insert_with(|| AutomationBinding {
                target,
                spec,
                lanes: Vec::new(),
            })
            .lanes
            .push(compiled);
    }
    Ok(grouped.into_values().collect())
}
