//! AppKit window movement lives outside title layout and document controls.
use gpui::*;
#[cfg(target_os = "macos")]
#[allow(
    unexpected_cfgs,
    reason = "objc 0.2 macros check the legacy cargo-clippy feature"
)]
pub(crate) fn start_drag(window: &Window, cx: &App) {
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
pub(crate) fn start_drag(window: &Window, _: &App) {
    window.start_window_move();
}
