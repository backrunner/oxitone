//! Compose the desktop, diagnostics and modal layers; state is owned by Preview.
use crate::ui::Preview;
use gpui::{prelude::*, *};

impl Render for Preview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.close.open || self.show_shortcuts {
            crate::pointer_capture::cancel(self);
        }
        let theme = self.theme;
        let mut root = div()
            .id("preview-workspace")
            .track_focus(&self.workspace_focus)
            .capture_key_down(cx.listener(Self::modal_key))
            .on_key_down(cx.listener(Self::workspace_key))
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(theme.bg))
            .text_color(rgb(theme.text))
            .font_family("Helvetica Neue")
            .child(crate::pointer_capture::view(cx))
            .child(self.header(window, cx))
            .child(self.transport_bar(cx));
        if let Some(diagnostic) = self.active_diagnostic() {
            root = root.child(
                div()
                    .px_5()
                    .py_2()
                    .bg(rgb(theme.diagnostic_bg))
                    .text_sm()
                    .text_color(rgb(theme.diagnostic_text))
                    .child(format!(
                        "{} · {}  {}",
                        diagnostic.code,
                        diagnostic.message,
                        diagnostic.path.as_deref().unwrap_or("")
                    )),
            );
        }
        if self.project.is_some() {
            let desktop = self.document.windows.desktop.clone();
            root = root.child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(
                        canvas(move |area, _, _| desktop.set(area), |_, _, _, _| {})
                            .absolute()
                            .size_full(),
                    )
                    .child(crate::workspace::panels(self, window, cx))
                    .child(crate::internal_windows::overlay(self, cx)),
            );
        } else {
            root = root.child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .items_center()
                    .gap_4()
                    .child(div().text_lg().child("No project loaded"))
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(theme.muted))
                            .child("Waiting for a project build."),
                    ),
            );
        }
        root.child(self.document_review(cx))
            .when(self.document.show_code, |d| d.child(self.source_panel(cx)))
            .child(self.footer(cx))
            .when(
                self.piano.snap_menu.is_some()
                    && !self.document.manager.open
                    && !self.document.automation.open,
                |d| d.child(crate::piano_snap::menu(self, cx)),
            )
            .when(self.show_shortcuts, |d| {
                d.child(crate::shortcut_help::view(self, cx))
            })
            .when(self.close.open, |d| d.child(self.close_review(cx)))
    }
}
