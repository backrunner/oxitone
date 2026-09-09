use crate::{document_review::text_pane, document_wire::DocumentOperation, ui::Preview};
use gpui::{prelude::*, *};

impl Preview {
    pub(super) fn rack_controls(
        &self,
        entry: &crate::plugin_manager::CatalogEntry,
        cx: &mut Context<Self>,
    ) -> Div {
        let selected = self.document.configuration.selected.as_ref();
        let usage = entry
            .usages
            .iter()
            .find(|usage| usage.kind != "instrument" && selected == Some(&usage.handle()));
        let Some(usage) = usage else {
            return div();
        };
        let rack = self.document.view.as_ref().and_then(|view| {
            view.rack_sites.iter().find(|rack| {
                rack.scope == "reference"
                    && rack.usages.len() == 1
                    && rack.usages[0].owner == usage.owner
            })
        });
        let Some(rack) = rack else {
            return div();
        };
        let operation = DocumentOperation::PlanMaterializeRack {
            site: rack.handle.clone(),
            owner: Some(usage.owner.clone()),
        };
        div().child(
            self.theme
                .button("configuration-detach-chain", "Detach this chain…")
                .opacity(if self.document_ready() { 1. } else { 0.4 })
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.document_request(operation.clone());
                    cx.notify();
                })),
        )
    }
    pub(super) fn rack_review(&self, cx: &mut Context<Self>) -> Div {
        let Some(plan) = self
            .document
            .view
            .as_ref()
            .and_then(|view| view.rack_materialization.as_ref())
        else {
            return div();
        };
        let theme = self.theme;
        let confirm = plan.plan_id.clone();
        let cancel = confirm.clone();
        div().p_3().flex_shrink_0().bg(rgb(theme.panel)).flex().flex_col().gap_2()
            .child(format!("Detach serial chain · {} effects × {} uses · {}", plan.effects, plan.affected_owners.len(), plan.file_name))
            .child("Each effect becomes local configuration with the same order, parameters, mix, bypass, resources and automation. Preset updates stop applying to this chain. Plugin libraries and dependency imports are retained.")
            .when(plan.retains_original_evaluation, |panel| panel.child("The original expression still runs once to preserve its effects; its returned chain is replaced."))
            .child(div().flex().gap_3().child(text_pane("rack-before", "Current source", &plan.before_text)).child(text_pane("rack-after", "Proposed source", &plan.after_text)))
            .child(div().flex().gap_2()
                .child(theme.button("rack-confirm", "Confirm this change").opacity(if self.document_ready() { 1. } else { 0.4 }).on_click(cx.listener(move |this, _, _, cx| {
                    this.document_request(DocumentOperation::ConfirmMaterialize { plan_id: confirm.clone() }); cx.notify();
                })))
                .child(theme.button("rack-cancel", "Cancel").on_click(cx.listener(move |this, _, _, cx| {
                    this.document_request(DocumentOperation::CancelMaterialize { plan_id: cancel.clone() }); cx.notify();
                }))))
    }
}
