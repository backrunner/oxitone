//! The library doubles as an explicit instrument/insert picker, addressed by instance identity.
use crate::{document_wire::DocumentOperation, ui::Preview, window_manager::WindowId};
use gpui::{prelude::*, *};

#[derive(Clone)]
pub struct Slot {
    pub owner: String,
    pub target: String,
    pub instance: Option<String>,
    pub label: String,
}
impl Slot {
    pub fn instrument(&self) -> bool {
        self.target == "instrument"
    }
    pub fn action(&self) -> &'static str {
        if self.instrument() || self.instance.is_some() {
            "Replace"
        } else {
            "Add"
        }
    }
}
impl Preview {
    pub fn choose_plugin(&mut self, slot: Slot, window: &mut Window) {
        if !self.document_ready() {
            return;
        }
        let state = &mut self.document.manager;
        state.open = true;
        state.category = if slot.instrument() { 1 } else { 2 };
        state.assignment = Some(slot);
        state.assignment_pending = false;
        state.query.clear();
        state.selected = None;
        state.section = None;
        state.adding = false;
        state.searching = true;
        state.select_all = true;
        state.input_error = None;
        self.document.windows.focus(WindowId::Plugins);
        self.workspace_focus.focus(window);
    }
    pub fn assign_selected_plugin(&mut self) {
        let state = &self.document.manager;
        let (Some(slot), Some(handle)) = (&state.assignment, &state.selected) else {
            return;
        };
        let Some(entry) = self
            .library_entries()
            .into_iter()
            .find(|e| &e.handle == handle)
        else {
            return;
        };
        if entry.availability != "available"
            || (entry.kind == "instrument") != slot.instrument()
            || !self.document_ready()
        {
            return;
        }
        let request = DocumentOperation::AssignPlugin {
            plugin: handle.clone(),
            owner: slot.owner.clone(),
            target: slot.target.clone(),
            instance: slot.instance.clone(),
        };
        self.document_request(request);
        self.document.manager.assignment_pending = self.document.pending.is_some();
    }
    pub fn remove_plugin_slot(&mut self, slot: &Slot) {
        if !self.document_ready() || slot.instrument() {
            return;
        }
        let Some(p) = &self.project else {
            return;
        };
        let Some(order) = self
            .document
            .view
            .as_ref()
            .and_then(|v| v.arrangement_order.as_ref())
        else {
            return;
        };
        let bus = slot.target == "busInsert";
        let owners = if bus {
            &order.mixer_channels
        } else {
            &order.channels
        };
        let Some(index) = owners.iter().position(|id| id == &slot.owner) else {
            return;
        };
        let effects = if bus {
            p.snapshot
                .mixer_channels
                .iter()
                .find(|b| b.id == slot.owner)
                .map(|b| &b.inserts)
        } else {
            p.snapshot
                .channels
                .iter()
                .find(|c| c.id == slot.owner)
                .map(|c| &c.effect_chain)
        };
        let Some(position) = effects.and_then(|effects| {
            effects
                .iter()
                .position(|e| e.instance_id.is_some() && e.instance_id == slot.instance)
        }) else {
            return;
        };
        self.document_request(DocumentOperation::Project {
            edit: crate::project_edit::ProjectEdit::Effect {
                owner: if bus { "bus" } else { "channel" }.into(),
                index,
                slot: Some(position),
                config: None,
            },
        });
    }
}
pub fn bar(this: &Preview, cx: &Context<Preview>) -> Div {
    let Some(slot) = &this.document.manager.assignment else {
        return div();
    };
    let t = this.theme;
    let available = this.library_entries().into_iter().any(|e| {
        Some(&e.handle) == this.document.manager.selected.as_ref() && e.availability == "available"
    });
    div()
        .h(px(34.))
        .flex_shrink_0()
        .px_2()
        .flex()
        .items_center()
        .gap_2()
        .border_b_1()
        .border_color(rgb(t.border))
        .text_size(px(11.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .child(slot.label.clone()),
        )
        .child(
            t.ghost("plugin-picker-cancel", "Cancel")
                .on_click(cx.listener(|this, _, _, cx| {
                    if !this.document.manager.assignment_pending {
                        this.document.manager.assignment = None;
                    }
                    cx.notify();
                })),
        )
        .child(
            t.button("plugin-picker-use", slot.action())
                .opacity(if available && this.document_ready() {
                    1.
                } else {
                    0.4
                })
                .on_click(cx.listener(|this, _, _, cx| {
                    this.assign_selected_plugin();
                    cx.notify();
                })),
        )
}
