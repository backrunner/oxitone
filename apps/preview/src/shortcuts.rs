use crate::{playback_controls::frame, ui::Preview};
use gpui::*;
use oxitone_core::Beat;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackShortcut {
    Toggle,
    Stop,
    Restart,
    Step(i32, bool),
    Start,
    End,
    Marker(i32),
    Loop,
}
impl PlaybackShortcut {
    pub fn repeats(self) -> bool {
        matches!(self, Self::Step(..) | Self::Marker(_))
    }
}
pub fn playback(key: &Keystroke) -> Option<PlaybackShortcut> {
    use PlaybackShortcut::*;
    let m = key.modifiers;
    if m.platform || m.control {
        return if !m.alt && !m.shift {
            match key.key.as_str() {
                "home" => Some(Start),
                "end" => Some(End),
                _ => None,
            }
        } else {
            None
        };
    }
    if m.alt {
        return match key.key.as_str() {
            "left" => Some(Step(-1, m.shift)),
            "right" => Some(Step(1, m.shift)),
            _ => None,
        };
    }
    if m.shift {
        return (key.key == "space").then_some(Stop);
    }
    match key.key.as_str() {
        "space" => Some(Toggle),
        "enter" => Some(Restart),
        "l" => Some(Loop),
        "[" => Some(Marker(-1)),
        "]" => Some(Marker(1)),
        _ => None,
    }
}

pub fn marker(project: &crate::model::ViewProject, at: u64, direction: i32) -> u64 {
    let positions = project
        .snapshot
        .markers
        .iter()
        .map(|m| frame(project, m.start_beat.to_f64()));
    if direction < 0 {
        positions.filter(|p| *p < at).max().unwrap_or(0)
    } else {
        positions
            .filter(|p| *p > at)
            .min()
            .unwrap_or_else(|| frame(project, project.end()))
    }
}
pub fn step(project: &crate::model::ViewProject, at: u64, direction: i32, bar: bool) -> u64 {
    let current = project.plan.tempo.frame_to_beat(at);
    let distance = if bar {
        let map = &project.plan.time_signatures;
        let (bar, _) = map.beat_to_bar_beat(current);
        let start = map.bar_beat_to_beat(bar, Beat::ZERO).unwrap();
        let next = map
            .bar_beat_to_beat(bar.saturating_add(1), Beat::ZERO)
            .unwrap_or(start);
        next.to_f64() - start.to_f64()
    } else {
        1.
    };
    frame(
        project,
        (current.to_f64() + f64::from(direction) * distance).clamp(0., project.end()),
    )
}

impl Preview {
    pub fn playback_shortcut(&mut self, shortcut: PlaybackShortcut) {
        use PlaybackShortcut::*;
        let Some(project) = self.project.clone() else {
            return;
        };
        match shortcut {
            Toggle => self.toggle_playback(),
            Stop => self.stop_at_cue(),
            Restart => self.play_from(self.cue_frame),
            Loop => self.toggle_loop(),
            Start => self.locate_frame(0),
            End => self.locate_frame(frame(&project, project.end())),
            Marker(dir) => self.locate_frame(marker(&project, self.position_frame(), dir)),
            Step(dir, bar) => self.locate_frame(step(&project, self.position_frame(), dir, bar)),
        }
    }
    pub fn workspace_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = &event.keystroke;
        if key.key == "escape" && self.document.plugin.gesture.is_some() {
            self.document.plugin.cancel();
            cx.stop_propagation();
            cx.notify();
            return;
        }
        if (key.modifiers.platform || key.modifiers.control)
            && key.key == "w"
            && !self.document.windows.hidden
        {
            if let Some(id) = self.document.windows.visible.last().copied() {
                self.close_internal(id, window);
                cx.stop_propagation();
                cx.notify();
                return;
            }
        }
        if key.key == "escape" && self.document.windows.drag.is_some() {
            self.document.windows.cancel();
            cx.stop_propagation();
            cx.notify();
            return;
        }
        if self.configuration_key(event, cx) {
            return;
        }
        if self.tempo_key(event) {
            cx.stop_propagation();
            cx.notify();
            return;
        }
        if self.plugin_search_key(event, cx) {
            return;
        }
        let key = &event.keystroke;
        if key.key == "escape" && self.workspace.gesture.is_some() {
            if let Some(crate::workspace::Gesture::Resize { region, .. }) =
                self.workspace.gesture.take()
            {
                crate::workspace_resize::apply(self, region, point(px(0.), px(0.)));
            }
            cx.stop_propagation();
            cx.notify();
            return;
        }
        if !key.modifiers.platform && !key.modifiers.control && !key.modifiers.alt {
            let mode = match key.key.as_str() {
                "f7" => Some(crate::workspace_layout::EditorMode::Piano),
                "f9" => Some(crate::workspace_layout::EditorMode::Mixer),
                "f5" => Some(crate::workspace_layout::EditorMode::Split),
                _ => None,
            };
            if let Some(mode) = mode {
                if !event.is_held {
                    self.toggle_editor(mode, window);
                }
                cx.stop_propagation();
                cx.notify();
                return;
            }
        }
        if self.document.view.is_some()
            && (key.modifiers.platform || key.modifiers.control)
            && !key.modifiers.alt
        {
            let operation = match key.key.as_str() {
                "s" => Some(crate::document_wire::DocumentOperation::Save),
                "z" if key.modifiers.shift => Some(crate::document_wire::DocumentOperation::Redo),
                "z" => Some(crate::document_wire::DocumentOperation::Undo),
                _ => None,
            };
            if let Some(operation) = operation {
                if !event.is_held {
                    self.document_request(operation);
                }
                cx.stop_propagation();
                cx.notify();
                return;
            }
        }
        if key.key == "escape"
            && (self.document.gesture.is_some()
                || self.document.notes.marquee.is_some()
                || self.document.automation.gesture.is_some()
                || self.document.playlist.drag.is_some())
            || key.key == "escape" && self.document.mixer.gesture.is_some()
        {
            self.document.gesture = None;
            self.document.notes.marquee = None;
            self.document.automation.gesture = None;
            self.document.playlist.drag = None;
            self.document.mixer.gesture = None;
            cx.stop_propagation();
            cx.notify();
            return;
        }
        if self.show_shortcuts {
            if key.key == "escape" || key.key == "?" {
                self.show_shortcuts = false;
            }
            cx.stop_propagation();
            cx.notify();
            return;
        }
        let plain = !key.modifiers.platform && !key.modifiers.control && !key.modifiers.alt;
        if self.workspace_focus.is_focused(window) && self.playlist_key(event) {
            cx.stop_propagation();
            cx.notify();
            return;
        }
        if plain && (key.key == "?" || (key.key == "/" && key.modifiers.shift)) {
            self.show_shortcuts = true;
            self.workspace_focus.focus(window);
        } else if let Some(shortcut) = playback(key) {
            if !event.is_held || shortcut.repeats() {
                self.playback_shortcut(shortcut);
            }
        } else {
            return;
        }
        cx.stop_propagation();
        cx.notify();
    }
}
