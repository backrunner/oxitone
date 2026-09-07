use crate::{model::ViewProject, playback_controls::frame, ui::Preview};
use gpui::*;

pub fn playlist_beat(x: f32, origin: f32, offset: f32, zoom: f32, end: f64) -> f64 {
    (f64::from((x - origin - offset) / zoom)).clamp(0., end)
}

/// Pick the current repetition when the cursor is in the clip; otherwise its first pass.
pub fn piano_beat(project: &ViewProject, clip_id: &str, local: f64, at: u64) -> Option<f64> {
    let clip = project
        .snapshot
        .pattern_clips
        .iter()
        .find(|c| c.id == clip_id)?;
    let pattern = project
        .snapshot
        .patterns
        .iter()
        .find(|p| p.id == clip.pattern_id)?;
    let (start, end) = project.clip_bounds(clip);
    let global = project.beat(at);
    let length = pattern.length_beats.to_f64();
    let cycle = if global >= start && global < end {
        ((project.global_to_local(&clip.track_id, global) - clip.start_beat.to_f64()) / length)
            .floor()
    } else {
        0.
    };
    Some(
        project
            .local_to_global(
                &clip.track_id,
                clip.start_beat.to_f64() + cycle * length + local.clamp(0., length),
            )
            .clamp(start, end),
    )
}

impl Preview {
    pub fn timeline_click(&mut self, beat: f64, event: &MouseDownEvent) {
        if event.click_count >= 2 || event.modifiers.alt {
            if let Some(project) = &self.project {
                self.play_from(frame(project, beat));
            }
        } else {
            self.seek(beat);
        }
    }
    pub fn press_playlist(&mut self, event: &MouseDownEvent) {
        if let Some(project) = &self.project {
            let scroll = &self.workspace.arrangement;
            let beat = playlist_beat(
                f32::from(event.position.x),
                f32::from(scroll.bounds().origin.x),
                f32::from(scroll.offset().x),
                self.zoom,
                project.end(),
            );
            self.timeline_click(beat, event);
        }
    }
}
