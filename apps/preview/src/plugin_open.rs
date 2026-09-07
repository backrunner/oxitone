use crate::{
    plugin_details::{self, DetailTarget},
    plugin_window::{resolve_panel, PluginWindow},
    ui::Preview,
};
use gpui::*;
impl Preview {
    pub fn open_plugin(
        &mut self,
        target: DetailTarget,
        cx: &mut Context<Self>,
    ) -> Option<WindowHandle<PluginWindow>> {
        self.plugin_windows
            .retain(|_, handle| handle.read(cx).is_ok());
        if let Some(handle) = self.plugin_windows.get(&target).copied() {
            if handle
                .update(cx, |_, window, _| window.activate_window())
                .is_ok()
            {
                return Some(handle);
            }
        }
        let project = self.project.clone()?;
        let details = plugin_details::resolve(&project, &target)?;
        let panel = resolve_panel(&project, Some(&details))?;
        let owner = cx.entity();
        let status = crate::plugin_window::sync_status(self);
        let key = target.clone();
        let mut bounds = Bounds::centered(
            None,
            size(px(panel.size.width as f32), px(panel.size.height as f32)),
            cx,
        );
        let offset = px((self.plugin_windows.len() % 6) as f32 * 24.);
        bounds.origin += point(offset, offset);
        match cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Oxitone · Plugin Details".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(18.), px(21.))),
                }),
                window_min_size: Some(size(px(440.), px(280.))),
                ..Default::default()
            },
            |window, cx| {
                cx.new(|cx| {
                    let detail = PluginWindow::new(target, project, owner, status, window, cx);
                    window.set_window_title(&detail.title());
                    detail
                })
            },
        ) {
            Ok(handle) => {
                self.plugin_windows.insert(key, handle);
                Some(handle)
            }
            Err(error) => {
                self.diagnostic = Some(crate::model::Diagnostic {
                    code: "PreviewWindowFailed".into(),
                    message: error.to_string(),
                    path: None,
                });
                cx.notify();
                None
            }
        }
    }
}
