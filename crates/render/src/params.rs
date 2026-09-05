//! Host `setParameter` events (04-api-contracts.md §Wire messages:
//! `{type:'setParameter', entityId, parameterId, value, atFrame?}`). The
//! control thread validates and resolves the target at enqueue time and the
//! event queue is drained at control rate in `RenderGraph::process_block`
//! (`frame <= block start`), so offline renders driven through the graph see
//! the change; on the realtime path the event travels through the worker
//! command queue and takes effect at the ring horizon. Values are physical
//! (the plugin ABI's convention for parameter events), range-checked against
//! the target `ParameterSpec`. Channel `swing` stays automation-only:
//! dispatch reads the immutable plan value, so a runtime override could not
//! take effect and is rejected instead of silently dropped.

use std::collections::{BTreeMap, BTreeSet};

use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::wire::ParameterSpec;
use oxitone_graph::builtin_params::{
    find_parameter, insert_parameter, parse_insert_parameter, parse_send_ratio_parameter,
    send_ratio_parameter, BuiltinEntityKind, InsertParam,
};
use oxitone_graph::topology::MASTER_MIXER_CHANNEL_ID;

use crate::bindings::RtTarget;
use crate::graph::RenderGraph;

/// One host parameter change (physical value; `at_frame` = absolute
/// transport frame, `None` applies at the next processed block).
#[derive(Debug, Clone, PartialEq)]
pub struct ParameterEventInput {
    pub entity_id: String,
    pub parameter_id: String,
    pub value: f64,
    pub at_frame: Option<u64>,
}

/// Resolved and validated queue entry (no ID lookups on the drain path).
pub struct QueuedParameterEvent {
    pub frame: u64,
    pub target: RtTarget,
    pub value: f64,
}

fn target_invalid(message: impl Into<String>) -> OxitoneError {
    OxitoneError::new(codes::AUTOMATION_TARGET_INVALID, message)
}

/// Everything parameter resolution needs from the graph, abstracted so the
/// realtime session can resolve against an immutable snapshot
/// ([`ParamTargetIndex`]) while the compiled graph lives on the render
/// worker thread.
pub(crate) trait ParamGraphView {
    fn channel_index(&self, entity: &str) -> Option<usize>;
    fn channel_id(&self, channel: usize) -> &str;
    fn channel_insert_count(&self, channel: usize) -> usize;
    fn instrument_param_ids(&self, channel: usize) -> &[String];
    fn instrument_specs(&self, channel: usize) -> &[ParameterSpec];
    fn has_mixer_bus(&self, id: &str) -> bool;
    fn has_send_route(&self, route: &(String, String)) -> bool;
    fn clip_index(&self, entity: &str) -> Option<usize>;
}

/// Immutable snapshot of every resolvable parameter target, built at
/// compile/session start (control thread). Lets `setParameter` keep its
/// synchronous validation while the graph itself is owned by the realtime
/// worker.
pub struct ParamTargetIndex {
    channels: BTreeMap<String, usize>,
    channel_ids: Vec<String>,
    insert_counts: Vec<usize>,
    instrument_param_ids: Vec<std::sync::Arc<Vec<String>>>,
    instrument_specs: Vec<std::sync::Arc<Vec<ParameterSpec>>>,
    mixer_buses: BTreeSet<String>,
    mixer_sends: BTreeSet<(String, String)>,
    clip_ids: BTreeMap<String, usize>,
}

impl ParamTargetIndex {
    pub fn from_graph(graph: &RenderGraph) -> Self {
        let mut channels = BTreeMap::new();
        let mut channel_ids = Vec::new();
        let mut insert_counts = Vec::new();
        let mut instrument_param_ids = Vec::new();
        let mut instrument_specs = Vec::new();
        for (index, channel) in graph.channels.iter().enumerate() {
            channels.insert(channel.id.clone(), index);
            channel_ids.push(channel.id.clone());
            insert_counts.push(channel.inserts.len());
            instrument_param_ids.push(channel.instrument_param_ids.clone());
            instrument_specs.push(channel.instrument_specs.clone());
        }
        Self {
            channels,
            channel_ids,
            insert_counts,
            instrument_param_ids,
            instrument_specs,
            mixer_buses: graph.mixer.bus_order().into_iter().collect(),
            mixer_sends: graph.mixer_sends.clone(),
            clip_ids: graph
                .plan
                .sample_clips
                .iter()
                .enumerate()
                .map(|(index, clip)| (clip.id.clone(), index))
                .collect(),
        }
    }
}

impl ParamGraphView for ParamTargetIndex {
    fn channel_index(&self, entity: &str) -> Option<usize> {
        self.channels.get(entity).copied()
    }

    fn channel_id(&self, channel: usize) -> &str {
        &self.channel_ids[channel]
    }

    fn channel_insert_count(&self, channel: usize) -> usize {
        self.insert_counts[channel]
    }

    fn instrument_param_ids(&self, channel: usize) -> &[String] {
        &self.instrument_param_ids[channel]
    }

    fn instrument_specs(&self, channel: usize) -> &[ParameterSpec] {
        &self.instrument_specs[channel]
    }

    fn has_mixer_bus(&self, id: &str) -> bool {
        self.mixer_buses.contains(id)
    }

    fn has_send_route(&self, route: &(String, String)) -> bool {
        self.mixer_sends.contains(route)
    }

    fn clip_index(&self, entity: &str) -> Option<usize> {
        self.clip_ids.get(entity).copied()
    }
}

impl ParamGraphView for RenderGraph {
    fn channel_index(&self, entity: &str) -> Option<usize> {
        self.channel_index.get(entity).copied()
    }

    fn channel_id(&self, channel: usize) -> &str {
        &self.channels[channel].id
    }

    fn channel_insert_count(&self, channel: usize) -> usize {
        self.channels[channel].inserts.len()
    }

    fn instrument_param_ids(&self, channel: usize) -> &[String] {
        &self.channels[channel].instrument_param_ids
    }

    fn instrument_specs(&self, channel: usize) -> &[ParameterSpec] {
        &self.channels[channel].instrument_specs
    }

    fn has_mixer_bus(&self, id: &str) -> bool {
        self.mixer.bus_order().iter().any(|bus| bus == id)
    }

    fn has_send_route(&self, route: &(String, String)) -> bool {
        self.mixer_sends.contains(route)
    }

    fn clip_index(&self, entity: &str) -> Option<usize> {
        self.plan
            .sample_clips
            .iter()
            .position(|clip| clip.id == entity)
    }
}

fn resolve_channel(
    view: &dyn ParamGraphView,
    channel: usize,
    parameter_id: &str,
) -> Result<(RtTarget, ParameterSpec), OxitoneError> {
    if let Some(spec) = find_parameter(BuiltinEntityKind::Channel, parameter_id) {
        let target = match parameter_id {
            "level" => RtTarget::ChannelLevel(channel),
            "pan" => RtTarget::ChannelPan(channel),
            "mute" => RtTarget::ChannelMute(channel),
            other => {
                return Err(target_invalid(format!(
                    "channel parameter {other:?} is not settable at runtime; use an automation lane"
                )))
            }
        };
        return Ok((target, spec));
    }
    if let Some((index, param)) = parse_insert_parameter(parameter_id) {
        if index >= view.channel_insert_count(channel) {
            return Err(target_invalid(format!(
                "channel {:?} has no insert #{index}",
                view.channel_id(channel)
            )));
        }
        let target = match param {
            InsertParam::Mix => RtTarget::InsertMix {
                channel,
                insert: index,
            },
            InsertParam::Bypass => RtTarget::InsertBypass {
                channel,
                insert: index,
            },
        };
        return Ok((target, insert_parameter(param)));
    }
    let index = view
        .instrument_param_ids(channel)
        .iter()
        .position(|id| id == parameter_id)
        .ok_or_else(|| {
            target_invalid(format!(
                "channel {:?} instrument declares no parameter {parameter_id:?}",
                view.channel_id(channel)
            ))
        })?;
    Ok((
        RtTarget::InstrumentParam { channel, index },
        view.instrument_specs(channel)[index].clone(),
    ))
}

fn resolve_mixer(
    view: &dyn ParamGraphView,
    entity_id: &str,
    parameter_id: &str,
) -> Result<(RtTarget, ParameterSpec), OxitoneError> {
    if let Some(spec) = find_parameter(BuiltinEntityKind::MixerChannel, parameter_id) {
        let target = match parameter_id {
            "level" => RtTarget::MixerLevel(entity_id.to_string()),
            "balance" => RtTarget::MixerBalance(entity_id.to_string()),
            "mute" => RtTarget::MixerMute(entity_id.to_string()),
            "masterSendRatio" => {
                if entity_id == MASTER_MIXER_CHANNEL_ID {
                    return Err(target_invalid(
                        "masterSendRatio does not apply to the Master bus",
                    ));
                }
                RtTarget::MixerMasterSendRatio(entity_id.to_string())
            }
            other => {
                return Err(target_invalid(format!(
                    "mixer bus has no parameter {other:?}"
                )))
            }
        };
        return Ok((target, spec));
    }
    let destination = parse_send_ratio_parameter(parameter_id)
        .ok_or_else(|| target_invalid(format!("mixer bus has no parameter {parameter_id:?}")))?;
    let route = (entity_id.to_string(), destination.to_string());
    if !view.has_send_route(&route) {
        return Err(target_invalid(format!(
            "no send from {entity_id:?} to {destination:?}"
        )));
    }
    Ok((
        RtTarget::MixerSendRatio {
            bus: entity_id.to_string(),
            destination: destination.to_string(),
        },
        send_ratio_parameter(destination),
    ))
}

fn resolve_clip(
    clip: usize,
    parameter_id: &str,
) -> Result<(RtTarget, ParameterSpec), OxitoneError> {
    let spec = find_parameter(BuiltinEntityKind::SampleClip, parameter_id)
        .ok_or_else(|| target_invalid(format!("sample clip has no parameter {parameter_id:?}")))?;
    let target = match parameter_id {
        "level" => RtTarget::ClipLevel(clip),
        "tone" => RtTarget::ClipTone(clip),
        "gain" => RtTarget::ClipGain(clip),
        "pan" => RtTarget::ClipPan(clip),
        "rate" => RtTarget::ClipRate(clip),
        other => {
            return Err(target_invalid(format!(
                "sample clip has no parameter {other:?}"
            )))
        }
    };
    Ok((target, spec))
}

fn resolve_target(
    view: &dyn ParamGraphView,
    entity_id: &str,
    parameter_id: &str,
) -> Result<(RtTarget, ParameterSpec), OxitoneError> {
    if let Some(channel) = view.channel_index(entity_id) {
        return resolve_channel(view, channel, parameter_id);
    }
    if view.has_mixer_bus(entity_id) {
        return resolve_mixer(view, entity_id, parameter_id);
    }
    if let Some(clip) = view.clip_index(entity_id) {
        return resolve_clip(clip, parameter_id);
    }
    Err(target_invalid(format!(
        "unknown parameter target entity {entity_id:?}"
    )))
}

/// Resolve and validate a host parameter event against any graph view
/// (the graph itself on the offline path, the session's
/// [`ParamTargetIndex`] on the realtime path). `default_frame` applies
/// when the event carries no `at_frame` (the session passes the current
/// transport cursor).
pub fn resolve_parameter_event(
    view: &ParamTargetIndex,
    event: &ParameterEventInput,
    default_frame: u64,
) -> Result<QueuedParameterEvent, OxitoneError> {
    let (target, spec) = resolve_target(view, &event.entity_id, &event.parameter_id)?;
    validate_parameter_value(&event.parameter_id, &spec, event.value)?;
    Ok(QueuedParameterEvent {
        frame: event.at_frame.unwrap_or(default_frame),
        target,
        value: event.value,
    })
}

fn validate_parameter_value(
    parameter_id: &str,
    spec: &ParameterSpec,
    value: f64,
) -> Result<(), OxitoneError> {
    if !value.is_finite() {
        return Err(OxitoneError::new(
            codes::AUTOMATION_RANGE,
            format!("parameter {parameter_id:?} value must be finite, got {value}"),
        ));
    }
    if value < spec.min || value > spec.max {
        return Err(OxitoneError::new(
            codes::AUTOMATION_RANGE,
            format!(
                "parameter {parameter_id:?} value {value} is outside {}..={}",
                spec.min, spec.max
            ),
        ));
    }
    Ok(())
}

impl RenderGraph {
    /// Validate and enqueue a host parameter event. Resolution failures are
    /// `AutomationTargetInvalid`; non-finite or out-of-`ParameterSpec`-range
    /// values are `AutomationRange`.
    pub fn enqueue_parameter(&mut self, event: &ParameterEventInput) -> Result<(), OxitoneError> {
        let (target, spec) = resolve_target(self, &event.entity_id, &event.parameter_id)?;
        validate_parameter_value(&event.parameter_id, &spec, event.value)?;
        let frame = event.at_frame.unwrap_or(self.transport.cursor);
        self.insert_queued_parameter(QueuedParameterEvent {
            frame,
            target,
            value: event.value,
        });
        Ok(())
    }

    /// Insert a pre-resolved event (realtime worker path: resolution
    /// happened on the control thread against the session's
    /// [`ParamTargetIndex`]). `frame` is used as-is.
    pub fn insert_queued_parameter(&mut self, event: QueuedParameterEvent) {
        let position = self
            .param_queue
            .iter()
            .position(|queued| queued.frame > event.frame)
            .unwrap_or(self.param_queue.len());
        self.param_queue.insert(position, event);
    }

    /// Number of queued, not-yet-applied host parameter events.
    pub fn pending_parameter_events(&self) -> usize {
        self.param_queue.len()
    }
}
