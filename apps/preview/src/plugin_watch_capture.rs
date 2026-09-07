//! Opt-in observation for the multiwindow watch failure/recovery integration smoke.
use crate::ui::Preview;
use gpui::App;
use serde_json::json;

pub fn state(this: &Preview, cx: &App) -> String {
    let windows: Vec<_> = this.plugin_windows.values().filter_map(|handle| {
        let window = handle.read(cx).ok()?;
        let details = window.details.as_ref()?;
        let descriptor = &details.info.descriptor;
        Some(json!({"pluginId":descriptor.plugin_id,"revision":window.project.snapshot.revision,
            "title":window.panel.as_ref().map(|p| &p.title), "page":window.page,
            "sync":window.sync_status, "source":details.source,
            "uiError":window.project.panels.error(&(descriptor.plugin_id.clone(), descriptor.plugin_version.clone()))}))
    }).collect();
    json!({"revision":this.project.as_ref().map(|p| p.snapshot.revision),"windows":windows,
        "error":this.diagnostic.as_ref().map(|d| format!("{}: {}",d.code,d.message))})
    .to_string()
}
