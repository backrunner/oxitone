//! Immutable presentation data derived from the accepted authoring snapshot.
use oxitone_core::{wire::ProjectSnapshot, Beat};
use oxitone_graph::compile::RenderPlan;
use oxitone_render::preview::PreviewTelemetry;
use oxitone_transport::{timesig::CompiledTimeSignatureMap, CompiledTempoMap};
use std::sync::Arc;

pub struct ViewProject {
    pub snapshot: ProjectSnapshot,
    pub plan: ViewPlan,
    pub telemetry: Arc<PreviewTelemetry>,
    pub graph_latency: u64,
}

pub struct ViewPlan {
    pub tempo: CompiledTempoMap,
    pub time_signatures: CompiledTimeSignatureMap,
    pub content_end_beat: Beat,
    pub samples: Vec<(String, u64, u64)>,
}
impl From<&RenderPlan> for ViewPlan {
    fn from(plan: &RenderPlan) -> Self {
        Self {
            tempo: plan.tempo.clone(),
            time_signatures: plan.time_signatures.clone(),
            content_end_beat: plan.content_end_beat,
            samples: plan
                .sample_clips
                .iter()
                .map(|c| (c.id.clone(), c.start_frame, c.end_frame))
                .collect(),
        }
    }
}

impl ViewProject {
    pub fn beat(&self, frame: u64) -> f64 {
        self.plan.tempo.frame_to_beat(frame).to_f64()
    }
    pub fn end(&self) -> f64 {
        // Disabled clips remain visible even though they do not extend playback.
        self.snapshot
            .pattern_clips
            .iter()
            .map(|clip| self.clip_bounds(clip).1)
            .chain(self.plan.samples.iter().map(|(_, _, end)| self.beat(*end)))
            .fold(self.plan.content_end_beat.to_f64().max(16.0), f64::max)
    }
    pub fn clip_bounds(&self, clip: &oxitone_core::wire::PatternClipSpec) -> (f64, f64) {
        let length = self
            .snapshot
            .patterns
            .iter()
            .find(|p| p.id == clip.pattern_id)
            .map_or(0.0, |p| p.length_beats.to_f64());
        let start = clip.start_beat.to_f64();
        let end = clip.last_beat.map_or_else(
            || {
                start
                    + clip
                        .duration_beats
                        .map_or(length * f64::from(clip.loop_count.unwrap_or(1)), |b| {
                            b.to_f64()
                        })
            },
            |b| b.to_f64(),
        );
        (
            self.local_to_global(&clip.track_id, start),
            self.local_to_global(&clip.track_id, end),
        )
    }
    pub fn local_to_global(&self, track: &str, beat: f64) -> f64 {
        match self
            .snapshot
            .tracks
            .iter()
            .find(|t| t.id == track)
            .and_then(|t| t.tempo)
        {
            Some(bpm) => self.plan.tempo.seconds_to_beat(beat * 60.0 / bpm).to_f64(),
            None => beat,
        }
    }
    pub fn global_to_local(&self, track: &str, beat: f64) -> f64 {
        match self
            .snapshot
            .tracks
            .iter()
            .find(|t| t.id == track)
            .and_then(|t| t.tempo)
        {
            Some(bpm) => {
                self.plan
                    .tempo
                    .beat_to_seconds(Beat::from_f64(beat.max(0.0)).unwrap_or(Beat::ZERO))
                    * bpm
                    / 60.0
            }
            None => beat,
        }
    }
}

#[derive(Clone, Default)]
pub struct PlaybackStatus {
    pub cursor: u64,
    pub audible: u64,
    pub playing: bool,
    pub load: f64,
    pub xruns: u64,
    pub faults: u64,
    pub fault_nodes: String,
    pub latency: u64,
}

#[derive(Clone)]
pub struct Diagnostic {
    pub code: String,
    pub message: String,
    pub path: Option<String>,
}

pub enum UiEvent {
    Accepted(Arc<ViewProject>),
    Diagnostic(Diagnostic),
    Status(String),
    Playback(PlaybackStatus),
    Shutdown,
}
