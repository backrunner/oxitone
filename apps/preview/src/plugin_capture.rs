//! Developer smoke through the same window-opening path as Mixer clicks.
use crate::{
    plugin_details::DetailTarget,
    plugin_window::{DetailTab, PluginWindow},
    ui::Preview,
};
use gpui::*;

pub fn open(
    this: &mut Preview,
    mode: &str,
    cx: &mut Context<Preview>,
) -> Vec<WindowHandle<PluginWindow>> {
    let project = this.project.as_ref().unwrap().clone();
    let channel = project
        .snapshot
        .channels
        .iter()
        .find(|c| mode != "synth" || c.instrument.plugin_id == "oxitone.wavetable")
        .expect("capture needs an instrument");
    let instrument = DetailTarget::Instrument(channel.id.clone());
    let first = this.open_plugin(instrument.clone(), cx).unwrap();
    assert_eq!(
        this.open_plugin(instrument.clone(), cx),
        Some(first),
        "repeat open reuses window"
    );
    let (owner, _) = project
        .snapshot
        .channels
        .iter()
        .find_map(|c| c.effect_chain.first().map(|e| (c, e)))
        .expect("capture needs an effect");
    let second = this
        .open_plugin(DetailTarget::ChannelInsert(owner.id.clone(), 0), cx)
        .unwrap();
    assert_ne!(first, second, "instrument/effect have independent windows");
    select(first, second, mode, cx)
}

pub fn close_effect(this: &Preview, cx: &mut Context<Preview>) -> WindowHandle<PluginWindow> {
    let handle = *this
        .plugin_windows
        .iter()
        .find(|(target, _)| target.slot().is_some())
        .unwrap()
        .1;
    handle
        .update(cx, |_, window, _| window.remove_window())
        .unwrap();
    handle
}

pub fn reopen(
    this: &mut Preview,
    closed: WindowHandle<PluginWindow>,
    mode: &str,
    cx: &mut Context<Preview>,
) -> Vec<WindowHandle<PluginWindow>> {
    let (target, _) = this
        .plugin_windows
        .iter()
        .find(|(_, h)| **h == closed)
        .unwrap();
    let target = target.clone();
    let first = *this
        .plugin_windows
        .iter()
        .find(|(t, _)| t.slot().is_none())
        .unwrap()
        .1;
    assert!(
        first.read(cx).is_ok(),
        "closing an effect keeps the instrument open"
    );
    let second = this.open_plugin(target, cx).unwrap();
    assert_ne!(closed, second, "closed window can be reopened");
    assert_eq!(this.plugin_windows.len(), 2);
    select(first, second, mode, cx)
}

fn select(
    first: WindowHandle<PluginWindow>,
    second: WindowHandle<PluginWindow>,
    mode: &str,
    cx: &mut Context<Preview>,
) -> Vec<WindowHandle<PluginWindow>> {
    match mode {
        "instrument" | "synth" => {
            first
                .update(cx, |_, window, _| window.activate_window())
                .unwrap();
            vec![second, first]
        }
        "effect" | "info" => {
            second
                .update(cx, |view, window, cx| {
                    if mode == "info" {
                        view.tab = DetailTab::Plugin;
                        cx.notify();
                    }
                    window.activate_window();
                })
                .unwrap();
            vec![first, second]
        }
        _ => panic!("OXITONE_PREVIEW_CAPTURE_PLUGIN must be instrument, synth, effect or info"),
    }
}

pub fn navigate(this: &Preview, frame: usize, cx: &mut Context<Preview>) {
    if !(10..=13).contains(&frame) {
        return;
    }
    let handle = this
        .plugin_windows
        .iter()
        .find(|(t, _)| t.slot().is_none())
        .unwrap()
        .1;
    if frame == 10 || frame == 12 {
        cx.update_window((*handle).into(), |_, window, cx| {
            window.dispatch_keystroke(
                Keystroke::parse(if frame == 10 { "end" } else { "home" }).unwrap(),
                cx,
            );
        })
        .unwrap();
    } else {
        let view = handle.read(cx).unwrap();
        let expected = if frame == 11 {
            -view.scroll.max_offset().height
        } else {
            px(0.)
        };
        assert!(
            (f32::from(view.scroll.offset().y - expected)).abs() < 1.,
            "detail keyboard scrolling"
        );
    }
}

pub fn verify(this: &Preview, cx: &App) {
    for handle in this.plugin_windows.values() {
        let detail = handle.read(cx).unwrap();
        assert_eq!(
            detail.project.snapshot.revision,
            this.project.as_ref().unwrap().snapshot.revision
        );
        let info = detail.details.as_ref().unwrap();
        let expected =
            crate::plugin_details::resolve(this.project.as_ref().unwrap(), &detail.target).unwrap();
        assert_eq!(
            info.source, expected.source,
            "open detail follows the accepted source"
        );
        assert!(!info.parameters.is_empty());
        eprintln!(
            "Preview plugin-window state {}",
            serde_json::json!({
                "revision": detail.project.snapshot.revision,
                "pluginId": info.info.descriptor.plugin_id,
                "parameters": info.source["parameters"]
            })
        );
        // The bundled drum example exercises actual registered C ABI metadata.
        if info.info.descriptor.plugin_id == "example.drums"
            || info.info.descriptor.plugin_id == "fixture.gain"
        {
            let library = info
                .info
                .library
                .as_ref()
                .expect("registered dynamic origin");
            assert_eq!(library.sha256.len(), 64);
        }
    }
    assert!(!this.playback.playing);
    eprintln!("Preview plugin-window smoke passed");
}
