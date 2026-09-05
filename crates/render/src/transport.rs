//! Offline transport (02-domain-spec.md §Playback 与 Export 语义). The
//! sample frame is the authoritative cursor; states are
//! `stopped|playing|paused|rendering`. Offline rendering drives the block
//! renderer directly in `rendering` state. Seek flushes voices/DSP state at
//! the `RenderGraph` level (`RenderGraph::seek`).

use oxitone_core::beat::Beat;
use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::wire::MarkerSpec;
use oxitone_graph::compile::RenderPlan;

/// Transport states (02-domain-spec.md §Playback 与 Export 语义).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportState {
    Stopped,
    Playing,
    Paused,
    Rendering,
}

/// One endpoint of a render/play range: exactly one of bar, beat, timecode
/// (seconds or frames), or marker. `start` and `end` must use the same kind
/// (04-api-contracts.md §RenderOptions: 混用报错).
#[derive(Debug, Clone, PartialEq)]
pub enum RangePoint {
    Bar(u32),
    Beat(f64),
    Seconds(f64),
    Frames(u64),
    Marker(String),
}

impl RangePoint {
    /// Kind family for the mixed-kinds check (`Seconds`/`Frames` are both
    /// the `Timecode` family).
    fn family(&self) -> &'static str {
        match self {
            RangePoint::Bar(_) => "bar",
            RangePoint::Beat(_) => "beat",
            RangePoint::Seconds(_) | RangePoint::Frames(_) => "timecode",
            RangePoint::Marker(_) => "marker",
        }
    }
}

/// Resolve a range endpoint to an absolute sample frame.
pub fn resolve_point(
    point: &RangePoint,
    plan: &RenderPlan,
    markers: &[MarkerSpec],
) -> Result<u64, OxitoneError> {
    match point {
        RangePoint::Bar(bar) => {
            let beat = plan.time_signatures.bar_beat_to_beat(*bar, Beat::ZERO)?;
            Ok(plan.tempo.beat_to_frame(beat))
        }
        RangePoint::Beat(beat) => {
            if !beat.is_finite() || *beat < 0.0 {
                return Err(OxitoneError::with_path(
                    codes::INVALID_PROJECT,
                    format!("beat must be finite and >= 0, got {beat}"),
                    "$.start.beat",
                ));
            }
            Ok(plan.tempo.beat_to_frame(Beat::from_f64(*beat)?))
        }
        RangePoint::Seconds(seconds) => {
            if !seconds.is_finite() || *seconds < 0.0 {
                return Err(OxitoneError::with_path(
                    codes::INVALID_PROJECT,
                    format!("timecode seconds must be finite and >= 0, got {seconds}"),
                    "$.start.timecode",
                ));
            }
            Ok((seconds * f64::from(plan.sample_rate) + 0.5).floor() as u64)
        }
        RangePoint::Frames(frames) => Ok(*frames),
        RangePoint::Marker(id) => {
            let marker = markers.iter().find(|m| &m.id == id).ok_or_else(|| {
                OxitoneError::with_path(
                    codes::INVALID_PROJECT,
                    format!("unknown marker {id:?}"),
                    "$.start.marker",
                )
            })?;
            Ok(plan.tempo.beat_to_frame(marker.start_beat))
        }
    }
}

/// Resolve `(start, end)` frames; enforces the same-kind rule.
pub fn resolve_range(
    start: Option<&RangePoint>,
    end: Option<&RangePoint>,
    plan: &RenderPlan,
    markers: &[MarkerSpec],
) -> Result<(u64, u64), OxitoneError> {
    if let (Some(s), Some(e)) = (start, end) {
        if s.family() != e.family() {
            return Err(OxitoneError::with_path(
                codes::INVALID_PROJECT,
                format!(
                    "start ({}) and end ({}) must use the same range kind (bar/beat/timecode/marker)",
                    s.family(),
                    e.family()
                ),
                "$.end",
            ));
        }
    }
    let start_frame = match start {
        Some(point) => resolve_point(point, plan, markers)?,
        None => 0,
    };
    let end_frame = match end {
        Some(point) => resolve_point(point, plan, markers)?,
        None => plan.tempo.beat_to_frame(plan.content_end_beat),
    };
    if end_frame <= start_frame {
        return Err(OxitoneError::with_path(
            codes::INVALID_PROJECT,
            format!("render end ({end_frame}) must be after start ({start_frame})"),
            "$.end",
        ));
    }
    Ok((start_frame, end_frame))
}

/// Offline transport state machine.
pub struct Transport {
    pub state: TransportState,
    pub cursor: u64,
    /// `[start, end)` loop region in frames (playing state only).
    pub loop_region: Option<(u64, u64)>,
}

impl Transport {
    pub fn new() -> Self {
        Self {
            state: TransportState::Stopped,
            cursor: 0,
            loop_region: None,
        }
    }

    pub fn play_from(&mut self, frame: u64, loop_region: Option<(u64, u64)>) {
        self.cursor = frame;
        self.loop_region = loop_region.filter(|(start, end)| end > start);
        self.state = TransportState::Playing;
    }

    pub fn pause(&mut self) {
        if self.state == TransportState::Playing {
            self.state = TransportState::Paused;
        }
    }

    pub fn stop(&mut self) {
        self.state = TransportState::Stopped;
        self.cursor = 0;
        self.loop_region = None;
    }

    pub fn begin_render(&mut self, frame: u64) {
        self.cursor = frame;
        self.loop_region = None;
        self.state = TransportState::Rendering;
    }

    /// Whether blocks advance the cursor and produce audio.
    pub fn running(&self) -> bool {
        matches!(
            self.state,
            TransportState::Playing | TransportState::Rendering
        )
    }

    /// Advance after a rendered block, wrapping the loop region.
    pub fn advance(&mut self, frames: u64) {
        self.cursor += frames;
        if self.state == TransportState::Playing {
            if let Some((start, end)) = self.loop_region {
                while self.cursor >= end {
                    self.cursor = start + (self.cursor - end);
                }
            }
        }
    }
}

impl Default for Transport {
    fn default() -> Self {
        Self::new()
    }
}
