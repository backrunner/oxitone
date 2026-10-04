//! Native pointer regression: an XY graph edit is one undoable, instance-local transaction.
use crate::{capture_builtin::pointer, plugin_window::PluginWindow, ui::Preview};
use gpui::*;

#[derive(Default)]
pub struct Smoke {
    stage: u8,
    pointer: Point<Pixels>,
    before: [f64; 2],
    after: [f64; 2],
    other_before: [Option<f64>; 2],
    revision: u64,
    auxiliary: crate::capture_graph_aux::Smoke,
    envelope: crate::capture_envelope::Smoke,
}
impl Smoke {
    pub fn step(
        &mut self,
        view: &Entity<Preview>,
        panel: &Entity<PluginWindow>,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        let instrument = panel.read(cx).target.slot().is_none();
        let filter = panel
            .read(cx)
            .details
            .as_ref()
            .unwrap()
            .info
            .descriptor
            .plugin_id
            == "oxitone.filter";
        let compressor = panel
            .read(cx)
            .details
            .as_ref()
            .unwrap()
            .info
            .descriptor
            .plugin_id
            == "oxitone.compressor";
        let ids = if instrument {
            ["amp.decay", "amp.sustain"]
        } else if filter {
            ["cutoffHz", "resonance"]
        } else if compressor {
            ["thresholdDb", "makeupDb"]
        } else {
            ["band2.freqHz", "band2.gainDb"]
        };
        let key = format!("graph:{}:{}", ids[0], ids[1]);
        let accepted = |cx: &App| {
            let owner = view.read(cx);
            let details = crate::plugin_details::resolve(
                owner.project.as_ref().unwrap(),
                &panel.read(cx).target,
            )
            .unwrap();
            ids.map(|id| {
                details
                    .parameters
                    .iter()
                    .find(|p| !p.host && p.spec.id == id)
                    .unwrap()
                    .value
            })
        };
        let revision = |cx: &App| view.read(cx).document.view.as_ref().unwrap().revision;
        let other = |cx: &App| {
            let channel = &view.read(cx).project.as_ref().unwrap().snapshot.channels[1];
            let parameters = if instrument {
                &channel.instrument.parameters
            } else {
                &channel.effect_chain[0].parameters
            };
            ids.map(|id| parameters.get(id).copied())
        };
        match self.stage {
            0 => {
                self.before = accepted(cx);
                self.other_before = other(cx);
                self.revision = revision(cx);
                self.pointer = panel.read(cx).parameter_bounds.borrow()[&key].center();
                if !instrument && !filter {
                    self.pointer += point(px(0.), px(26.));
                }
                pointer(window, self.pointer, 0, cx);
            }
            1 => {
                assert!(
                    view.read(cx).document.plugin.gesture.is_some(),
                    "graph handle must receive native pointer"
                );
                self.pointer += point(px(36.), px(-12.));
                pointer(window, self.pointer, 1, cx);
            }
            2 => {
                if instrument {
                    let center = panel.read(cx).parameter_bounds.borrow()[&key].center();
                    assert!(
                        f32::from(center.x - self.pointer.x).abs() < 1.,
                        "Decay node must follow the pointer on a stable time axis"
                    );
                }
                let details = panel.read(cx).details.as_ref().unwrap();
                for (i, id) in ids.iter().enumerate() {
                    assert!(
                        details
                            .parameters
                            .iter()
                            .find(|p| p.spec.id == *id)
                            .unwrap()
                            .value
                            > self.before[i],
                        "both graph axes project immediately"
                    );
                }
                assert_eq!(
                    revision(cx),
                    self.revision,
                    "drag must not write source yet"
                );
                pointer(window, self.pointer, 2, cx);
            }
            3 => {
                self.after = accepted(cx);
                assert!(self
                    .after
                    .iter()
                    .zip(self.before)
                    .all(|(after, before)| *after > before));
                assert_eq!(revision(cx), self.revision + 1, "one XY edit, one revision");
                assert_eq!(
                    other(cx),
                    self.other_before,
                    "graph edits stay local to the selected instance"
                );
                window.dispatch_keystroke(Keystroke::parse("cmd-z").unwrap(), cx);
            }
            4 => {
                assert_eq!(
                    accepted(cx),
                    self.before,
                    "Undo restores both axes together"
                );
                window.dispatch_keystroke(Keystroke::parse("cmd-shift-z").unwrap(), cx);
            }
            5 => {
                assert_eq!(accepted(cx), self.after);
                self.revision = revision(cx);
                self.pointer = panel.read(cx).parameter_bounds.borrow()[&key].center();
                pointer(window, self.pointer, 0, cx);
            }
            6 => {
                self.pointer += point(px(-28.), px(18.));
                pointer(window, self.pointer, 1, cx);
            }
            7 => {
                window.dispatch_keystroke(Keystroke::parse("escape").unwrap(), cx);
            }
            8 => {
                pointer(window, self.pointer, 2, cx);
            }
            9 => {
                assert_eq!(accepted(cx), self.after);
                assert_eq!(
                    revision(cx),
                    self.revision,
                    "Escape must discard the entire XY gesture"
                );
                self.pointer = panel.read(cx).parameter_bounds.borrow()[&key].center();
                pointer(window, self.pointer, 0, cx);
            }
            10 => {
                pointer(window, self.pointer, 2, cx);
            }
            11 => {
                assert_eq!(
                    revision(cx),
                    self.revision,
                    "click without movement must be a no-op"
                );
                crate::capture_pointer::dispatch(
                    window,
                    PlatformInput::MouseDown(MouseDownEvent {
                        position: self.pointer,
                        click_count: 2,
                        ..Default::default()
                    }),
                    cx,
                );
            }
            12 => {
                let details = panel.read(cx).details.as_ref().unwrap();
                for id in ids {
                    let parameter = details.parameters.iter().find(|p| p.spec.id == id).unwrap();
                    assert_eq!(
                        parameter.value, parameter.spec.default,
                        "double click resets both axes"
                    );
                }
                pointer(window, self.pointer, 2, cx);
                window.dispatch_keystroke(Keystroke::parse("cmd-z").unwrap(), cx);
            }
            13 => {
                assert_eq!(accepted(cx), self.after);
                if !self.auxiliary.step(view, panel, window, cx) {
                    return false;
                }
                if !self.envelope.step(view, panel, window, cx) {
                    return false;
                }
                assert_eq!(
                    accepted(cx),
                    self.after,
                    "Alt gesture must preserve both primary axes"
                );
                view.update(cx, |s, cx| {
                    s.document_request(crate::document_wire::DocumentOperation::Save);
                    cx.notify();
                });
            }
            14 => {
                assert!(!view.read(cx).document.view.as_ref().unwrap().modified);
                eprintln!("Graph editing passed: surface/XY projection, Alt auxiliary isolation, atomic Undo/Redo, Escape, no-op, reset and Save");
                return true;
            }
            _ => unreachable!(),
        }
        self.stage += 1;
        false
    }
}
