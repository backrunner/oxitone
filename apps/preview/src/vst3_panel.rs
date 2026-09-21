//! Local VST3 offline tools, using the existing internal Plugin Library window.
use crate::vst3_controls::field;
use crate::{
    document_wire::DocumentOperation,
    plugin_manager::CatalogEntry,
    ui::Preview,
    vst3_model::{Command, Field},
};
use gpui::{prelude::*, *};

fn frame(this: &Preview, title: String, cx: &Context<Preview>) -> Div {
    let t = this.theme;
    div()
        .flex_1()
        .min_h_0()
        .min_w_0()
        .flex()
        .flex_col()
        .bg(rgb(t.panel))
        .child(
            div()
                .h(px(36.))
                .px_2()
                .flex_shrink_0()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    t.ghost("vst3-back", "Back")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.document.manager.vst3.adding = false;
                            this.document.manager.vst3.input = None;
                            this.document.manager.section = None;
                            cx.notify();
                        })),
                )
                .child(div().flex_1().truncate().text_size(px(12.)).child(title)),
        )
}
fn errors(this: &Preview) -> Div {
    let error = this
        .document
        .manager
        .vst3
        .error
        .clone()
        .or_else(|| this.document.error.as_ref().map(|e| e.message.clone()));
    div()
        .text_size(px(11.))
        .text_color(rgb(this.theme.danger))
        .children(error)
}
pub fn add(this: &Preview, cx: &Context<Preview>) -> AnyElement {
    let state = &this.document.manager.vst3;
    frame(this, "Add local VST3".into(), cx)
        .child(
            div()
                .id("vst3-add-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .p_3()
                .flex()
                .flex_col()
                .gap_2()
                .child(field(this, "Bundle path", Field::Bundle, state.bundle.clone(), cx))
                .child(field(this, "Class ID (optional)", Field::Class, state.class_id.clone(), cx))
                .child(div().text_size(px(11.)).text_color(rgb(this.theme.muted))
                    .child("Leave Class ID empty to discover the plugins in this bundle. Discovery loads the bundle in an isolated process."))
                .child(errors(this))
                .child(
                    this.theme
                        .button("vst3-add-submit", if state.class_id.trim().is_empty() { "Discover plugins" } else { "Add to library" })
                        .relative()
                        .child(crate::vst3_controls::bounds(this, "add"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.submit_vst3(false);
                            cx.notify();
                        })),
                )
                .child(crate::vst3_scan::view(this, cx))
                .child(div().mt_3().text_size(px(12.)).child("Open a saved preset"))
                .child(crate::vst3_files::load(this, cx)),
        )
        .into_any_element()
}
pub fn workbench(this: &Preview, entry: &CatalogEntry, cx: &Context<Preview>) -> AnyElement {
    let catalog = entry.vst3.as_ref().unwrap();
    let t = this.theme;
    let state = &this.document.manager.vst3;
    let draft = state.drafts.get(&entry.handle).cloned().unwrap_or_default();
    let busy = this.document.pending.is_some();
    let verified = entry.validation == "verified";
    let mut body = div()
        .id("vst3-workbench-scroll")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .track_scroll(&this.document.manager.details_scroll)
        .p_3()
        .flex()
        .flex_col()
        .gap_2()
        .child(div().text_size(px(11.)).text_color(rgb(t.muted)).child(
            "Offline WAV · 48 kHz · 128 frames · 4/4 · Parameters use normalized values (0–1).",
        ))
        .when(verified, |d| {
            d.child(div().text_size(px(11.)).child(format!(
                "{} input / {} output channels",
                catalog.input_channels.unwrap_or(0),
                catalog.output_channels.unwrap_or(0)
            )))
        })
        .child(field(
            this,
            "Input WAV",
            Field::Input,
            draft.input.clone(),
            cx,
        ))
        .child(field(
            this,
            "Output WAV",
            Field::Output,
            draft.output.clone(),
            cx,
        ))
        .child(field(
            this,
            "Seconds",
            Field::Seconds,
            draft.seconds.clone(),
            cx,
        ))
        .child(field(
            this,
            "Tail seconds",
            Field::Tail,
            draft.tail.clone(),
            cx,
        ))
        .child(field(this, "Tempo", Field::Tempo, draft.tempo.clone(), cx));
    if catalog.note_input == Some(true) {
        body = body.child(field(
            this,
            "Test note",
            Field::Pitch,
            draft.pitch.clone(),
            cx,
        ));
    }
    if state.files {
        body = div()
            .id("vst3-files-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&this.document.manager.details_scroll)
            .p_3()
            .child(crate::vst3_files::panel(this, entry, cx));
    } else {
        body = body.child(crate::vst3_controls::parameters(this, entry, cx));
    }
    body = body.child(errors(this));
    if let Some(report) = &catalog.render {
        body = body.child(
            div()
                .text_size(px(11.))
                .child(format!("Wrote {} · peak {:.5}", report.path, report.peak)),
        );
    }
    let handle = entry.handle.clone();
    let editor_handle = entry.handle.clone();
    let mut actions = div()
        .p_2()
        .flex_shrink_0()
        .flex()
        .gap_2()
        .child(
            t.ghost("vst3-editor", "Native editor")
                .opacity(if verified && !busy { 1. } else { 0.4 })
                .on_click(cx.listener(move |this, _, _, cx| {
                    if verified && !busy && this.commit_vst3_field() {
                        let parameters = this
                            .document
                            .manager
                            .vst3
                            .drafts
                            .get(&editor_handle)
                            .map(|d| d.parameters.clone())
                            .unwrap_or_default();
                        this.document_request(DocumentOperation::Vst3 {
                            command: Command::Edit {
                                plugin: editor_handle.clone(),
                                parameters,
                            },
                        });
                    }
                    cx.notify();
                })),
        )
        .child(
            t.ghost(
                "vst3-files",
                if state.files {
                    "Parameters"
                } else {
                    "Files / project"
                },
            )
            .relative()
            .child(crate::vst3_controls::bounds(this, "files"))
            .on_click(cx.listener(|this, _, _, cx| {
                if this.commit_vst3_field() {
                    this.document.manager.vst3.files = !this.document.manager.vst3.files;
                    this.document
                        .manager
                        .details_scroll
                        .set_offset(point(px(0.), px(0.)));
                }
                cx.notify();
            })),
        )
        .child(
            t.ghost("vst3-inspect", "Inspect")
                .relative()
                .child(crate::vst3_controls::bounds(this, "inspect"))
                .opacity(if busy { 0.4 } else { 1. })
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.document_request(DocumentOperation::VerifyPlugin {
                        plugin: handle.clone(),
                    });
                    cx.notify();
                })),
        )
        .child(
            t.button("vst3-render", "Render WAV")
                .relative()
                .child(crate::vst3_controls::bounds(this, "render"))
                .opacity(if verified && !busy { 1. } else { 0.4 })
                .on_click(cx.listener(move |this, _, _, cx| {
                    if verified && !busy {
                        this.submit_vst3(true);
                    }
                    cx.notify();
                })),
        );
    if busy {
        actions = actions.child(t.ghost("vst3-cancel", "Cancel task").on_click(cx.listener(
            |this, _, _, cx| {
                this.cancel_vst3();
                cx.notify();
            },
        )));
    } else if entry.package_name.is_none() {
        let handle = entry.handle.clone();
        actions = actions.child(t.ghost("vst3-remove", "Remove").on_click(cx.listener(
            move |this, _, _, cx| {
                this.document_request(DocumentOperation::Vst3 {
                    command: Command::Remove {
                        plugin: handle.clone(),
                    },
                });
                this.document.manager.section = None;
                this.document.manager.vst3.drafts.remove(&handle);
                cx.notify();
            },
        )));
    }
    frame(this, format!("{} · VST3 offline", entry.display_name), cx)
        .child(body)
        .child(actions)
        .into_any_element()
}
