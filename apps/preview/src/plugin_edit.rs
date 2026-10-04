//! One panel gesture is one instance-scoped source transaction. No direct DSP setter.
use crate::{
    configuration_wire::{ConfigurationEdit, HostEdit},
    document_wire::DocumentOperation,
    plugin_details::{DetailTarget, ParameterDetail},
    plugin_parameter_drag::{Binding, Range},
    ui::Preview,
};
use gpui::{MouseDownEvent, MouseMoveEvent, Pixels, Point};
use std::collections::BTreeMap;

#[derive(Clone)]
pub struct Change {
    pub identity: String,
    pub parameter: String,
    pub host: bool,
    pub before: f64,
    pub value: f64,
}
pub struct Gesture {
    changes: Vec<Change>,
    bindings: Vec<Binding>,
    pointer: Point<Pixels>,
    site: String,
    usage: Option<String>,
}
#[derive(Default)]
pub struct PluginEdit {
    pub gesture: Option<Gesture>,
    pub pending: Vec<Change>,
    pub stamp: u64,
}
impl PluginEdit {
    pub fn changes(&self) -> &[Change] {
        self.gesture.as_ref().map_or(&self.pending, |g| &g.changes)
    }
    pub fn cancel(&mut self) {
        if self.gesture.take().is_some() {
            self.stamp += 1;
        }
    }
    pub fn clear_pending(&mut self) {
        if !self.pending.is_empty() {
            self.pending.clear();
            self.stamp += 1;
        }
    }
}
impl Preview {
    pub fn begin_plugin_parameter(
        &mut self,
        target: &DetailTarget,
        identity: &str,
        parameter: &ParameterDetail,
        event: &MouseDownEvent,
        horizontal: bool,
    ) {
        self.begin_plugin_gesture(
            target,
            identity,
            vec![Binding::new(
                parameter.clone(),
                Range::parameter(&parameter.spec),
                horizontal,
                180.,
            )],
            event,
        );
    }
    pub fn begin_plugin_gesture(
        &mut self,
        target: &DetailTarget,
        identity: &str,
        bindings: Vec<Binding>,
        event: &MouseDownEvent,
    ) -> bool {
        if !self.document_ready() || self.close.open || self.show_shortcuts {
            return false;
        }
        let Some((site, usage)) = self.plugin_configuration_target(target) else {
            return false;
        };
        let Some(project) = &self.project else {
            return false;
        };
        if crate::plugin_identity::key(project, target) != identity {
            return false;
        }
        self.document.plugin.gesture = Some(Gesture {
            changes: bindings
                .iter()
                .map(|binding| Change {
                    identity: crate::plugin_identity::key(project, target),
                    parameter: binding.parameter.spec.id.clone(),
                    host: binding.parameter.host,
                    before: binding.parameter.value,
                    value: binding.parameter.value,
                })
                .collect(),
            bindings,
            pointer: event.position,
            site,
            usage,
        });
        self.document.plugin.stamp += 1;
        true
    }
    pub fn move_plugin_parameter(&mut self, event: &MouseMoveEvent) {
        let Some(gesture) = &mut self.document.plugin.gesture else {
            return;
        };
        let delta = event.position - gesture.pointer;
        for (change, binding) in gesture.changes.iter_mut().zip(&mut gesture.bindings) {
            change.value = binding.advance(
                f32::from(delta.x) as f64,
                f32::from(delta.y) as f64,
                event.modifiers.shift,
            );
        }
        gesture.pointer = event.position;
        self.document.plugin.stamp += 1;
    }
    pub fn finish_plugin_parameter(&mut self) {
        let Some(gesture) = self.document.plugin.gesture.take() else {
            return;
        };
        self.submit_plugin_changes(gesture.changes, gesture.site, gesture.usage);
    }
    pub fn set_plugin_parameter(
        &mut self,
        target: &DetailTarget,
        identity: &str,
        parameter: &ParameterDetail,
        value: f64,
    ) {
        if !self.document_ready() || self.close.open || self.show_shortcuts {
            return;
        }
        let Some((site, usage)) = self.plugin_configuration_target(target) else {
            return;
        };
        let Some(project) = &self.project else {
            return;
        };
        if crate::plugin_identity::key(project, target) != identity {
            return;
        }
        let change = Change {
            identity: crate::plugin_identity::key(project, target),
            parameter: parameter.spec.id.clone(),
            host: parameter.host,
            before: parameter.value,
            value: value.clamp(parameter.spec.min, parameter.spec.max),
        };
        self.submit_plugin_changes(vec![change], site, usage);
    }
    pub fn reset_plugin_graph(
        &mut self,
        target: &DetailTarget,
        identity: &str,
        bindings: Vec<Binding>,
        event: &MouseDownEvent,
    ) {
        if !self.begin_plugin_gesture(target, identity, bindings, event) {
            return;
        }
        if let Some(gesture) = &mut self.document.plugin.gesture {
            for (change, binding) in gesture.changes.iter_mut().zip(&gesture.bindings) {
                change.value = binding.parameter.spec.default;
            }
            self.finish_plugin_parameter();
        }
    }
    fn submit_plugin_changes(
        &mut self,
        mut changes: Vec<Change>,
        site: String,
        usage: Option<String>,
    ) {
        self.document.plugin.stamp += 1;
        if !self.document_ready() || changes.iter().any(|c| !c.value.is_finite()) {
            return;
        }
        changes.retain(|c| c.value != c.before);
        let Some(change) = changes.first() else {
            return;
        };
        let edit = if change.host {
            ConfigurationEdit::Host {
                values: HostEdit {
                    mix: (change.parameter == "mix").then_some(change.value),
                    bypass: (change.parameter == "bypass").then_some(change.value >= 0.5),
                },
            }
        } else {
            ConfigurationEdit::Parameters {
                values: changes
                    .iter()
                    .map(|c| (c.parameter.clone(), c.value))
                    .collect::<BTreeMap<_, _>>(),
            }
        };
        self.document_request(DocumentOperation::Configuration { site, usage, edit });
        if self.document.pending.is_some() {
            self.document.plugin.pending = changes;
        }
    }
}
