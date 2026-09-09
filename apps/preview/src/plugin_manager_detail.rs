//! On-demand maintenance, separate from browsing and instance parameters.
use crate::{
    document_wire::DocumentOperation,
    plugin_manager::{CatalogEntry, LibrarySection},
    ui::Preview,
};
use gpui::{prelude::*, *};

impl Preview {
    pub(super) fn plugin_catalog_detail(
        &self,
        entry: &CatalogEntry,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let t = self.theme;
        let mut panel = div()
            .id("plugin-catalog-detail")
            .h(px((self
                .document
                .windows
                .bounds(crate::window_manager::WindowId::Plugins)
                .height
                * 0.35)
                .clamp(90., 190.)))
            .flex_shrink_0()
            .overflow_y_scroll()
            .track_scroll(&self.document.manager.details_scroll)
            .border_t_1()
            .border_color(rgb(t.border))
            .px_3()
            .py_2()
            .flex()
            .flex_col()
            .gap_2();
        if self.document.manager.section == Some(LibrarySection::Uses) {
            return panel.child(crate::plugin_usage_list::view(self, entry, cx));
        }
        if let Some(error) = &entry.diagnostic {
            panel = panel.child(
                div()
                    .text_size(px(11.))
                    .text_color(rgb(t.danger))
                    .child(error.clone()),
            );
        }
        let mut actions = div().flex().flex_wrap().gap_1();
        let mut operations = Vec::new();
        if entry.source != "builtin" && entry.availability == "available" {
            operations.push((
                "Verify",
                DocumentOperation::VerifyPlugin {
                    plugin: entry.handle.clone(),
                },
            ));
        }
        if let Some(package) = &entry.package_name {
            if entry.availability != "available" {
                operations.push((
                    "Install",
                    DocumentOperation::InstallPlugin {
                        package_name: package.clone(),
                        version: None,
                    },
                ));
            }
            operations.push((
                "Repair",
                DocumentOperation::RepairPlugin {
                    package_name: package.clone(),
                },
            ));
            operations.push((
                "Update",
                DocumentOperation::UpgradePlugin {
                    package_name: package.clone(),
                    version: None,
                },
            ));
            operations.push((
                "Uninstall",
                DocumentOperation::UninstallPlugin {
                    package_name: package.clone(),
                },
            ));
        }
        for (label, operation) in operations {
            let enabled = self.document_operation_ready(&operation);
            actions = actions.child(
                t.ghost(format!("plugin-action-{label}"), label)
                    .opacity(if enabled { 1. } else { 0.4 })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.document_request(operation.clone());
                        cx.notify();
                    })),
            );
        }
        panel
            .child(actions)
            .child(crate::plugin_manager_info::information(t, entry))
    }
}
