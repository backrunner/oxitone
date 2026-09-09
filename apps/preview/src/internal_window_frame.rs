//! Common titlebar, pointer isolation and eight resize handles.
use crate::{
    ui::Preview,
    ui_icons::Icon,
    window_manager::{GestureKind, WindowId},
};
use gpui::{prelude::*, *};

pub fn view(
    this: &Preview,
    id: WindowId,
    title: String,
    content: AnyElement,
    cx: &mut Context<Preview>,
) -> impl IntoElement {
    let t = this.theme;
    let bounds = this.document.windows.bounds(id);
    let state = this.document.windows.state(id);
    let mut header = div()
        .h(px(28.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .px_1()
        .bg(rgb(t.raised))
        .rounded_t(px(7.))
        .border_b_1()
        .border_color(rgb(t.border))
        .child(
            div()
                .id("internal-title")
                .flex_1()
                .min_w_0()
                .h_full()
                .px_2()
                .flex()
                .items_center()
                .text_size(px(11.))
                .font_weight(FontWeight::MEDIUM)
                .child(div().truncate().child(title))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                        this.workspace_focus.focus(window);
                        if event.click_count == 2 {
                            this.document.windows.toggle_maximize(id);
                        } else {
                            this.document
                                .windows
                                .begin(id, event.position, GestureKind::Move);
                        }
                        cx.notify();
                        cx.stop_propagation();
                    }),
                ),
        );
    if matches!(id, WindowId::Piano | WindowId::Mixer) {
        header = header.child(
            t.icon_button("internal-dock", Icon::Split, "Dock")
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.dock_editor(id, window);
                    cx.notify();
                })),
        );
    }
    header = header
        .child(
            t.icon_button(
                "internal-maximize",
                if state.maximized {
                    Icon::Restore
                } else {
                    Icon::Maximize
                },
                "Maximize / restore",
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.document.windows.toggle_maximize(id);
                cx.notify();
            })),
        )
        .child(
            t.icon_button("internal-close", Icon::Close, "Close")
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.close_internal(id, window);
                    cx.notify();
                })),
        );
    let mut frame = div()
        .id(SharedString::from(format!("internal-{id:?}")))
        .absolute()
        .left(px(bounds.x))
        .top(px(bounds.y))
        .w(px(bounds.width))
        .h(px(bounds.height))
        .flex()
        .flex_col()
        .overflow_hidden()
        .rounded(px(8.))
        .bg(rgb(t.bg))
        .border_1()
        .border_color(rgb(if state.z == this.document.windows.next_z {
            t.accent
        } else {
            t.border
        }))
        .shadow_lg()
        .occlude()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| {
                this.document.windows.focus(id);
                cx.notify();
                cx.stop_propagation();
            }),
        )
        .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
        .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
        .child(header)
        .child(
            div()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .flex()
                .flex_col()
                .overflow_hidden()
                // GPUI masks are rectangular. Keep child canvases above the curved bottom edge.
                .mb(px(7.))
                .child(content),
        );
    if !state.maximized {
        for (n, left, right, top, bottom) in [
            (0, true, false, false, false),
            (1, false, true, false, false),
            (2, false, false, true, false),
            (3, false, false, false, true),
            (4, true, false, true, false),
            (5, false, true, true, false),
            (6, true, false, false, true),
            (7, false, true, false, true),
        ] {
            let corner = (left || right) && (top || bottom);
            let mut handle = div().id(("window-resize", n as usize)).absolute();
            handle = if left || right {
                handle.w(px(if corner { 10. } else { 4. }))
            } else {
                handle.left(px(10.)).right(px(10.))
            };
            handle = if top || bottom {
                handle.h(px(if corner { 10. } else { 4. }))
            } else {
                handle.top(px(10.)).bottom(px(10.))
            };
            if left {
                handle = handle.left_0();
            }
            if right {
                handle = handle.right_0();
            }
            if top {
                handle = handle.top_0();
            }
            if bottom {
                handle = handle.bottom_0();
            }
            frame = frame.child(
                handle
                    .cursor(if corner {
                        if left == top {
                            CursorStyle::ResizeUpLeftDownRight
                        } else {
                            CursorStyle::ResizeUpRightDownLeft
                        }
                    } else if left || right {
                        CursorStyle::ResizeLeftRight
                    } else {
                        CursorStyle::ResizeUpDown
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            this.document.windows.begin(
                                id,
                                event.position,
                                GestureKind::Resize {
                                    left,
                                    right,
                                    top,
                                    bottom,
                                },
                            );
                            cx.notify();
                            cx.stop_propagation();
                        }),
                    ),
            );
        }
    }
    frame
}
