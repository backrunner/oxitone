//! Static Track beat clock, anchored to project time zero. Control-thread
//! conversion preserves the global tempo map and the original clip IDs.
use crate::CompiledTempoMap;
use oxitone_core::{
    error::{codes, OxitoneError},
    Beat,
};

#[derive(Clone, Copy)]
pub struct TrackClock<'a> {
    project: &'a CompiledTempoMap,
    bpm: Option<f64>,
}

impl<'a> TrackClock<'a> {
    pub fn new(project: &'a CompiledTempoMap, bpm: Option<f64>) -> Result<Self, OxitoneError> {
        if bpm.is_some_and(|v| !v.is_finite() || !(20.0..=999.0).contains(&v)) {
            return Err(OxitoneError::with_path(
                codes::TEMPO_RANGE,
                "track tempo must be finite and in 20..=999",
                "track.tempo",
            ));
        }
        Ok(Self { project, bpm })
    }

    pub fn seconds(self, beat: Beat) -> f64 {
        self.bpm.map_or_else(
            || self.project.beat_to_seconds(beat),
            |bpm| beat.to_f64() * 60.0 / bpm,
        )
    }

    pub fn project_beat(self, beat: Beat) -> Beat {
        if self.bpm.is_none() {
            beat
        } else {
            self.project.seconds_to_beat(self.seconds(beat))
        }
    }

    pub fn frame(self, beat: Beat) -> u64 {
        (self.seconds(beat) * f64::from(self.project.sample_rate()) + 0.5).floor() as u64
    }

    pub fn duration_beats(self, start: Beat, seconds: f64) -> f64 {
        self.bpm.map_or_else(
            || {
                self.project
                    .seconds_to_beat(self.seconds(start) + seconds)
                    .to_f64()
                    - start.to_f64()
            },
            |bpm| seconds * bpm / 60.0,
        )
    }

    pub fn bpm_range(self, start: Beat, end: Beat) -> (f64, f64) {
        self.bpm.map_or_else(
            || self.project.bpm_range(start.to_f64(), end.to_f64()),
            |bpm| (bpm, bpm),
        )
    }
}
