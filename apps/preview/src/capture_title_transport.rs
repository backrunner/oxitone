//! Hit-tested title transport and native menu isolation on the simulated DAW fixture.
use crate::{native_menu, ui::Preview};
use gpui::*;

#[derive(Default)]
pub struct Smoke {
    stage: u8,
    at: Point<Pixels>,
    revision: u64,
    cursor: u64,
    loop_before: bool,
}
impl Smoke {
    pub fn complete(&self) -> bool {
        self.stage == 16
    }

    pub fn step(&mut self, view: &Entity<Preview>, window: &mut Window, cx: &mut App) {
        match self.stage {
            0 => {
                let s = view.read(cx);
                let bounds = s.transport_bounds.get();
                assert!(bounds.left() >= px(100.) && bounds.top() >= px(4.));
                assert!(bounds.bottom() <= px(40.));
                assert!(bounds.right() + px(4.) <= s.workflow_bounds.get().left());
                assert!(s.workflow_bounds.get().right() < s.view_menu.button.get().left());
                assert!(s.view_menu.button.get().right() <= window.viewport_size().width);
                assert_eq!(s.document.windows.desktop.get().top(), px(44.));
                self.revision = s.document.view.as_ref().unwrap().revision;
                self.cursor = s.position_frame();
                self.at = point(bounds.left() + px(14.), bounds.center().y);
                pointer(window, self.at, false, cx);
            }
            1 | 4 | 7 => pointer(window, self.at, true, cx),
            2 => {
                let s = view.read(cx);
                if s.requested_playing.is_some() || !s.playback.playing {
                    return;
                }
                assert!(
                    s.playback.cursor > self.cursor,
                    "native playback must advance"
                );
            }
            3 => pointer(window, self.at, false, cx),
            5 => {
                let s = view.read(cx);
                if s.requested_playing.is_some() {
                    return;
                }
                assert!(!s.playback.playing, "title Play becomes Pause");
                self.at.x += px(32.);
            }
            6 => pointer(window, self.at, false, cx),
            8 => {
                let s = view.read(cx);
                if s.requested_playing.is_some() || s.requested_position.is_some() {
                    return;
                }
                assert!(!s.playback.playing && s.playback.cursor == s.cue_frame);
                self.loop_before = s.loop_enabled;
                window.dispatch_action(Box::new(native_menu::ToggleLoop), cx);
            }
            9 => {
                assert_ne!(view.read(cx).loop_enabled, self.loop_before);
                window.dispatch_action(Box::new(native_menu::ToggleLoop), cx);
            }
            10 => {
                assert_eq!(view.read(cx).loop_enabled, self.loop_before);
                window.dispatch_action(Box::new(crate::about::ShowAbout), cx);
            }
            11 => {
                assert!(view.read(cx).show_about);
                blocked_commands(window, cx);
            }
            12 => {
                self.assert_blocked(view, cx);
                window.dispatch_keystroke(Keystroke::parse("escape").unwrap(), cx);
                view.update(cx, |s, cx| {
                    s.document.tempo.input = Some("120".into());
                    cx.notify();
                });
            }
            13 => blocked_commands(window, cx),
            14 => {
                self.assert_blocked(view, cx);
                view.update(cx, |s, cx| {
                    assert_eq!(s.document.tempo.input.as_deref(), Some("120"));
                    s.document.tempo.input = None;
                    cx.notify();
                });
            }
            15 => {
                assert_eq!(
                    view.read(cx).document.view.as_ref().unwrap().revision,
                    self.revision
                );
                eprintln!("Title transport smoke passed: titlebar bounds, native Play/Pause/Stop, cue return, native loop menu, About/input isolation, unchanged source");
            }
            _ => unreachable!(),
        }
        self.stage += 1;
    }

    fn assert_blocked(&self, view: &Entity<Preview>, cx: &App) {
        let s = view.read(cx);
        assert!(s.document.pending.is_none());
        assert_eq!(s.loop_enabled, self.loop_before);
        assert_eq!(s.document.view.as_ref().unwrap().revision, self.revision);
    }
}

fn blocked_commands(window: &mut Window, cx: &mut App) {
    window.dispatch_action(Box::new(native_menu::SaveProject), cx);
    window.dispatch_action(Box::new(native_menu::UndoEdit), cx);
    window.dispatch_action(Box::new(native_menu::RedoEdit), cx);
    window.dispatch_action(Box::new(native_menu::ToggleLoop), cx);
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
