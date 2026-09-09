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
        if self.document.gesture.is_some()
            || self.document.notes.marquee.is_some()
            || self.workspace.gesture.is_some()
        {
            return;
        }
        if event.modifiers.platform || event.modifiers.control {
            let at = event.position - self.piano.origin.get();
            let factor = (f32::from(delta.y) / 220.).exp();
            self.zoom_piano_at(
                if event.modifiers.shift { 1. } else { factor },
                if event.modifiers.shift { factor } else { 1. },
                (f32::from(at.x) - KEY_WIDTH, f32::from(at.y) - RULER),
            );
        } else {
            let (x, y) = if event.modifiers.shift {
                (f32::from(delta.x + delta.y), 0.)
            } else {
                (f32::from(delta.x), f32::from(delta.y))
            };
            self.piano.set_offset((
                (l.scroll_x - x).clamp(0., l.max_x),
                (l.scroll_y - y).clamp(0., l.max_y),
            ));
        }
    }
    pub fn pan_piano(&mut self, event: &MouseDownEvent) -> bool {
        let command = event.modifiers.platform || event.modifiers.control;
        if event.button != MouseButton::Middle
            && !(event.button == MouseButton::Left && command && event.modifiers.alt)
        {
            return false;
        }
        if self.document.gesture.is_some() {
            return true;
        }
        let Some(l) = self.piano_layout() else {
            return false;
        };
        self.workspace.gesture = Some(Gesture::PianoPan {
            pointer: event.position,
            offset: (l.scroll_x, l.scroll_y),
            button: event.button,
        });
        true
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
            self.piano.set_offset(offset);
            self.workspace.gesture = Some(Gesture::PianoScroll {
                axis,
                pointer: axis.coordinate(event.position),
                offset,
                scale: max / (track - length),
            });
        } else if x >= KEY_WIDTH && x < l.width - SCROLLBAR && y >= 0. && y < l.height - SCROLLBAR {
            if let Some(project) = &self.project {
                if let Some(beat) = self.selected_clip.as_ref().and_then(|id| {
                    crate::timeline_input::piano_beat(project, id, l.beat(x), self.position_frame())
                }) {
                    self.timeline_click(beat, event);
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
            "end" => (
                (l.length as f32 * l.beat_width - l.grid_width * 0.8).max(0.) - l.scroll_x,
                0.,
            ),
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
        self.piano.set_offset((
            (l.scroll_x + dx).clamp(0., l.max_x),
            (l.scroll_y + dy).clamp(0., l.max_y),
        ));
        true
    }
    pub fn piano_layout(&self) -> Option<PianoLayout> {
        let pattern = self.piano_pattern()?;
        Some(
            self.piano
                .layout(self.piano_period()?, pattern.notes.iter().map(|n| n.pitch)),
        )
    }
    pub fn zoom_piano(&mut self, horizontal: f32, vertical: f32) {
        let Some(old) = self.piano_layout() else {
            return;
        };
        self.zoom_piano_at(
            horizontal,
            vertical,
            (old.grid_width * 0.5, old.grid_height * 0.5),
        );
    }
    pub fn zoom_piano_at(&mut self, horizontal: f32, vertical: f32, anchor: (f32, f32)) {
        let Some(old) = self.piano_layout() else {
            return;
        };
        self.piano.zoom = (self.piano.zoom * horizontal).clamp(1. / 96., 128.);
        self.piano.key_zoom = (self.piano.key_zoom * vertical).clamp(0.5, 4.);
        let new = self.piano_layout().unwrap();
        let anchor = (
            anchor.0.clamp(0., old.grid_width),
            anchor.1.clamp(0., old.grid_height),
        );
        self.piano.set_offset((
            ((old.scroll_x + anchor.0) / old.beat_width * new.beat_width - anchor.0)
                .clamp(0., new.max_x),
            ((old.scroll_y + anchor.1) / old.key_height * new.key_height - anchor.1)
                .clamp(0., new.max_y),
        ));
    }
}
