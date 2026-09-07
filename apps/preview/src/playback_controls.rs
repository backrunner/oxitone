//! Viewer transport intent. Musical state remains owned by the native engine.
use crate::{model::ViewProject, ui::Preview};
use oxitone_core::Beat;
use serde_json::{json, Value};

pub fn frame(project: &ViewProject, beat: f64) -> u64 {
    project
        .plan
        .tempo
        .beat_to_frame(Beat::from_f64(beat.max(0.)).unwrap_or(Beat::ZERO))
}

impl Preview {
    pub fn is_playing(&self) -> bool {
        self.requested_playing.unwrap_or(self.playback.playing)
    }
    pub fn position_frame(&self) -> u64 {
        self.requested_position.map_or_else(
            || {
                if self.is_playing() {
                    self.playback.audible
                } else {
                    self.playback.cursor
                }
            },
            |(frame, _)| frame,
        )
    }
    fn remember_position(&mut self, frame: u64) {
        let epoch = self.project.as_ref().map_or(0, |p| {
            p.telemetry.epoch.load(std::sync::atomic::Ordering::Relaxed)
        });
        self.requested_position = Some((frame, epoch));
    }
    pub fn seek(&mut self, beat: f64) {
        if let Some(project) = &self.project {
            self.locate_frame(frame(project, beat));
        }
    }
    pub fn locate_frame(&mut self, frame: u64) {
        if self.project.is_none() {
            return;
        }
        self.cue_frame = frame;
        self.remember_position(frame);
        let outside = self.outside_loop(frame);
        if outside {
            self.loop_enabled = false;
        }
        if outside && self.is_playing() {
            self.send_play(Some(frame));
        } else {
            self.transport(json!({"command":"seek","frame":frame.to_string()}));
        }
    }
    pub fn play_from(&mut self, frame: u64) {
        if self.project.is_none() {
            return;
        }
        self.cue_frame = frame;
        self.remember_position(frame);
        if self.outside_loop(frame) {
            self.loop_enabled = false;
        }
        self.send_play(Some(frame));
    }
    fn outside_loop(&self, at: u64) -> bool {
        self.loop_enabled
            && self
                .project
                .as_ref()
                .is_some_and(|p| at < frame(p, self.loop_start) || at >= frame(p, self.loop_end))
    }
    fn play_command(&self, from: Option<u64>) -> Value {
        let mut command = json!({"command":"play"});
        if let Some(from) = from {
            command["frame"] = json!(from.to_string());
        }
        if self.loop_enabled {
            if let Some(p) = &self.project {
                command["loopRegion"] = json!({
                    "startFrame":frame(p, self.loop_start).to_string(),
                    "endFrame":frame(p, self.loop_end).to_string()
                });
            }
        }
        command
    }
    fn send_play(&mut self, from: Option<u64>) {
        self.transport(self.play_command(from));
        self.requested_playing = Some(true);
    }
    pub fn play(&mut self) {
        if self.project.is_some() {
            self.send_play(None);
        }
    }
    pub fn toggle_playback(&mut self) {
        if self.is_playing() {
            self.transport(json!({"command":"pause"}));
            self.requested_playing = Some(false);
        } else {
            self.play();
        }
    }
    pub fn stop_at_cue(&mut self) {
        if self.project.is_none() {
            return;
        }
        self.transport(json!({"command":"stop"}));
        // The native queue preserves Stop -> Seek ordering, including before first Play.
        self.transport(json!({"command":"seek","frame":self.cue_frame.to_string()}));
        self.remember_position(self.cue_frame);
        self.requested_playing = Some(false);
    }
    pub fn toggle_loop(&mut self) {
        self.loop_enabled = !self.loop_enabled;
        if self.outside_loop(self.position_frame()) {
            self.seek(self.loop_start);
        }
        if self.is_playing() {
            self.play();
        }
    }
    pub fn observe_transport(&mut self, status: crate::model::PlaybackStatus) {
        if self.requested_playing == Some(status.playing) {
            self.requested_playing = None;
        }
        if let Some((at, epoch)) = self.requested_position {
            let processed = self.project.as_ref().is_some_and(|p| {
                p.telemetry.epoch.load(std::sync::atomic::Ordering::Relaxed) != epoch
            });
            if status.cursor == at || (processed && status.playing) {
                self.requested_position = None;
            }
        }
        self.playback = status;
    }
}
