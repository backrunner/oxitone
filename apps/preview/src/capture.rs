//! Opt-in developer capture of the real GPUI window, even without display-link ticks.
//! Never changes system appearance or musical state. Normal launches do no extra work.
use crate::ui::Preview;
use gpui::*;

pub fn window_size() -> Size<Pixels> {
    let default = size(px(1440.), px(920.));
    if std::env::var_os("OXITONE_PREVIEW_CAPTURE").is_none() {
        return default;
    }
    let Ok(value) = std::env::var("OXITONE_PREVIEW_CAPTURE_SIZE") else {
        return default;
    };
    let (w, h) = value
        .split_once('x')
        .expect("capture size must be WIDTHxHEIGHT");
    let w = w
        .parse::<u16>()
        .expect("invalid capture width")
        .clamp(1060, 3840);
    let h = h
        .parse::<u16>()
        .expect("invalid capture height")
        .clamp(720, 2160);
    size(px(f32::from(w)), px(f32::from(h)))
}

#[cfg(target_os = "macos")]
#[allow(
    unexpected_cfgs,
    reason = "objc 0.2 macros check the legacy cargo-clippy feature"
)]
pub fn schedule(window: &Window, cx: &mut Context<Preview>) {
    use objc::{class, msg_send, rc::StrongPtr, runtime::Object, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let Some(path) = std::env::var_os("OXITONE_PREVIEW_CAPTURE") else {
        return;
    };
    let appearance = std::env::var("OXITONE_PREVIEW_APPEARANCE").unwrap_or_default();
    let navigation = std::env::var("OXITONE_PREVIEW_CAPTURE_NAVIGATION").is_ok_and(|v| v == "1");
    let window_handle = window.window_handle();
    let name = match appearance.as_str() {
        "light" => Some(c"NSAppearanceNameAqua"),
        "dark" => Some(c"NSAppearanceNameDarkAqua"),
        "" => None,
        _ => panic!("OXITONE_PREVIEW_APPEARANCE must be light or dark"),
    };
    let RawWindowHandle::AppKit(handle) = HasWindowHandle::window_handle(window).unwrap().as_raw()
    else {
        return;
    };
    // SAFETY: on GPUI's UI thread; retain the NSView until the task finishes.
    // displayLayer is scheduled outside GPUI dispatch to avoid reentrant borrows.
    let view = unsafe { StrongPtr::retain(handle.ns_view.as_ptr().cast::<Object>()) };
    cx.spawn(async move |this, cx| {
        let number: isize;
        unsafe {
            let native_window: *mut Object = msg_send![*view, window];
            number = msg_send![native_window, windowNumber];
            if let Some(name) = name {
                let string: *mut Object =
                    msg_send![class!(NSString), stringWithUTF8String: name.as_ptr()];
                let appearance: *mut Object =
                    msg_send![class!(NSAppearance), appearanceNamed: string];
                let _: () = msg_send![native_window, setAppearance: appearance];
            }
        }
        let mut ready_frames = 0;
        for _ in 0..100 {
            Timer::after(std::time::Duration::from_millis(100)).await;
            let Ok(ready) = this.update(cx, |this, _| this.project.is_some()) else {
                return;
            };
            unsafe {
                let layer: *mut Object = msg_send![*view, layer];
                let _: () = msg_send![*view, displayLayer: layer];
            }
            if ready {
                ready_frames += 1;
            }
            if navigation {
                let _ = cx.update_window(window_handle, |_, window, cx| {
                    if let Some(view) = this.upgrade() {
                        crate::capture_navigation::step(ready_frames, &view, window, cx);
                    }
                });
            }
            if ready_frames >= 14 {
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        std::process::Command::new("/usr/sbin/screencapture")
                            .args(["-x", "-o", "-l", &number.to_string()])
                            .arg(path)
                            .status()
                    })
                    .await;
                match result {
                    Ok(status) if status.success() => eprintln!("Preview capture saved"),
                    other => eprintln!("Preview capture failed: {other:?}"),
                }
                let _ = cx.update(|cx| cx.quit());
                return;
            }
        }
        eprintln!("Preview capture timed out waiting for a valid project");
        let _ = cx.update(|cx| cx.quit());
    })
    .detach();
}

#[cfg(not(target_os = "macos"))]
pub fn schedule(_: &Window, _: &mut Context<Preview>) {}
