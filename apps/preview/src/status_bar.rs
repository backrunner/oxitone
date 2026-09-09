//! Quiet summary with explicit disclosure for engine and document diagnostics.
use crate::ui::Preview;
use gpui::{prelude::*, *};

impl Preview {
    pub(crate) fn footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let fault = self.playback.faults > 0 || self.playback.xruns > 0;
        let view = self.document.view.as_ref();
        let (label, color) = if view.is_some_and(|v| !v.conflicts.is_empty()) {
            ("Conflict", t.danger)
        } else if view.is_some_and(|v| v.status == "closed") {
            ("Disconnected", t.danger)
        } else if view.is_some_and(|v| v.status == "invalid") {
            ("Invalid code", t.danger)
        } else if self.active_diagnostic().is_some() {
            ("Error", t.danger)
        } else if view.is_some_and(|v| v.saving) || self.close.saving() {
            ("Saving", t.gold)
        } else if self.document.pending.is_some()
            || self
                .document
                .view
                .as_ref()
                .is_some_and(|v| v.saving || v.status != "ready")
        {
            ("Syncing", t.gold)
        } else if view.is_some() && !self.document_ready() {
            ("Updating audio", t.gold)
        } else if self.document.view.as_ref().is_some_and(|v| v.modified) {
            ("Modified", t.gold)
        } else if self.document.view.is_some() {
            ("Saved", t.accent)
        } else {
            ("Preview", t.muted)
        };
        let latency = self
            .project
            .as_ref()
            .map(|p| self.playback.latency as f64 / f64::from(p.snapshot.sample_rate) * 1000.);
        let mut root = div()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .bg(rgb(t.panel))
            .border_t_1()
            .border_color(rgb(t.border));
        if self.status_details {
            let revision = self.project.as_ref().map_or(0, |p| p.snapshot.revision);
            root = root.child(
                div()
                    .px_4()
                    .py_3()
                    .flex()
                    .justify_between()
                    .gap_4()
                    .text_size(px(11.))
                    .border_b_1()
                    .border_color(rgb(t.border))
                    .child(div().flex_1().min_w_0().child(self.status.clone()).child(
                        div().mt_1().text_color(rgb(t.muted)).child(
                            self.document.view.as_ref().map_or(
                                format!("Graph revision {revision}"),
                                |v| {
                                    format!(
                                        "Source {} · Accepted {} · Saved {} · Graph {revision}",
                                        v.revision, v.accepted_revision, v.saved_revision
                                    )
                                },
                            ),
                        ),
                    ))
                    .child(
                        div()
                            .text_color(rgb(if fault { t.danger } else { t.muted }))
                            .child(format!(
                                "{:.1} kHz · {:.1} ms · {} xruns · {} plugin faults {}",
                                self.project
                                    .as_ref()
                                    .map_or(0., |p| p.snapshot.sample_rate as f64 / 1000.),
                                latency.unwrap_or(0.),
                                self.playback.xruns,
                                self.playback.faults,
                                self.playback.fault_nodes
                            )),
                    ),
            );
        }
        root.child(
            div()
                .h(px(28.))
                .px_4()
                .flex()
                .items_center()
                .justify_between()
                .gap_4()
                .text_size(px(10.))
                .child(
                    div()
                        .id("document-status")
                        .flex()
                        .items_center()
                        .gap_2()
                        .cursor_pointer()
                        .child(div().size(px(6.)).rounded_full().bg(rgb(color)))
                        .child(div().text_color(rgb(t.muted)).child(label))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.status_details = !this.status_details;
                            cx.notify();
                        })),
                )
                .child(div().flex_1())
                .child(
                    t.icon_tool(
                        "scopes",
                        crate::ui_icons::Icon::Wave,
                        "Show / hide analysis",
                        self.show_scopes,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.show_scopes = !this.show_scopes;
                        cx.notify();
                    })),
                )
                .child(
                    div()
                        .id("engine-status")
                        .flex()
                        .items_center()
                        .gap_3()
                        .cursor_pointer()
                        .text_color(rgb(t.muted))
                        .child(div().child(format!("CPU {:>4.1}%", self.playback.load * 100.)))
                        .when(fault, |d| {
                            d.child(div().text_color(rgb(t.danger)).child("Audio alerts"))
                        })
                        .child(crate::ui_icons::icon(
                            if self.status_details {
                                crate::ui_icons::Icon::ChevronDown
                            } else {
                                crate::ui_icons::Icon::ChevronUp
                            },
                            t.muted,
                        ))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.status_details = !this.status_details;
                            cx.notify();
                        })),
                ),
        )
    }
}
