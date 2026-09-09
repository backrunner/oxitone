//! Document toolbar and source inspector; requests are routed by DocumentUi.
use crate::{document_wire::DocumentOperation, ui::Preview};
use gpui::{prelude::*, *};
impl Preview {
    pub fn document_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let mut row = div().flex().gap_1().items_center().flex_shrink_0();
        if self.document.view.is_none() {
            return row;
        }
        for (id, icon, label, operation) in [
            (
                "source-undo",
                crate::ui_icons::Icon::Undo,
                "Undo · ⌘ Z",
                DocumentOperation::Undo,
            ),
            (
                "source-redo",
                crate::ui_icons::Icon::Redo,
                "Redo · ⌘ Shift Z",
                DocumentOperation::Redo,
            ),
            (
                "source-save",
                crate::ui_icons::Icon::Save,
                "Save · ⌘ S",
                DocumentOperation::Save,
            ),
        ] {
            row = row.child(
                theme
                    .icon_button(id, icon, label)
                    .opacity(if self.document_operation_ready(&operation) {
                        1.
                    } else {
                        0.4
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.document_request(operation.clone());
                        cx.notify();
                    })),
            );
        }
        row.child(
            theme
                .icon_tool(
                    "source-code",
                    crate::ui_icons::Icon::Code,
                    "Source code",
                    self.document.show_code,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.document.show_code = !this.document.show_code;
                    cx.notify();
                })),
        )
    }
    pub fn pattern_controls(&self, cx: &mut Context<Self>) -> Div {
        let theme = self.theme;
        div()
            .flex()
            .items_center()
            .gap_1()
            .child(
                theme
                    .ghost(
                        "source-scope",
                        if self.composite_selected() {
                            "Pattern part"
                        } else if self.document.edit_shared {
                            "Shared definition"
                        } else {
                            "This clip"
                        },
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        if !this.composite_selected() {
                            this.document.edit_shared = !this.document.edit_shared;
                        }
                        this.document.notes.clear();
                        this.document.gesture = None;
                        cx.notify();
                    })),
            )
            .child(
                theme
                    .ghost("source-detach", "Detach…")
                    .opacity(if self.document_ready() && self.pattern_site().is_some() {
                        1.
                    } else {
                        0.4
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(site) = this.pattern_site() {
                            this.document_request(DocumentOperation::PlanMaterialize {
                                site: site.handle.clone(),
                                placement: this.edit_placement(),
                                edits: vec![],
                            });
                            cx.notify();
                        }
                    })),
            )
    }

    pub fn source_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let site = self.pattern_site();
        let file = self.document.view.as_ref().and_then(|view| {
            site.and_then(|site| view.files.iter().find(|file| file.path == site.file_name))
                .or_else(|| view.files.first())
        });
        let mut panel = div()
            .h(px(230.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .p_3()
            .gap_2()
            .bg(rgb(theme.panel));
        if let Some(site) = site {
            panel = panel.child(div().text_size(px(11.)).truncate().child(format!(
                "{} · {} placements · {}",
                site.label, site.references, site.expression
            )));
        }
        if let Some(file) = file {
            let text = file.text.clone();
            let path = file.path.clone();
            panel = panel.child(
                div()
                    .flex()
                    .justify_between()
                    .child(theme.label(&path))
                    .child(
                        theme
                            .button("copy-source", "Copy code")
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
                            })),
                    ),
            );
            panel = panel.child(
                div()
                    .id("source-code-scroll")
                    .overflow_y_scroll()
                    .flex_1()
                    .font_family("Menlo")
                    .text_size(px(11.))
                    .child(file.text.clone()),
            );
        }
        panel
    }
}
