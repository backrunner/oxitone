//! Channel execution preserves ID summation order and uses MIDI dependency order only for processing.
use super::*;
use oxitone_core::OxitoneError;
use oxitone_graph::{
    execution::{Execution, ProcessPosition},
    midi::MAX_MIDI_EVENTS,
};

impl RenderGraph {
    pub(super) fn process_channels<E: Execution>(
        &mut self,
        frames: usize,
        cur: u64,
        position: &ProcessPosition,
    ) -> Result<(), OxitoneError> {
        let any_solo = self.respect_solo && self.channels.iter().any(|c| c.solo);
        if self.plan.midi_routing.is_none() {
            for channel in &mut self.channels {
                channel.process_instrument::<E>(frames, self.sample_rate, position)?;
            }
            self.mix_sample_clips(frames, cur);
            for channel in &mut self.channels {
                let audible = !(channel.mute || (any_solo && !channel.solo));
                channel.process_chain::<E>(frames, self.sample_rate, audible, position)?;
            }
            return Ok(());
        }
        // Render each clip/smoother once. Save a channel's clip mix in reusable scratch before
        // its instrument overwrites dry storage, then add it back before inserts.
        for channel in &mut self.channels {
            channel.dry_l[..frames].fill(0.);
            channel.dry_r[..frames].fill(0.);
        }
        self.mix_sample_clips(frames, cur);
        let routing = self.plan.midi_routing.as_ref().unwrap();
        for &source in &routing.order {
            let channel = &mut self.channels[source];
            std::mem::swap(&mut channel.dry_l, &mut self.midi_clip_l);
            std::mem::swap(&mut channel.dry_r, &mut self.midi_clip_r);
            channel.process_instrument::<E>(frames, self.sample_rate, position)?;
            for i in 0..frames {
                channel.dry_l[i] += self.midi_clip_l[i];
                channel.dry_r[i] += self.midi_clip_r[i];
            }
            let audible = !(channel.mute || (any_solo && !channel.solo));
            channel.process_chain::<E>(frames, self.sample_rate, audible, position)?;
            for route in &routing.routes[source] {
                let channel = &self.channels[source];
                let (events, payload) = match route.insert {
                    Some(index) => channel.inserts[index].instance.output_midi(),
                    None => channel.instrument.output_midi(),
                };
                if events.len() > MAX_MIDI_EVENTS
                    || payload.len() > oxitone_core::midi_bytes::MAX_MIDI_PAYLOAD_BYTES
                    || events.iter().any(|e| !e.valid(frames, payload))
                {
                    return Err(OxitoneError::new(
                        "RealtimeFault",
                        "invalid MIDI source output",
                    ));
                }
                let count = events.len();
                if count == 0 {
                    continue;
                }
                for &destination in &route.destinations {
                    let [source, destination] = self
                        .channels
                        .get_disjoint_mut([source, destination])
                        .unwrap();
                    let (events, payload) = match route.insert {
                        Some(index) => source.inserts[index].instance.output_midi(),
                        None => source.instrument.output_midi(),
                    };
                    destination.instrument.stage_midi(events, payload)?;
                }
            }
        }
        Ok(())
    }

    fn mix_sample_clips(&mut self, frames: usize, cur: u64) {
        // Sample clips: render dry, tone tilt, then mix into each owning
        // channel's pre-insert buffer with smoothed level/gain/pan.
        let tempo = &self.plan.tempo;
        let RenderGraph {
            clips,
            channels,
            clip_l,
            clip_r,
            clip_gain,
            clip_pan,
            ..
        } = self;
        for clip in clips.iter_mut() {
            if !clip.render(cur, frames, tempo, clip_l, clip_r) {
                continue;
            }
            clip.apply_tone(&mut clip_l[..frames], &mut clip_r[..frames]);
            for i in 0..frames {
                clip_gain[i] = clip.level.next_sample() * clip.gain.next_sample();
                clip_pan[i] = clip.pan.next_sample();
            }
            for &ch in &clip.channels {
                let node = &mut channels[ch];
                for i in 0..frames {
                    let (gain_l, gain_r) = equal_power_gains(clip_pan[i]);
                    node.dry_l[i] += clip_l[i] * clip_gain[i] * gain_l;
                    node.dry_r[i] += clip_r[i] * clip_gain[i] * gain_r;
                }
            }
        }
    }
}
