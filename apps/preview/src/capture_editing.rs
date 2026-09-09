//! Opt-in regression on a temporary project, using the real GPUI focus and document pipeline.
use crate::{document_wire::SourceNote, ui::Preview};
use gpui::*;

#[derive(Default)]
pub struct Smoke {
    pub stage: u8,
    revision: u64,
    notes: Vec<SourceNote>,
    source: Option<oxitone_core::wire::AutomationSourceSpec>,
    piano: crate::capture_piano::Smoke,
}
impl Smoke {
    pub fn complete(&self) -> bool {
        std::env::var_os("OXITONE_PREVIEW_CAPTURE_EDITING").is_none()
            || (self.stage == 10 && (self.source.is_some() || self.piano.complete()))
    }
    pub fn step(&mut self, view: &Entity<Preview>, window: &mut Window, cx: &mut App) {
        if self.complete() || !view.read(cx).document_ready() {
            return;
        }
        if view.read(cx).document.automation.open {
            self.curve(view, window, cx);
            return;
        }
        if self.stage == 10 {
            self.piano.step(view, window, cx);
            return;
        }
        let revision = view.read(cx).document.view.as_ref().unwrap().revision;
        match self.stage {
            0 => {
                self.revision = revision;
                self.notes = view
                    .read(cx)
                    .pattern_site()
                    .unwrap()
                    .outputs
                    .iter()
                    .map(|o| o.note.clone())
                    .collect();
                view.update(cx, |state, cx| {
                    state.document.show_code = false;
                    state.document.manager.open = false;
                    state.piano_focus.focus(window);
                    cx.notify();
                });
                self.stage = 1;
            }
            1 => {
                window.dispatch_keystroke(Keystroke::parse("cmd-a").unwrap(), cx);
                assert_eq!(view.read(cx).document.notes.indices.len(), self.notes.len());
                window.dispatch_keystroke(Keystroke::parse("right").unwrap(), cx);
                assert!(view.read(cx).document.pending.is_some());
                self.stage = 2;
            }
            2 if revision == self.revision + 1 => {
                let state = view.read(cx);
                let outputs = &state.pattern_site().unwrap().outputs;
                assert_eq!(outputs.len(), self.notes.len());
                for note in &self.notes {
                    assert!(outputs
                        .iter()
                        .any(|o| o.note.pitch == note.pitch && o.note.start == note.start + 0.25));
                }
                assert_eq!(
                    state.document.notes.indices.len(),
                    self.notes.len(),
                    "selection must follow the acknowledged notes"
                );
                window.dispatch_keystroke(Keystroke::parse("cmd-d").unwrap(), cx);
                self.stage = 3;
            }
            3 if revision == self.revision + 2 => {
                assert_eq!(
                    view.read(cx).pattern_site().unwrap().outputs.len(),
                    self.notes.len() * 2
                );
                window.dispatch_keystroke(Keystroke::parse("cmd-z").unwrap(), cx);
                self.stage = 4;
            }
            4 if revision == self.revision + 3 => {
                assert_eq!(
                    view.read(cx).pattern_site().unwrap().outputs.len(),
                    self.notes.len()
                );
                window.dispatch_keystroke(Keystroke::parse("cmd-z").unwrap(), cx);
                self.stage = 5;
            }
            5 if revision == self.revision + 4 => {
                window.dispatch_keystroke(Keystroke::parse("cmd-a").unwrap(), cx);
                window.dispatch_keystroke(Keystroke::parse("shift-right").unwrap(), cx);
                self.stage = 6;
            }
            6 if revision == self.revision + 5 => {
                let outputs = &view.read(cx).pattern_site().unwrap().outputs;
                for note in &self.notes {
                    assert!(outputs
                        .iter()
                        .any(|o| o.note.pitch == note.pitch
                            && o.note.duration == note.duration + 0.25));
                }
                window.dispatch_keystroke(Keystroke::parse("cmd-z").unwrap(), cx);
                self.stage = 7;
            }
            7 if revision == self.revision + 6 => {
                view.update(cx, |state, cx| {
                    let layout = state.piano_layout().unwrap();
                    let note = &state.pattern_site().unwrap().outputs[0].note;
                    let at = state.piano.origin.get()
                        + point(
                            px(layout.x(note.start + note.duration * 0.5)),
                            px(layout.y(note.pitch) + layout.key_height * 0.5),
                        );
                    assert!(state.press_note(&MouseDownEvent {
                        position: at,
                        ..Default::default()
                    }));
                    state.move_note(&MouseMoveEvent {
                        position: at + point(px(layout.beat_width * 0.5), px(-layout.key_height)),
                        pressed_button: Some(MouseButton::Left),
                        ..Default::default()
                    });
                    assert!(state.document.gesture.is_some());
                    cx.notify();
                });
                window.dispatch_keystroke(Keystroke::parse("escape").unwrap(), cx);
                view.update(cx, |state, _| {
                    state.finish_note();
                    assert!(state.document.pending.is_none());
                });
                assert_eq!(
                    view.read(cx).document.view.as_ref().unwrap().revision,
                    revision
                );
                assert_eq!(
                    view.read(cx)
                        .pattern_site()
                        .unwrap()
                        .outputs
                        .iter()
                        .map(|o| o.note.clone())
                        .collect::<Vec<_>>(),
                    self.notes
                );
                eprintln!("Editing smoke passed: real GPUI multiselect, batch move, acknowledged selection, duplicate, group resize, Undo, Escape cancellation");
                self.stage = 10;
            }
            _ => {}
        }
    }
    fn curve(&mut self, view: &Entity<Preview>, window: &mut Window, cx: &mut App) {
        let revision = view.read(cx).document.view.as_ref().unwrap().revision;
        match self.stage {
            0 => {
                self.revision = revision;
                self.source = Some(view.read(cx).automation_site().unwrap().source.clone());
                view.update(cx, |state, cx| {
                    let curve =
                        crate::automation_curve::editable(&state.automation_site().unwrap().source)
                            .unwrap();
                    let a = &curve.points[0];
                    let b = &curve.points[1];
                    let bounds = state.document.automation.bounds.get();
                    let at = bounds.origin
                        + point(
                            bounds.size.width
                                * (((a.beat + b.beat) * 0.5) / state.document.automation.beats)
                                    as f32,
                            bounds.size.height * (1. - (a.value + b.value) * 0.5) as f32,
                        );
                    state.press_curve(&MouseDownEvent {
                        position: at,
                        modifiers: Modifiers {
                            alt: true,
                            ..Default::default()
                        },
                        ..Default::default()
                    });
                    assert!(state.document.automation.gesture.is_some());
                    state.move_curve(at - point(px(0.), bounds.size.height * 0.1), false);
                    state.finish_automation();
                    assert!(state.document.pending.is_some());
                    cx.notify();
                });
                self.stage = 1;
            }
            1 if revision == self.revision + 1 => {
                let state = view.read(cx);
                let curve =
                    crate::automation_curve::editable(&state.automation_site().unwrap().source)
                        .unwrap();
                assert!(matches!(
                    curve.points[0].curve,
                    Some(oxitone_core::wire::Curve::Bezier { .. })
                ));
                let compiled = state.document.automation.compiled.as_ref().unwrap();
                assert!(
                    compiled.0.starts_with(&format!("{revision}:")),
                    "curve cache must follow source revision"
                );
                window.dispatch_keystroke(Keystroke::parse("cmd-z").unwrap(), cx);
                self.stage = 2;
            }
            2 if revision == self.revision + 2 => {
                assert_eq!(
                    &view.read(cx).automation_site().unwrap().source,
                    self.source.as_ref().unwrap()
                );
                eprintln!("Editing smoke passed: curve tension, native Bezier source, revision-aware preview, Undo");
                self.stage = 10;
            }
            _ => {}
        }
    }
}
