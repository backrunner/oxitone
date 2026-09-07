use crate::{
    piano_layout::*,
    ui::Preview,
    workspace::{thumb, Axis, Gesture},
};
use gpui::*;

impl Preview {
    pub fn scroll_piano(&mut self, event: &ScrollWheelEvent) {
        let Some(l) = self.piano_layout() else { return };
        let delta = event.delta.pixel_delta(px(20.));
        if event.modifiers.platform || event.modifiers.control {
            self.zoom_piano((f32::from(delta.y) / 150.).exp(), 1.);
        } else {
            let (x, y) = if event.modifiers.shift {
                (f32::from(delta.x + delta.y), 0.)
            } else {
                (f32::from(delta.x), f32::from(delta.y))
            };
            self.piano.offset = Some((
                (l.scroll_x - x).clamp(0., l.max_x),
                (l.scroll_y - y).clamp(0., l.max_y),
            ));
        }
    }
    pub fn press_piano(&mut self, event: &MouseDownEvent) {
        let Some(l) = self.piano_layout() else { return };
        let at = event.position - self.piano.origin.get();
        let (x, y) = (f32::from(at.x), f32::from(at.y));
        let scroll = if x >= l.width - SCROLLBAR && y >= RULER && y < RULER + l.grid_height {
            Some((
                Axis::Vertical,
                y - RULER,
                l.grid_height,
                l.max_y,
                l.scroll_y,
            ))
        } else if y >= l.height - SCROLLBAR && x >= KEY_WIDTH {
            Some((
                Axis::Horizontal,
                x - KEY_WIDTH,
                l.grid_width,
                l.max_x,
                l.scroll_x,
            ))
        } else {
            None
        };
        if let Some((axis, local, track, max, offset)) = scroll {
            let (start, length) = thumb(track, track, max, -offset);
            if max <= 0. || track <= length {
                return;
            }
            let next = if local < start || local > start + length {
                ((local - length * 0.5) / (track - length)).clamp(0., 1.) * max
            } else {
                offset
            };
            let offset = match axis {
                Axis::Horizontal => (next, l.scroll_y),
                Axis::Vertical => (l.scroll_x, next),
            };
            self.piano.offset = Some(offset);
            self.workspace.gesture = Some(Gesture::PianoScroll {
                axis,
                pointer: axis.coordinate(event.position),
                offset,
                scale: max / (track - length),
            });
        } else if y < RULER && x >= KEY_WIDTH {
            if let Some(project) = &self.project {
                if let Some(clip) = project
                    .snapshot
                    .pattern_clips
                    .iter()
                    .find(|c| Some(&c.id) == self.selected_clip.as_ref())
                {
                    self.seek(
                        project
                            .local_to_global(&clip.track_id, clip.start_beat.to_f64() + l.beat(x)),
                    );
                }
            }
        }
    }
    pub fn piano_key(&mut self, key: &str) -> bool {
        let Some(l) = self.piano_layout() else {
            return false;
        };
        let (dx, dy) = match key {
            "left" => (-l.beat_width, 0.),
            "right" => (l.beat_width, 0.),
            "up" => (0., -l.key_height * 3.),
            "down" => (0., l.key_height * 3.),
            "pageup" => (0., -l.grid_height),
            "pagedown" => (0., l.grid_height),
            "home" => (-l.max_x, 0.),
            "end" => (l.max_x, 0.),
            "=" | "+" => {
                self.zoom_piano(1.25, 1.);
                return true;
            }
            "-" => {
                self.zoom_piano(0.8, 1.);
                return true;
            }
            "f" => {
                self.piano.fit();
                return true;
            }
            _ => return false,
        };
        self.piano.offset = Some((
            (l.scroll_x + dx).clamp(0., l.max_x),
            (l.scroll_y + dy).clamp(0., l.max_y),
        ));
        true
    }
    pub fn piano_layout(&self) -> Option<PianoLayout> {
        let project = self.project.as_ref()?;
        let clip = project
            .snapshot
            .pattern_clips
            .iter()
            .find(|c| Some(&c.id) == self.selected_clip.as_ref())?;
        let pattern = project
            .snapshot
            .patterns
            .iter()
            .find(|p| p.id == clip.pattern_id)?;
        Some(self.piano.layout(
            pattern.length_beats.to_f64(),
            pattern.notes.iter().map(|n| n.pitch),
        ))
    }
    pub fn zoom_piano(&mut self, horizontal: f32, vertical: f32) {
        let Some(old) = self.piano_layout() else {
            return;
        };
        self.piano.zoom = (self.piano.zoom * horizontal).clamp(0.25, 128.);
        self.piano.key_zoom = (self.piano.key_zoom * vertical).clamp(0.5, 4.);
        let new = self.piano_layout().unwrap();
        self.piano.offset = Some((
            (old.scroll_x + old.grid_width * 0.5) / old.beat_width * new.beat_width
                - new.grid_width * 0.5,
            (old.scroll_y + old.grid_height * 0.5) / old.key_height * new.key_height
                - new.grid_height * 0.5,
        ));
    }
}
