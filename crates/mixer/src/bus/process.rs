//! Per-block render path of the mixer engine. Everything here is RT-safe:
//! no allocation, no locks, no logging; all buffers were preallocated by
//! `MixerEngine::build`.

use oxitone_dsp::gain_pan::equal_power_gains;
use oxitone_graph::abi::ProcessContext;

use super::{Bus, ChannelInput, MixerEngine};

impl MixerEngine {
    /// Render one block. `inputs` are summed into their buses first (in
    /// slice order), then buses run in topological order. RT-safe: no
    /// allocation, no locks. Output is the Master fader output.
    pub fn process_block(
        &mut self,
        inputs: &[ChannelInput<'_>],
        out_left: &mut [f32],
        out_right: &mut [f32],
    ) {
        self.process_block_with(
            inputs.iter().map(|input| ChannelInput {
                bus_id: input.bus_id,
                left: input.left,
                right: input.right,
            }),
            out_left,
            out_right,
        );
    }

    /// Iterator form of [`MixerEngine::process_block`] (additive entry
    /// point for hosts that assemble inputs without a slice; identical
    /// semantics and summation order — inputs are consumed in iteration
    /// order, which the caller fixes as part of the deterministic order).
    pub fn process_block_with<'a>(
        &mut self,
        inputs: impl IntoIterator<Item = ChannelInput<'a>>,
        out_left: &mut [f32],
        out_right: &mut [f32],
    ) {
        let frames = out_left.len().min(self.max_block);
        for bus in &mut self.buses {
            bus.sum_l[..frames].fill(0.0);
            bus.sum_r[..frames].fill(0.0);
            bus.sc_l[..frames].fill(0.0);
            bus.sc_r[..frames].fill(0.0);
        }
        for input in inputs {
            if let Some(&index) = self.index.get(input.bus_id) {
                let bus = &mut self.buses[index];
                for n in 0..frames {
                    bus.sum_l[n] += input.left[n];
                    bus.sum_r[n] += input.right[n];
                }
            }
        }
        let any_solo = self.respect_solo && self.buses.iter().any(|b| b.solo && !b.is_master);
        let sample_rate = self.sample_rate;
        for i in 0..self.buses.len() {
            let tap = self.stem_taps.as_mut().map(|taps| &mut taps[i]);
            process_bus(&mut self.buses, i, frames, any_solo, sample_rate, tap);
        }
        let master = &self.buses[self.buses.len() - 1];
        self.true_peak
            .add_block(&master.out_l[..frames], &master.out_r[..frames]);
        out_left[..frames].copy_from_slice(&master.out_l[..frames]);
        out_right[..frames].copy_from_slice(&master.out_r[..frames]);
    }
}

/// Process one bus and deliver its sends. `buses[i]`'s destinations all
/// come after `i` (topological order), so `split_at_mut` hands both over
/// safely. RT-safe.
fn process_bus(
    buses: &mut [Bus],
    i: usize,
    frames: usize,
    any_solo: bool,
    sample_rate: f64,
    stem_tap: Option<&mut [Vec<f32>; 2]>,
) {
    // Insert chain, ping-ponging between the sum and work buffers.
    {
        let bus = &mut buses[i];
        let mut in_sum = true;
        for slot in &mut bus.inserts {
            let (input_l, input_r, output_l, output_r) = if in_sum {
                (&bus.sum_l, &bus.sum_r, &mut bus.work_l, &mut bus.work_r)
            } else {
                (&bus.work_l, &bus.work_r, &mut bus.sum_l, &mut bus.sum_r)
            };
            let sidechain: Option<[&[f32]; 2]> = if slot.accepts_sidechain {
                Some([&bus.sc_l[..frames], &bus.sc_r[..frames]])
            } else {
                None
            };
            slot.pending.with_events(|events| {
                let inputs: [&[f32]; 2] = [&input_l[..frames], &input_r[..frames]];
                let mut outputs: [&mut [f32]; 2] =
                    [&mut output_l[..frames], &mut output_r[..frames]];
                let mut ctx = ProcessContext {
                    frames,
                    sample_rate,
                    inputs: &inputs,
                    outputs: &mut outputs,
                    note_events: &[],
                    parameter_events: events,
                    sidechain: sidechain.as_ref().map(|sc| &sc[..]),
                };
                slot.instance.process(&mut ctx);
            });
            slot.dry_l[..frames].fill(0.0);
            slot.dry_r[..frames].fill(0.0);
            slot.dry_delay.process_add(
                &input_l[..frames],
                &input_r[..frames],
                1.0,
                &mut slot.dry_l[..frames],
                &mut slot.dry_r[..frames],
            );
            for n in 0..frames {
                let smoothed = slot.mix.next_sample();
                let mix = if slot.bypass { 0.0 } else { smoothed };
                output_l[n] = output_l[n] * mix + slot.dry_l[n] * (1.0 - mix);
                output_r[n] = output_r[n] * mix + slot.dry_r[n] * (1.0 - mix);
            }
            in_sum = !in_sum;
        }
        if !in_sum {
            bus.sum_l[..frames].copy_from_slice(&bus.work_l[..frames]);
            bus.sum_r[..frames].copy_from_slice(&bus.work_r[..frames]);
        }
    }

    // Pre-fader send taps (post-insert signal).
    deliver_sends(buses, i, frames, true);

    // Fader: level (smoothed), equal-power balance, mute/solo.
    {
        let bus = &mut buses[i];
        let audible = !(bus.mute || (any_solo && !bus.solo && !bus.is_master));
        let (gain_l, gain_r) = equal_power_gains(bus.balance);
        for n in 0..frames {
            let level = bus.level.next_sample();
            let gain = if audible { level } else { 0.0 };
            bus.out_l[n] = bus.sum_l[n] * gain * gain_l;
            bus.out_r[n] = bus.sum_r[n] * gain * gain_r;
        }
    }

    // Post-fader sends and the masterSendRatio route to Master.
    deliver_sends(buses, i, frames, false);
    if !buses[i].is_master {
        let master = buses.len() - 1;
        let (head, tail) = buses.split_at_mut(master);
        let bus = &mut head[i];
        let ratio = bus.master_send_ratio;
        match stem_tap {
            Some(tap) => {
                let [tap_l, tap_r] = tap;
                tap_l[..frames].fill(0.0);
                tap_r[..frames].fill(0.0);
                bus.master_delay.process_add(
                    &bus.out_l[..frames],
                    &bus.out_r[..frames],
                    ratio,
                    &mut tap_l[..frames],
                    &mut tap_r[..frames],
                );
                for n in 0..frames {
                    tail[0].sum_l[n] += tap_l[n];
                    tail[0].sum_r[n] += tap_r[n];
                }
            }
            None => {
                bus.master_delay.process_add(
                    &bus.out_l[..frames],
                    &bus.out_r[..frames],
                    ratio,
                    &mut tail[0].sum_l[..frames],
                    &mut tail[0].sum_r[..frames],
                );
            }
        }
    }

    // Meter on the fader output; Master adds the true-peak estimate.
    {
        let bus = &mut buses[i];
        bus.meter
            .add_block(&bus.out_l[..frames], &bus.out_r[..frames]);
    }
}

/// Deliver all sends of `buses[i]` for one tap point. Sidechain sends tap
/// post-insert/pre-fader by convention and feed only the destination's
/// detector buffers. RT-safe.
fn deliver_sends(buses: &mut [Bus], i: usize, frames: usize, pre_fader: bool) {
    let send_count = buses[i].sends.len();
    for k in 0..send_count {
        let slot_pre = buses[i].sends[k].pre_fader;
        let is_sidechain = buses[i].sends[k].sidechain;
        // Sidechain taps are always pre-fader (post-insert).
        if if is_sidechain {
            !pre_fader
        } else {
            slot_pre != pre_fader
        } {
            continue;
        }
        let dest = buses[i].sends[k].dest_index;
        debug_assert!(dest > i, "send destinations follow sources");
        let (head, tail) = buses.split_at_mut(dest);
        let bus = &mut head[i];
        let slot = &mut bus.sends[k];
        let (tap_l, tap_r) = if slot.pre_fader || slot.sidechain {
            (&bus.sum_l, &bus.sum_r)
        } else {
            (&bus.out_l, &bus.out_r)
        };
        let (out_l, out_r) = if slot.sidechain {
            (&mut tail[0].sc_l, &mut tail[0].sc_r)
        } else {
            (&mut tail[0].sum_l, &mut tail[0].sum_r)
        };
        slot.delay.process_add(
            &tap_l[..frames],
            &tap_r[..frames],
            slot.ratio,
            &mut out_l[..frames],
            &mut out_r[..frames],
        );
    }
}
