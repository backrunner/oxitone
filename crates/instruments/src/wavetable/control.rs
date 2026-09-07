//! WavetableSynth control logic: runtime parameter application and
//! poly/mono/legato note handling with glide (02-domain-spec.md §内置
//! WavetableSynth). All methods run inside `process` and are RT-safe.

use super::instance::{WavetableSynthInstance, MAX_HELD};
use super::params as p;

impl WavetableSynthInstance {
    pub(super) fn set_parameter(&mut self, index: usize, value: f64) {
        self.values[index] = value;
        for i in 0..self.pool.capacity() {
            if self.pool.slot(i).is_active() {
                self.pool.slot_mut(i).voice.invalidate_control();
            }
        }
        match index {
            p::LEVEL => self.level.set_target(value as f32),
            p::PAN => self.pan.set_target(value as f32),
            p::OSC_MIX => self.mix.set_target(value as f32),
            p::FILTER_CUTOFF => self.cutoff.set_target(value as f32),
            p::FILTER_RESONANCE => self.resonance.set_target(value as f32),
            p::AMP_ATTACK | p::AMP_DECAY | p::AMP_SUSTAIN | p::AMP_RELEASE => {
                for i in 0..self.pool.capacity() {
                    if !self.pool.slot(i).is_active() {
                        continue;
                    }
                    self.pool.slot_mut(i).voice.amp.set_params(
                        self.values[p::AMP_ATTACK],
                        self.values[p::AMP_DECAY],
                        self.values[p::AMP_SUSTAIN] as f32,
                        self.values[p::AMP_RELEASE],
                    );
                }
            }
            p::FILTER_ENV_ATTACK
            | p::FILTER_ENV_DECAY
            | p::FILTER_ENV_SUSTAIN
            | p::FILTER_ENV_RELEASE => {
                for i in 0..self.pool.capacity() {
                    if !self.pool.slot(i).is_active() {
                        continue;
                    }
                    self.pool.slot_mut(i).voice.fenv.set_params(
                        self.values[p::FILTER_ENV_ATTACK],
                        self.values[p::FILTER_ENV_DECAY],
                        self.values[p::FILTER_ENV_SUSTAIN] as f32,
                        self.values[p::FILTER_ENV_RELEASE],
                    );
                }
            }
            p::OSC_A_WAVETABLE
            | p::OSC_A_PITCH
            | p::OSC_A_UNISON
            | p::OSC_A_DETUNE
            | p::OSC_A_SPREAD
            | p::OSC_B_WAVETABLE
            | p::OSC_B_PITCH
            | p::OSC_B_UNISON
            | p::OSC_B_DETUNE
            | p::OSC_B_SPREAD => {
                for i in 0..self.pool.capacity() {
                    if !self.pool.slot(i).is_active() {
                        continue;
                    }
                    let voice = &mut self.pool.slot_mut(i).voice;
                    voice.osc_a.configure(
                        &self.tables[self.values[p::OSC_A_WAVETABLE] as usize],
                        voice.base_freq,
                        self.values[p::OSC_A_PITCH],
                        self.values[p::OSC_A_UNISON] as usize,
                        self.values[p::OSC_A_DETUNE],
                        self.values[p::OSC_A_SPREAD] as f32,
                    );
                    voice.osc_b.configure(
                        &self.tables[self.values[p::OSC_B_WAVETABLE] as usize],
                        voice.base_freq,
                        self.values[p::OSC_B_PITCH],
                        self.values[p::OSC_B_UNISON] as usize,
                        self.values[p::OSC_B_DETUNE],
                        self.values[p::OSC_B_SPREAD] as f32,
                    );
                }
            }
            _ => {}
        }
    }

    pub(super) fn start_voice(&mut self, slot: usize, pitch: u8, velocity: f32, from_glide: bool) {
        let voice = &mut self.pool.slot_mut(slot).voice;
        if !from_glide {
            voice.reset_playback_state();
        }
        let target = f64::from(pitch);
        if !from_glide || self.values[p::GLIDE] <= 0.0 {
            voice.pitch_semis = target;
            voice.glide_step = 0.0;
        } else {
            let glide_samples = (self.values[p::GLIDE] * self.sample_rate).max(1.0);
            voice.glide_step = (target - voice.pitch_semis) / glide_samples;
        }
        voice.pitch_target = target;
        voice.note = pitch;
        voice.velocity = velocity;
        voice.base_freq = 440.0 * 2f64.powf((target - 69.0) / 12.0);
        voice.amp.set_params(
            self.values[p::AMP_ATTACK],
            self.values[p::AMP_DECAY],
            self.values[p::AMP_SUSTAIN] as f32,
            self.values[p::AMP_RELEASE],
        );
        voice.fenv.set_params(
            self.values[p::FILTER_ENV_ATTACK],
            self.values[p::FILTER_ENV_DECAY],
            self.values[p::FILTER_ENV_SUSTAIN] as f32,
            self.values[p::FILTER_ENV_RELEASE],
        );
        voice.amp.note_on();
        voice.fenv.note_on();
        voice.osc_a.configure(
            &self.tables[self.values[p::OSC_A_WAVETABLE] as usize],
            voice.base_freq,
            self.values[p::OSC_A_PITCH],
            self.values[p::OSC_A_UNISON] as usize,
            self.values[p::OSC_A_DETUNE],
            self.values[p::OSC_A_SPREAD] as f32,
        );
        voice.osc_b.configure(
            &self.tables[self.values[p::OSC_B_WAVETABLE] as usize],
            voice.base_freq,
            self.values[p::OSC_B_PITCH],
            self.values[p::OSC_B_UNISON] as usize,
            self.values[p::OSC_B_DETUNE],
            self.values[p::OSC_B_SPREAD] as f32,
        );
        if !from_glide {
            voice.osc_a.start_phases(
                self.values[p::OSC_A_PHASE],
                self.values[p::OSC_A_PHASE_SPREAD],
            );
            voice.osc_b.start_phases(
                self.values[p::OSC_B_PHASE],
                self.values[p::OSC_B_PHASE_SPREAD],
            );
        }
        voice.motion.reset(pitch, self.values[p::LFO_PHASE]);
        voice.invalidate_control();
    }

    pub(super) fn note_on(&mut self, pitch: u8, velocity: f32) {
        match self.values[p::VOICE_MODE] as usize {
            p::MODE_MONO | p::MODE_LEGATO => {
                let legato_continue =
                    self.values[p::VOICE_MODE] as usize == p::MODE_LEGATO && self.held_count > 0;
                if self.held_count < MAX_HELD {
                    self.held[self.held_count] = pitch;
                    self.held_count += 1;
                }
                let (slot, reused) = match self.mono_slot.filter(|&s| self.pool.slot(s).is_active())
                {
                    Some(slot) => (slot, true),
                    None => {
                        let slot = self.pool.allocate(velocity);
                        self.mono_slot = Some(slot);
                        (slot, false)
                    }
                };
                if legato_continue {
                    // Legato: keep envelopes, glide (or snap) to the new pitch.
                    let voice = &mut self.pool.slot_mut(slot).voice;
                    voice.invalidate_control();
                    let target = f64::from(pitch);
                    let glide = self.values[p::GLIDE];
                    if glide > 0.0 {
                        voice.glide_step =
                            (target - voice.pitch_semis) / (glide * self.sample_rate).max(1.0);
                    } else {
                        voice.pitch_semis = target;
                        voice.glide_step = 0.0;
                    }
                    voice.pitch_target = target;
                    voice.note = pitch;
                    voice.velocity = velocity;
                    voice.base_freq = 440.0 * 2f64.powf((target - 69.0) / 12.0);
                } else {
                    self.start_voice(slot, pitch, velocity, reused);
                }
            }
            _ => {
                debug_assert_eq!(self.values[p::VOICE_MODE] as usize, p::MODE_POLY);
                let slot = self.pool.allocate(velocity);
                self.start_voice(slot, pitch, velocity, false);
            }
        }
    }

    pub(super) fn note_off(&mut self, pitch: u8) {
        match self.values[p::VOICE_MODE] as usize {
            p::MODE_MONO | p::MODE_LEGATO => {
                if let Some(pos) = self.held[..self.held_count]
                    .iter()
                    .position(|&n| n == pitch)
                {
                    self.held.copy_within(pos + 1..self.held_count, pos);
                    self.held_count -= 1;
                }
                let Some(slot) = self.mono_slot.filter(|&s| self.pool.slot(s).is_active()) else {
                    return;
                };
                if self.pool.slot(slot).voice.note != pitch {
                    return;
                }
                if self.held_count > 0 {
                    let prev = self.held[self.held_count - 1];
                    let mono = self.values[p::VOICE_MODE] as usize == p::MODE_MONO;
                    let velocity = self.pool.slot(slot).voice.velocity;
                    if mono {
                        self.start_voice(slot, prev, velocity, true);
                    } else {
                        let voice = &mut self.pool.slot_mut(slot).voice;
                        voice.invalidate_control();
                        let target = f64::from(prev);
                        let glide = self.values[p::GLIDE];
                        if glide > 0.0 {
                            voice.glide_step =
                                (target - voice.pitch_semis) / (glide * self.sample_rate).max(1.0);
                        } else {
                            voice.pitch_semis = target;
                            voice.glide_step = 0.0;
                        }
                        voice.pitch_target = target;
                        voice.note = prev;
                        voice.base_freq = 440.0 * 2f64.powf((target - 69.0) / 12.0);
                    }
                } else {
                    let voice = &mut self.pool.slot_mut(slot).voice;
                    voice.amp.note_off();
                    voice.fenv.note_off();
                }
            }
            _ => {
                for i in 0..self.pool.capacity() {
                    if !self.pool.slot(i).is_active() {
                        continue;
                    }
                    let voice = &mut self.pool.slot_mut(i).voice;
                    if voice.note == pitch && voice.amp.is_active() {
                        voice.amp.note_off();
                        voice.fenv.note_off();
                    }
                }
            }
        }
    }
}
