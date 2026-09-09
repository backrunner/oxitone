//! One source transaction per fader/pan gesture, with local preview and Escape cancellation.
use crate::{
    document_wire::DocumentOperation,
    project_edit::{MixValues, ProjectEdit},
    ui::Preview,
};
use gpui::*;

#[derive(Clone, Copy, PartialEq)]
pub enum Control {
    Level,
    Pan,
}
#[derive(Clone)]
pub struct Gesture {
    pub owner: String,
    pub instrument: bool,
    pub control: Control,
    pub original: f64,
    pub value: f64,
    pub last: Point<Pixels>,
}
#[derive(Default)]
pub struct MixerUi {
    pub gesture: Option<Gesture>,
    pub pending: Option<Gesture>,
    pub bounds: std::rc::Rc<std::cell::RefCell<std::collections::HashMap<String, Bounds<Pixels>>>>,
}

fn adjust(control: Control, value: f64, delta: f64) -> f64 {
    match control {
        Control::Pan => (value + delta / 100.).clamp(-1., 1.),
        Control::Level => {
            let db = (20. * value.max(0.001).log10() + delta * 66. / 130.)
                .clamp(-60., 20. * 2_f64.log10());
            if db <= -60. {
                0.
            } else {
                10_f64.powf(db / 20.)
            }
        }
    }
}
impl Preview {
    pub fn mix_value(&self, owner: &str, control: Control, fallback: f64) -> f64 {
        self.document
            .mixer
            .gesture
            .as_ref()
            .or(self.document.mixer.pending.as_ref())
            .filter(|g| g.owner == owner && g.control == control)
            .map_or(fallback, |g| g.value)
    }
    pub fn begin_mix(
        &mut self,
        owner: String,
        instrument: bool,
        control: Control,
        original: f64,
        event: &MouseDownEvent,
    ) {
        if !self.document_ready() {
            return;
        }
        self.select_mixer(&owner);
        if event.click_count == 2 {
            self.commit_mix(
                &owner,
                instrument,
                match control {
                    Control::Level => MixValues {
                        level: Some(1.),
                        ..Default::default()
                    },
                    Control::Pan => MixValues {
                        pan: Some(0.),
                        ..Default::default()
                    },
                },
            );
            return;
        }
        self.document.mixer.gesture = Some(Gesture {
            owner,
            instrument,
            control,
            original,
            value: original,
            last: event.position,
        });
    }
    pub fn move_mix(&mut self, event: &MouseMoveEvent) {
        let Some(g) = self.document.mixer.gesture.as_mut() else {
            return;
        };
        let delta = match g.control {
            Control::Level => f64::from(f32::from(g.last.y - event.position.y)),
            Control::Pan => f64::from(f32::from(
                event.position.x - g.last.x + g.last.y - event.position.y,
            )),
        };
        g.value = adjust(
            g.control,
            g.value,
            delta * if event.modifiers.shift { 0.1 } else { 1. },
        );
        g.last = event.position;
    }
    pub fn finish_mix(&mut self) {
        let Some(g) = self.document.mixer.gesture.take() else {
            return;
        };
        if (g.value - g.original).abs() < 1e-9 {
            return;
        }
        let values = match g.control {
            Control::Level => MixValues {
                level: Some(g.value),
                ..Default::default()
            },
            Control::Pan => MixValues {
                pan: Some(g.value),
                ..Default::default()
            },
        };
        self.commit_mix(&g.owner, g.instrument, values);
        if self.document.pending.is_some() {
            self.document.mixer.pending = Some(g);
        }
    }
    pub fn commit_mix(&mut self, owner: &str, instrument: bool, values: MixValues) {
        let Some(order) = self
            .document
            .view
            .as_ref()
            .and_then(|v| v.arrangement_order.as_ref())
        else {
            return;
        };
        let owners = if instrument {
            &order.channels
        } else {
            &order.mixer_channels
        };
        let Some(index) = owners.iter().position(|id| id == owner) else {
            return;
        };
        let edit = if instrument {
            ProjectEdit::Channel { index, values }
        } else {
            ProjectEdit::Bus { index, values }
        };
        self.document_request(DocumentOperation::Project { edit });
    }
}
#[cfg(test)]
mod tests {
    use super::{adjust, Control};
    #[test]
    fn fader_has_silence_unity_and_six_db_ceiling() {
        assert_eq!(adjust(Control::Level, 1., -130.), 0.);
        assert!((adjust(Control::Level, 0., 60. / 66. * 130.) - 1.).abs() < 1e-9);
        assert!((adjust(Control::Level, 1., 130.) - 2.).abs() < 1e-9);
        assert_eq!(adjust(Control::Pan, 0., 250.), 1.);
    }
}
