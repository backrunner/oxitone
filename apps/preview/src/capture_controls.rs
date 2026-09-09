//! Real pointer and keyboard smoke for source-backed mixing, Playlist and plugin assignment.
#[path = "capture_controls_plugins.rs"]
mod plugins;
use crate::{
    plugin_picker::Slot,
    ui::Preview,
    window_manager::{WindowBounds, WindowId},
    workspace_layout::EditorMode,
};
use gpui::*;
#[derive(Default)]
pub struct Smoke {
    up: Option<Point<Pixels>>,
    cancel_moved: bool,
    configured: bool,
    stage: u8,
    revision: u64,
    owner: String,
    at: Point<Pixels>,
    clips: usize,
    instance: String,
    track_drag: crate::capture_track_drag::Smoke,
}
impl Smoke {
    pub fn complete(&self) -> bool {
        std::env::var_os("OXITONE_PREVIEW_CAPTURE_CONTROLS").is_none() || self.stage == 27
    }
    pub fn step(&mut self, view: &Entity<Preview>, window: &mut Window, cx: &mut App) {
        if self.complete() || !view.read(cx).document_ready() {
            return;
        }
        if let Some(at) = self.up.take() {
            pointer(window, at, 2, cx);
            return;
        }
        let revision = view.read(cx).document.view.as_ref().unwrap().revision;
        match self.stage {
            0 => {
                self.revision = revision;
                view.update(cx, |s, cx| {
                    self.owner = s.project.as_ref().unwrap().snapshot.channels[0].id.clone();
                    self.clips = s.project.as_ref().unwrap().snapshot.pattern_clips.len();
                    s.document.manager.open = false;
                    s.document.configuration.open = false;
                    s.document.show_code = false;
                    s.show_shortcuts = false;
                    s.float_editor(EditorMode::Mixer);
                    s.document.windows.set_bounds(
                        WindowId::Mixer,
                        WindowBounds {
                            x: 8.,
                            y: 8.,
                            width: 750.,
                            height: 520.,
                        },
                    );
                    cx.notify();
                });
            }
            1 => {
                let bounds =
                    view.read(cx).document.mixer.bounds.borrow()[&format!("{}-level", self.owner)];
                self.at = bounds.origin + point(px(40.), px(88.));
                pointer(window, self.at, 0, cx);
            }
            2 => {
                assert!(
                    view.read(cx).document.mixer.gesture.is_some(),
                    "fader hit starts gesture"
                );
                self.at.y -= px(8.);
                pointer(window, self.at, 1, cx);
            }
            3 => {
                pointer(window, self.at, 2, cx);
            }
            4 => {
                assert_eq!(
                    revision,
                    self.revision + 1,
                    "one fader release = one source revision"
                );
                assert!(view.read(cx).project.as_ref().unwrap().snapshot.channels[0].level > 1.);
                let bounds =
                    view.read(cx).document.mixer.bounds.borrow()[&format!("{}-pan", self.owner)];
                self.at = bounds.center();
                pointer(window, self.at, 0, cx);
            }
            5 => {
                self.at.x += px(25.);
                pointer(window, self.at, 1, cx);
            }
            6 => {
                pointer(window, self.at, 2, cx);
            }
            7 => {
                assert!(
                    (view.read(cx).project.as_ref().unwrap().snapshot.channels[0].pan - 0.25).abs()
                        < 0.001
                );
                let bounds =
                    view.read(cx).document.mixer.bounds.borrow()[&format!("{}-mute", self.owner)];
                self.at = bounds.center();
                pointer(window, self.at, 0, cx);
                self.up = Some(self.at);
            }
            8 => {
                assert_eq!(
                    view.read(cx).project.as_ref().unwrap().snapshot.channels[0].mute,
                    Some(true)
                );
                self.revision = revision;
                let bounds =
                    view.read(cx).document.mixer.bounds.borrow()[&format!("{}-level", self.owner)];
                self.at = bounds.origin + point(px(40.), px(88.));
                pointer(window, self.at, 0, cx);
            }
            9 => {
                if !self.cancel_moved {
                    self.at.y += px(20.);
                    pointer(window, self.at, 1, cx);
                    self.cancel_moved = true;
                    return;
                }
                window.dispatch_keystroke(Keystroke::parse("escape").unwrap(), cx);
                pointer(window, self.at, 2, cx);
                assert!(view.read(cx).document.mixer.gesture.is_none());
                assert_eq!(
                    view.read(cx).document.view.as_ref().unwrap().revision,
                    self.revision
                );
                view.update(cx, |s, cx| {
                    s.document.windows.hidden = true;
                    cx.notify();
                });
            }
            10 => {
                let s = view.read(cx);
                let p = s.project.as_ref().unwrap();
                let c = &p.snapshot.pattern_clips[0];
                let row = s.document.playlist.rows.borrow()[&c.track_id];
                self.at =
                    row.origin + point(px((c.start_beat.to_f64() as f32 + 0.25) * s.zoom), px(20.));
                pointer(window, self.at, 0, cx);
                self.up = Some(self.at);
            }
            11 => {
                assert!(view.read(cx).document.playlist.selected.is_some());
                window.dispatch_keystroke(Keystroke::parse("cmd-d").unwrap(), cx);
            }
            12 => {
                assert_eq!(
                    view.read(cx)
                        .project
                        .as_ref()
                        .unwrap()
                        .snapshot
                        .pattern_clips
                        .len(),
                    self.clips + 1
                );
                window.dispatch_keystroke(Keystroke::parse("cmd-z").unwrap(), cx);
            }
            13..=18 => {
                if !self.plugins(view, window, cx) {
                    return;
                }
            }
            19 => {
                let bounds = view.read(cx).document.tempo.bounds.get();
                pointer(window, bounds.center(), 0, cx);
                self.up = Some(bounds.center());
            }
            20 => {
                assert!(view.read(cx).document.tempo.input.is_some());
                for k in ["1", "3", "2", "enter"] {
                    window.dispatch_keystroke(Keystroke::parse(k).unwrap(), cx);
                }
            }
            21 => {
                assert_eq!(
                    view.read(cx).project.as_ref().unwrap().snapshot.tempo_map[0].bpm,
                    132.
                );
                window.dispatch_keystroke(Keystroke::parse("cmd-s").unwrap(), cx);
            }
            22 => {
                if view.read(cx).document.view.as_ref().unwrap().modified {
                    return;
                }
                view.update(cx, |s, cx| {
                    s.document.windows.hidden = false;
                    s.document.windows.focus(WindowId::Mixer);
                    cx.notify();
                });
            }
            24 => {
                if !self.track_drag.step(view, window, cx) {
                    return;
                }
            }
            26 => {
                eprintln!("Controls smoke passed: fader/pan/mute hit testing, Escape, Playlist copy/Undo, plugin add/replace/remove, tempo and Save");
            }
            _ => {}
        }
        self.stage += 1;
    }
}
fn pointer(window: &Window, position: Point<Pixels>, phase: u8, cx: &App) {
    let event = match phase {
        0 => PlatformInput::MouseDown(MouseDownEvent {
            position,
            ..Default::default()
        }),
        1 => PlatformInput::MouseMove(MouseMoveEvent {
            position,
            pressed_button: Some(MouseButton::Left),
            ..Default::default()
        }),
        _ => PlatformInput::MouseUp(MouseUpEvent {
            position,
            ..Default::default()
        }),
    };
    crate::capture_pointer::dispatch(window, event, cx);
}
