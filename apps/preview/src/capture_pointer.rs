//! Targeted NSEvents exercise GPUI hit testing without moving the user's mouse.
pub fn dispatch(window: &gpui::Window, input: gpui::PlatformInput, cx: &gpui::App) {
    dispatch_checked(window, input, cx, |_| {});
}
#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)]
pub fn dispatch_checked(
    window: &gpui::Window,
    input: gpui::PlatformInput,
    cx: &gpui::App,
    after: impl FnOnce(&gpui::App) + 'static,
) {
    use gpui::*;
    use objc::{class, msg_send, rc::StrongPtr, runtime::Object, sel, sel_impl, Encode, Encoding};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn CGEventSetIntegerValueField(event: *mut std::ffi::c_void, field: u32, value: i64);
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct NativePoint {
        x: f64,
        y: f64,
    }
    // SAFETY: CGPoint contains two CGFloat/f64 fields on supported macOS 64-bit targets.
    unsafe impl Encode for NativePoint {
        fn encode() -> Encoding {
            unsafe { Encoding::from_str("{CGPoint=dd}") }
        }
    }
    let (position, kind) = match input {
        PlatformInput::MouseDown(e) => (
            e.position,
            if e.button == MouseButton::Right { 3 } else { 1 },
        ),
        PlatformInput::MouseUp(e) => (
            e.position,
            if e.button == MouseButton::Right { 4 } else { 2 },
        ),
        PlatformInput::MouseMove(e) => (
            e.position,
            if e.pressed_button == Some(MouseButton::Right) {
                7
            } else if e.pressed_button.is_some() {
                6
            } else {
                5
            },
        ),
        _ => panic!("capture pointer accepts mouse events only"),
    };
    let RawWindowHandle::AppKit(handle) = HasWindowHandle::window_handle(window).unwrap().as_raw()
    else {
        panic!("AppKit capture requires a native view");
    };
    let height = f32::from(window.viewport_size().height) as f64;
    // SAFETY: called on the UI thread with a live view. Retain across dispatch and run after
    // all GPUI borrows have ended, just like the native capture redraw path.
    let view = unsafe { StrongPtr::retain(handle.ns_view.as_ptr().cast::<Object>()) };
    let app = cx.to_async();
    cx.foreground_executor().spawn(async move {
        unsafe {
            let native: *mut Object = msg_send![*view, window];
            let number: isize = msg_send![native, windowNumber];
            let mut event: *mut Object = msg_send![class!(NSEvent), mouseEventWithType: kind as usize
                location: NativePoint { x: f32::from(position.x) as f64, y: height - f32::from(position.y) as f64 }
                modifierFlags: 0usize timestamp: 0_f64 windowNumber: number context: std::ptr::null_mut::<Object>()
                eventNumber: 0isize clickCount: 1isize pressure: 1_f32];
            assert!(!event.is_null());
            if matches!(kind, 3 | 4 | 7) {
                // The NSEvent constructor leaves buttonNumber at zero, even for right events.
                let cg: *mut std::ffi::c_void = msg_send![event, CGEvent];
                assert!(!cg.is_null());
                CGEventSetIntegerValueField(cg, 3, 1); // kCGMouseEventButtonNumber
                event = msg_send![class!(NSEvent), eventWithCGEvent: cg];
                assert!(!event.is_null());
                let button: isize = msg_send![event, buttonNumber];
                assert_eq!(button, 1, "right-pointer capture must report the right button");
                let at: NativePoint = msg_send![event, locationInWindow];
                assert!((at.x - f32::from(position.x) as f64).abs() < 0.01
                    && (at.y - (height - f32::from(position.y) as f64)).abs() < 0.01,
                    "right-pointer capture must preserve window coordinates");
            }
            match kind {
                1 => { let _: () = msg_send![*view, mouseDown: event]; }
                2 => { let _: () = msg_send![*view, mouseUp: event]; }
                3 => { let _: () = msg_send![*view, rightMouseDown: event]; }
                4 => { let _: () = msg_send![*view, rightMouseUp: event]; }
                6 => { let _: () = msg_send![*view, mouseDragged: event]; }
                7 => { let _: () = msg_send![*view, rightMouseDragged: event]; }
                _ => { let _: () = msg_send![*view, mouseMoved: event]; }
            }
        }
        app.update(|cx| after(cx)).unwrap();
    }).detach();
}
#[cfg(not(target_os = "macos"))]
pub fn dispatch_checked(
    _: &gpui::Window,
    _: gpui::PlatformInput,
    _: &gpui::App,
    _: impl FnOnce(&gpui::App) + 'static,
) {
    panic!("native pointer capture requires macOS");
}
