//! Effect control staging, beat conversion, and insert runtime storage.
use oxitone_core::wire::ParameterSpec;
use oxitone_dsp::gain_pan::OnePoleSmoother;
use oxitone_graph::abi::PluginInstance;
use oxitone_mixer::DelayLine;
use std::sync::Arc;

/// Beat-unit (`unit: 'beats'`) effect parameter converted by the host
/// (02-domain-spec.md §Mixer: 时间类效果参数经 tempo map 换算). The authoring
/// value stays in beats; the DSP-facing `<name>Seconds` event is recomputed
/// per block from the effective tempo and pushed only when it changes.
pub struct BeatParam {
    /// Authoring value in beats, retained across tempo changes.
    pub beats: f64,
    pub parameter_index: usize,
    pub active: bool,
    /// Index of the seconds parameter in the instance's `param_ids`.
    pub seconds_index: usize,
    pub last_sent: f64,
}

impl BeatParam {
    /// Seconds value at `bpm`; returns `Some` once per change.
    pub fn poll(&mut self, bpm: f64) -> Option<(usize, f64)> {
        if !self.active {
            return None;
        }
        let seconds = self.beats * 60.0 / bpm.max(1e-9);
        if (seconds - self.last_sent).abs() < 1e-12 {
            None
        } else {
            self.last_sent = seconds;
            Some((self.seconds_index, seconds))
        }
    }
}

/// One channel insert: effect instance plus host-side mix/bypass state.
pub struct InsertNode {
    pub instance: Box<dyn PluginInstance>,
    pub param_ids: Arc<Vec<String>>,
    pub specs: Arc<Vec<ParameterSpec>>,
    /// Initial parameter events (physical values), applied on the first
    /// block after compile. Reset preserves plugin parameters.
    pub initial: Vec<(usize, f64)>,
    pub mix: OnePoleSmoother,
    pub bypass: bool,
    pub beat_params: Vec<BeatParam>,
    /// Coalesced parameter events for the current render segment.
    pub staged: oxitone_mixer::parameter_queue::ParameterQueue,
    pub first_block: bool,
    pub dry_delay: DelayLine,
    pub delayed_l: Vec<f32>,
    pub delayed_r: Vec<f32>,
}

impl InsertNode {
    /// Stage initial values before host events and automation.
    pub fn stage_initial(&mut self) {
        if self.first_block {
            let initial = std::mem::take(&mut self.initial);
            for &(index, value) in &initial {
                self.staged.set(index, value);
            }
            self.initial = initial;
            self.first_block = false;
        }
    }

    pub fn stage_tempo(&mut self, bpm: f64) {
        for i in 0..self.beat_params.len() {
            if let Some((index, seconds)) = self.beat_params[i].poll(bpm) {
                self.staged.set(index, seconds);
            }
        }
    }

    pub fn set_parameter(&mut self, index: usize, value: f64) {
        for state in &mut self.beat_params {
            if state.parameter_index == index {
                state.beats = value;
                state.active = true;
                return;
            }
            if state.seconds_index == index {
                state.active = false;
                state.last_sent = f64::NAN;
            }
        }
        self.staged.set(index, value);
    }
}
