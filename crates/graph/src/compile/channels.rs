//! Channel plans and deterministic physical output routing, after snapshot validation.
use super::{AutomationBinding, BindingTarget};
use oxitone_core::wire::{ChannelSpec, EffectRef, EntityId, InstrumentRef};
use std::collections::BTreeMap;

/// One mixer-bound channel of the arrangement (sorted by channel ID).
#[derive(Debug, Clone)]
pub struct ChannelPlan {
    pub midi_routes: BTreeMap<EntityId, Vec<EntityId>>,
    pub id: EntityId,
    pub mixer_channel_id: EntityId,
    pub output_routes: BTreeMap<usize, EntityId>,
    pub level: f64,
    pub pan: f64,
    /// Static swing; a `ChannelSwing` binding (if any) replaces it.
    pub swing: f64,
    pub mute: bool,
    pub solo: bool,
    pub instrument: InstrumentRef,
    pub effect_chain: Vec<EffectRef>,
    /// Index into `RenderPlan::bindings` for the automated swing lane.
    pub swing_binding: Option<usize>,
}

pub(super) fn compile(
    channel_specs: &[&ChannelSpec],
    bindings: &[AutomationBinding],
) -> (Vec<ChannelPlan>, bool) {
    let mut has_swing = channel_specs.iter().any(|c| c.swing.unwrap_or(0.0) != 0.0);
    let mut channels = Vec::with_capacity(channel_specs.len());
    for (i, spec) in channel_specs.iter().enumerate() {
        let swing_binding = bindings
            .iter()
            .position(|b| matches!(&b.target, BindingTarget::ChannelSwing(c) if *c == i));
        has_swing |= swing_binding.is_some();
        channels.push(ChannelPlan {
            midi_routes: spec.midi_routes.clone().unwrap_or_default(),
            id: spec.id.clone(),
            mixer_channel_id: spec.mixer_channel_id.clone(),
            output_routes: spec
                .output_routes
                .iter()
                .flatten()
                .map(|(index, destination)| {
                    (
                        index.parse().expect("validated output bus"),
                        destination.clone(),
                    )
                })
                .collect(),
            level: spec.level,
            pan: spec.pan,
            swing: spec.swing.unwrap_or(0.0),
            mute: spec.mute.unwrap_or(false),
            solo: spec.solo.unwrap_or(false),
            instrument: spec.instrument.clone(),
            effect_chain: spec.effect_chain.clone(),
            swing_binding,
        });
    }

    (channels, has_swing)
}
