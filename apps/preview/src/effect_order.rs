use crate::{document_wire::DocumentOperation, plugin_manager::CatalogEntry, ui::Preview};
use gpui::{prelude::*, *};

impl Preview {
    pub(super) fn effect_order_operation(
        &self,
        owner: &str,
        instance: &str,
        direction: isize,
    ) -> Option<DocumentOperation> {
        let view = self.document.view.as_ref()?;
        let builder_order = view.arrangement_order.as_ref()?;
        let snapshot = &self.project.as_ref()?.snapshot;
        let effects = snapshot
            .channels
            .iter()
            .find(|channel| channel.id == owner)
            .map(|channel| &channel.effect_chain)
            .or_else(|| {
                snapshot
                    .mixer_channels
                    .iter()
                    .find(|bus| bus.id == owner)
                    .map(|bus| &bus.inserts)
            })?;
        let mut order: Vec<_> = effects
            .iter()
            .map(|effect| effect.instance_id.clone())
            .collect::<Option<_>>()?;
        let index = order.iter().position(|id| id == instance)?;
        let destination = index
            .checked_add_signed(direction)
            .filter(|destination| *destination < order.len())?;
        order.swap(index, destination);
        let channel = builder_order.channels.iter().position(|id| id == owner);
        let owner_index = channel.or_else(|| {
            builder_order
                .mixer_channels
                .iter()
                .position(|id| id == owner)
        })?;
        let order = order
            .iter()
            .map(|id| {
                effects
                    .iter()
                    .position(|e| e.instance_id.as_ref() == Some(id))
                    .unwrap()
            })
            .collect();
        Some(DocumentOperation::Project {
            edit: crate::project_edit::ProjectEdit::EffectOrder {
                owner: if channel.is_some() { "channel" } else { "bus" }.into(),
                index: owner_index,
                order,
            },
        })
    }
    pub(super) fn effect_order_controls(
        &self,
        entry: &CatalogEntry,
        cx: &mut Context<Self>,
    ) -> Div {
        let selected = self.document.configuration.selected.as_ref();
        let Some(usage) = entry
            .usages
            .iter()
            .find(|usage| usage.kind != "instrument" && selected == Some(&usage.handle()))
        else {
            return div();
        };
        let mut row = div().flex().gap_2();
        for (label, icon, direction) in [
            ("Move earlier in chain", crate::ui_icons::Icon::Left, -1),
            ("Move later in chain", crate::ui_icons::Icon::Right, 1),
        ] {
            if let Some(operation) =
                self.effect_order_operation(&usage.owner, &usage.handle(), direction)
            {
                row = row.child(
                    self.theme
                        .icon_button(format!("effect-order-{direction}"), icon, label)
                        .opacity(if self.document_ready() { 1. } else { 0.4 })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.document_request(operation.clone());
                            cx.notify();
                        })),
                );
            }
        }
        row
    }
}
