//! Tempo-lane loop/hold coordinates and their exact boundary mapping.
use super::{CompiledAutomation, EvalContext, TEMPO_BAKE_MAX_SEGMENTS};
use oxitone_core::{
    codes,
    wire::{AutomationCombine, AutomationLaneSpec},
    OxitoneError,
};

pub(super) struct LaneTime {
    start: f64,
    length: Option<f64>,
    end: f64,
}

impl LaneTime {
    pub fn new(lane: Option<&AutomationLaneSpec>) -> Result<Self, OxitoneError> {
        let Some(lane) = lane else {
            return Ok(Self {
                start: 0.0,
                length: None,
                end: f64::INFINITY,
            });
        };
        if lane
            .combine
            .is_some_and(|c| c != AutomationCombine::Replace)
        {
            return Err(OxitoneError::new(
                codes::TEMPO_AUTOMATION_CONFLICT,
                "tempo lane combine must be replace",
            ));
        }
        if let Some(region) = &lane.loop_spec {
            let start = region.start_beat.map_or(0.0, |b| b.to_f64());
            let length = region.length_beats.to_f64();
            if length <= 0.0
                || region.count == Some(0)
                || (region.count.is_some() && region.last_beat.is_some())
                || lane.last_beat.is_some()
            {
                return Err(OxitoneError::new(
                    codes::INVALID_PROJECT,
                    "invalid tempo lane loop/lastBeat",
                ));
            }
            let end = region
                .count
                .map(|n| start + length * f64::from(n))
                .or_else(|| region.last_beat.map(|b| b.to_f64()))
                .unwrap_or(f64::INFINITY);
            if end <= start {
                return Err(OxitoneError::new(
                    codes::INVALID_PROJECT,
                    "tempo loop must end after its start",
                ));
            }
            Ok(Self {
                start,
                length: Some(length),
                end,
            })
        } else {
            Ok(Self {
                start: 0.0,
                length: None,
                end: lane.last_beat.map_or(f64::INFINITY, |b| b.to_f64()),
            })
        }
    }

    pub fn map(&self, beat: f64) -> f64 {
        let Some(length) = self.length else {
            return beat.min(self.end);
        };
        if beat < self.start {
            return beat;
        }
        let bounded = beat.min(self.end);
        let offset = bounded - self.start;
        let phase = offset.rem_euclid(length);
        if bounded == self.end && phase == 0.0 && offset > 0.0 {
            self.start + length
        } else {
            self.start + phase
        }
    }

    pub fn boundaries(
        &self,
        evaluator: &CompiledAutomation,
        horizon: f64,
        ctx: &EvalContext,
    ) -> Result<Vec<f64>, OxitoneError> {
        let end = horizon.min(self.end);
        let Some(length) = self.length else {
            let mut points = evaluator.discontinuities(0.0, end, ctx);
            if self.end <= horizon {
                points.push(self.end);
            }
            return Ok(points);
        };
        let count = ((end - self.start).max(0.0) / length).ceil();
        if count > TEMPO_BAKE_MAX_SEGMENTS as f64 {
            return Err(OxitoneError::new(
                codes::TEMPO_MAP_COMPLEXITY,
                "too many tempo loop boundaries",
            ));
        }
        let mut points = evaluator.discontinuities(0.0, end.min(self.start), ctx);
        let local = evaluator.discontinuities(self.start, self.start + length, ctx);
        for cycle in 0..count as usize {
            let offset = cycle as f64 * length;
            points.push(self.start + offset);
            points.extend(local.iter().map(|p| p + offset).filter(|p| *p <= end));
            if points.len() > TEMPO_BAKE_MAX_SEGMENTS {
                return Err(OxitoneError::new(
                    codes::TEMPO_MAP_COMPLEXITY,
                    "too many tempo source boundaries",
                ));
            }
        }
        if end >= self.start {
            points.push(end);
        }
        if self.end <= horizon {
            points.push(self.end);
        }
        Ok(points)
    }
}
