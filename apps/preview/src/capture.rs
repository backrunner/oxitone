//! Opt-in developer capture of the real GPUI window, even without display-link ticks.
//! Never changes system appearance. Only explicit DAW smoke edits its disposable source fixture.
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
pub fn schedule(window: &Window, cx: &mut Context<Preview>) {
    use crate::capture_surface::Surface;
    let Some(path) = std::env::var_os("OXITONE_PREVIEW_CAPTURE") else {
        return;
    };
    let appearance = std::env::var("OXITONE_PREVIEW_APPEARANCE").unwrap_or_default();
    let navigation = std::env::var("OXITONE_PREVIEW_CAPTURE_NAVIGATION").is_ok_and(|v| v == "1");
    let plugin = std::env::var("OXITONE_PREVIEW_CAPTURE_PLUGIN").unwrap_or_default();
    let mixer = std::env::var("OXITONE_PREVIEW_CAPTURE_MIXER").unwrap_or_default();
    let transport = crate::capture_transport::enabled();
    let daw = crate::capture_daw::enabled();
    let builtin = std::env::var("OXITONE_PREVIEW_CAPTURE_BUILTIN").unwrap_or_default();
    let watch = !plugin.is_empty()
        && std::env::var("OXITONE_PREVIEW_CAPTURE_WATCH").is_ok_and(|v| v == "1");
    assert!(
        !transport || (!navigation && plugin.is_empty() && mixer.is_empty()),
        "transport capture must run separately from other capture modes"
    );
    let revision = std::env::var("OXITONE_PREVIEW_CAPTURE_REVISION")
        .ok()
        .map(|v| v.parse::<u64>().expect("capture revision must be u64"))
        .unwrap_or(1);
    let window_handle = window.window_handle();
    let Some(surface) = Surface::new(window) else {
        return;
    };
    cx.spawn(async move |this, cx| {
        surface.appearance(&appearance);
        let mut closed = None;
        let mut ready_frames = 0;
        let mut transport_smoke = crate::capture_transport::Smoke::default();
        let mut daw_smoke = crate::capture_daw::Smoke::default();
        let mut builtin_smoke = crate::capture_builtin::Smoke::default();
        let mut watch_state = String::new();
        let mut settled_frames = 0;
        for _ in 0..if std::env::var_os("OXITONE_PREVIEW_CAPTURE_EDITING").is_some()
            || std::env::var_os("OXITONE_PREVIEW_CAPTURE_UI_REVIEW").is_some()
            || !builtin.is_empty()
        {
            900
        } else if watch {
            600
        } else if daw {
            300
        } else {
            150
        } {
            Timer::after(std::time::Duration::from_millis(100)).await;
            let Ok(current) = this.update(cx, |this, _| {
                this.project.as_ref().map(|p| p.snapshot.revision)
            }) else {
                return;
            };
            // Metal may grow its instance buffer and request another frame. Without a
            // display link, explicitly invalidate the scene so that retry is presented.
            let _ = cx.update_window(window_handle, |_, window, _| window.refresh());
            surface.redraw();
            if current.is_some() {
                ready_frames += 1;
            }
            if !mixer.is_empty() {
                this.update(cx, |this, cx| {
                    crate::capture_mixer::step(this, ready_frames, &mixer, cx)
                })
                .unwrap();
            }
            if !plugin.is_empty() {
                // Let AppKit finish presenting/closing between lifecycle steps.
                // Closing in the creation dispatch leaves GPUI's initial native draw queued.
                if ready_frames == 2 {
                    this.update(cx, |this, cx| {
                        crate::plugin_capture::open(this, &plugin, cx)
                    })
                    .unwrap()
                } else if ready_frames == 6 {
                    closed = Some(
                        this.update(cx, |this, cx| crate::plugin_capture::close_effect(this, cx))
                            .unwrap(),
                    );
                } else if ready_frames == 8 {
                    this.update(cx, |this, cx| {
                        crate::plugin_capture::reopen(this, closed.take().unwrap(), &plugin, cx)
                    })
                    .unwrap()
                } else {
                };
                if ready_frames == 8 {
                    eprintln!("Preview plugin windows opened");
                }
            }
            if watch && ready_frames >= 8 {
                let state = this
                    .update(cx, |this, cx| crate::plugin_watch_capture::state(this, cx))
                    .unwrap();
                if state != watch_state {
                    eprintln!("Preview panel-watch state {state}");
                    watch_state = state;
                }
            }
            if !plugin.is_empty() {
                cx.update_window(window_handle, |_, window, cx| {
                    if let Some(entity) = this.upgrade() {
                        let plugins: Vec<_> =
                            entity.read(cx).plugin_windows.values().cloned().collect();
                        crate::plugin_capture::navigate(&plugins, ready_frames, window, cx);
                    }
                })
                .unwrap();
            }
            if navigation {
                let _ = cx.update_window(window_handle, |_, window, cx| {
                    if let Some(view) = this.upgrade() {
                        crate::capture_navigation::step(ready_frames, &view, window, cx);
                    }
                });
            }
            if !mixer.is_empty() {
                let _ = cx.update_window(window_handle, |_, window, cx| {
                    if let Some(view) = this.upgrade() {
                        crate::capture_mixer::keyboard(ready_frames, &view, window, cx);
                    }
                });
            }
            if transport {
                assert!(
                    !daw,
                    "DAW capture must run separately from transport capture"
                );
                cx.update_window(window_handle, |_, window, cx| {
                    if let Some(view) = this.upgrade() {
                        transport_smoke.step(ready_frames, &view, window, cx);
                    }
                })
                .unwrap();
            }
            if daw {
                cx.update_window(window_handle, |_, window, cx| {
                    if let Some(view) = this.upgrade() {
                        daw_smoke.step(ready_frames, &view, window, cx);
                    }
                })
                .unwrap();
            }
            if !builtin.is_empty() {
                cx.update_window(window_handle, |_, window, cx| {
                    if let Some(view) = this.upgrade() {
                        builtin_smoke.step(&builtin, &view, window, cx);
                    }
                })
                .unwrap();
            }
            if ready_frames >= if transport { 36 } else { 14 }
                && current.is_some_and(|v| v >= revision)
                && (!daw || daw_smoke.complete())
                && (builtin.is_empty() || builtin_smoke.complete())
            {
                // Let the final workspace transition reach the native surface before capture.
                settled_frames += 1;
                if settled_frames < 4 {
                    continue;
                }
                if !plugin.is_empty() {
                    this.update(cx, |this, cx| crate::plugin_capture::verify(this, cx))
                        .unwrap();
                }
                let number = surface.number;
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
                // Exercise the main window's native close callback with details still open.
                if std::env::var_os("OXITONE_PREVIEW_CAPTURE_UI_REVIEW").is_some() {
                    this.update(cx, |this, cx| this.save_and_close(cx)).unwrap();
                } else {
                    surface.close();
                }
                return;
            }
        }
        eprintln!("Preview capture timed out waiting for a valid project/revision");
        let _ = cx.update(|cx| cx.quit());
    })
    .detach();
}

#[cfg(not(target_os = "macos"))]
pub fn schedule(_: &Window, _: &mut Context<Preview>) {}
