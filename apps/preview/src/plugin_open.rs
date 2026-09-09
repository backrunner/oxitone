//! Open a reusable plugin view inside the workspace; no native window or DSP creation.
use crate::{
    plugin_details::{self, DetailTarget},
    plugin_window::PluginWindow,
    ui::Preview,
    window_manager::WindowId,
};
use gpui::*;
impl Preview {
    pub fn open_plugin(
        &mut self,
        target: DetailTarget,
        cx: &mut Context<Self>,
    ) -> Option<Entity<PluginWindow>> {
        let project = self.project.clone()?;
        let key = crate::plugin_identity::key(&project, &target);
        if let Some(entity) = self.plugin_windows.get(&key).cloned() {
            self.document
                .windows
                .focus(WindowId::Plugin(entity.entity_id().as_u64()));
            entity.update(cx, |view, cx| {
                view.request_focus = true;
                cx.notify();
            });
            cx.notify();
            return Some(entity);
        }
        plugin_details::resolve(&project, &target)?;
        let owner = cx.entity();
        let status = crate::plugin_window::sync_status(self);
        let theme = self.theme;
        let entity = cx.new(|cx| PluginWindow::new(target, project, owner, status, theme, cx));
        let id = WindowId::Plugin(entity.entity_id().as_u64());
        if let Some(panel) = &entity.read(cx).panel {
            let mut bounds = self.document.windows.state(id).bounds;
            bounds.width = panel.size.width as f32;
            bounds.height = panel.size.height as f32 + 28.;
            let desktop = self.document.windows.desktop.get().size;
            if desktop.width > px(300.) && desktop.height > px(200.) {
                bounds.width = bounds.width.min(f32::from(desktop.width) * 0.9);
                bounds.height = bounds.height.min(f32::from(desktop.height) * 0.85);
            }
            self.document.windows.set_bounds(id, bounds);
        }
        self.document.windows.focus(id);
        self.plugin_windows.insert(key, entity.clone());
        cx.notify();
        Some(entity)
    }
}
