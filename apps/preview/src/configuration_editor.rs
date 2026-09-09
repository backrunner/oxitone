//! Instance settings are a separate editor; library selection cannot retarget an open edit.
use crate::{plugin_manager::CatalogEntry, ui::Preview};
use gpui::{prelude::*, *};

impl Preview {
    fn configuration_entry(&self) -> Option<&CatalogEntry> {
        let handle = self.document.configuration.plugin.as_ref()?;
        self.document
            .view
            .as_ref()?
            .plugins
            .iter()
            .find(|entry| &entry.handle == handle)
    }
    pub fn configuration_title(&self) -> String {
        let Some(entry) = self.configuration_entry() else {
            return "Plugin settings".into();
        };
        let usage = entry
            .usages
            .iter()
            .find(|usage| self.document.configuration.selected.as_ref() == Some(&usage.handle()));
        usage.map_or_else(
            || entry.display_name.clone(),
            |usage| {
                if usage.kind == "instrument" {
                    format!("{} — {}", entry.display_name, usage.label)
                } else {
                    format!(
                        "{} — {}, insert {}",
                        entry.display_name,
                        usage.label,
                        usage.index + 1
                    )
                }
            },
        )
    }
    pub fn configuration_editor(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let mut body = div()
            .id("instance-configuration")
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_y_scroll()
            .track_scroll(&self.document.configuration.scroll)
            .p_3()
            .bg(rgb(t.panel))
            .text_size(px(12.));
        if let Some(entry) = self.configuration_entry() {
            body = body.child(self.configuration_panel(entry, cx));
        } else {
            body = body.child("This plugin is no longer available");
        }
        body
    }
}
