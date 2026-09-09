//! Real hit-tested internal-window and Browser-drop regression.
use crate::{
    ui::Preview,
    window_manager::{WindowBounds, WindowId},
    workspace_layout::EditorMode,
};
use gpui::*;
#[derive(Default)]
pub struct Smoke {
    stage: u8,
    id: Option<WindowId>,
    at: Point<Pixels>,
    original: Option<WindowBounds>,
    revision: u64,
    clips: usize,
}
impl Smoke {
    pub fn complete(&self) -> bool {
        std::env::var_os("OXITONE_PREVIEW_CAPTURE_WINDOWS").is_none() || self.stage == 21
    }
    pub fn step(&mut self, view: &Entity<Preview>, window: &mut Window, cx: &mut App) {
        if self.complete() {
            return;
        }
        match self.stage {
            0 => {
                view.update(cx, |s, cx| {
                    s.document.show_code = false;
                    s.show_scopes = false;
                    s.document.patterns_open = true;
                    s.document.automation.open = true;
                    s.document.manager.open = true;
                    s.float_editor(EditorMode::Piano);
                    s.float_editor(EditorMode::Mixer);
                    let channel = s.project.as_ref().unwrap().snapshot.channels[0].id.clone();
                    let plugin = s
                        .open_plugin(crate::plugin_details::DetailTarget::Instrument(channel), cx)
                        .unwrap();
                    self.id = Some(WindowId::Plugin(plugin.entity_id().as_u64()));
                    cx.notify();
                });
            }
            1 => {
                assert_eq!(
                    cx.windows().len(),
                    1,
                    "plugins must not create native windows"
                );
                let s = view.read(cx);
                assert_eq!(s.document.windows.visible.len(), 6);
                let b = s.document.windows.bounds(self.id.unwrap());
                self.original = Some(b);
                self.at =
                    s.document.windows.desktop.get().origin + point(px(b.x + 100.), px(b.y + 14.));
                down(window, self.at, cx);
            }
            2 => {
                assert!(
                    view.read(cx).document.windows.drag.is_some(),
                    "title must receive actual mouse hit"
                );
                motion(window, self.at + point(px(-30.), px(20.)), cx);
            }
            3 => {
                assert_ne!(
                    view.read(cx).document.windows.bounds(self.id.unwrap()),
                    self.original.unwrap()
                );
                up(window, self.at + point(px(-30.), px(20.)), cx);
            }
            4 => {
                let s = view.read(cx);
                let b = s.document.windows.bounds(self.id.unwrap());
                self.original = Some(b);
                self.at = s.document.windows.desktop.get().origin
                    + point(px(b.x + 2.), px(b.y + b.height - 2.));
                down(window, self.at, cx);
            }
            5 => {
                assert!(view.read(cx).document.windows.drag.is_some());
                motion(window, self.at + point(px(60.), px(-30.)), cx);
            }
            6 => {
                up(window, self.at + point(px(60.), px(-30.)), cx);
            }
            7 => {
                let s = view.read(cx);
                let b = s.document.windows.bounds(self.id.unwrap());
                assert!(b.width < self.original.unwrap().width - 40.);
                self.original = Some(b);
                self.at = s.document.windows.desktop.get().origin
                    + point(px(b.x + b.width - 45.), px(b.y + 14.));
                down(window, self.at, cx);
            }
            8 => {
                up(window, self.at, cx);
            }
            9 => {
                let s = view.read(cx);
                assert!(s.document.windows.state(self.id.unwrap()).maximized);
                let b = s.document.windows.bounds(self.id.unwrap());
                assert_eq!(b.y, 0.);
                assert_eq!(
                    b.height,
                    f32::from(s.document.windows.desktop.get().size.height)
                );
                self.at =
                    s.document.windows.desktop.get().origin + point(px(b.width - 45.), px(14.));
                down(window, self.at, cx);
            }
            10 => {
                up(window, self.at, cx);
            }
            11 => {
                assert_eq!(
                    view.read(cx).document.windows.bounds(self.id.unwrap()),
                    self.original.unwrap()
                );
                window.dispatch_keystroke(Keystroke::parse("cmd-w").unwrap(), cx);
            }
            12 => {
                assert!(
                    view.read(cx).plugin_windows.is_empty(),
                    "close removes only embedded plugin view"
                );
                assert_eq!(cx.windows().len(), 1);
                view.update(cx, |s, cx| {
                    s.document.windows.piano_open = false;
                    s.document.windows.mixer_open = false;
                    s.document.manager.open = false;
                    s.document.automation.open = false;
                    s.workspace.dock_open = false;
                    s.document.windows.focus(WindowId::Patterns);
                    s.document.windows.set_bounds(
                        WindowId::Patterns,
                        WindowBounds {
                            x: 12.,
                            y: 12.,
                            width: 244.,
                            height: 320.,
                        },
                    );
                    self.revision = s.document.view.as_ref().unwrap().revision;
                    self.clips = s.project.as_ref().unwrap().snapshot.pattern_clips.len();
                    cx.notify();
                });
            }
            13 => {
                let s = view.read(cx);
                self.at = s.document.windows.desktop.get().origin
                    + point(px(90.), px(12. + 28. + 4. + 26. + 14.));
                down(window, self.at, cx);
            }
            14 => {
                let s = view.read(cx);
                assert!(
                    s.document.playlist.drag.is_some(),
                    "Browser resource starts a real drag"
                );
                let row = *s
                    .document
                    .playlist
                    .rows
                    .borrow()
                    .values()
                    .min_by(|a, b| a.origin.y.partial_cmp(&b.origin.y).unwrap())
                    .unwrap();
                self.at = point(row.origin.x + px(240.), row.origin.y + px(24.));
                motion(window, self.at, cx);
            }
            15 => {
                assert!(view
                    .read(cx)
                    .document
                    .playlist
                    .drag
                    .as_ref()
                    .unwrap()
                    .target
                    .is_some());
                up(window, self.at, cx);
            }
            16 => {
                let s = view.read(cx);
                if !s.document_ready()
                    || s.document.view.as_ref().unwrap().revision == self.revision
                {
                    return;
                }
                assert_eq!(
                    s.project.as_ref().unwrap().snapshot.pattern_clips.len(),
                    self.clips + 1
                );
                window.dispatch_keystroke(Keystroke::parse("cmd-s").unwrap(), cx);
            }
            17 => {
                if view.read(cx).document.view.as_ref().unwrap().modified {
                    return;
                }
                view.update(cx, |s, cx| {
                    s.float_editor(EditorMode::Mixer);
                    s.float_editor(EditorMode::Piano);
                    let size = s.document.windows.desktop.get().size;
                    let (w, h) = (f32::from(size.width), f32::from(size.height));
                    s.document.windows.set_bounds(
                        WindowId::Mixer,
                        WindowBounds {
                            x: 260.,
                            y: h * 0.48,
                            width: w - 272.,
                            height: h * 0.5,
                        },
                    );
                    s.document.windows.set_bounds(
                        WindowId::Piano,
                        WindowBounds {
                            x: 260.,
                            y: 8.,
                            width: w - 272.,
                            height: h * 0.46,
                        },
                    );
                    cx.notify();
                });
            }
            20 => {
                eprintln!("Internal windows smoke passed: one native window, six embedded views, hit-tested move/resize/maximize/restore/close, Browser drop and Save");
            }
            _ => {}
        }
        self.stage += 1;
    }
}
fn down(window: &Window, position: Point<Pixels>, cx: &App) {
    crate::capture_pointer::dispatch(
        window,
        PlatformInput::MouseDown(MouseDownEvent {
            position,
            ..Default::default()
        }),
        cx,
    );
}
fn up(window: &Window, position: Point<Pixels>, cx: &App) {
    crate::capture_pointer::dispatch(
        window,
        PlatformInput::MouseUp(MouseUpEvent {
            position,
            ..Default::default()
        }),
        cx,
    );
}
fn motion(window: &Window, position: Point<Pixels>, cx: &App) {
    crate::capture_pointer::dispatch(
        window,
        PlatformInput::MouseMove(MouseMoveEvent {
            position,
            pressed_button: Some(MouseButton::Left),
            ..Default::default()
        }),
        cx,
    );
}
