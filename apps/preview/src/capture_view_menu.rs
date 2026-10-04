//! Native hit testing and key isolation for the compact view dropdown.
use crate::{
    ui::Preview, window_manager::WindowId, window_navigation::VIEWS, workspace_layout::EditorMode,
};
use gpui::*;

#[derive(Default)]
pub struct Smoke {
    stage: u8,
    item: usize,
    revision: u64,
    cue: u64,
    at: Point<Pixels>,
}
impl Smoke {
    pub fn complete(&self) -> bool {
        std::env::var_os("OXITONE_PREVIEW_CAPTURE_VIEW_MENU").is_none() || self.stage == 35
    }
    pub fn step(&mut self, view: &Entity<Preview>, window: &mut Window, cx: &mut App) {
        if self.complete() {
            return;
        }
        match self.stage {
            0 => view.update(cx, |s, cx| {
                self.revision = s.document.view.as_ref().unwrap().revision;
                s.document.show_code = false;
                s.open_view(VIEWS[0], window);
                cx.notify();
            }),
            1 | 6 | 9 | 12 | 17 | 21 | 23 | 26 | 29 | 32 => {
                self.at = view.read(cx).view_menu.button.get().center();
                pointer(window, self.at, false, cx);
            }
            2 | 7 | 10 | 13 | 18 | 22 | 24 | 27 | 30 | 33 => pointer(window, self.at, true, cx),
            3 => {
                let s = view.read(cx);
                assert!(
                    s.view_menu.selected.is_some(),
                    "native Views button opens menu"
                );
                let bounds = s.view_menu.bounds.get();
                assert!(bounds.size.width >= px(190.) && bounds.size.height > px(180.));
                assert!(bounds.left() >= px(0.) && bounds.right() <= window.viewport_size().width);
                self.at = bounds.origin
                    + point(
                        px(70.),
                        px(19.
                            + self.item as f32 * 30.
                            + if self.item >= 4 { 9. } else { 0. }
                            + if self.item == 7 { 9. } else { 0. }),
                    );
                pointer(window, self.at, false, cx);
            }
            4 => pointer(window, self.at, true, cx),
            5 => {
                let s = view.read(cx);
                assert!(s.view_menu.selected.is_none(), "selection dismisses menu");
                assert!(
                    VIEWS[self.item].active(s),
                    "menu must open {}",
                    VIEWS[self.item].label()
                );
                self.item += 1;
                if view.read(cx).show_about {
                    press("escape", window, cx);
                }
                if self.item < VIEWS.len() {
                    self.stage = 1;
                    return;
                }
            }
            8 => {
                // Capture must precede child handlers even if a child retains focus.
                view.read(cx).piano_focus.focus(window);
                for key in ["delete", "cmd-z", "cmd-w"] {
                    press(key, window, cx);
                }
                assert!(view.read(cx).view_menu.selected.is_some());
                assert!(VIEWS[6].active(view.read(cx)));
                assert!(view.read(cx).document.pending.is_none());
                press("home", window, cx);
                press("up", window, cx);
                assert_eq!(view.read(cx).view_menu.selected, Some(7));
                press("down", window, cx);
                press("down", window, cx);
                press("enter", window, cx);
                assert!(VIEWS[1].active(view.read(cx)));
                assert!(view.read(cx).view_menu.selected.is_none());
            }
            11 => {
                press("escape", window, cx);
                assert!(view.read(cx).view_menu.selected.is_none());
                view.update(cx, |s, cx| {
                    s.close_internal(WindowId::Piano, window);
                    s.open_view(VIEWS[0], window);
                    cx.notify();
                });
            }
            14 => {
                assert!(view.read(cx).view_menu.selected.is_some());
                assert!(!view.read(cx).document.windows.piano_open);
                assert!(view.read(cx).piano_layout().unwrap().height > 100.);
                self.cue = view.read(cx).cue_frame;
                self.at = view.read(cx).piano.origin.get() + point(px(120.), px(80.));
                pointer(window, self.at, false, cx);
            }
            15 => pointer(window, self.at, true, cx),
            16 => {
                let s = view.read(cx);
                assert!(s.view_menu.selected.is_none());
                assert!(s.document.gesture.is_none() && s.document.pending.is_none());
                assert_eq!(
                    s.cue_frame, self.cue,
                    "outside click must not seek the piano"
                );
                assert!(!s.is_playing());
                assert_eq!(s.document.view.as_ref().unwrap().revision, self.revision);
            }
            19 => {
                press("f9", window, cx);
                assert!(view.read(cx).view_menu.selected.is_none());
                assert!(VIEWS[2].active(view.read(cx)));
            }
            20 => {
                press("f5", window, cx);
                assert!(VIEWS[0].active(view.read(cx)));
                assert_eq!(view.read(cx).workspace.mode, EditorMode::Split);
            }
            25 => assert!(
                view.read(cx).view_menu.selected.is_none(),
                "trigger toggles closed"
            ),
            28 => {
                press("tab", window, cx);
                assert!(view.read(cx).view_menu.selected.is_none());
            }
            31 => {
                press("f7", window, cx);
                assert!(view.read(cx).view_menu.selected.is_none());
                assert!(VIEWS[1].active(view.read(cx)));
                press("f5", window, cx);
            }
            34 => {
                let s = view.read(cx);
                assert!(s.view_menu.selected.is_some());
                assert_eq!(s.document.view.as_ref().unwrap().revision, self.revision);
                assert_eq!(cx.windows().len(), 1);
                eprintln!("View menu smoke passed: eight native menu targets, current-view selection, keyboard navigation, Escape/Tab/trigger dismissal, outside dismissal without editing or seeking, F5/F7/F9, unchanged source, one native window");
            }
            _ => unreachable!(),
        }
        self.stage += 1;
    }
}
fn press(key: &str, window: &mut Window, cx: &mut App) {
    window.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx);
}
fn pointer(window: &Window, position: Point<Pixels>, up: bool, cx: &App) {
    crate::capture_pointer::dispatch(
        window,
        if up {
            PlatformInput::MouseUp(MouseUpEvent {
                position,
                ..Default::default()
            })
        } else {
            PlatformInput::MouseDown(MouseDownEvent {
                position,
                ..Default::default()
            })
        },
        cx,
    );
}
