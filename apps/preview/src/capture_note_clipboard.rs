//! Clipboard acceptance uses actual Piano focus and keyboard routing.
use crate::{ui::Preview, workspace_layout::EditorMode};
use gpui::*;
#[derive(Default)]
pub struct Smoke {
    stage: u8,
    count: usize,
}
impl Smoke {
    pub fn complete(&self) -> bool {
        std::env::var_os("OXITONE_PREVIEW_CAPTURE_SAMPLES").is_none() || self.stage == 9
    }
    pub fn step(&mut self, view: &Entity<Preview>, window: &mut Window, cx: &mut App) {
        if self.complete() || !view.read(cx).document_ready() {
            return;
        }
        match self.stage {
            0 => view.update(cx, |s, cx| {
                s.selected_clip = Some(
                    s.project.as_ref().unwrap().snapshot.pattern_clips[0]
                        .id
                        .clone(),
                );
                s.workspace.mode = EditorMode::Piano;
                s.workspace.dock_open = true;
                s.piano_focus.focus(window);
                self.count = s.pattern_site().unwrap().outputs.len();
                cx.notify();
            }),
            1 => {
                press("cmd-a", window, cx);
                press("cmd-c", window, cx);
                press("cmd-x", window, cx);
            }
            2 => {
                assert!(view.read(cx).pattern_site().unwrap().outputs.is_empty());
                assert_eq!(view.read(cx).document.notes.clipboard.len(), self.count);
                press("cmd-shift-v", window, cx);
            }
            3 => {
                assert_eq!(
                    view.read(cx).pattern_site().unwrap().outputs.len(),
                    self.count
                );
                assert_eq!(view.read(cx).document.notes.indices.len(), self.count);
                press("cmd-z", window, cx);
            }
            4 => {
                assert!(view.read(cx).pattern_site().unwrap().outputs.is_empty());
                press("cmd-z", window, cx);
            }
            5 => {
                assert_eq!(
                    view.read(cx).pattern_site().unwrap().outputs.len(),
                    self.count
                );
                press("cmd-s", window, cx);
            }
            6 => {
                if view.read(cx).document.view.as_ref().unwrap().modified {
                    return;
                }
                view.update(cx, |s, cx| {
                    s.workspace.dock_open = false;
                    s.workspace.mode = EditorMode::Split;
                    s.workspace_focus.focus(window);
                    cx.notify();
                });
                eprintln!("Piano clipboard smoke passed: focused Copy/Cut/Paste, acknowledged selection, Undo and Save");
                self.stage = 9;
                return;
            }
            _ => unreachable!(),
        }
        self.stage += 1;
    }
}
fn press(key: &str, window: &mut Window, cx: &mut App) {
    window.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx);
}
