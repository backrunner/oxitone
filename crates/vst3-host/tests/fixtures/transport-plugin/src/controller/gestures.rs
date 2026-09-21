//! Programmatic controller fixture: these are real IComponentHandler callbacks, without a window.
use super::*;
impl GainController {
    pub(super) fn emit_gestures(&self, value: f64) {
        let handler = self.handler.borrow();
        let Some(handler) = handler.as_ref() else {
            return;
        };
        unsafe {
            match (value * 8.).round() as u8 {
                1 => {
                    handler.beginEdit(0);
                    self.gain.set(0.25);
                    handler.performEdit(0, 0.25);
                    self.gain.set(0.75);
                    handler.performEdit(0, 0.75);
                    handler.endEdit(0);
                }
                2 => {
                    handler.beginEdit(0);
                }
                3 => {
                    handler.endEdit(0);
                }
                4 => {
                    for _ in 0..4097 {
                        handler.performEdit(0, 0.5);
                    }
                }
                5 => {
                    handler.performEdit(999, 0.5);
                }
                6 => {
                    handler.performEdit(0, f64::NAN);
                }
                7 => {
                    handler.performEdit(99, 0.5);
                }
                8 => {
                    for index in 0..1024 {
                        handler.performEdit(0, f64::from(index) / 1024.);
                    }
                }
                _ => {}
            }
        }
    }
}
