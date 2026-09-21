//! System directory discovery lists paths only. Selecting a bundle loads its factory on request.
use crate::{document_wire::DocumentOperation, ui::Preview, vst3_model::Command};
use gpui::{prelude::*, *};

pub fn view(this: &Preview, cx: &Context<Preview>) -> Div {
    let theme = this.theme;
    let busy = this.document.pending.is_some();
    let paths = this
        .document
        .view
        .as_ref()
        .and_then(|v| v.vst3_bundles.as_ref());
    let mut body = div().mt_3().flex().flex_col().gap_2().child(
        this.theme
            .ghost(
                "vst3-scan",
                if paths.is_some() {
                    "Rescan installed VST3"
                } else {
                    "Scan installed VST3"
                },
            )
            .opacity(if busy { 0.4 } else { 1. })
            .on_click(cx.listener(move |this, _, _, cx| {
                if !busy {
                    this.document_request(DocumentOperation::Vst3 {
                        command: Command::Scan,
                    });
                }
                cx.notify();
            })),
    );
    let Some(paths) = paths else {
        return body;
    };
    body = body.child(div().text_size(px(11.)).text_color(rgb(this.theme.muted)).child(
        format!("{} bundles in user and system VST3 folders. Select one to fill its path, then discover its plugins.", paths.len()),
    ));
    let mut list = div()
        .id("vst3-scan-results")
        .max_h(px(220.))
        .overflow_y_scroll()
        .flex()
        .flex_col();
    for (index, path) in paths.iter().enumerate() {
        let selected = path.clone();
        let name = std::path::Path::new(path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        list = list.child(
            div()
                .id(("vst3-bundle", index))
                .px_2()
                .py_1()
                .flex()
                .flex_col()
                .cursor_pointer()
                .hover(move |s| s.bg(rgb(theme.panel)))
                .child(div().text_size(px(12.)).truncate().child(name))
                .child(
                    div()
                        .text_size(px(10.))
                        .text_color(rgb(this.theme.muted))
                        .truncate()
                        .child(path.clone()),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    if !busy {
                        this.document.manager.vst3.bundle = selected.clone();
                        this.document.manager.vst3.class_id.clear();
                        this.document.manager.vst3.input = None;
                        this.document.manager.vst3.error = None;
                    }
                    cx.notify();
                })),
        );
    }
    body.child(list)
}
