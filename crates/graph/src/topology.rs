//! Mixer routing topology (02-domain-spec.md §Mixer 与 routing). Send edges —
//! including `sidechain: true` detector edges — form a DAG over mixer
//! channels; any feedback cycle is `InvalidProject` with the full cycle path.
//! The compiler reuses [`MixerRouting`] for processing order and PDC.

use std::collections::{BTreeMap, BTreeSet};

use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::wire::{EntityId, MixerChannelSpec};

/// Reserved ID of the implicit, undeletable Master bus. A `MixerChannelSpec`
/// with this ID *is* the explicit Master declaration (inserts/meter config);
/// it must not carry sends or `masterSendRatio`.
pub const MASTER_MIXER_CHANNEL_ID: &str = "mix_master";

/// One routing edge between mixer buses. `sidechain` edges drive only the
/// destination's detector inputs but participate in the topology equally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutingEdge {
    pub source: EntityId,
    pub destination: EntityId,
    pub sidechain: bool,
}

/// Deterministic routing result for a validated mixer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MixerRouting {
    /// Processing order: every bus appears after all buses feeding it; the
    /// Master bus is always last.
    pub order: Vec<EntityId>,
    /// All edges (sends, sidechain detector feeds, and masterSendRatio routes
    /// to Master), sorted by (source, destination, sidechain).
    pub edges: Vec<RoutingEdge>,
}

/// Build the routing for `channels`. Assumes reference checks already passed
/// (dangling destinations are ignored here and rejected by the validator).
/// Deterministic: the same specs always yield the same order regardless of
/// declaration order.
pub fn build_mixer_routing(channels: &[MixerChannelSpec]) -> Result<MixerRouting, OxitoneError> {
    let ids: BTreeSet<&str> = channels.iter().map(|c| c.id.as_str()).collect();
    let mut edges: Vec<RoutingEdge> = Vec::new();
    for channel in channels {
        if channel.id == MASTER_MIXER_CHANNEL_ID {
            continue;
        }
        for send in &channel.sends {
            if ids.contains(send.destination_id.as_str()) {
                edges.push(RoutingEdge {
                    source: channel.id.clone(),
                    destination: send.destination_id.clone(),
                    sidechain: send.sidechain.unwrap_or(false),
                });
            }
        }
        if channel.master_send_ratio.unwrap_or(1.0) > 0.0 {
            edges.push(RoutingEdge {
                source: channel.id.clone(),
                destination: MASTER_MIXER_CHANNEL_ID.to_string(),
                sidechain: false,
            });
        }
    }
    edges.sort_by(|a, b| {
        (&a.source, &a.destination, a.sidechain).cmp(&(&b.source, &b.destination, b.sidechain))
    });

    let mut adjacency: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    let mut indegree: BTreeMap<&str, usize> = BTreeMap::new();
    for id in &ids {
        if *id != MASTER_MIXER_CHANNEL_ID {
            indegree.insert(id, 0);
        }
    }
    for edge in &edges {
        if edge.destination == MASTER_MIXER_CHANNEL_ID || edge.source == MASTER_MIXER_CHANNEL_ID {
            continue;
        }
        if adjacency
            .entry(edge.source.as_str())
            .or_default()
            .insert(edge.destination.as_str())
        {
            *indegree.entry(edge.destination.as_str()).or_insert(0) += 1;
        }
    }

    let mut ready: BTreeSet<&str> = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(id, _)| *id)
        .collect();
    let mut order: Vec<EntityId> = Vec::with_capacity(indegree.len() + 1);
    while let Some(&node) = ready.iter().next() {
        ready.remove(node);
        order.push(node.to_string());
        if let Some(targets) = adjacency.get(node) {
            for target in targets {
                let degree = indegree
                    .get_mut(target)
                    .expect("send destination is a topology node");
                *degree -= 1;
                if *degree == 0 {
                    ready.insert(target);
                }
            }
        }
    }

    if order.len() < indegree.len() {
        let remaining: BTreeSet<&str> = indegree
            .iter()
            .filter(|(_, degree)| **degree > 0)
            .map(|(id, _)| *id)
            .collect();
        let cycle = find_cycle(&remaining, &adjacency);
        return Err(OxitoneError::with_path(
            codes::INVALID_PROJECT,
            format!("mixer routing cycle detected: {}", cycle.join(" -> ")),
            "$.mixerChannels",
        ));
    }

    order.push(MASTER_MIXER_CHANNEL_ID.to_string());
    Ok(MixerRouting { order, edges })
}

/// Deterministically extract one full cycle among the unresolved nodes.
fn find_cycle<'a>(
    remaining: &BTreeSet<&'a str>,
    adjacency: &BTreeMap<&'a str, BTreeSet<&'a str>>,
) -> Vec<&'a str> {
    let mut state: BTreeMap<&str, u8> = BTreeMap::new(); // 0=unvisited, 1=on stack, 2=done
    let mut stack: Vec<&'a str> = Vec::new();
    for &start in remaining {
        if state.get(start).copied().unwrap_or(0) != 0 {
            continue;
        }
        let mut work = vec![(start, adjacency.get(start).into_iter().flatten().copied())];
        state.insert(start, 1);
        stack.push(start);
        while let Some((node, mut targets)) = work.pop() {
            let next = targets
                .by_ref()
                .find(|t| remaining.contains(t) && state.get(t).copied().unwrap_or(0) != 2);
            match next {
                Some(target) if state.get(target).copied() == Some(1) => {
                    let at = stack.iter().position(|n| *n == target).expect("on stack");
                    let mut cycle: Vec<&str> = stack[at..].to_vec();
                    cycle.push(target);
                    return cycle;
                }
                Some(target) => {
                    state.insert(target, 1);
                    stack.push(target);
                    work.push((node, targets));
                    work.push((target, adjacency.get(target).into_iter().flatten().copied()));
                }
                None => {
                    state.insert(node, 2);
                    stack.pop();
                }
            }
        }
    }
    remaining.iter().copied().collect()
}
