//! Plugin delay compensation (02-domain-spec.md §PDC, 03-audio-runtime-spec
//! §DSP 处理顺序). Every path converging on a bus is aligned to the longest
//! path with a preallocated integer compensation delay on each incoming
//! edge; sidechain detector edges are aligned by the same rule. The
//! compensation delays are ordinary preallocated delay nodes participating
//! in the same topological order.

use std::collections::BTreeMap;

use oxitone_core::wire::EntityId;
use oxitone_dsp::ftz::flush_denormal;
use oxitone_graph::topology::MixerRouting;

/// Static PDC plan for one compiled mixer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdcPlan {
    /// Compensation delay per routing edge, keyed by (source, destination,
    /// sidechain) — the same key order `MixerRouting::edges` is sorted by.
    pub edge_delays: BTreeMap<(EntityId, EntityId, bool), u64>,
    /// Latency at each bus's output taps (insert-chain latency included).
    pub bus_output_latency: BTreeMap<EntityId, u64>,
    /// Total graph latency: the Master bus output latency.
    pub graph_latency_frames: u64,
}

/// Compute the PDC plan. `insert_latency` carries each bus's summed insert
/// `latency_frames`; buses missing from the map have zero internal latency.
///
/// In routing order: a bus's input latency is the maximum output latency of
/// its sources; each edge gets `input_latency[dest] - output_latency[src]`
/// compensation frames; a bus's output latency adds its insert latency.
pub fn plan_pdc(routing: &MixerRouting, insert_latency: &BTreeMap<EntityId, u64>) -> PdcPlan {
    let mut incoming: BTreeMap<&str, Vec<&oxitone_graph::topology::RoutingEdge>> = BTreeMap::new();
    for edge in &routing.edges {
        incoming
            .entry(edge.destination.as_str())
            .or_default()
            .push(edge);
    }
    let mut input_latency: BTreeMap<EntityId, u64> = BTreeMap::new();
    let mut output_latency: BTreeMap<EntityId, u64> = BTreeMap::new();
    let mut edge_delays: BTreeMap<(EntityId, EntityId, bool), u64> = BTreeMap::new();
    for bus in &routing.order {
        let in_latency = incoming
            .get(bus.as_str())
            .into_iter()
            .flatten()
            .map(|edge| output_latency.get(&edge.source).copied().unwrap_or(0))
            .max()
            .unwrap_or(0);
        input_latency.insert(bus.clone(), in_latency);
        if let Some(edges) = incoming.get(bus.as_str()) {
            for edge in edges {
                let source_latency = output_latency.get(&edge.source).copied().unwrap_or(0);
                edge_delays.insert(
                    (
                        edge.source.clone(),
                        edge.destination.clone(),
                        edge.sidechain,
                    ),
                    in_latency - source_latency,
                );
            }
        }
        output_latency.insert(
            bus.clone(),
            in_latency + insert_latency.get(bus).copied().unwrap_or(0),
        );
    }
    let graph_latency_frames = routing
        .order
        .last()
        .and_then(|master| output_latency.get(master))
        .copied()
        .unwrap_or(0);
    PdcPlan {
        edge_delays,
        bus_output_latency: output_latency,
        graph_latency_frames,
    }
}

/// Stereo integer compensation delay. Preallocated at compile time;
/// `process_add` is RT-safe. The line is always fed, even when the send
/// ratio is zero, so alignment survives ratio automation.
pub struct DelayLine {
    left: Vec<f32>,
    right: Vec<f32>,
    pos: usize,
    delay: usize,
}

impl DelayLine {
    /// Allocate for up to `max_delay` frames plus one block.
    pub fn new(max_delay: usize, max_block: usize) -> Self {
        let len = max_delay + max_block + 1;
        Self {
            left: vec![0.0; len],
            right: vec![0.0; len],
            pos: 0,
            delay: 0,
        }
    }

    /// Control thread: set the compensation delay in frames.
    pub fn set_delay(&mut self, delay: usize) {
        debug_assert!(delay < self.left.len());
        self.delay = delay.min(self.left.len() - 1);
    }

    pub fn delay_frames(&self) -> usize {
        self.delay
    }

    pub fn reset(&mut self) {
        self.left.fill(0.0);
        self.right.fill(0.0);
        self.pos = 0;
    }

    /// Push the input and accumulate the delayed signal × `gain` into
    /// `out_left`/`out_right`. RT-safe.
    pub fn process_add(
        &mut self,
        in_left: &[f32],
        in_right: &[f32],
        gain: f32,
        out_left: &mut [f32],
        out_right: &mut [f32],
    ) {
        let len = self.left.len();
        let delay = self.delay;
        for n in 0..in_left.len() {
            // delay == 0 is a same-block passthrough; the ring is still fed
            // so a later control-side delay change stays glitch free.
            let (yl, yr) = if delay == 0 {
                (in_left[n], in_right[n])
            } else {
                let read = (self.pos + len - delay) % len;
                (self.left[read], self.right[read])
            };
            self.left[self.pos] = flush_denormal(in_left[n]);
            self.right[self.pos] = flush_denormal(in_right[n]);
            self.pos = (self.pos + 1) % len;
            out_left[n] += yl * gain;
            out_right[n] += yr * gain;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxitone_core::wire::{MixerChannelSpec, SendSpec};
    use oxitone_graph::topology::build_mixer_routing;

    fn channel(id: &str, destinations: &[&str]) -> MixerChannelSpec {
        MixerChannelSpec {
            id: id.to_string(),
            name: None,
            level: 1.0,
            balance: 0.0,
            master_send_ratio: Some(1.0),
            inserts: vec![],
            sends: destinations
                .iter()
                .map(|destination| SendSpec {
                    destination_id: destination.to_string(),
                    ratio: 1.0,
                    pre_fader: None,
                    sidechain: None,
                })
                .collect(),
            mute: None,
            solo: None,
        }
    }

    #[test]
    fn parallel_paths_align_to_the_longest() {
        // a -> c with 10 frames of insert latency, b -> c with none.
        let routing = build_mixer_routing(&[
            channel("a", &["c"]),
            channel("b", &["c"]),
            channel("c", &[]),
        ])
        .unwrap();
        let mut latencies = BTreeMap::new();
        latencies.insert("a".to_string(), 10u64);
        let plan = plan_pdc(&routing, &latencies);
        // Edge b->c is delayed by 10 so both paths land together.
        assert_eq!(
            plan.edge_delays[&("b".to_string(), "c".to_string(), false)],
            10
        );
        assert_eq!(
            plan.edge_delays[&("a".to_string(), "c".to_string(), false)],
            0
        );
        assert_eq!(plan.bus_output_latency["c"], 10);
        assert_eq!(plan.graph_latency_frames, 10);
    }

    #[test]
    fn delay_line_shifts_by_exact_frames() {
        let mut line = DelayLine::new(8, 4);
        line.set_delay(3);
        let input_l = [1.0f32, 0.0, 0.0, 0.0];
        let input_r = [0.5f32, 0.0, 0.0, 0.0];
        let mut out_l = [0.0f32; 4];
        let mut out_r = [0.0f32; 4];
        line.process_add(&input_l, &input_r, 1.0, &mut out_l, &mut out_r);
        assert_eq!(out_l, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(out_r[3], 0.5);
    }
}
