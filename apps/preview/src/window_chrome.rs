//! Integrated title area. Interactive controls are siblings of the drag target.
use crate::ui::Preview;
use gpui::{prelude::*, *};

impl Preview {
    pub(crate) fn header(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let name = self
            .project
            .as_ref()
            .and_then(|p| p.snapshot.name.clone())
            .unwrap_or_else(|| "Open an Oxitone project".into());
        div()
            .h(px(56.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .bg(rgb(theme.bg))
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
                    .pr_4()
                    .flex()
                    .items_center()
                    .gap_5()
                    .on_mouse_down(MouseButton::Left, |event, window, cx| {
                        if window.is_fullscreen() {
                            return;
                        }
                        if event.click_count == 2 {
                            window.titlebar_double_click();
                        } else {
                            start_drag(window, cx);
                        }
                    })
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(theme.accent))
                            .child("OXITONE"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .pl_5()
                            .border_l_1()
                            .border_color(rgb(theme.border))
                            .child(div().text_sm().truncate().child(name))
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(rgb(theme.muted))
                                    .child("PROJECT PREVIEW · READ ONLY"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_shrink_0()
                            .gap_2()
                            .items_center()
                            .child(div().size(px(6.)).rounded_full().bg(rgb(
                                if self.diagnostic.is_some() {
                                    theme.danger
                                } else {
                                    theme.accent
                                },
                            )))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(theme.muted))
                                    .child(self.status.clone()),
                            ),
                    ),
            )
            .child(
                theme
                    .button(
                        "scopes",
                        if self.show_scopes {
                            "Scopes on"
                        } else {
                            "Scopes off"
                        },
                    )
                    .mr_5()
                    .flex_shrink_0()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.show_scopes = !this.show_scopes;
                        cx.notify();
                    })),
            )
    }
}

#[cfg(target_os = "macos")]
#[allow(
    unexpected_cfgs,
    reason = "objc 0.2 macros check the legacy cargo-clippy feature"
)]
fn start_drag(window: &Window, cx: &App) {
    use objc::{class, msg_send, rc::StrongPtr, runtime::Object, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return;
    };
    // GPUI's pinned macOS start_window_move is a no-op. AppKit provides native
    // dragging/snap behavior for the entire custom header via the current event.
    // SAFETY: this is called only on the UI thread with a live GPUI NSView.
    // Retain both objects until the foreground task finishes; never store borrowed
    // native pointers. Schedule outside GPUI dispatch, since AppKit runs a nested loop.
    unsafe {
        let view = handle.ns_view.as_ptr().cast::<Object>();
        let native_window: *mut Object = msg_send![view, window];
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        let event: *mut Object = msg_send![app, currentEvent];
        if native_window.is_null() || event.is_null() {
            return;
        }
        let native_window = StrongPtr::retain(native_window);
        let event = StrongPtr::retain(event);
        cx.foreground_executor()
            .spawn(async move {
                let _: () = msg_send![*native_window, performWindowDragWithEvent: *event];
            })
            .detach();
    }
}

#[cfg(not(target_os = "macos"))]
fn start_drag(window: &Window, _: &App) {
    window.start_window_move();
}
