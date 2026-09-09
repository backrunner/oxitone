//! Explicit smoke mutates only the temporary project supplied by the developer script.
use crate::ui::Preview;
use gpui::*;

pub fn enabled() -> bool {
    std::env::var_os("OXITONE_PREVIEW_CAPTURE").is_some()
        && std::env::var("OXITONE_PREVIEW_CAPTURE_DAW").is_ok_and(|value| value == "1")
}
#[derive(Default)]
pub struct Smoke {
    stage: u8,
    pitch: u8,
    review_frame: usize,
    configuration: crate::capture_configuration::Smoke,
    editing: crate::capture_editing::Smoke,
    workspace: crate::capture_workspace::Smoke,
    windows: crate::capture_windows::Smoke,
    review: crate::capture_ui_review::Smoke,
    library: crate::capture_library::Smoke,
    controls: crate::capture_controls::Smoke,
}
impl Smoke {
    pub fn complete(&self) -> bool {
        self.stage == 15
            && self.configuration.complete()
            && self.editing.complete()
            && self.workspace.complete()
            && self.windows.complete()
            && self.review.complete()
            && self.library.complete()
            && self.controls.complete()
    }
    pub fn step(
        &mut self,
        frame: usize,
        view: &Entity<Preview>,
        window: &mut Window,
        cx: &mut App,
    ) {
        if self.complete() {
            return;
        }
        if self.stage == 15 {
            if frame % 20 == 0 {
                let state = view.read(cx);
                eprintln!(
                    "Extended editing smoke: stage {} / {} · {:?} · {:?} · {:?}",
                    self.editing.stage,
                    self.workspace.stage,
                    state
                        .document
                        .view
                        .as_ref()
                        .map(|v| (v.revision, &v.status)),
                    state.document.pending,
                    state.diagnostic.as_ref().map(|d| (&d.code, &d.message))
                );
            }
            self.configuration.step(frame, view, window, cx);
            if !self.configuration.complete() {
                return;
            }
            self.editing.step(view, window, cx);
            if !self.editing.complete() {
                return;
            }
            self.workspace.step(view, window, cx);
            if !self.workspace.complete() {
                return;
            }
            self.windows.step(view, window, cx);
            if !self.windows.complete() {
                return;
            }
            self.library.step(view, window, cx);
            if !self.library.complete() {
                return;
            }
            self.controls.step(view, window, cx);
            if !self.controls.complete() {
                return;
            }
            self.review.step(view, window, cx);
            return;
        }
        if frame % 20 == 0 {
            let state = view.read(cx);
            eprintln!(
                "DAW smoke stage {} · {:?} · pending {:?} · document {:?} · diagnostic {:?}",
                self.stage,
                state.document.view.as_ref().map(|view| (
                    view.revision,
                    view.accepted_revision,
                    &view.status
                )),
                state.document.pending,
                state.document.error.as_ref().map(|d| (&d.code, &d.message)),
                state
                    .diagnostic
                    .as_ref()
                    .map(|diagnostic| (&diagnostic.code, &diagnostic.message))
            );
        }
        if frame < 3 || !view.read(cx).document_ready() {
            return;
        }
        match self.stage {
            0 => {
                view.update(cx, |state, cx| {
                    let note = state.pattern_site().unwrap().outputs[0].note.clone();
                    self.pitch = note.pitch;
                    let layout = state.piano_layout().unwrap();
                    let at = state.piano.origin.get()
                        + point(
                            px(layout.x(note.start + note.duration * 0.5)),
                            px(layout.y(note.pitch) + layout.key_height * 0.5),
                        );
                    state.piano_focus.focus(window);
                    assert!(state.press_note(&MouseDownEvent {
                        position: at,
                        ..Default::default()
                    }));
                    assert!(state.document.gesture.is_some());
                    state.move_note(&MouseMoveEvent {
                        position: at + point(px(layout.beat_width * 0.25), px(-layout.key_height)),
                        pressed_button: Some(MouseButton::Left),
                        ..Default::default()
                    });
                    state.finish_note();
                    assert!(state.document.pending.is_some());
                    cx.notify();
                });
                self.stage = 1;
            }
            1 if view.read(cx).document_ready()
                && view.read(cx).document.view.as_ref().unwrap().revision == 1 =>
            {
                assert!(
                    view.read(cx)
                        .pattern_site()
                        .unwrap()
                        .outputs
                        .iter()
                        .any(|output| output.note.pitch == self.pitch + 1
                            && output.note.start == 0.25)
                );
                window.dispatch_keystroke(Keystroke::parse("cmd-s").unwrap(), cx);
                self.stage = 2;
            }
            2 if !view.read(cx).document.view.as_ref().unwrap().modified => {
                window.dispatch_keystroke(Keystroke::parse("cmd-z").unwrap(), cx);
                self.stage = 3;
            }
            3 if view.read(cx).document_ready()
                && view.read(cx).document.view.as_ref().unwrap().revision == 2 =>
            {
                assert_eq!(
                    view.read(cx).pattern_site().unwrap().outputs[0].note.pitch,
                    self.pitch
                );
                window.dispatch_keystroke(Keystroke::parse("cmd-shift-z").unwrap(), cx);
                self.stage = 4;
            }
            4 if view.read(cx).document_ready()
                && view.read(cx).document.view.as_ref().unwrap().revision == 3 =>
            {
                assert!(
                    view.read(cx)
                        .pattern_site()
                        .unwrap()
                        .outputs
                        .iter()
                        .any(|output| output.note.pitch == self.pitch + 1
                            && output.note.start == 0.25)
                );
                window.dispatch_keystroke(Keystroke::parse("cmd-s").unwrap(), cx);
                self.stage = 5;
            }
            5 if !view.read(cx).document.view.as_ref().unwrap().modified => {
                view.update(cx, |state, _| {
                    state.document_request(
                        crate::document_wire::DocumentOperation::PlanMaterialize {
                            site: state.pattern_site().unwrap().handle.clone(),
                            placement: state.edit_placement(),
                            edits: vec![],
                        },
                    );
                });
                self.stage = 6;
            }
            6 if view
                .read(cx)
                .document
                .view
                .as_ref()
                .unwrap()
                .materialization
                .is_some() =>
            {
                let plan = view
                    .read(cx)
                    .document
                    .view
                    .as_ref()
                    .unwrap()
                    .materialization
                    .as_ref()
                    .unwrap();
                assert_eq!(plan.affected_clips.len(), 1);
                assert_eq!(plan.after_notes, 3);
                assert!(plan.after_text.contains("new Pattern("));
                self.review_frame = frame;
                self.stage = 7;
            }
            7 if frame > self.review_frame + 3 => {
                view.update(cx, |state, _| {
                    let plan_id = state
                        .document
                        .view
                        .as_ref()
                        .unwrap()
                        .materialization
                        .as_ref()
                        .unwrap()
                        .plan_id
                        .clone();
                    state.document_request(
                        crate::document_wire::DocumentOperation::ConfirmMaterialize { plan_id },
                    );
                });
                self.stage = 8;
            }
            8 if view.read(cx).document_ready()
                && view.read(cx).document.view.as_ref().unwrap().revision == 4 =>
            {
                window.dispatch_keystroke(Keystroke::parse("cmd-s").unwrap(), cx);
                self.stage = 9;
            }
            9 if !view.read(cx).document.view.as_ref().unwrap().modified => {
                view.update(cx, |state, cx| {
                    let manager = std::env::var_os("OXITONE_PREVIEW_CAPTURE_MANAGER").is_some();
                    let automation =
                        std::env::var_os("OXITONE_PREVIEW_CAPTURE_AUTOMATION").is_some();
                    let patterns = std::env::var_os("OXITONE_PREVIEW_CAPTURE_PATTERNS").is_some();
                    state.document.automation.open = automation;
                    state.document.patterns_open = patterns;
                    state.document.show_code = !manager && !automation && !patterns;
                    state.document.manager.open = manager;
                    state.workspace_focus.focus(window);
                    cx.notify();
                });
                eprintln!("DAW smoke passed: GPUI drag, isolated placement, native projection, Save, Undo, Redo, materialization review, confirmation, Save");
                self.review_frame = frame;
                self.stage = 10;
            }
            10 if frame > self.review_frame + 3 => {
                if std::env::var_os("OXITONE_PREVIEW_CAPTURE_PATTERNS").is_some() {
                    let state = view.read(cx);
                    assert!(state.document.patterns_open);
                    assert!(!state.document.manager.open);
                    assert!(!state.document.automation.open);
                    assert!(!state
                        .project
                        .as_ref()
                        .expect("Pattern capture requires a project")
                        .snapshot
                        .patterns
                        .is_empty());
                    eprintln!("Pattern browser smoke passed: active view has patterns");
                }
                self.stage = if std::env::var_os("OXITONE_PREVIEW_CAPTURE_AUTOMATION").is_some() {
                    11
                } else {
                    15
                };
            }
            11 => {
                view.update(cx, |state, cx| {
                    let bounds = state.document.automation.bounds.get();
                    assert!(f32::from(bounds.size.height) > 0.);
                    state.press_automation(
                        bounds.origin
                            + point(bounds.size.width * (2.1 / 8.), bounds.size.height * 0.8),
                    );
                    state.move_automation(
                        bounds.origin
                            + point(bounds.size.width * (2.6 / 8.), bounds.size.height * 0.3),
                    );
                    state.finish_automation();
                    assert!(state.document.pending.is_some());
                    cx.notify();
                });
                self.stage = 12;
            }
            12 if view.read(cx).document_ready()
                && view.read(cx).document.view.as_ref().unwrap().revision == 5 =>
            {
                let project = view.read(cx).project.as_ref().unwrap();
                assert!(matches!(
                    project.snapshot.automation[0].source,
                    oxitone_core::wire::AutomationSourceSpec::ReplaceRange { .. }
                ));
                assert!(matches!(
                    project.snapshot.automation[1].source,
                    oxitone_core::wire::AutomationSourceSpec::Wave { .. }
                ));
                window.dispatch_keystroke(Keystroke::parse("cmd-s").unwrap(), cx);
                self.stage = 13;
            }
            13 if !view.read(cx).document.view.as_ref().unwrap().modified => {
                self.review_frame = frame;
                self.stage = 14;
                eprintln!("Automation smoke passed: source range drawn, sibling lane unchanged, native projection, TS Save");
            }
            14 if frame > self.review_frame + 3 => {
                self.stage = 15;
            }
            _ => {}
        }
    }
}
