//! One panel gesture is one instance-scoped source transaction. No direct DSP setter.
use crate::{
    configuration_wire::{ConfigurationEdit, HostEdit},
    document_wire::DocumentOperation,
    plugin_details::{DetailTarget, ParameterDetail},
    ui::Preview,
};
use gpui::{MouseDownEvent, MouseMoveEvent, Pixels, Point};
use oxitone_core::wire::{ParameterMapping, ParameterSpec, ParameterUnit};
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
    change: Change,
    spec: ParameterSpec,
    pointer: Point<Pixels>,
    fraction: f64,
    horizontal: bool,
    site: String,
    usage: Option<String>,
}
#[derive(Default)]
pub struct PluginEdit {
    pub gesture: Option<Gesture>,
    pub pending: Option<Change>,
    pub stamp: u64,
}
impl PluginEdit {
    pub fn change(&self) -> Option<&Change> {
        self.gesture
            .as_ref()
            .map(|g| &g.change)
            .or(self.pending.as_ref())
    }
    pub fn cancel(&mut self) {
        if self.gesture.take().is_some() {
            self.stamp += 1;
        }
    }
    pub fn clear_pending(&mut self) {
        if self.pending.take().is_some() {
            self.stamp += 1;
        }
    }
}
pub fn from_fraction(spec: &ParameterSpec, fraction: f64) -> f64 {
    let f = fraction.clamp(0., 1.);
    let value = if spec.mapping == Some(ParameterMapping::Log) && spec.min > 0. {
        spec.min * (spec.max / spec.min).powf(f)
    } else {
        spec.min + (spec.max - spec.min) * f
    };
    if spec.unit == ParameterUnit::Enum {
        value.round()
    } else {
        value
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
        self.document.plugin.gesture = Some(Gesture {
            change: Change {
                identity: crate::plugin_identity::key(project, target),
                parameter: parameter.spec.id.clone(),
                host: parameter.host,
                before: parameter.value,
                value: parameter.value,
            },
            spec: parameter.spec.clone(),
            pointer: event.position,
            fraction: parameter.fraction() as f64,
            horizontal,
            site,
            usage,
        });
        self.document.plugin.stamp += 1;
    }
    pub fn move_plugin_parameter(&mut self, event: &MouseMoveEvent) {
        let Some(gesture) = &mut self.document.plugin.gesture else {
            return;
        };
        let delta = event.position - gesture.pointer;
        let distance = if gesture.horizontal {
            f32::from(delta.x)
        } else {
            -f32::from(delta.y)
        };
        if distance == 0. {
            gesture.pointer = event.position;
            return;
        }
        gesture.fraction = (gesture.fraction
            + f64::from(distance) / 180. * if event.modifiers.shift { 0.1 } else { 1. })
        .clamp(0., 1.);
        gesture.pointer = event.position;
        gesture.change.value = from_fraction(&gesture.spec, gesture.fraction);
        self.document.plugin.stamp += 1;
    }
    pub fn finish_plugin_parameter(&mut self) {
        let Some(gesture) = self.document.plugin.gesture.take() else {
            return;
        };
        self.submit_plugin_change(gesture.change, gesture.site, gesture.usage);
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
        self.submit_plugin_change(change, site, usage);
    }
    fn submit_plugin_change(&mut self, change: Change, site: String, usage: Option<String>) {
        self.document.plugin.stamp += 1;
        if change.value == change.before || !change.value.is_finite() || !self.document_ready() {
            return;
        }
        let edit = if change.host {
            ConfigurationEdit::Host {
                values: HostEdit {
                    mix: (change.parameter == "mix").then_some(change.value),
                    bypass: (change.parameter == "bypass").then_some(change.value >= 0.5),
                },
            }
        } else {
            ConfigurationEdit::Parameters {
                values: BTreeMap::from([(change.parameter.clone(), change.value)]),
            }
        };
        self.document_request(DocumentOperation::Configuration { site, usage, edit });
        if self.document.pending.is_some() {
            self.document.plugin.pending = Some(change);
        }
    }
}
