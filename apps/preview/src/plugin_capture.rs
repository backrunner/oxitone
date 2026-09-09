//! Real embedded plugin lifecycle smoke; native window count remains one.
use crate::{
    plugin_details::DetailTarget, plugin_window::DetailTab, ui::Preview, window_manager::WindowId,
};
use gpui::*;

pub fn open(this: &mut Preview, mode: &str, cx: &mut Context<Preview>) {
    let project = this.project.as_ref().unwrap().clone();
    let channel = project
        .snapshot
        .channels
        .iter()
        .find(|c| mode != "synth" || c.instrument.plugin_id == "oxitone.wavetable")
        .unwrap();
    let target = DetailTarget::Instrument(channel.id.clone());
    let first = this.open_plugin(target.clone(), cx).unwrap();
    assert_eq!(this.open_plugin(target, cx).unwrap(), first);
    let channel = project
        .snapshot
        .channels
        .iter()
        .find(|c| !c.effect_chain.is_empty())
        .unwrap();
    let second = this
        .open_plugin(DetailTarget::ChannelInsert(channel.id.clone(), 0), cx)
        .unwrap();
    assert_ne!(first, second);
    select(this, mode, cx);
}
pub fn close_effect(this: &mut Preview, cx: &mut Context<Preview>) -> DetailTarget {
    let (key, target) = this
        .plugin_windows
        .iter()
        .find(|(_, entity)| entity.read(cx).target.slot().is_some())
        .map(|(key, entity)| (key.clone(), entity.read(cx).target.clone()))
        .unwrap();
    this.plugin_windows.remove(&key);
    assert_eq!(this.plugin_windows.len(), 1);
    target
}
pub fn reopen(this: &mut Preview, target: DetailTarget, mode: &str, cx: &mut Context<Preview>) {
    this.open_plugin(target, cx).unwrap();
    assert_eq!(this.plugin_windows.len(), 2);
    select(this, mode, cx);
}
fn select(this: &mut Preview, mode: &str, cx: &mut Context<Preview>) {
    let instrument = matches!(mode, "instrument" | "synth");
    let entity = this
        .plugin_windows
        .iter()
        .find(|(_, entity)| entity.read(cx).target.slot().is_none() == instrument)
        .unwrap()
        .1
        .clone();
    let id = WindowId::Plugin(entity.entity_id().as_u64());
    this.document.windows.focus(id);
    if let Ok(dimensions) = std::env::var("OXITONE_PREVIEW_CAPTURE_PLUGIN_SIZE") {
        let (w, h) = dimensions.split_once('x').unwrap();
        let mut bounds = this.document.windows.state(id).bounds;
        bounds.width = w.parse().unwrap();
        bounds.height = h.parse().unwrap();
        this.document.windows.set_bounds(id, bounds);
    }
    entity.update(cx, |view, cx| {
        if let Ok(page) = std::env::var("OXITONE_PREVIEW_CAPTURE_PAGE") {
            view.page = page;
        }
        if mode == "info" {
            view.tab = DetailTab::Plugin;
        }
        view.request_focus = true;
        cx.notify();
    });
}
pub fn navigate(
    entities: &[Entity<crate::plugin_window::PluginWindow>],
    frame: usize,
    window: &mut Window,
    cx: &mut App,
) {
    if !(10..=13).contains(&frame) {
        return;
    }
    let entity = entities
        .iter()
        .find(|entity| entity.read(cx).target.slot().is_none())
        .unwrap()
        .clone();
    if frame == 10 || frame == 12 {
        entity.update(cx, |view, _| view.focus(window));
        window.dispatch_keystroke(
            Keystroke::parse(if frame == 10 { "end" } else { "home" }).unwrap(),
            cx,
        );
    } else {
        let view = entity.read(cx);
        let expected = if frame == 11 {
            -view.scroll.max_offset().height
        } else {
            px(0.)
        };
        assert!(f32::from(view.scroll.offset().y - expected).abs() < 1.);
    }
}
pub fn verify(this: &Preview, cx: &App) {
    for handle in this.plugin_windows.values() {
        let detail = handle.read(cx);
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
