//! Time-signature map: bar/beat arithmetic with 1-based bars and 0-based
//! beat offsets (`.agents/docs/02-domain-spec.md` §Project 与时间轴). All
//! positions stay rational (`Beat`); segments only change at bar boundaries
//! by construction since they are keyed by `startBar`.

use oxitone_core::beat::Beat;
use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::wire::TimeSignatureSegment;

use crate::beat_cmp;

/// Stateless compiler entry point for time-signature maps.
pub struct TimeSignatureMap;

#[derive(Debug, Clone, Copy)]
struct CompiledSegment {
    start_bar: u32,
    bar_beats: Beat,
    start_beat: Beat,
}

/// Immutable compiled time-signature map; queries are exact rational
/// arithmetic and lock-free.
#[derive(Debug, Clone)]
pub struct CompiledTimeSignatureMap {
    segments: Vec<CompiledSegment>,
}

impl TimeSignatureMap {
    /// Validate `segments` and precompute the cumulative beat at every
    /// segment start. Bars are 1-based; the first segment must start at bar
    /// 1, start bars must strictly increase, numerators must be > 0, and
    /// denominators must be powers of two (`InvalidProject` with path).
    pub fn compile(
        segments: &[TimeSignatureSegment],
    ) -> Result<CompiledTimeSignatureMap, OxitoneError> {
        if segments.is_empty() {
            return Err(OxitoneError::with_path(
                codes::INVALID_PROJECT,
                "time signature map needs at least one segment",
                "$.timeSignatureMap",
            ));
        }
        if segments[0].start_bar != 1 {
            return Err(OxitoneError::with_path(
                codes::INVALID_PROJECT,
                "first time signature segment must start at bar 1",
                "$.timeSignatureMap[0].startBar",
            ));
        }
        let mut compiled: Vec<CompiledSegment> = Vec::with_capacity(segments.len());
        let mut start_beat = Beat::ZERO;
        for (i, seg) in segments.iter().enumerate() {
            let path = |field: &str| format!("$.timeSignatureMap[{i}].{field}");
            if seg.numerator == 0 {
                return Err(OxitoneError::with_path(
                    codes::INVALID_PROJECT,
                    "time signature numerator must be > 0",
                    path("numerator"),
                ));
            }
            if !seg.denominator.is_power_of_two() {
                return Err(OxitoneError::with_path(
                    codes::INVALID_PROJECT,
                    format!(
                        "time signature denominator must be a power of two, got {}",
                        seg.denominator
                    ),
                    path("denominator"),
                ));
            }
            let bar_beats = Beat::new(i64::from(seg.numerator) * 4, seg.denominator)?;
            if let Some(prev) = compiled.last() {
                if seg.start_bar <= prev.start_bar {
                    return Err(OxitoneError::with_path(
                        codes::INVALID_PROJECT,
                        "time signature start bars must strictly increase",
                        path("startBar"),
                    ));
                }
                let span = Beat::new(i64::from(seg.start_bar - prev.start_bar), 1)?;
                start_beat = start_beat.checked_add(prev.bar_beats.checked_mul(span)?)?;
            }
            compiled.push(CompiledSegment {
                start_bar: seg.start_bar,
                bar_beats,
                start_beat,
            });
        }
        Ok(CompiledTimeSignatureMap { segments: compiled })
    }
}

impl CompiledTimeSignatureMap {
    /// Map an absolute beat to its 1-based bar and 0-based beat-in-bar.
    pub fn beat_to_bar_beat(&self, beat: Beat) -> (u32, Beat) {
        let idx = self
            .segments
            .partition_point(|s| beat_cmp(s.start_beat, beat).is_le());
        let seg = &self.segments[idx.saturating_sub(1)];
        let offset = beat.checked_sub(seg.start_beat).unwrap_or(Beat::ZERO);
        let num = offset.numerator() as i128 * i128::from(seg.bar_beats.denominator());
        let den = offset.denominator() as i128 * i128::from(seg.bar_beats.numerator());
        let bar_index = num / den;
        let index_beat = seg
            .bar_beats
            .checked_mul(Beat::new(bar_index as i64, 1).unwrap_or(Beat::ZERO))
            .unwrap_or(Beat::ZERO);
        let beat_in_bar = offset.checked_sub(index_beat).unwrap_or(Beat::ZERO);
        let bar = i128::from(seg.start_bar) + bar_index;
        (bar.clamp(1, i128::from(u32::MAX)) as u32, beat_in_bar)
    }

    /// Map a 1-based bar and 0-based beat-in-bar back to an absolute beat.
    /// `bar` must be >= 1 and `beat_in_bar` must be smaller than the bar
    /// length at that position (`InvalidProject` otherwise).
    pub fn bar_beat_to_beat(&self, bar: u32, beat_in_bar: Beat) -> Result<Beat, OxitoneError> {
        if bar == 0 {
            return Err(OxitoneError::with_path(
                codes::INVALID_PROJECT,
                "bars are 1-based; bar 0 does not exist",
                "$.bar",
            ));
        }
        let idx = self.segments.partition_point(|s| s.start_bar <= bar);
        let seg = &self.segments[idx.saturating_sub(1)];
        if !beat_cmp(beat_in_bar, seg.bar_beats).is_lt() {
            return Err(OxitoneError::with_path(
                codes::INVALID_PROJECT,
                "beat offset must be smaller than the bar length",
                "$.beat",
            ));
        }
        let span = Beat::new(i64::from(bar - seg.start_bar), 1)?;
        seg.start_beat
            .checked_add(seg.bar_beats.checked_mul(span)?)?
            .checked_add(beat_in_bar)
    }
}
