//! Static project tempo input. Automation and tempo-map edits keep their own authoring semantics.
use crate::{document_wire::DocumentOperation, project_edit::ProjectEdit, ui::Preview};
use gpui::{prelude::*, *};
#[derive(Default)]
pub struct TempoUi {
    pub input: Option<String>,
    pub select_all: bool,
    pub bounds: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>>,
}
impl Preview {
    fn static_tempo(&self) -> bool {
        self.project.as_ref().is_some_and(|p| {
            p.snapshot.tempo_map.len() == 1
                && !p.snapshot.automation.iter().any(|l| {
                    l.target.entity_id == p.snapshot.id && l.target.parameter_id == "tempo"
                })
        })
    }
    pub fn tempo_key(&mut self, event: &KeyDownEvent) -> bool {
        let Some(text) = self.document.tempo.input.as_mut() else {
            return false;
        };
        let key = &event.keystroke;
        let command = key.modifiers.platform || key.modifiers.control;
        match key.key.as_str() {
            "escape" => self.document.tempo.input = None,
            "enter" => {
                if let Ok(bpm) = text.parse::<f64>() {
                    if (20. ..=999.).contains(&bpm) && self.document_ready() {
                        self.document.tempo.input = None;
                        self.document_request(DocumentOperation::Project {
                            edit: ProjectEdit::Tempo { bpm },
                        });
                    }
                }
            }
            "a" if command => self.document.tempo.select_all = true,
            "backspace" | "delete" => {
                if self.document.tempo.select_all {
                    text.clear();
                } else {
                    text.pop();
                }
                self.document.tempo.select_all = false;
            }
            _ if !command && !key.modifiers.alt => {
                let value = event.keystroke.key_char.as_deref().unwrap_or(&key.key);
                if value.chars().all(|c| c.is_ascii_digit() || c == '.') && text.len() < 12 {
                    if self.document.tempo.select_all {
                        text.clear();
                    }
                    text.push_str(value);
                    self.document.tempo.select_all = false;
                }
            }
            _ => {}
        }
        true
    }
}
pub fn view(this: &Preview, bpm: f64, cx: &Context<Preview>) -> impl IntoElement {
    let t = this.theme;
    let editable = this.document_ready() && this.static_tempo();
    let bounds = this.document.tempo.bounds.clone();
    div()
        .id("project-tempo")
        .relative()
        .flex()
        .items_baseline()
        .gap_1()
        .px_1()
        .border_b_1()
        .child(
            canvas(move |area, _, _| bounds.set(area), |_, _, _, _| {})
                .absolute()
                .size_full(),
        )
        .border_color(rgb(if this.document.tempo.input.is_some() {
            t.accent
        } else {
            t.panel
        }))
        .when(editable, |d| d.cursor(CursorStyle::IBeam))
        .child(
            div().text_size(px(13.)).child(
                this.document
                    .tempo
                    .input
                    .clone()
                    .unwrap_or_else(|| format!("{bpm:.1}")),
            ),
        )
        .child(
            div()
                .text_size(px(9.))
                .text_color(rgb(t.muted))
                .child("BPM"),
        )
        .on_click(cx.listener(move |this, _, window, cx| {
            if editable {
                this.document.configuration.input = None;
                this.document.manager.searching = false;
                this.document.tempo.input = Some(format!("{bpm:.1}"));
                this.document.tempo.select_all = true;
                this.workspace_focus.focus(window);
            }
            cx.notify();
        }))
}
