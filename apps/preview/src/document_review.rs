use crate::{document_wire::DocumentOperation, ui::Preview};
use gpui::{prelude::*, *};

pub(super) fn text_pane(id: &'static str, label: &str, text: &str) -> impl IntoElement {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_2()
        .child(div().child(label.to_owned()))
        .child(
            div()
                .id(id)
                .overflow_y_scroll()
                .h(px(190.))
                .font_family("Menlo")
                .text_size(px(11.))
                .child(text.to_owned()),
        )
}

impl Preview {
    pub fn document_review(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let Some(view) = &self.document.view else {
            return div();
        };
        if let Some(conflict) = view.conflicts.first() {
            let draft = view
                .files
                .iter()
                .find(|file| file.path == conflict.path)
                .map_or("", |file| file.text.as_str());
            let mut controls = div().flex().gap_2();
            for (id, label, resolution) in [
                ("disk-use", "Use disk", "use-disk"),
                ("disk-keep", "Keep draft", "keep-draft"),
                ("disk-merge", "Merge", "merge"),
            ] {
                let operation = DocumentOperation::ResolveConflict {
                    file_name: conflict.path.clone(),
                    disk_hash: conflict.disk_hash.clone(),
                    resolution: resolution.into(),
                };
                controls = controls.child(
                    theme
                        .button(id, label)
                        .opacity(if self.document_operation_ready(&operation) {
                            1.
                        } else {
                            0.4
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.document_request(operation.clone());
                            cx.notify();
                        })),
                );
            }
            return div()
                .p_3()
                .flex_shrink_0()
                .bg(rgb(theme.panel))
                .flex()
                .flex_col()
                .gap_2()
                .child(format!(
                    "Resolve external edit · {} · {} file(s)",
                    conflict.path,
                    view.conflicts.len()
                ))
                .child(
                    div()
                        .flex()
                        .gap_3()
                        .child(text_pane("conflict-draft", "Unsaved draft", draft))
                        .child(text_pane("conflict-disk", "Disk version", &conflict.disk)),
                )
                .child(controls);
        }
        if let Some(plan) = &view.materialization {
            let confirm = plan.plan_id.clone();
            let cancel = plan.plan_id.clone();
            return div().p_3().flex_shrink_0().bg(rgb(theme.panel)).flex().flex_col().gap_2()
                .child(format!("Detach {} → {} notes · {} clip(s) · {}", plan.before_notes, plan.after_notes, plan.affected_clips.len(), plan.file_name))
                .child("These notes become a local Pattern. Generator updates stop applying to this selection. Existing imports and dependencies are retained.")
                .when(plan.retains_original_evaluation, |panel| panel.child("The original expression still runs once to preserve its effects; its returned notes are replaced."))
                .child(div().flex().gap_3().child(text_pane("detach-before", "Current source", &plan.before_text)).child(text_pane("detach-after", "Proposed source", &plan.after_text)))
                .child(div().flex().gap_2()
                    .child(theme.primary("detach-confirm", "Detach notes").opacity(if self.document_ready() { 1. } else { 0.4 })
                        .on_click(cx.listener(move |this, _, _, cx| { this.document_request(DocumentOperation::ConfirmMaterialize { plan_id: confirm.clone() }); cx.notify(); })))
                    .child(theme.button("detach-cancel", "Cancel").on_click(cx.listener(move |this, _, _, cx| { this.document_request(DocumentOperation::CancelMaterialize { plan_id: cancel.clone() }); cx.notify(); }))));
        }
        self.rack_review(cx)
    }
}
