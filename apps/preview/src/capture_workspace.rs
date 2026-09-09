//! Mouse events go through GPUI hit testing; pure-controller checks cannot prove divider capture.
use crate::{ui::Preview, workspace_layout::EditorMode};
use gpui::*;
#[derive(Default)]
pub struct Smoke {
    pub stage: u8,
    before: f32,
    at: Point<Pixels>,
}
impl Smoke {
    pub fn complete(&self) -> bool {
        std::env::var_os("OXITONE_PREVIEW_CAPTURE_EDITING").is_none() || self.stage == 17
    }
    pub fn step(&mut self, view: &Entity<Preview>, window: &mut Window, cx: &mut App) {
        if self.complete() {
            return;
        }
        match self.stage {
            0 => {
                view.update(cx, |s, cx| {
                    s.document.show_code = false;
                    s.document.manager.open = false;
                    s.document.automation.open = false;
                    s.workspace.mode = EditorMode::Split;
                    s.workspace.dock = crate::workspace_layout::DockMode::Split;
                    s.workspace.dock_open = true;
                    s.show_scopes = false;
                    cx.notify();
                });
            }
            1 => {
                self.before = view.read(cx).workspace.piano_fraction;
                self.at =
                    view.read(cx).workspace.dividers.borrow()["editor-width-divider"].center();
                crate::capture_pointer::dispatch(
                    window,
                    PlatformInput::MouseDown(MouseDownEvent {
                        position: self.at,
                        ..Default::default()
                    }),
                    cx,
                );
            }
            2 => {
                assert!(
                    view.read(cx).workspace.gesture.is_some(),
                    "divider must be hit through GPUI"
                );
                crate::capture_pointer::dispatch(
                    window,
                    PlatformInput::MouseMove(MouseMoveEvent {
                        position: self.at - point(px(120.), px(0.)),
                        pressed_button: Some(MouseButton::Left),
                        ..Default::default()
                    }),
                    cx,
                );
            }
            3 => {
                assert!(
                    view.read(cx).workspace.piano_fraction < self.before - 0.05,
                    "drag must cross sibling hitboxes"
                );
                crate::capture_pointer::dispatch(
                    window,
                    PlatformInput::MouseUp(MouseUpEvent {
                        position: self.at - point(px(120.), px(0.)),
                        ..Default::default()
                    }),
                    cx,
                );

                self.before = view.read(cx).workspace.editor_height;
                self.at =
                    view.read(cx).workspace.dividers.borrow()["editor-height-divider"].center();
                crate::capture_pointer::dispatch(
                    window,
                    PlatformInput::MouseDown(MouseDownEvent {
                        position: self.at,
                        ..Default::default()
                    }),
                    cx,
                );
            }
            4 => {
                crate::capture_pointer::dispatch(
                    window,
                    PlatformInput::MouseMove(MouseMoveEvent {
                        position: self.at + point(px(0.), px(70.)),
                        pressed_button: Some(MouseButton::Left),
                        ..Default::default()
                    }),
                    cx,
                );
            }
            5 => {
                assert!(view.read(cx).workspace.editor_height < self.before - 20.);
                crate::capture_pointer::dispatch(
                    window,
                    PlatformInput::MouseUp(MouseUpEvent {
                        position: self.at + point(px(0.), px(70.)),
                        ..Default::default()
                    }),
                    cx,
                );
                window.dispatch_keystroke(Keystroke::parse("f7").unwrap(), cx);
                assert_eq!(view.read(cx).workspace.mode, EditorMode::Piano);
            }
            6 => {
                let s = view.read(cx);
                let l = s.piano_layout().unwrap();
                assert!(l.width > f32::from(window.viewport_size().width) - 5.);
                assert!(
                    l.height > f32::from(s.workspace.bounds.get().size.height) - 145.,
                    "piano canvas must occupy the workspace below its two toolbars and footer"
                );
                self.before = l.velocity_height;
                self.at = s.workspace.dividers.borrow()["piano-velocity-divider"].center();
                assert!(
                    (f32::from(self.at.y - s.piano.origin.get().y)
                        - crate::piano_layout::RULER
                        - l.grid_height)
                        .abs()
                        < 1.,
                    "divider hitbox must follow the visible velocity boundary after maximizing: {:?}, origin {:?}, layout {:?}",
                    self.at, s.piano.origin.get(), l
                );
                crate::capture_pointer::dispatch(
                    window,
                    PlatformInput::MouseDown(MouseDownEvent {
                        position: self.at,
                        ..Default::default()
                    }),
                    cx,
                );
            }
            7 => {
                assert!(
                    matches!(view.read(cx).workspace.gesture,
                        Some(crate::workspace::Gesture::Resize {
                            region: crate::workspace_resize::Resize::Velocity { .. }, ..
                        })),
                    "velocity divider must be hit at {:?}; bounds {:?}, layout height {}, velocity {}",
                    self.at,
                    view.read(cx).workspace.dividers.borrow()["piano-velocity-divider"],
                    view.read(cx).piano_layout().unwrap().height,
                    view.read(cx).piano_layout().unwrap().velocity_height,
                );
                crate::capture_pointer::dispatch(
                    window,
                    PlatformInput::MouseMove(MouseMoveEvent {
                        position: self.at - point(px(0.), px(30.)),
                        pressed_button: Some(MouseButton::Left),
                        ..Default::default()
                    }),
                    cx,
                );
            }
            8 => {
                assert!(
                    view.read(cx).piano.velocity_height > self.before + 20.,
                    "velocity height {} must grow from {}",
                    view.read(cx).piano.velocity_height,
                    self.before
                );
                crate::capture_pointer::dispatch(
                    window,
                    PlatformInput::MouseUp(MouseUpEvent {
                        position: self.at - point(px(0.), px(30.)),
                        ..Default::default()
                    }),
                    cx,
                );
                window.dispatch_keystroke(Keystroke::parse("f9").unwrap(), cx);
                assert_eq!(view.read(cx).workspace.mode, EditorMode::Mixer);
            }
            9 => {
                assert!(f32::from(view.read(cx).workspace.mixer.bounds().size.height) > 400.);
                window.dispatch_keystroke(Keystroke::parse("f5").unwrap(), cx);
                assert_eq!(view.read(cx).workspace.mode, EditorMode::Split);
            }
            10 => {
                view.update(cx, |s, cx| {
                    s.show_scopes = true;
                    cx.notify();
                });
            }
            11 => {
                assert!(view
                    .read(cx)
                    .workspace
                    .dividers
                    .borrow()
                    .contains_key("analysis-height-divider"));
                view.update(cx, |s, cx| {
                    s.show_scopes = false;
                    s.workspace.mode = EditorMode::Piano;
                    s.document.notes.clear();
                    s.piano.fit();
                    s.zoom_piano(2., 1.);
                    cx.notify();
                });
            }
            12 => {
                view.update(cx, |s, cx| {
                    let l = s.piano_layout().unwrap();
                    let at = s.piano.origin.get() + point(px(400.), px(150.));
                    assert!(s.pan_piano(&MouseDownEvent {
                        button: MouseButton::Middle,
                        position: at,
                        ..Default::default()
                    }));
                    s.move_gesture(
                        &MouseMoveEvent {
                            position: at - point(px(80.), px(40.)),
                            pressed_button: Some(MouseButton::Middle),
                            ..Default::default()
                        },
                        window,
                    );
                    let after = s.piano_layout().unwrap();
                    assert!((after.scroll_x - l.scroll_x - 80.).abs() < 0.1);
                    assert!((after.scroll_y - l.scroll_y - 40.).abs() < 0.1);
                    assert!(s.document.gesture.is_none() && s.document.pending.is_none());
                    s.workspace.gesture = None;
                    s.workspace.mode = EditorMode::Split;
                    s.workspace.dock = crate::workspace_layout::DockMode::Piano;
                    s.piano.fit();
                    cx.notify();
                });
            }
            13 => {
                assert!(
                    view.read(cx).piano_layout().unwrap().width
                        > f32::from(window.viewport_size().width) - 5.
                );
                view.update(cx, |s, cx| {
                    s.workspace.dock = crate::workspace_layout::DockMode::Mixer;
                    cx.notify();
                });
            }
            14 => {
                assert!(f32::from(view.read(cx).workspace.mixer.bounds().size.width) > 900.);
                view.update(cx, |s, cx| {
                    s.workspace.dock_open = false;
                    cx.notify();
                });
            }
            15 => {
                assert!(f32::from(view.read(cx).workspace.arrangement.bounds().size.height) > 500.);
                view.update(cx, |s, cx| {
                    s.workspace.dock_open = true;
                    s.workspace.dock = crate::workspace_layout::DockMode::Piano;
                    s.workspace.editor_height = 380.;
                    s.workspace.mode =
                        match std::env::var("OXITONE_PREVIEW_CAPTURE_WORKSPACE").as_deref() {
                            Ok("mixer") => EditorMode::Mixer,
                            Ok("split") => EditorMode::Split,
                            _ => EditorMode::Piano,
                        };
                    if std::env::var_os("OXITONE_PREVIEW_CAPTURE_AUTOMATION").is_some() {
                        s.workspace.mode = EditorMode::Split;
                        s.document.automation.open = true;
                        s.document
                            .windows
                            .focus(crate::window_manager::WindowId::Automation);
                    }
                    cx.notify();
                });
            }
            16 => {
                eprintln!("Workspace smoke passed: hit-tested dividers, sibling-crossing capture, velocity resize, full editors, dock switching/collapse, piano pan, analysis collapse");
            }
            _ => {}
        }
        self.stage += 1;
    }
}
