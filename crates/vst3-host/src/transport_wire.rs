//! Explicit block context. No allocation, clock reads or DSP ownership.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Transport {
    pub project_frame: u64,
    pub continuous_frame: u64,
    pub project_beat: f64,
    pub bar_beat: f64,
    pub tempo: f64,
    pub time_signature: [i32; 2],
    pub playing: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cycle: Option<[f64; 2]>,
}

impl Transport {
    /// Constant-tempo continuation; the graph supplies explicit contexts for tempo changes/jumps.
    /// A stopped project freezes its musical/sample position while continuous time still advances.
    pub fn advanced(self, frames: usize, sample_rate: u32) -> Option<Self> {
        if sample_rate == 0 || !self.valid() {
            return None;
        }
        let mut next = self;
        next.continuous_frame = next.continuous_frame.checked_add(frames as u64)?;
        if self.playing && frames > 0 {
            next.project_frame = next.project_frame.checked_add(frames as u64)?;
            next.project_beat += frames as f64 * self.tempo / (60.0 * sample_rate as f64);
            let bar_length = self.time_signature[0] as f64 * 4.0 / self.time_signature[1] as f64;
            next.bar_beat +=
                ((next.project_beat - self.bar_beat) / bar_length).floor() * bar_length;
        }
        next.valid().then_some(next)
    }

    pub fn valid(&self) -> bool {
        const MAX: u64 = (1u64 << 53) - 1;
        self.project_frame <= MAX - 4096
            && self.continuous_frame <= MAX - 4096
            && self.project_beat.is_finite()
            && (0.0..=MAX as f64).contains(&self.project_beat)
            && self.bar_beat.is_finite()
            && (0.0..=self.project_beat).contains(&self.bar_beat)
            && self.tempo.is_finite()
            && (20.0..=999.0).contains(&self.tempo)
            && (1..=32).contains(&self.time_signature[0])
            && matches!(self.time_signature[1], 1 | 2 | 4 | 8 | 16)
            && self.cycle.is_none_or(|[start, end]| {
                start.is_finite() && end.is_finite() && start >= 0.0 && end > start
            })
    }

    #[cfg(feature = "host")]
    pub(crate) fn apply(&self, plugin: &mut vst3_host::Plugin) -> crate::Result<()> {
        if !self.valid() {
            return Err(crate::invalid("invalid VST3 transport"));
        }
        plugin
            .set_process_position(vst3_host::process_position::ProcessPosition {
                project_frame: self.project_frame as i64,
                continuous_frame: self.continuous_frame as i64,
                project_beat: self.project_beat,
                bar_beat: self.bar_beat,
                tempo: self.tempo,
                time_signature: self.time_signature,
                playing: self.playing,
                cycle: self.cycle,
            })
            .map_err(crate::native)
    }
}
