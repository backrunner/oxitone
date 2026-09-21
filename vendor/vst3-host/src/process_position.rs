//! Explicit host-owned position for the next process call (Oxitone extension).
use crate::{Error, Result};
use vst3::Steinberg::Vst::{ProcessContext, ProcessContext_::StatesAndFlags_ as F};

#[derive(Debug, Clone, Copy, PartialEq)]
/// Exact first-sample position for one host processing block.
pub struct ProcessPosition {
    /// Absolute project sample position.
    pub project_frame: i64,
    /// Monotonic engine sample position, independent of project loops.
    pub continuous_frame: i64,
    /// Absolute musical position in quarter notes.
    pub project_beat: f64,
    /// Start of the current bar in quarter notes.
    pub bar_beat: f64,
    /// Current tempo in quarter notes per minute.
    pub tempo: f64,
    /// Meter numerator and denominator.
    pub time_signature: [i32; 2],
    /// Whether the project transport is playing.
    pub playing: bool,
    /// Active cycle start and exclusive end, in quarter notes.
    pub cycle: Option<[f64; 2]>,
}

impl ProcessPosition {
    /// Reject invalid context values before any plugin or internal state is changed.
    pub fn validate(&self) -> Result<()> {
        if self.project_frame < 0
            || self.continuous_frame < 0
            || !self.project_beat.is_finite()
            || self.project_beat < 0.0
            || !self.bar_beat.is_finite()
            || self.bar_beat < 0.0
            || self.bar_beat > self.project_beat
            || !self.tempo.is_finite()
            || self.tempo <= 0.0
            || self.time_signature[0] <= 0
            || !matches!(self.time_signature[1], 1 | 2 | 4 | 8 | 16)
            || self.cycle.is_some_and(|[start, end]| {
                !start.is_finite() || !end.is_finite() || start < 0.0 || end <= start
            })
        {
            return Err(Error::InvalidParameter(
                "invalid explicit process position".into(),
            ));
        }
        Ok(())
    }

    #[allow(clippy::unnecessary_cast)]
    pub(crate) fn write_to(&self, context: &mut ProcessContext) {
        context.projectTimeSamples = self.project_frame;
        context.continousTimeSamples = self.continuous_frame;
        context.projectTimeMusic = self.project_beat;
        context.barPositionMusic = self.bar_beat;
        context.tempo = self.tempo;
        context.timeSigNumerator = self.time_signature[0];
        context.timeSigDenominator = self.time_signature[1];
        context.state &= !(F::kPlaying as u32 | F::kCycleActive as u32 | F::kCycleValid as u32);
        context.state |= F::kContTimeValid as u32
            | F::kProjectTimeMusicValid as u32
            | F::kBarPositionValid as u32
            | F::kTempoValid as u32
            | F::kTimeSigValid as u32;
        if self.playing {
            context.state |= F::kPlaying as u32;
        }
        let [start, end] = self.cycle.unwrap_or([0.0, 0.0]);
        context.cycleStartMusic = start;
        context.cycleEndMusic = end;
        if self.cycle.is_some() {
            context.state |= F::kCycleActive as u32 | F::kCycleValid as u32;
        }
    }
}
