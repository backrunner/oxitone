//! Preset files and frozen audio placement. All writes use Document Service commands.
use crate::{
    document_wire::DocumentOperation,
    plugin_manager::CatalogEntry,
    ui::Preview,
    vst3_controls::{bounds, field},
    vst3_model::{Command, Field},
};
use gpui::{prelude::*, *};

#[derive(Clone, Copy)]
pub enum Action {
    Load,
    Save,
    Attach,
}
impl Preview {
    pub fn submit_vst3_file(&mut self, action: Action) {
        if !self.commit_vst3_field() {
            return;
        }
        let manager = &mut self.document.manager;
        let command = if matches!(action, Action::Load) {
            if manager.vst3.preset.trim().is_empty() {
                manager.vst3.error = Some("Enter a preset JSON path".into());
                return;
            }
            Command::LoadPreset {
                path: manager.vst3.preset.clone(),
            }
        } else {
            let Some(handle) = manager.selected.clone() else {
                return;
            };
            if matches!(action, Action::Save) {
                if manager.vst3.preset.trim().is_empty() {
                    manager.vst3.error = Some("Enter a new preset JSON path".into());
                    return;
                }
                Command::SavePreset {
                    plugin: handle.clone(),
                    path: manager.vst3.preset.clone(),
                    parameters: manager
                        .vst3
                        .drafts
                        .entry(handle)
                        .or_default()
                        .parameters
                        .clone(),
                }
            } else {
                let start = if manager.vst3.start_beat.is_empty() {
                    "0"
                } else {
                    &manager.vst3.start_beat
                };
                let Some(start_beat) = start
                    .parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite() && (0.0..=1_000_000_000.).contains(v))
                else {
                    manager.vst3.error = Some("Start beat must be between 0 and 1000000000".into());
                    return;
                };
                let name = manager.vst3.track_name.trim();
                if name.encode_utf16().count() > 256 {
                    manager.vst3.error = Some("Track name is too long".into());
                    return;
                }
                Command::AttachRender {
                    plugin: handle,
                    name: if name.is_empty() {
                        "VST3 audio".into()
                    } else {
                        name.into()
                    },
                    start_beat,
                }
            }
        };
        self.document_request(DocumentOperation::Vst3 { command });
        if matches!(action, Action::Load) && self.document.pending.is_some() {
            self.document.manager.vst3.adding = false;
            self.document.manager.section = None;
            self.document.manager.query.clear();
            self.document.manager.category = 0;
        }
    }
}
pub fn load(this: &Preview, cx: &Context<Preview>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .flex_shrink_0()
        .child(field(
            this,
            "Preset JSON",
            Field::Preset,
            this.document.manager.vst3.preset.clone(),
            cx,
        ))
        .child(button(
            this,
            "load-preset",
            "Load preset",
            Action::Load,
            true,
            cx,
        ))
}
pub fn panel(this: &Preview, entry: &CatalogEntry, cx: &Context<Preview>) -> Div {
    let state = &this.document.manager.vst3;
    let catalog = entry.vst3.as_ref().unwrap();
    let mut body = div()
        .flex()
        .flex_col()
        .gap_2()
        .flex_shrink_0()
        .child(load(this, cx))
        .child(button(
            this,
            "save-preset",
            "Save new preset",
            Action::Save,
            entry.validation == "verified",
            cx,
        ))
        .child(
            div()
                .text_size(px(11.))
                .text_color(rgb(this.theme.muted))
                .child("Saves plugin state and parameter values. Inspect a loaded preset before rendering. Choose a new path to save."),
        );
    if let Some(path) = &catalog.preset_path {
        body = body.child(
            div()
                .text_size(px(11.))
                .truncate()
                .child(format!("Preset: {path}")),
        );
    }
    body.child(
        div()
            .mt_3()
            .text_size(px(12.))
            .child("Add last render to Playlist"),
    )
    .child(field(
        this,
        "Track name",
        Field::TrackName,
        state.track_name.clone(),
        cx,
    ))
    .child(field(
        this,
        "Start beat",
        Field::StartBeat,
        if state.start_beat.is_empty() {
            "0".into()
        } else {
            state.start_beat.clone()
        },
        cx,
    ))
    .child(
        div()
            .text_size(px(11.))
            .text_color(rgb(this.theme.muted))
            .child("Creates a new audio track from the last completed WAV, including its tail. Use Save to keep it, or Undo to remove it."),
    )
    .child(button(
        this,
        "attach-render",
        "Add audio to project",
        Action::Attach,
        catalog.render.is_some(),
        cx,
    ))
}
fn button(
    this: &Preview,
    id: &'static str,
    label: &'static str,
    action: Action,
    enabled: bool,
    cx: &Context<Preview>,
) -> Stateful<Div> {
    let enabled = enabled && this.document.pending.is_none();
    this.theme
        .ghost(id, label)
        .relative()
        .child(bounds(this, id))
        .opacity(if enabled { 1. } else { 0.4 })
        .on_click(cx.listener(move |this, _, _, cx| {
            if enabled {
                this.submit_vst3_file(action);
            }
            cx.notify();
        }))
}
