//! Compact lane selection and curve projection in a resizable workspace window.
use crate::ui::Preview;
use gpui::{prelude::*, *};
use oxitone_transport::CompiledAutomation;
use std::sync::Arc;
impl Preview {
    pub fn automation_panel(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let lane_id = self.automation_lane().map(|lane| lane.id.clone());
        let site = self.automation_site().cloned();
        let key = site.as_ref().map(|site| {
            format!(
                "{}:{}:{}",
                self.document.view.as_ref().map_or(0, |v| v.revision),
                lane_id.as_deref().unwrap_or(""),
                site.handle
            )
        });
        if self
            .document
            .automation
            .compiled
            .as_ref()
            .map(|(key, _)| key)
            != key.as_ref()
        {
            self.document.automation.compiled = site
                .as_ref()
                .and_then(|site| {
                    CompiledAutomation::compile(
                        &site.source,
                        self.project.as_ref().map_or(0, |p| p.snapshot.seed),
                    )
                    .ok()
                })
                .map(|source| (key.clone().unwrap(), Arc::new(source)));
        }

        div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .child(crate::automation_lanes::view(self, cx))
            .child(self.automation_editor(cx))
    }
}
