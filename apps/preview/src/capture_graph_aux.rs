//! Real pointer coverage for Option/Alt graph edits without changing the XY parameters.
use crate::{plugin_window::PluginWindow, ui::Preview};
use gpui::*;

#[derive(Default)]
pub struct Smoke {
    stage: u8,
    before: f64,
    after: f64,
    revision: u64,
    pointer: Point<Pixels>,
}
impl Smoke {
    pub fn step(
        &mut self,
        view: &Entity<Preview>,
        panel: &Entity<PluginWindow>,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        let eq = panel
            .read(cx)
            .details
            .as_ref()
            .unwrap()
            .info
            .descriptor
            .plugin_id
            == "oxitone.eq";
        let compressor = panel
            .read(cx)
            .details
            .as_ref()
            .unwrap()
            .info
            .descriptor
            .plugin_id
            == "oxitone.compressor";
        if !eq && !compressor {
            return true;
        }
        let (id, key) = if eq {
            ("band2.q", "graph:band2.freqHz:band2.gainDb")
        } else {
            ("kneeDb", "graph:thresholdDb:makeupDb")
        };
        let value = |cx: &App| {
            crate::plugin_details::resolve(
                view.read(cx).project.as_ref().unwrap(),
                &panel.read(cx).target,
            )
            .unwrap()
            .parameters
            .iter()
            .find(|p| !p.host && p.spec.id == id)
            .unwrap()
            .value
        };
        let modifiers = Modifiers {
            alt: true,
            ..Default::default()
        };
        match self.stage {
            0 => {
                self.before = value(cx);
                self.revision = view.read(cx).document.view.as_ref().unwrap().revision;
                self.pointer = panel.read(cx).parameter_bounds.borrow()[key].center();
                crate::capture_pointer::dispatch(
                    window,
                    PlatformInput::MouseDown(MouseDownEvent {
                        position: self.pointer,
                        modifiers,
                        ..Default::default()
                    }),
                    cx,
                );
            }
            1 => {
                assert_eq!(view.read(cx).document.plugin.changes().len(), 1);
                assert_eq!(view.read(cx).document.plugin.changes()[0].parameter, id);
                self.pointer += point(px(24.), px(-18.));
                crate::capture_pointer::dispatch(
                    window,
                    PlatformInput::MouseMove(MouseMoveEvent {
                        position: self.pointer,
                        modifiers,
                        pressed_button: Some(MouseButton::Left),
                        ..Default::default()
                    }),
                    cx,
                );
            }
            2 => {
                assert_eq!(
                    value(cx),
                    self.before,
                    "Alt drag is not accepted until release"
                );
                crate::capture_pointer::dispatch(
                    window,
                    PlatformInput::MouseUp(MouseUpEvent {
                        position: self.pointer,
                        modifiers,
                        ..Default::default()
                    }),
                    cx,
                );
            }
            3 => {
                self.after = value(cx);
                assert!(self.after > self.before);
                assert_eq!(
                    view.read(cx).document.view.as_ref().unwrap().revision,
                    self.revision + 1
                );
                let other =
                    &view.read(cx).project.as_ref().unwrap().snapshot.channels[1].effect_chain[0];
                assert!(
                    other.parameters.get(id).is_none(),
                    "auxiliary edit must stay instance-local"
                );
                window.dispatch_keystroke(Keystroke::parse("cmd-z").unwrap(), cx);
            }
            4 => {
                assert_eq!(value(cx), self.before);
                window.dispatch_keystroke(Keystroke::parse("cmd-shift-z").unwrap(), cx);
            }
            5 => {
                assert_eq!(value(cx), self.after);
                return true;
            }
            _ => unreachable!(),
        }
        self.stage += 1;
        false
    }
}
