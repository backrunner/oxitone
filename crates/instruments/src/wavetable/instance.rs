//! WavetableSynth plugin instance: 64-voice pool with quietest-then-oldest
//! stealing, poly/mono/legato voice modes with glide, and block-split
//! sample-accurate parameter application. `process` is allocation-free.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};

use oxitone_dsp::biquad::BiquadKind;
use oxitone_dsp::envelope::Adsr;
use oxitone_dsp::gain_pan::OnePoleSmoother;
use oxitone_dsp::oscillator::Wavetable;
use oxitone_dsp::voice::VoicePool;
use oxitone_graph::abi::{NoteEventKind, PluginInstance, ProcessContext};

use super::params as p;
use super::voice::{Voice, VoiceContext, FILTER_CHUNK};

const MAX_VOICES: usize = 64;
const TABLE_LEN: usize = 2048;
const TABLE_LEVELS: usize = 8;
pub(super) const MAX_HELD: usize = 16;
/// One-pole smoothing time for level/pan/mix/cutoff/resonance.
const SMOOTH_MS: f64 = 5.0;

/// Built-in single-cycle waveforms, mip-mapped per sample rate in `prepare`.
fn base_cycle(kind: usize) -> Vec<f32> {
    (0..TABLE_LEN)
        .map(|i| super::cycle_value(kind, i as f64 / TABLE_LEN as f64) as f32)
        .collect()
}

pub struct WavetableSynthInstance {
    pub(super) sample_rate: f64,
    max_block: usize,
    pub(super) tables: Arc<Vec<Wavetable>>,
    pub(super) values: Vec<f64>,
    pub(super) pool: VoicePool<Voice, MAX_VOICES>,
    pub(super) level: OnePoleSmoother,
    pub(super) pan: OnePoleSmoother,
    pub(super) mix: OnePoleSmoother,
    pub(super) cutoff: OnePoleSmoother,
    pub(super) resonance: OnePoleSmoother,
    mix_env: Vec<f32>,
    cutoff_chunks: Vec<f64>,
    resonance_chunks: Vec<f64>,
    pub(super) held: [u8; MAX_HELD],
    pub(super) held_count: usize,
    pub(super) mono_slot: Option<usize>,
}

impl WavetableSynthInstance {
    pub fn new(sample_rate: f64, max_block: usize, values: Vec<f64>) -> Self {
        let mut instance = Self {
            sample_rate,
            max_block,
            tables: Self::build_tables(sample_rate),
            values,
            pool: VoicePool::new(),
            level: OnePoleSmoother::new(sample_rate, SMOOTH_MS),
            pan: OnePoleSmoother::new(sample_rate, SMOOTH_MS),
            mix: OnePoleSmoother::new(sample_rate, SMOOTH_MS),
            cutoff: OnePoleSmoother::new(sample_rate, SMOOTH_MS),
            resonance: OnePoleSmoother::new(sample_rate, SMOOTH_MS),
            mix_env: vec![0.0; max_block],
            cutoff_chunks: vec![0.0; max_block / FILTER_CHUNK + 2],
            resonance_chunks: vec![0.0; max_block / FILTER_CHUNK + 2],
            held: [0; MAX_HELD],
            held_count: 0,
            mono_slot: None,
        };
        instance.fix_voice_sample_rates();
        instance.snap_smoothers();
        instance
    }

    /// Mip tables depend only on the sample rate and the fixed built-in
    /// base cycles, so all instances at one rate share a single
    /// (control-thread) cached set — the naive DFT build is O(N²) per kind.
    fn build_tables(sample_rate: f64) -> Arc<Vec<Wavetable>> {
        static CACHE: OnceLock<Mutex<BTreeMap<u64, Arc<Vec<Wavetable>>>>> = OnceLock::new();
        let cache = CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
        cache
            .lock()
            .expect("wavetable table cache poisoned")
            .entry(sample_rate.to_bits())
            .or_insert_with(|| {
                Arc::new(
                    (0..p::WAVETABLE_COUNT)
                        .map(|kind| Wavetable::new(&base_cycle(kind), TABLE_LEVELS, sample_rate))
                        .collect(),
                )
            })
            .clone()
    }

    /// `VoicePool` builds voices via `Default`; point their envelopes at the
    /// actual sample rate.
    fn fix_voice_sample_rates(&mut self) {
        for i in 0..self.pool.capacity() {
            let voice = &mut self.pool.slot_mut(i).voice;
            voice.amp = Adsr::new(self.sample_rate);
            voice.fenv = Adsr::new(self.sample_rate);
        }
    }

    fn snap_smoothers(&mut self) {
        self.level.snap(self.values[p::LEVEL] as f32);
        self.pan.snap(self.values[p::PAN] as f32);
        self.mix.snap(self.values[p::OSC_MIX] as f32);
        self.cutoff.snap(self.values[p::FILTER_CUTOFF] as f32);
        self.resonance.snap(self.values[p::FILTER_RESONANCE] as f32);
    }

    #[cfg(test)]
    pub(crate) fn active_voice_count(&self) -> usize {
        self.pool.active_count()
    }

    fn render_segment(&mut self, out_l: &mut [f32], out_r: &mut [f32]) {
        let frames = out_l.len();
        for x in self.mix_env[..frames].iter_mut() {
            *x = self.mix.next_sample();
        }
        let chunks = frames / FILTER_CHUNK + 1;
        for c in 0..chunks {
            let n = FILTER_CHUNK.min(frames - c * FILTER_CHUNK);
            let (mut cutoff, mut resonance) = (0.0f32, 0.0f32);
            for _ in 0..n {
                cutoff = self.cutoff.next_sample();
                resonance = self.resonance.next_sample();
            }
            self.cutoff_chunks[c] = f64::from(cutoff);
            self.resonance_chunks[c] = f64::from(resonance);
        }
        let filter_kind = match self.values[p::FILTER_TYPE] as usize {
            p::FILTER_HIGHPASS => BiquadKind::Highpass,
            p::FILTER_BANDPASS => BiquadKind::Bandpass,
            kind => {
                debug_assert_eq!(kind, p::FILTER_LOWPASS);
                BiquadKind::Lowpass
            }
        };
        let motion = super::motion::Motion::from_values(&self.values, self.sample_rate);
        for i in 0..self.pool.capacity() {
            if !self.pool.slot(i).is_active() {
                continue;
            }
            let ctx = VoiceContext {
                table_a: &self.tables[self.values[p::OSC_A_WAVETABLE] as usize],
                table_b: &self.tables[self.values[p::OSC_B_WAVETABLE] as usize],
                morph_a: &self.tables[self.values[p::OSC_A_MORPH_TO] as usize],
                morph_b: &self.tables[self.values[p::OSC_B_MORPH_TO] as usize],
                motion,
                mix: &self.mix_env[..frames],
                cutoff_chunks: &self.cutoff_chunks[..chunks],
                resonance_chunks: &self.resonance_chunks[..chunks],
                filter_kind,
                fenv_amount_semis: self.values[p::FILTER_ENV_AMOUNT],
                sample_rate: self.sample_rate,
            };
            let voice = &mut self.pool.slot_mut(i).voice;
            voice.render(out_l, out_r, &ctx);
            let level = voice.amp.level() * voice.velocity;
            let still_active = voice.amp.is_active();
            self.pool.update_level(i, level);
            if !still_active {
                self.pool.release(i);
                if self.mono_slot == Some(i) {
                    self.mono_slot = None;
                }
            }
        }
        // Global level/pan after the voice sum. `pan` is a stereo balance
        // here (unity gain at center); per-voice/unison pans already placed
        // the mono material with the equal-power law.
        let mut k = 0usize;
        while k < frames {
            let end = (k + FILTER_CHUNK).min(frames);
            let pan = self.pan.value();
            let (gl, gr) = (1.0 - pan.max(0.0), 1.0 + pan.min(0.0));
            for j in k..end {
                let gain = self.level.next_sample();
                self.pan.next_sample();
                out_l[j] *= gain * gl;
                out_r[j] *= gain * gr;
            }
            k = end;
        }
    }
}

impl PluginInstance for WavetableSynthInstance {
    fn prepare(&mut self, sample_rate: f64, max_block_size: u32) {
        let max_block = max_block_size as usize;
        if sample_rate != self.sample_rate {
            self.sample_rate = sample_rate;
            self.tables = Self::build_tables(sample_rate);
            self.pool = VoicePool::new();
            self.level = OnePoleSmoother::new(sample_rate, SMOOTH_MS);
            self.pan = OnePoleSmoother::new(sample_rate, SMOOTH_MS);
            self.mix = OnePoleSmoother::new(sample_rate, SMOOTH_MS);
            self.cutoff = OnePoleSmoother::new(sample_rate, SMOOTH_MS);
            self.resonance = OnePoleSmoother::new(sample_rate, SMOOTH_MS);
            self.fix_voice_sample_rates();
            self.mono_slot = None;
            self.held_count = 0;
        }
        if max_block != self.max_block {
            self.max_block = max_block;
            self.mix_env = vec![0.0; max_block];
            self.cutoff_chunks = vec![0.0; max_block / FILTER_CHUNK + 2];
            self.resonance_chunks = vec![0.0; max_block / FILTER_CHUNK + 2];
        }
        self.snap_smoothers();
    }

    fn process(&mut self, ctx: &mut ProcessContext<'_>) {
        let frames = ctx.frames.min(self.max_block);
        let (out_l, out_r) = ctx.outputs.split_at_mut(1);
        let out_l = &mut out_l[0][..frames];
        let out_r = &mut out_r[0][..frames];
        out_l.fill(0.0);
        out_r.fill(0.0);
        let specs = super::parameter_specs();
        crate::block::walk_block(frames, ctx.note_events, ctx.parameter_events, |step| {
            use crate::block::Walk;
            match step {
                Walk::Parameters(events) => {
                    for event in events {
                        if let Some(index) = crate::params::index_of(specs, event.parameter_id) {
                            let value =
                                crate::params::sanitize_event_value(&specs[index], event.value);
                            self.set_parameter(index, value);
                        }
                    }
                }
                Walk::Notes(notes) => {
                    for note in notes {
                        match note.kind {
                            NoteEventKind::NoteOn => self.note_on(note.pitch, note.velocity),
                            NoteEventKind::NoteOff => self.note_off(note.pitch),
                        }
                    }
                }
                Walk::Render(offset, len) => self.render_segment(
                    &mut out_l[offset..offset + len],
                    &mut out_r[offset..offset + len],
                ),
            }
        });
    }

    fn reset(&mut self) {
        self.pool = VoicePool::new();
        self.fix_voice_sample_rates();
        self.mono_slot = None;
        self.held_count = 0;
        self.snap_smoothers();
    }

    fn tail_frames(&self) -> u64 {
        if self.pool.active_count() == 0 {
            return 0;
        }
        ((self.values[p::AMP_ATTACK] + self.values[p::AMP_DECAY] + self.values[p::AMP_RELEASE])
            * self.sample_rate) as u64
            + 2
    }

    fn latency_frames(&self) -> u64 {
        0
    }
}
