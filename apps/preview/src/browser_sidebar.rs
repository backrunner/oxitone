//! The resource Browser can float independently or dock to the left of the desktop.
use crate::{ui::Preview, ui_icons::Icon, window_manager::WindowId};
use gpui::{prelude::*, *};

impl Preview {
    pub fn dock_browser(&mut self) {
        self.workspace.browser_docked = true;
        self.document.browser_open = true;
        self.document
            .windows
            .visible
            .retain(|id| *id != WindowId::Browser);
        crate::pointer_capture::cancel(self);
    }
    pub fn float_browser(&mut self) {
        self.workspace.browser_docked = false;
        self.document.browser_open = true;
        self.document.windows.focus(WindowId::Browser);
    }
}

pub fn view(this: &mut Preview, cx: &mut Context<Preview>) -> impl IntoElement {
    let t = this.theme;
    div()
        .w(px(this.workspace.browser_width))
        .flex_shrink_0()
        .h_full()
        .min_h_0()
        .flex()
        .flex_col()
        .bg(rgb(t.panel))
        .child(
            div()
                .h(px(36.))
                .px_2()
                .gap(px(4.))
                .border_b_1()
                .border_color(rgb(t.border))
                .flex_shrink_0()
                .flex()
                .items_center()
                .child(
                    div()
                        .flex_1()
                        .text_size(px(11.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Browser"),
                )
                .child(
                    t.icon_button("browser-float", Icon::Restore, "Float Browser")
                        .size(px(24.))
                        .rounded(px(5.))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.float_browser();
                            cx.notify();
                        })),
                )
                .child(
                    t.icon_button("browser-hide", Icon::Close, "Hide Browser")
                        .size(px(24.))
                        .rounded(px(5.))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.document.browser_open = false;
                            cx.notify();
                        })),
                ),
        )
        .child(crate::browser_tree_view::body(this, cx))
}
