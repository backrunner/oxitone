//! Main-window close review. Musical writes still go through the Document Service.
use crate::{
    document_wire::DocumentOperation,
    ui::{alpha, Preview},
};
use gpui::{prelude::*, *};

impl Preview {
    pub fn request_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let unfinished = self.close.saving()
            || self.document.pending.is_some()
            || self
                .document
                .view
                .as_ref()
                .is_some_and(|v| v.modified || v.saving || v.status == "building");
        if !unfinished {
            cx.defer(|cx| cx.quit());
            return true;
        }
        self.close.open = true;
        self.show_shortcuts = false;
        crate::pointer_capture::cancel(self);
        self.workspace_focus.focus(window);
        cx.notify();
        false
    }
    pub fn save_and_close(&mut self, cx: &mut Context<Self>) {
        if !self.close.open || self.close.saving() || !self.document_ready() {
            return;
        }
        self.document_request(DocumentOperation::Save);
        if let (Some(request), Some(view)) = (&self.document.pending, &self.document.view) {
            self.close.begin_save(request.clone(), view);
        }
        cx.notify();
    }
    pub fn close_busy(&self) -> bool {
        self.close.saving()
            || self.document.pending.is_some()
            || self
                .document
                .view
                .as_ref()
                .is_some_and(|v| v.saving || v.status == "building")
    }
    pub fn modal_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        if self.close.open {
            if key.key == "escape" {
                self.close.cancel();
                self.workspace_focus.focus(window);
            } else if key.key == "enter" && !event.is_held {
                self.save_and_close(cx);
            }
            cx.stop_propagation();
            cx.notify();
        } else if self.show_shortcuts {
            if key.key == "escape" || key.key == "?" || (key.key == "/" && key.modifiers.shift) {
                self.show_shortcuts = false;
                self.workspace_focus.focus(window);
            }
            cx.stop_propagation();
            cx.notify();
        } else if key.key == "q"
            && key.modifiers.platform
            && !key.modifiers.alt
            && !key.modifiers.shift
        {
            if !event.is_held {
                self.request_close(window, cx);
            }
            cx.stop_propagation();
        }
    }
    pub fn close_review(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let busy = self.close_busy();
        let message = if let Some(error) = &self.close.error {
            format!("Couldn’t save and close. Your draft is still open. {error}")
        } else if self.close.saving() {
            "Saving your TypeScript files…".into()
        } else if busy {
            "Wait for the current operation to finish, or cancel to keep working.".into()
        } else if !self.document_ready() {
            "The draft has errors or conflicts. Return to the project to resolve them, or close without saving.".into()
        } else {
            "Save your changes to the project’s TypeScript files before closing.".into()
        };
        div()
            .id("close-review-overlay")
            .absolute()
            .inset_0()
            .bg(alpha(0, 0.45))
            .flex()
            .items_center()
            .justify_center()
            .occlude()
            .child(
                div()
                    .w(px(500.))
                    .p_5()
                    .rounded_lg()
                    .bg(rgb(t.panel))
                    .border_1()
                    .border_color(rgb(t.border))
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Save changes before closing?"),
                    )
                    .child(div().text_sm().text_color(rgb(t.muted)).child(message))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                t.button("close-discard", "Close without saving")
                                    .opacity(if busy { 0.4 } else { 1. })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if !this.close_busy() {
                                            cx.quit();
                                        }
                                    })),
                            )
                            .child(div().flex_1())
                            .child(t.button("close-cancel", "Cancel").on_click(cx.listener(
                                |this, _, window, cx| {
                                    this.close.cancel();
                                    this.workspace_focus.focus(window);
                                    cx.notify();
                                },
                            )))
                            .child(
                                t.primary("close-save", "Save & close")
                                    .opacity(if !busy && self.document_ready() {
                                        1.
                                    } else {
                                        0.4
                                    })
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.save_and_close(cx)),
                                    ),
                            ),
                    ),
            )
    }
}
