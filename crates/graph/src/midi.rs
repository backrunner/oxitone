//! Main MIDI bus capability and deterministic Channel dependency compilation.
use crate::PluginRegistry;
use oxitone_core::{wire::ChannelSpec, OxitoneError};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_MIDI_EVENTS: usize = 256;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MidiMessage {
    Channel([u8; 3]),
    SysEx(oxitone_core::midi_bytes::MidiBytes),
}
impl Default for MidiMessage {
    fn default() -> Self {
        Self::Channel([0; 3])
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MidiEvent {
    pub frame_offset: u32,
    pub message: MidiMessage,
}
impl MidiEvent {
    pub fn valid(&self, frames: usize, payload: &[u8]) -> bool {
        if self.frame_offset as usize >= frames {
            return false;
        }
        let [status, a, b] = match self.message {
            MidiMessage::SysEx(data) => return data.get(payload).is_some(),
            MidiMessage::Channel(message) => message,
        };
        (0x80..=0xef).contains(&status)
            && a < 128
            && b < 128
            && (!matches!(status & 0xf0, 0xc0 | 0xd0) || b == 0)
    }
}

pub struct MidiRoute {
    /// None addresses the source instrument; Some addresses a Channel insert.
    pub insert: Option<usize>,
    pub destinations: Vec<usize>,
}
pub struct MidiRouting {
    pub order: Vec<usize>,
    pub routes: Vec<Vec<MidiRoute>>,
}
fn invalid(message: &str) -> OxitoneError {
    OxitoneError::new("InvalidProject", message)
}

/// Autonomous output-only generators may own a Channel, but must not silently discard a score.
pub(crate) fn validate_note_inputs(
    channels: &[&ChannelSpec],
    registry: &PluginRegistry,
    scheduler: &oxitone_transport::scheduler::Scheduler,
) -> Result<(), OxitoneError> {
    for channel in channels {
        let reference = &channel.instrument;
        if registry
            .lookup_instance(
                &reference.plugin_id,
                &reference.plugin_version,
                reference.instance_id.as_deref(),
            )
            .is_some_and(|p| p.output_bus_count() == 0 && p.midi_output() && !p.midi_input())
            && scheduler
                .events_in_range(0, u64::MAX)
                .iter()
                .any(|event| event.channel_id == channel.id)
        {
            return Err(OxitoneError::new(
                "PluginCapabilityUnsupported",
                "MIDI generator has no input for authored notes",
            ));
        }
    }
    Ok(())
}

/// Channel indices always refer to ID order; MIDI processing order never changes summation/bindings.
pub fn compile(
    channels: &[ChannelSpec],
    registry: Option<&PluginRegistry>,
) -> Result<Option<MidiRouting>, OxitoneError> {
    if channels
        .iter()
        .all(|c| c.midi_routes.as_ref().is_none_or(|r| r.is_empty()))
    {
        return Ok(None);
    }
    let mut channels: Vec<_> = channels.iter().collect();
    channels.sort_by(|a, b| a.id.cmp(&b.id));
    let ids: BTreeMap<_, _> = channels
        .iter()
        .enumerate()
        .map(|(i, c)| (c.id.as_str(), i))
        .collect();
    let mut outgoing = vec![BTreeSet::new(); channels.len()];
    let mut incoming = vec![0; channels.len()];
    let mut routes = Vec::with_capacity(channels.len());
    for (source, channel) in channels.iter().enumerate() {
        let mut channel_routes = Vec::new();
        for (instance, destinations) in channel.midi_routes.iter().flatten() {
            let insert = channel
                .effect_chain
                .iter()
                .position(|e| e.instance_id.as_ref() == Some(instance));
            let (plugin_id, plugin_version) = if let Some(index) = insert {
                let r = &channel.effect_chain[index];
                (&r.plugin_id, &r.plugin_version)
            } else if channel.instrument.instance_id.as_ref() == Some(instance) {
                (
                    &channel.instrument.plugin_id,
                    &channel.instrument.plugin_version,
                )
            } else {
                return Err(invalid("MIDI route source is not owned by its Channel"));
            };
            if destinations.is_empty() || destinations.len() > 256 {
                return Err(invalid("MIDI route requires 1..256 targets"));
            }
            if let Some(registry) = registry {
                if !registry
                    .lookup_instance(plugin_id, plugin_version, Some(instance))
                    .is_some_and(|p| p.midi_output())
                {
                    return Err(OxitoneError::new(
                        "PluginCapabilityUnsupported",
                        "MIDI route source has no MIDI output",
                    ));
                }
            }
            let mut targets = BTreeSet::new();
            for destination in destinations {
                let target = *ids
                    .get(destination.as_str())
                    .ok_or_else(|| invalid("unknown MIDI route destination"))?;
                if source == target || !targets.insert(target) {
                    return Err(invalid("self or duplicate MIDI route"));
                }
                if let Some(registry) = registry {
                    let reference = &channels[target].instrument;
                    if !registry
                        .lookup_instance(
                            &reference.plugin_id,
                            &reference.plugin_version,
                            reference.instance_id.as_deref(),
                        )
                        .is_some_and(|p| p.midi_input())
                    {
                        return Err(OxitoneError::new(
                            "PluginCapabilityUnsupported",
                            "MIDI destination requires a native MIDI instrument",
                        ));
                    }
                }
                if outgoing[source].insert(target) {
                    incoming[target] += 1;
                }
            }
            channel_routes.push(MidiRoute {
                insert,
                destinations: targets.into_iter().collect(),
            });
        }
        // Instance declaration order is stable even when ID spelling changes.
        channel_routes.sort_by_key(|r| r.insert.map_or(0, |i| i + 1));
        routes.push(channel_routes);
    }
    let mut ready: BTreeSet<_> = incoming
        .iter()
        .enumerate()
        .filter_map(|(i, &n)| (n == 0).then_some(i))
        .collect();
    let mut order = Vec::with_capacity(channels.len());
    while let Some(source) = ready.pop_first() {
        order.push(source);
        for &target in &outgoing[source] {
            incoming[target] -= 1;
            if incoming[target] == 0 {
                ready.insert(target);
            }
        }
    }
    if order.len() != channels.len() {
        return Err(invalid("MIDI routing cycle"));
    }
    Ok(Some(MidiRouting { order, routes }))
}
