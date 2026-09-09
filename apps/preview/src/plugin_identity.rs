//! Instance identities keep an open panel attached across insert reordering and replacement.
use crate::{model::ViewProject, plugin_details::DetailTarget};
pub fn key(project: &ViewProject, target: &DetailTarget) -> String {
    let s = &project.snapshot;
    let id = match target {
        DetailTarget::Instrument(owner) => s
            .channels
            .iter()
            .find(|c| &c.id == owner)
            .and_then(|c| c.instrument.instance_id.clone()),
        DetailTarget::ChannelInsert(owner, slot) => s
            .channels
            .iter()
            .find(|c| &c.id == owner)
            .and_then(|c| c.effect_chain.get(*slot))
            .and_then(|e| e.instance_id.clone()),
        DetailTarget::BusInsert(owner, slot) => s
            .mixer_channels
            .iter()
            .find(|c| &c.id == owner)
            .and_then(|c| c.inserts.get(*slot))
            .and_then(|e| e.instance_id.clone()),
    };
    id.unwrap_or_else(|| format!("legacy:{target:?}"))
}
pub fn follow(
    project: &ViewProject,
    previous: &DetailTarget,
    identity: &str,
) -> Option<DetailTarget> {
    if identity.starts_with("legacy:") {
        return Some(previous.clone());
    }
    for channel in &project.snapshot.channels {
        if channel.instrument.instance_id.as_deref() == Some(identity) {
            return Some(DetailTarget::Instrument(channel.id.clone()));
        }
        if let Some(index) = channel
            .effect_chain
            .iter()
            .position(|e| e.instance_id.as_deref() == Some(identity))
        {
            return Some(DetailTarget::ChannelInsert(channel.id.clone(), index));
        }
    }
    for bus in &project.snapshot.mixer_channels {
        if let Some(index) = bus
            .inserts
            .iter()
            .position(|e| e.instance_id.as_deref() == Some(identity))
        {
            return Some(DetailTarget::BusInsert(bus.id.clone(), index));
        }
    }
    None
}
