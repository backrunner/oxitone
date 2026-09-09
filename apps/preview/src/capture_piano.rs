//! Real native pointer regression: velocity drawing, open-ended authoring and source Save.
use crate::{document_wire::DocumentOperation, piano_layout::RULER, ui::Preview};
use gpui::*;

#[derive(Default)]
pub struct Smoke {
    stage: u8,
    revision: u64,
    at: Point<Pixels>,
    velocities: Vec<f64>,
}
impl Smoke {
    pub fn complete(&self) -> bool {
        self.stage == 16
    }
    pub fn step(&mut self, view: &Entity<Preview>, window: &mut Window, cx: &mut App) {
        if self.complete() || !view.read(cx).document_ready() {
            return;
        }
        match self.stage {
            0 => view.update(cx, |s, cx| {
                s.document.windows.hidden = true;
                s.document.notes.clear();
                s.piano.fit();
                s.piano_focus.focus(window);
                self.revision = s.document.view.as_ref().unwrap().revision;
                self.velocities = s.pattern_site().unwrap().outputs.iter().map(|o| o.note.velocity).collect();
                cx.notify();
            }),
            1 => {
                let s = view.read(cx);
                let l = s.piano_layout().unwrap();
                self.at = s.piano.origin.get() + point(px(l.x(0.) + 3.), px(l.height - 16.));
                pointer(window, self.at, 0, cx);
            }
            2 => {
                let s = view.read(cx);
                assert!(s.document.gesture.is_some());
                let l = s.piano_layout().unwrap();
                self.at = s.piano.origin.get() + point(px(l.x(0.25) + 3.), px(RULER + l.grid_height + 7.));
                pointer(window, self.at, 1, cx);
            }
            3 => {
                let s = view.read(cx);
                assert_eq!(s.document.view.as_ref().unwrap().revision, self.revision);
                let g = s.document.gesture.as_ref().unwrap();
                assert!(g.changes.iter().any(|c| c.after.velocity < 0.4));
                assert!(g.changes.iter().any(|c| c.after.velocity > 0.9));
                let entity = view.clone();
                crate::capture_pointer::dispatch_checked(window, PlatformInput::MouseUp(MouseUpEvent {
                    position: self.at, ..Default::default()
                }), cx, move |cx| {
                    let s = entity.read(cx);
                    assert!(s.document.pending.is_some());
                    assert!(s.document.pending_notes.is_some());
                });
            }
            4 => {
                let s = view.read(cx);
                assert_eq!(s.document.view.as_ref().unwrap().revision, self.revision + 1);
                assert!(s.document.notes.indices.is_empty(), "drawing all notes does not select a subset for the next stroke");
                window.dispatch_keystroke(Keystroke::parse("cmd-z").unwrap(), cx);
            }
            5 => {
                let s = view.read(cx);
                assert_eq!(s.pattern_site().unwrap().outputs.iter().map(|o| o.note.velocity).collect::<Vec<_>>(), self.velocities);
                window.dispatch_keystroke(Keystroke::parse("cmd-shift-z").unwrap(), cx);
            }
            6 => view.update(cx, |s, cx| {
                let l = s.piano_layout().unwrap();
                s.piano.set_offset((20. * l.beat_width - l.grid_width * 0.5, l.scroll_y));
                s.piano.note_length = 0.5;
                cx.notify();
            }),
            7 => {
                let s = view.read(cx);
                let l = s.piano_layout().unwrap();
                self.at = s.piano.origin.get() + point(px(l.x(20.) + 3.), px(l.y(64) + l.key_height * 0.5));
                pointer(window, self.at, 0, cx);
            }
            8 => {
                let g = view.read(cx).document.gesture.as_ref().unwrap();
                assert_eq!(g.changes[0].after.start, 20.);
                pointer(window, self.at, 2, cx);
            }
            9 => {
                let s = view.read(cx);
                assert_eq!(s.piano_pattern().unwrap().length_beats.to_f64(), 21.);
                assert!(s.pattern_site().unwrap().outputs.iter().any(|o| o.note.start == 20.));
                window.dispatch_keystroke(Keystroke::parse("cmd-z").unwrap(), cx);
            }
            10 => {
                assert!(view.read(cx).piano_pattern().unwrap().length_beats.to_f64() < 20.);
                window.dispatch_keystroke(Keystroke::parse("cmd-shift-z").unwrap(), cx);
            }
            11 => view.update(cx, |s, cx| {
                assert_eq!(s.piano_pattern().unwrap().length_beats.to_f64(), 21.);
                s.document_request(DocumentOperation::Save);
                cx.notify();
            }),
            12 => view.update(cx, |s, cx| {
                assert!(!s.document.view.as_ref().unwrap().modified);
                s.piano.fit();
                cx.notify();
            }),
            13 => {
                // Right click in the velocity lane cannot become the note eraser.
                let s = view.read(cx);
                let l = s.piano_layout().unwrap();
                self.at = s.piano.origin.get() + point(px(l.x(0.) + 3.), px(l.height - 20.));
                crate::capture_pointer::dispatch(window, PlatformInput::MouseDown(MouseDownEvent {
                    position: self.at, button: MouseButton::Right, ..Default::default()
                }), cx);
            }
            14 => {
                assert!(view.read(cx).document.gesture.is_none());
                crate::capture_pointer::dispatch(window, PlatformInput::MouseUp(MouseUpEvent {
                    position: self.at, button: MouseButton::Right, ..Default::default()
                }), cx);
            }
            15 => eprintln!("Piano smoke passed: native velocity stroke, pending projection, Undo/Redo, beyond-end note and length, Save, right-click isolation"),
            _ => {}
        }
        self.stage += 1;
    }
}
fn pointer(window: &Window, at: Point<Pixels>, kind: u8, cx: &App) {
    crate::capture_pointer::dispatch(
        window,
        match kind {
            0 => PlatformInput::MouseDown(MouseDownEvent {
                position: at,
                ..Default::default()
            }),
            1 => PlatformInput::MouseMove(MouseMoveEvent {
                position: at,
                pressed_button: Some(MouseButton::Left),
                ..Default::default()
            }),
            _ => PlatformInput::MouseUp(MouseUpEvent {
                position: at,
                ..Default::default()
            }),
        },
        cx,
    );
}
