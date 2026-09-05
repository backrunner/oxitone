//! Tempo map compilation and closed-form beat/seconds/frame conversion
//! (`.agents/docs/03-audio-runtime-spec.md` §时间和调度). Seconds are only
//! ever produced by the closed-form segment integrals plus precomputed
//! boundary totals; nothing accumulates per block.

use oxitone_core::beat::Beat;
use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::wire::{TempoCurve, TempoSegment};

use crate::beat_cmp;

const MIN_BPM: f64 = 20.0;
const MAX_BPM: f64 = 999.0;

/// Stateless compiler entry point for project tempo maps.
pub struct TempoMap;

#[derive(Debug, Clone, Copy)]
struct CompiledSegment {
    start_beat: f64,
    start_seconds: f64,
    length: f64,
    b0: f64,
    b1: f64,
    curve: TempoCurve,
}

/// Immutable compiled tempo map. All queries are closed-form, monotonic,
/// and lock-free; every path funnels through `beat -> seconds -> frame`,
/// so a given beat always lands on the same frame.
#[derive(Debug, Clone)]
pub struct CompiledTempoMap {
    segments: Vec<CompiledSegment>,
    sample_rate: u32,
}

impl TempoMap {
    /// Validate `segments` and precompute cumulative seconds at every
    /// segment boundary. The first segment must start at beat 0 and start
    /// beats must strictly increase (`TempoMapOrder`); every BPM must be
    /// finite and in `20..=999` (`TempoRange`). The last segment is always
    /// treated as `step` because its curve has no successor to ramp toward.
    pub fn compile(
        segments: &[TempoSegment],
        sample_rate: u32,
    ) -> Result<CompiledTempoMap, OxitoneError> {
        if sample_rate == 0 {
            return Err(OxitoneError::new(
                codes::INVALID_PROJECT,
                "sample rate must be > 0",
            ));
        }
        if segments.is_empty() {
            return Err(OxitoneError::new(
                codes::TEMPO_MAP_ORDER,
                "tempo map needs at least one segment",
            ));
        }
        if segments[0].start_beat != Beat::ZERO {
            return Err(OxitoneError::with_path(
                codes::TEMPO_MAP_ORDER,
                "first tempo segment must start at beat 0",
                "$.tempoMap[0].startBeat",
            ));
        }
        for (i, seg) in segments.iter().enumerate() {
            if !seg.bpm.is_finite() || !(MIN_BPM..=MAX_BPM).contains(&seg.bpm) {
                return Err(OxitoneError::with_path(
                    codes::TEMPO_RANGE,
                    format!("bpm must be finite and in 20..=999, got {}", seg.bpm),
                    format!("$.tempoMap[{i}].bpm"),
                ));
            }
            if i > 0 && beat_cmp(seg.start_beat, segments[i - 1].start_beat).is_le() {
                return Err(OxitoneError::with_path(
                    codes::TEMPO_MAP_ORDER,
                    "tempo segment start beats must strictly increase",
                    format!("$.tempoMap[{i}].startBeat"),
                ));
            }
        }

        let mut compiled = Vec::with_capacity(segments.len());
        let mut seconds = 0.0_f64;
        for (i, seg) in segments.iter().enumerate() {
            let start = seg.start_beat.to_f64();
            let (length, b1, curve) = match segments.get(i + 1) {
                Some(next) => (
                    next.start_beat.to_f64() - start,
                    next.bpm,
                    seg.curve.unwrap_or(TempoCurve::Step),
                ),
                None => (f64::INFINITY, seg.bpm, TempoCurve::Step),
            };
            let cs = CompiledSegment {
                start_beat: start,
                start_seconds: seconds,
                length,
                b0: seg.bpm,
                b1,
                curve,
            };
            if i + 1 < segments.len() {
                seconds += local_seconds(&cs, start + length);
            }
            compiled.push(cs);
        }
        Ok(CompiledTempoMap {
            segments: compiled,
            sample_rate,
        })
    }
}

fn local_seconds(seg: &CompiledSegment, beat: f64) -> f64 {
    let dx = beat - seg.start_beat;
    match seg.curve {
        TempoCurve::Step => 60.0 * dx / seg.b0,
        TempoCurve::Linear => {
            if seg.b1 == seg.b0 {
                60.0 * dx / seg.b0
            } else {
                let bpm = seg.b0 + (seg.b1 - seg.b0) * dx / seg.length;
                60.0 * seg.length / (seg.b1 - seg.b0) * (bpm / seg.b0).ln()
            }
        }
        TempoCurve::Exponential => {
            if seg.b1 == seg.b0 {
                60.0 * dx / seg.b0
            } else {
                let lr = (seg.b1 / seg.b0).ln();
                let bpm = seg.b0 * (dx / seg.length * lr).exp();
                60.0 * seg.length / (seg.b0 * lr) * (1.0 - seg.b0 / bpm)
            }
        }
    }
}

fn local_beat(seg: &CompiledSegment, seconds: f64) -> f64 {
    match seg.curve {
        TempoCurve::Step => seg.start_beat + seconds * seg.b0 / 60.0,
        TempoCurve::Linear => {
            if seg.b1 == seg.b0 {
                seg.start_beat + seconds * seg.b0 / 60.0
            } else {
                let bpm = seg.b0 * (seconds * (seg.b1 - seg.b0) / (60.0 * seg.length)).exp();
                seg.start_beat + seg.length * (bpm - seg.b0) / (seg.b1 - seg.b0)
            }
        }
        TempoCurve::Exponential => {
            if seg.b1 == seg.b0 {
                seg.start_beat + seconds * seg.b0 / 60.0
            } else {
                let lr = (seg.b1 / seg.b0).ln();
                let bpm = seg.b0 / (1.0 - seconds * seg.b0 * lr / (60.0 * seg.length));
                seg.start_beat + seg.length * (bpm / seg.b0).ln() / lr
            }
        }
    }
}

impl CompiledTempoMap {
    /// Sample rate the map was compiled for.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Closed-form beat to seconds conversion.
    pub fn beat_to_seconds(&self, beat: Beat) -> f64 {
        self.beat_f64_to_seconds(beat.to_f64())
    }

    /// Closed-form seconds to beat conversion (inverse of
    /// [`CompiledTempoMap::beat_to_seconds`]); non-finite or negative input
    /// maps to beat zero.
    pub fn seconds_to_beat(&self, seconds: f64) -> Beat {
        if !seconds.is_finite() || seconds <= 0.0 {
            return Beat::ZERO;
        }
        let idx = self
            .segments
            .partition_point(|s| s.start_seconds <= seconds);
        let seg = &self.segments[idx.saturating_sub(1)];
        let beat = local_beat(seg, seconds - seg.start_seconds);
        Beat::from_f64(beat.max(0.0)).unwrap_or(Beat::ZERO)
    }

    /// Beat to sample frame: closed-form seconds times the sample rate,
    /// rounded half-up to `u64`.
    pub fn beat_to_frame(&self, beat: Beat) -> u64 {
        self.beat_f64_to_frame(beat.to_f64())
    }

    /// Sample frame to beat (inverse of [`CompiledTempoMap::beat_to_frame`]
    /// up to frame quantization).
    pub fn frame_to_beat(&self, frame: u64) -> Beat {
        self.seconds_to_beat(frame as f64 / f64::from(self.sample_rate))
    }

    /// BPM at a beat, evaluated in closed form inside the covering segment.
    /// Beats past the last segment use the last (step) BPM. Used by the
    /// render graph for tempo-following playback (`tempoSync`), beat-unit
    /// effect parameters, and stretch-ratio computation; additive query
    /// only — it does not change any existing conversion path.
    pub fn bpm_at_beat(&self, beat: f64) -> f64 {
        let idx = self.segments.partition_point(|s| s.start_beat <= beat);
        let seg = &self.segments[idx.saturating_sub(1)];
        let u = if seg.length.is_finite() && seg.length > 0.0 {
            ((beat - seg.start_beat) / seg.length).clamp(0.0, 1.0)
        } else {
            0.0
        };
        match seg.curve {
            TempoCurve::Step => seg.b0,
            TempoCurve::Linear => seg.b0 + (seg.b1 - seg.b0) * u,
            TempoCurve::Exponential => {
                if seg.b1 == seg.b0 {
                    seg.b0
                } else {
                    seg.b0 * (u * (seg.b1 / seg.b0).ln()).exp()
                }
            }
        }
    }

    /// BPM at a sample frame (`frame -> beat -> bpm`).
    pub fn bpm_at_frame(&self, frame: u64) -> f64 {
        self.bpm_at_beat(self.frame_to_beat(frame).to_f64())
    }

    /// `(min, max)` BPM over the closed beat interval `[start, end]`. Every
    /// segment curve is monotonic, so extrema occur at the interval ends or
    /// at segment boundaries inside it. Compile-time helper.
    pub fn bpm_range(&self, start: f64, end: f64) -> (f64, f64) {
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        let visit = |map: &CompiledTempoMap, beat: f64, lo: &mut f64, hi: &mut f64| {
            let v = map.bpm_at_beat(beat);
            *lo = lo.min(v);
            *hi = hi.max(v);
        };
        visit(self, start, &mut lo, &mut hi);
        visit(self, end, &mut lo, &mut hi);
        for seg in &self.segments {
            if seg.start_beat > start && seg.start_beat < end {
                visit(self, seg.start_beat, &mut lo, &mut hi);
            }
        }
        (lo, hi)
    }

    pub(crate) fn beat_f64_to_seconds(&self, beat: f64) -> f64 {
        let idx = self.segments.partition_point(|s| s.start_beat <= beat);
        let seg = &self.segments[idx.saturating_sub(1)];
        seg.start_seconds + local_seconds(seg, beat)
    }

    pub(crate) fn beat_f64_to_frame(&self, beat: f64) -> u64 {
        let frames = self.beat_f64_to_seconds(beat) * f64::from(self.sample_rate);
        (frames + 0.5).floor().max(0.0) as u64
    }
}
