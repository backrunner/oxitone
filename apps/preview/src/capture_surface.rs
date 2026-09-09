//! UI-thread-only native handles used by opt-in screenshots, never by audio.
#![allow(
    unexpected_cfgs,
    reason = "objc macros check the legacy cargo-clippy feature"
)]
use gpui::*;
use objc::{class, msg_send, rc::StrongPtr, runtime::Object, sel, sel_impl};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

pub struct Surface {
    view: StrongPtr,
    pub number: isize,
}
impl Surface {
    pub fn new(window: &Window) -> Option<Self> {
        let RawWindowHandle::AppKit(handle) = HasWindowHandle::window_handle(window).ok()?.as_raw()
        else {
            return None;
        };
        // SAFETY: GPUI owns a live NSView on the UI thread; retain it for this capture task.
        unsafe {
            let view = StrongPtr::retain(handle.ns_view.as_ptr().cast::<Object>());
            let native: *mut Object = msg_send![*view, window];
            let number = msg_send![native, windowNumber];
            Some(Self { view, number })
        }
    }
    pub fn appearance(&self, value: &str) {
        let name = match value {
            "light" => Some(c"NSAppearanceNameAqua"),
            "dark" => Some(c"NSAppearanceNameDarkAqua"),
            "" => None,
            _ => panic!("OXITONE_PREVIEW_APPEARANCE must be light or dark"),
        };
        // SAFETY: called on the UI executor outside GPUI dispatch; appearance is window-local.
        unsafe {
            if let Some(name) = name {
                let native: *mut Object = msg_send![*self.view, window];
                let string: *mut Object =
                    msg_send![class!(NSString), stringWithUTF8String: name.as_ptr()];
                let appearance: *mut Object =
                    msg_send![class!(NSAppearance), appearanceNamed: string];
                let _: () = msg_send![native, setAppearance: appearance];
            }
        }
    }
    pub fn redraw(&self) {
        // SAFETY: retained NSView, invoked outside all GPUI borrows on the UI executor.
        unsafe {
            let native: *mut Object = msg_send![*self.view, window];
            if native.is_null() {
                return;
            }
            let layer: *mut Object = msg_send![*self.view, layer];
            let _: () = msg_send![*self.view, displayLayer: layer];
            // Capture runs without a live display link on locked desktops. Commit the
            // transaction used by GPUI's displayLayer path before the system screenshot.
            let _: () = msg_send![class!(CATransaction), flush];
        }
    }
    pub fn close(&self) {
        // SAFETY: UI executor, outside GPUI borrows; AppKit runs the normal should-close path.
        unsafe {
            let native: *mut Object = msg_send![*self.view, window];
            let _: () = msg_send![native, performClose: std::ptr::null_mut::<Object>()];
        }
    }
}
