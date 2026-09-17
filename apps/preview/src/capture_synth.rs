//! Real hit-testing of the synth picker and waveform through the document service.
use crate::{capture_builtin::pointer, plugin_window::PluginWindow, ui::Preview};
use gpui::*;

#[derive(Default)]
pub struct Smoke {
    stage: u8,
    at: Point<Pixels>,
    revision: u64,
}
impl Smoke {
    pub fn step(
        &mut self,
        view: &Entity<Preview>,
        panel: &Entity<PluginWindow>,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        let hit = |key: &str, cx: &App| panel.read(cx).parameter_bounds.borrow()[key].center();
        let click = |at, cx: &App| {
            pointer(window, at, 0, cx);
            pointer(window, at, 2, cx);
        };
        let value = |id: &str, cx: &App| {
            crate::plugin_synth_visibility::value(panel.read(cx).details.as_ref().unwrap(), id)
        };
        match self.stage {
            0 => click(hit("plugin:oscA.wavetable", cx), cx),
            1 => {
                assert_eq!(
                    panel.read(cx).choice_open.as_deref(),
                    Some("oscA.wavetable")
                );
                click(hit("choice:oscA.wavetable:4", cx), cx);
            }
            2 => {
                assert_eq!(value("oscA.wavetable", cx), 4.);
                assert!(panel.read(cx).choice_open.is_none());
                click(hit("plugin:oscA.bank", cx), cx);
            }
            3 => click(hit("choice:oscA.bank:2", cx), cx),
            4 => {
                assert_eq!(value("oscA.bank", cx), 2.);
                let p = panel.read(cx);
                for key in [
                    "plugin:oscA.wavetable",
                    "plugin:oscA.morphTo",
                    "plugin:oscA.warp",
                    "plugin:oscA.detune",
                ] {
                    assert!(
                        !p.parameter_bounds.borrow().contains_key(key),
                        "inactive control {key}"
                    );
                }
                self.revision = view.read(cx).document.view.as_ref().unwrap().revision;
                self.at = hit("wave:oscA.position", cx);
                pointer(window, self.at, 0, cx);
            }
            5 => {
                assert!(view.read(cx).document.plugin.gesture.is_some());
                self.at.x += px(36.);
                pointer(window, self.at, 1, cx);
            }
            6 => {
                assert!((value("oscA.position", cx) - 0.2).abs() < 0.001);
                assert_eq!(
                    view.read(cx).document.view.as_ref().unwrap().revision,
                    self.revision
                );
                pointer(window, self.at, 2, cx);
            }
            7 => {
                assert_eq!(
                    view.read(cx).document.view.as_ref().unwrap().revision,
                    self.revision + 1
                );
                window.dispatch_keystroke(Keystroke::parse("cmd-z").unwrap(), cx);
            }
            8 => {
                assert_eq!(value("oscA.position", cx), 0.);
                window.dispatch_keystroke(Keystroke::parse("cmd-shift-z").unwrap(), cx);
            }
            9 => {
                assert!((value("oscA.position", cx) - 0.2).abs() < 0.001);
                click(hit("plugin:oscA.bank", cx), cx);
            }
            10 => {
                assert!(panel.read(cx).choice_open.is_some());
                self.at = hit("plugin:oscA.level", cx);
                pointer(window, self.at, 0, cx);
            }
            11 => {
                assert!(panel.read(cx).choice_open.is_none());
                self.at.y += px(24.);
                pointer(window, self.at, 1, cx);
            }
            12 => {
                window.dispatch_keystroke(Keystroke::parse("escape").unwrap(), cx);
            }
            13 => {
                assert!(view.read(cx).document.plugin.gesture.is_none());
                pointer(window, self.at, 2, cx);
                click(hit("plugin:oscA.bank", cx), cx);
            }
            14 => {
                assert!(panel.read(cx).choice_open.is_some());
                window.dispatch_keystroke(Keystroke::parse("escape").unwrap(), cx);
            }
            15 => {
                assert!(panel.read(cx).choice_open.is_none());
                assert_eq!(value("oscA.level", cx), 1.);
                assert!((value("oscA.position", cx) - 0.2).abs() < 0.001);
                let other = &view.read(cx).project.as_ref().unwrap().snapshot.channels[1]
                    .instrument
                    .parameters;
                assert_eq!(other.get("oscA.position").copied().unwrap_or(0.), 0.);
                assert_eq!(other.get("oscA.bank").copied().unwrap_or(0.), 0.);
                view.update(cx, |s, cx| {
                    s.document_request(crate::document_wire::DocumentOperation::Save);
                    cx.notify();
                });
            }
            16 => {
                assert!(!view.read(cx).document.view.as_ref().unwrap().modified);
                eprintln!("Synth editing passed: waveform picker, conditional controls, waveform drag, projection, Undo/Redo, Escape, instance isolation and Save");
                return true;
            }
            _ => unreachable!(),
        }
        self.stage += 1;
        false
    }
}
