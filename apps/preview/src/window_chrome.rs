//! Native title drag area stays separate from navigation and project actions.
use crate::ui::Preview;
use gpui::{prelude::*, *};

impl Preview {
    pub(crate) fn header(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let name = self
            .project
            .as_ref()
            .and_then(|p| p.snapshot.name.clone())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| "Untitled".into());
        let title = div()
            .h(px(44.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .pr_4()
            .gap_3()
            .bg(rgb(theme.panel))
            .border_b_1()
            .border_color(rgb(theme.border))
            .child(
                div()
                    .id("window-drag-region")
                    .window_control_area(WindowControlArea::Drag)
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .pl(px(
                        if cfg!(target_os = "macos") && !window.is_fullscreen() {
                            100.
                        } else {
                            20.
                        },
                    ))
                    .flex()
                    .items_center()
                    .gap_2()
                    .on_mouse_down(MouseButton::Left, |event, window, cx| {
                        if !window.is_fullscreen() {
                            if event.click_count == 2 {
                                window.titlebar_double_click();
                            } else {
                                start_drag(window, cx);
                            }
                        }
                    })
                    .child(
                        div().min_w_0().child(
                            div()
                                .text_size(px(12.))
                                .font_weight(FontWeight::MEDIUM)
                                .truncate()
                                .child(name),
                        ),
                    )
                    .when(
                        self.document.view.as_ref().is_some_and(|v| v.modified),
                        |d| d.child(div().size(px(5.)).rounded_full().bg(rgb(theme.muted))),
                    ),
            )
            .child(self.editor_tabs(cx))
            .child(self.header_tools(cx));
        title
    }
}

pub(crate) use crate::window_drag::start_drag;
