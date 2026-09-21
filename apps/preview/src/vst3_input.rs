//! Offline form input emits Document Service commands, never touches files or DSP.
use crate::{
    document_wire::DocumentOperation,
    ui::Preview,
    vst3_model::{Command, Field, Source},
};
use gpui::*;

impl Preview {
    pub fn observe_vst3_catalog(&mut self, view: &crate::document_wire::DocumentView) {
        let old = self.document.view.as_ref();
        self.document.manager.vst3.drafts.retain(|handle, draft| {
            let Some(entry) = view.plugins.iter().find(|entry| &entry.handle == handle) else {
                return false;
            };
            if entry.validation != "verified"
                || old
                    .and_then(|v| v.plugins.iter().find(|entry| &entry.handle == handle))
                    .is_some_and(|previous| {
                        previous.sha256 != entry.sha256
                            || previous.vst3.as_ref().map(|c| &c.parameters)
                                != entry.vst3.as_ref().map(|c| &c.parameters)
                    })
            {
                draft.parameters.clear();
            }
            true
        });
    }
    pub fn start_vst3_field(&mut self, field: Field, value: String) {
        if self
            .document
            .manager
            .vst3
            .input
            .as_ref()
            .is_some_and(|(active, _)| *active == field)
        {
            return;
        }
        if !self.commit_vst3_field() {
            return;
        }
        let state = &mut self.document.manager.vst3;
        state.input = Some((field, value));
        state.select_all = true;
        state.error = None;
    }
    pub fn submit_vst3(&mut self, render: bool) {
        if !self.commit_vst3_field() {
            return;
        }
        let manager = &mut self.document.manager;
        let command = if !render {
            let class_id = manager.vst3.class_id.trim().to_ascii_lowercase();
            if !class_id.is_empty()
                && (class_id.len() != 32 || !class_id.bytes().all(|v| v.is_ascii_hexdigit()))
            {
                manager.vst3.error =
                    Some("Class ID must contain exactly 32 hexadecimal characters".into());
                return;
            }
            if manager.vst3.bundle.trim().is_empty() {
                manager.vst3.error = Some("Enter the path of a local .vst3 bundle".into());
                return;
            }
            if class_id.is_empty() {
                Command::AddBundle {
                    bundle_path: manager.vst3.bundle.clone(),
                }
            } else {
                Command::Add {
                    source: Source {
                        bundle_path: manager.vst3.bundle.clone(),
                        class_id,
                    },
                }
            }
        } else {
            let Some(handle) = manager.selected.clone() else {
                return;
            };
            let Some(catalog) = self
                .document
                .view
                .as_ref()
                .and_then(|v| v.plugins.iter().find(|p| p.handle == handle))
                .and_then(|p| p.vst3.as_ref())
            else {
                return;
            };
            match manager
                .vst3
                .drafts
                .entry(handle.clone())
                .or_default()
                .options(catalog)
            {
                Ok(options) => Command::Render {
                    plugin: handle,
                    options,
                },
                Err(error) => {
                    manager.vst3.error = Some(error);
                    return;
                }
            }
        };
        let operation = DocumentOperation::Vst3 { command };
        if !self.document_operation_ready(&operation) {
            return;
        }
        self.document_request(operation);
        if !render && self.document.pending.is_some() {
            self.document.manager.vst3.adding = false;
            self.document.manager.query.clear();
            self.document.manager.category = 0;
        }
    }
    pub(super) fn commit_vst3_field(&mut self) -> bool {
        let state = &mut self.document.manager;
        let Some((field, value)) = state.vst3.input.clone() else {
            return true;
        };
        if let Field::Parameter(id) = field {
            let number = value
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite() && (0.0..=1.0).contains(v));
            let Some(number) = number else {
                state.vst3.error =
                    Some("Parameter values must be normalized numbers from 0 to 1".into());
                return false;
            };
            if let Some(handle) = &state.selected {
                state
                    .vst3
                    .drafts
                    .entry(handle.clone())
                    .or_default()
                    .parameters
                    .insert(id.to_string(), number);
            }
        } else if field == Field::Bundle {
            state.vst3.bundle = value;
        } else if field == Field::Class {
            state.vst3.class_id = value;
        } else if field == Field::Preset {
            state.vst3.preset = value;
        } else if field == Field::TrackName {
            state.vst3.track_name = value;
        } else if field == Field::StartBeat {
            state.vst3.start_beat = value;
        } else if let Some(handle) = &state.selected {
            let draft = state.vst3.drafts.entry(handle.clone()).or_default();
            match field {
                Field::Input => draft.input = value,
                Field::Output => draft.output = value,
                Field::Seconds => draft.seconds = value,
                Field::Tail => draft.tail = value,
                Field::Tempo => draft.tempo = value,
                Field::Pitch => draft.pitch = value,
                _ => {}
            }
        }
        state.vst3.input = None;
        state.vst3.error = None;
        true
    }
    pub fn vst3_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        let state = &mut self.document.manager.vst3;
        let Some((_, text)) = state.input.as_mut() else {
            return false;
        };
        let key = &event.keystroke;
        let command = key.modifiers.platform || key.modifiers.control;
        match key.key.as_str() {
            "escape" => {
                state.input = None;
                state.error = None;
            }
            "enter" => {
                self.commit_vst3_field();
            }
            "a" if command => state.select_all = true,
            "backspace" => {
                if state.select_all {
                    text.clear();
                } else {
                    text.pop();
                }
                state.select_all = false;
            }
            "v" if command => {
                if let Some(value) = cx.read_from_clipboard().and_then(|c| c.text()) {
                    if state.select_all {
                        text.clear();
                    }
                    text.extend(value.chars().filter(|c| !c.is_control()).take(4096));
                    *text = text.chars().take(4096).collect();
                    state.select_all = false;
                }
            }
            _ if command || key.modifiers.alt => {}
            _ => {
                if let Some(value) = &key.key_char {
                    if state.select_all {
                        text.clear();
                    }
                    text.extend(value.chars().take(4096));
                    *text = text.chars().take(4096).collect();
                    state.select_all = false;
                }
            }
        }
        cx.stop_propagation();
        cx.notify();
        true
    }
}
