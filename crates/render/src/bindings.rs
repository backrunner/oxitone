//! Runtime automation application: plan bindings resolved to runtime
//! indices and applied at control rate at every block boundary
//! (03-audio-runtime-spec.md §时间和调度: control-rate targets evaluate at
//! most once per block).

use oxitone_core::wire::ParameterSpec;
use oxitone_graph::compile::{binding_value_at, map_normalized, BindingTarget};

use crate::graph::RenderGraph;

/// Binding target resolved to runtime indices.
#[derive(Debug, Clone)]
pub enum RtTarget {
    EffectInsert(crate::effect_targets::EffectTarget),
    ChannelLevel(usize),
    ChannelPan(usize),
    ChannelMute(usize),
    InstrumentParam { channel: usize, index: usize },
    InsertMix { channel: usize, insert: usize },
    InsertBypass { channel: usize, insert: usize },
    MixerLevel(String),
    MixerBalance(String),
    MixerMute(String),
    MixerMasterSendRatio(String),
    MixerSendRatio { bus: String, destination: String },
    ClipLevel(usize),
    ClipTone(usize),
    ClipGain(usize),
    ClipPan(usize),
    ClipRate(usize),
}

pub(crate) struct RtBinding {
    pub(crate) target: RtTarget,
    pub(crate) spec: ParameterSpec,
    pub(crate) plan_binding: usize,
}

/// Map plan bindings to runtime indices (instrument parameter IDs are
/// resolved against the instance's descriptor table). Swing bindings are
/// consumed per event at dispatch time and skipped here.
pub(crate) fn resolve_bindings(graph: &RenderGraph) -> Vec<RtBinding> {
    let mut out = Vec::new();
    for (index, binding) in graph.plan.bindings.iter().enumerate() {
        let target = match &binding.target {
            BindingTarget::EffectInsert {
                entity_id,
                parameter_id,
            } => {
                graph
                    .effect_targets
                    .resolve(entity_id, parameter_id)
                    .expect("validated insert target")
                    .0
            }
            BindingTarget::ChannelLevel(c) => RtTarget::ChannelLevel(*c),
            BindingTarget::ChannelPan(c) => RtTarget::ChannelPan(*c),
            BindingTarget::ChannelMute(c) => RtTarget::ChannelMute(*c),
            BindingTarget::ChannelSwing(_) => continue,
            BindingTarget::InstrumentParam {
                channel,
                parameter_id,
            } => {
                let Some(param_index) = graph.channels[*channel]
                    .instrument_param_ids
                    .iter()
                    .position(|id| id == parameter_id)
                else {
                    continue;
                };
                RtTarget::InstrumentParam {
                    channel: *channel,
                    index: param_index,
                }
            }
            BindingTarget::InsertMix { channel, insert } => RtTarget::InsertMix {
                channel: *channel,
                insert: *insert,
            },
            BindingTarget::InsertBypass { channel, insert } => RtTarget::InsertBypass {
                channel: *channel,
                insert: *insert,
            },
            BindingTarget::MixerLevel(bus) => RtTarget::MixerLevel(bus.clone()),
            BindingTarget::MixerBalance(bus) => RtTarget::MixerBalance(bus.clone()),
            BindingTarget::MixerMute(bus) => RtTarget::MixerMute(bus.clone()),
            BindingTarget::MixerMasterSendRatio(bus) => RtTarget::MixerMasterSendRatio(bus.clone()),
            BindingTarget::MixerSendRatio { bus, destination } => RtTarget::MixerSendRatio {
                bus: bus.clone(),
                destination: destination.clone(),
            },
            BindingTarget::SampleClipLevel(c) => RtTarget::ClipLevel(*c),
            BindingTarget::SampleClipTone(c) => RtTarget::ClipTone(*c),
            BindingTarget::SampleClipGain(c) => RtTarget::ClipGain(*c),
            BindingTarget::SampleClipPan(c) => RtTarget::ClipPan(*c),
            BindingTarget::SampleClipRate(c) => RtTarget::ClipRate(*c),
        };
        out.push(RtBinding {
            target,
            spec: binding.spec.clone(),
            plan_binding: index,
        });
    }
    out
}

pub(crate) fn binding_due(graph: &RenderGraph, binding: &RtBinding, frame: u64) -> bool {
    if binding.spec.rate == oxitone_core::wire::ParameterRate::Audio || frame == 0 {
        return true;
    }
    let previous = graph.plan.tempo.frame_to_beat(frame - 1).to_f64();
    let beat = graph.plan.tempo.frame_to_beat(frame).to_f64();
    graph.plan.bindings[binding.plan_binding]
        .lanes
        .iter()
        .any(|lane| {
            let a = oxitone_graph::compile::lane_beat(lane, previous);
            let b = oxitone_graph::compile::lane_beat(lane, beat);
            b < a || lane.automation.has_edge(a, b, &graph.eval_ctx)
        })
}

/// Apply automation at a block start or an intra-block edge.
pub(crate) fn apply_bindings(graph: &mut RenderGraph, beat: f64, first: bool) {
    let ctx = graph.eval_ctx;
    for binding in &graph.rt_bindings {
        if !first && !binding_due(graph, binding, graph.transport.cursor) {
            continue;
        }
        let value = binding_value_at(&graph.plan.bindings[binding.plan_binding], beat, &ctx);
        let physical = map_normalized(&binding.spec, value);
        // NOTE: mirrors `apply_rt_target`; kept inline so the
        // `&graph.rt_bindings` iteration borrow can coexist with the
        // disjoint field mutations (RT path, no per-block clones).
        match &binding.target {
            RtTarget::EffectInsert(target) => crate::effect_targets::apply(
                &mut graph.channels,
                &mut graph.mixer,
                &mut graph.mixer_beat_params,
                *target,
                physical,
            ),
            RtTarget::ChannelLevel(c) => graph.channels[*c].level.set_target(physical as f32),
            RtTarget::ChannelPan(c) => graph.channels[*c].pan.set_target(physical as f32),
            RtTarget::ChannelMute(c) => graph.channels[*c].mute = physical >= 0.5,
            RtTarget::InstrumentParam { channel, index } => {
                graph.channels[*channel]
                    .instrument_staged
                    .set(*index, physical);
            }
            RtTarget::InsertMix { channel, insert } => graph.channels[*channel].inserts[*insert]
                .mix
                .set_target(physical as f32),
            RtTarget::InsertBypass { channel, insert } => {
                graph.channels[*channel].inserts[*insert].bypass = physical >= 0.5
            }
            RtTarget::MixerLevel(bus) => {
                let _ = graph.mixer.set_level(bus, physical);
            }
            RtTarget::MixerBalance(bus) => {
                let _ = graph.mixer.set_balance(bus, physical);
            }
            RtTarget::MixerMute(bus) => {
                let _ = graph.mixer.set_mute(bus, physical >= 0.5);
            }
            RtTarget::MixerMasterSendRatio(bus) => {
                let _ = graph.mixer.set_master_send_ratio(bus, physical);
            }
            RtTarget::MixerSendRatio { bus, destination } => {
                let _ = graph.mixer.set_send_ratio(bus, destination, physical);
            }
            RtTarget::ClipLevel(c) => graph.clips[*c].level.set_target(physical as f32),
            RtTarget::ClipTone(c) => graph.clips[*c].tone = physical as f32,
            RtTarget::ClipGain(c) => graph.clips[*c].gain.set_target(physical as f32),
            RtTarget::ClipPan(c) => graph.clips[*c].pan.set_target(physical as f32),
            RtTarget::ClipRate(c) => graph.clips[*c].automation_rate = physical,
        }
    }
}

/// Apply one physical value to a resolved runtime target. Used by host
/// `setParameter` events drained from the parameter queue; the automation
/// path keeps an inline copy in `apply_bindings` for borrow reasons.
pub(crate) fn apply_rt_target(graph: &mut RenderGraph, target: &RtTarget, physical: f64) {
    match target {
        RtTarget::EffectInsert(target) => crate::effect_targets::apply(
            &mut graph.channels,
            &mut graph.mixer,
            &mut graph.mixer_beat_params,
            *target,
            physical,
        ),
        RtTarget::ChannelLevel(c) => graph.channels[*c].level.set_target(physical as f32),
        RtTarget::ChannelPan(c) => graph.channels[*c].pan.set_target(physical as f32),
        RtTarget::ChannelMute(c) => graph.channels[*c].mute = physical >= 0.5,
        RtTarget::InstrumentParam { channel, index } => {
            graph.channels[*channel]
                .instrument_staged
                .set(*index, physical);
        }
        RtTarget::InsertMix { channel, insert } => graph.channels[*channel].inserts[*insert]
            .mix
            .set_target(physical as f32),
        RtTarget::InsertBypass { channel, insert } => {
            graph.channels[*channel].inserts[*insert].bypass = physical >= 0.5
        }
        RtTarget::MixerLevel(bus) => {
            let _ = graph.mixer.set_level(bus, physical);
        }
        RtTarget::MixerBalance(bus) => {
            let _ = graph.mixer.set_balance(bus, physical);
        }
        RtTarget::MixerMute(bus) => {
            let _ = graph.mixer.set_mute(bus, physical >= 0.5);
        }
        RtTarget::MixerMasterSendRatio(bus) => {
            let _ = graph.mixer.set_master_send_ratio(bus, physical);
        }
        RtTarget::MixerSendRatio { bus, destination } => {
            let _ = graph.mixer.set_send_ratio(bus, destination, physical);
        }
        RtTarget::ClipLevel(c) => graph.clips[*c].level.set_target(physical as f32),
        RtTarget::ClipTone(c) => graph.clips[*c].tone = physical as f32,
        RtTarget::ClipGain(c) => graph.clips[*c].gain.set_target(physical as f32),
        RtTarget::ClipPan(c) => graph.clips[*c].pan.set_target(physical as f32),
        RtTarget::ClipRate(c) => graph.clips[*c].automation_rate = physical,
    }
}
