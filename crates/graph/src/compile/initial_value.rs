//! Snapshot value restored outside Playlist automation spans.
use super::BindingTarget;
use crate::insert_params::{parse, InsertParameter};
use oxitone_core::wire::{ParameterMapping, ParameterSpec, ProjectSnapshot};

pub(super) fn normalized(
    snapshot: &ProjectSnapshot,
    target: &BindingTarget,
    spec: &ParameterSpec,
) -> f64 {
    let mut channels: Vec<_> = snapshot.channels.iter().collect();
    channels.sort_by(|a, b| a.id.cmp(&b.id));
    let mut clips: Vec<_> = snapshot.sample_clips.iter().collect();
    clips.sort_by(|a, b| a.id.cmp(&b.id));
    let bus = |id: &str| snapshot.mixer_channels.iter().find(|m| m.id == id);
    let value = match target {
        BindingTarget::ChannelLevel(i) => Some(channels[*i].level),
        BindingTarget::ChannelPan(i) => Some(channels[*i].pan),
        BindingTarget::ChannelMute(i) => Some(f64::from(channels[*i].mute.unwrap_or(false))),
        BindingTarget::ChannelSwing(i) => Some(channels[*i].swing.unwrap_or(0.)),
        BindingTarget::InstrumentParam {
            channel,
            parameter_id,
        } => channels[*channel]
            .instrument
            .parameters
            .get(parameter_id)
            .copied(),
        BindingTarget::InsertMix { channel, insert } => {
            Some(channels[*channel].effect_chain[*insert].mix.unwrap_or(1.))
        }
        BindingTarget::InsertBypass { channel, insert } => Some(f64::from(
            channels[*channel].effect_chain[*insert]
                .bypass
                .unwrap_or(false),
        )),
        BindingTarget::MixerLevel(id) => bus(id).map(|m| m.level),
        BindingTarget::MixerBalance(id) => bus(id).map(|m| m.balance),
        BindingTarget::MixerMute(id) => bus(id).map(|m| f64::from(m.mute.unwrap_or(false))),
        BindingTarget::MixerMasterSendRatio(id) => {
            bus(id).map(|m| m.master_send_ratio.unwrap_or(1.))
        }
        BindingTarget::MixerSendRatio {
            bus: id,
            destination,
        } => bus(id).and_then(|m| {
            m.sends
                .iter()
                .find(|s| &s.destination_id == destination)
                .map(|s| s.ratio)
        }),
        BindingTarget::SampleClipGain(i) => Some(clips[*i].gain.unwrap_or(1.)),
        BindingTarget::SampleClipPan(i) => Some(clips[*i].pan.unwrap_or(0.)),
        BindingTarget::SampleClipRate(_) | BindingTarget::SampleClipLevel(_) => Some(1.),
        BindingTarget::SampleClipTone(_) => Some(0.),
        BindingTarget::EffectInsert {
            entity_id,
            parameter_id,
        } => {
            let effects = snapshot
                .channels
                .iter()
                .find(|c| &c.id == entity_id)
                .map(|c| &c.effect_chain)
                .or_else(|| bus(entity_id).map(|m| &m.inserts));
            effects.and_then(|effects| {
                let (index, param) = parse(parameter_id)?;
                let effect = effects.get(index)?;
                match param {
                    InsertParameter::Mix => Some(effect.mix.unwrap_or(1.)),
                    InsertParameter::Bypass => Some(f64::from(effect.bypass.unwrap_or(false))),
                    InsertParameter::Plugin(id) => effect.parameters.get(id).copied(),
                }
            })
        }
    }
    .unwrap_or(spec.default)
    .clamp(spec.min, spec.max);
    if spec.max == spec.min {
        return 0.;
    }
    match spec.mapping {
        Some(ParameterMapping::Log) => (value / spec.min).ln() / (spec.max / spec.min).ln(),
        _ => (value - spec.min) / (spec.max - spec.min),
    }
}
