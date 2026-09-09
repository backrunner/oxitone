//! Opt-in panel capture and real pointer/source regression on a disposable project.
use crate::{
    document_wire::DocumentOperation, plugin_details::DetailTarget, plugin_window::PluginWindow,
    ui::Preview,
};
use gpui::*;

#[derive(Default)]
pub struct Smoke {
    stage: u8,
    panel: Option<Entity<PluginWindow>>,
    at: Point<Pixels>,
    revision: u64,
    value: f64,
    settled: u8,
}
impl Smoke {
    pub fn complete(&self) -> bool {
        self.stage == 24
    }
    pub fn step(&mut self, mode: &str, view: &Entity<Preview>, window: &mut Window, cx: &mut App) {
        if self.complete() || !view.read(cx).document_ready() {
            return;
        }
        if self.stage == 1 && self.settled < 4 {
            self.settled += 1;
            return;
        }
        eprintln!("Builtin panel capture · {mode} · stage {}", self.stage);
        if self.stage == 0 {
            self.panel = view.update(cx, |s, cx| {
                let project = s.project.as_ref().unwrap();
                let channel = &project.snapshot.channels[0];
                let target = if mode == "instrument" {
                    DetailTarget::Instrument(channel.id.clone())
                } else {
                    DetailTarget::ChannelInsert(channel.id.clone(), 0)
                };
                let panel = s.open_plugin(target, cx).unwrap();
                if let Ok(page) = std::env::var("OXITONE_PREVIEW_CAPTURE_PAGE") {
                    panel.update(cx, |view, cx| {
                        assert!(view
                            .panel
                            .as_ref()
                            .unwrap()
                            .pages
                            .iter()
                            .any(|p| p.id == page));
                        view.page = page;
                        cx.notify();
                    });
                }
                let id = crate::window_manager::WindowId::Plugin(panel.entity_id().as_u64());
                let mut bounds = s.document.windows.state(id).bounds;
                if let Ok(size) = std::env::var("OXITONE_PREVIEW_CAPTURE_PLUGIN_SIZE") {
                    let (w, h) = size.split_once('x').unwrap();
                    bounds.width = w.parse().unwrap();
                    bounds.height = h.parse().unwrap();
                }
                bounds.x = 80.;
                bounds.y = 22.;
                s.document.windows.set_bounds(id, bounds);
                cx.notify();
                Some(panel)
            });
            self.stage = if mode == "edit" { 1 } else { 24 };
            return;
        }
        let panel = self.panel.as_ref().unwrap().clone();
        let get_value = |cx: &App| {
            view.read(cx).project.as_ref().unwrap().snapshot.channels[0].effect_chain[0].parameters
                ["cutoffHz"]
        };
        match self.stage {
            1 => {
                self.value = get_value(cx);
                self.revision = view.read(cx).document.view.as_ref().unwrap().revision;
                self.at = panel.read(cx).parameter_bounds.borrow()["plugin:cutoffHz"].center();
                eprintln!(
                    "Cutoff bounds {:?}; target {:?}",
                    panel.read(cx).parameter_bounds.borrow()["plugin:cutoffHz"],
                    view.read(cx)
                        .plugin_configuration_target(&panel.read(cx).target)
                );
                pointer(window, self.at, 0, cx);
            }
            2 => {
                assert!(
                    view.read(cx).document.plugin.gesture.is_some(),
                    "knob must be hit through GPUI"
                );
                self.at.y -= px(36.);
                pointer(window, self.at, 1, cx);
            }
            3 => {
                let s = view.read(cx);
                assert_eq!(s.document.view.as_ref().unwrap().revision, self.revision);
                let d = panel.read(cx).details.as_ref().unwrap();
                assert!(
                    d.parameters
                        .iter()
                        .find(|p| p.spec.id == "cutoffHz")
                        .unwrap()
                        .value
                        > self.value
                );
                pointer(window, self.at, 2, cx);
            }
            4 => {
                assert!(get_value(cx) > self.value);
                let s = view.read(cx);
                assert_eq!(
                    s.document.view.as_ref().unwrap().revision,
                    self.revision + 1
                );
                assert_eq!(
                    s.project.as_ref().unwrap().snapshot.channels[1].effect_chain[0].parameters
                        ["cutoffHz"],
                    self.value,
                    "shared preset edit is local to this instance"
                );
                window.dispatch_keystroke(Keystroke::parse("cmd-z").unwrap(), cx);
            }
            5 => {
                assert_eq!(get_value(cx), self.value);
                window.dispatch_keystroke(Keystroke::parse("cmd-shift-z").unwrap(), cx);
            }
            6 => {
                assert!(get_value(cx) > self.value);
                self.revision = view.read(cx).document.view.as_ref().unwrap().revision;
                self.at = panel.read(cx).parameter_bounds.borrow()["plugin:cutoffHz"].center();
                pointer(window, self.at, 0, cx);
            }
            7 => {
                self.at.y -= px(20.);
                pointer(window, self.at, 1, cx);
            }
            8 => {
                window.dispatch_keystroke(Keystroke::parse("escape").unwrap(), cx);
            }
            9 => {
                assert!(view.read(cx).document.plugin.gesture.is_none());
                pointer(window, self.at, 2, cx);
            }
            10 => {
                assert_eq!(
                    view.read(cx).document.view.as_ref().unwrap().revision,
                    self.revision
                );
                self.at = panel.read(cx).parameter_bounds.borrow()["host:mix"].center();
                pointer(window, self.at, 0, cx);
            }
            11 => {
                assert!(view.read(cx).document.plugin.gesture.is_some());
                self.at.x -= px(54.);
                pointer(window, self.at, 1, cx);
            }
            12 => {
                pointer(window, self.at, 2, cx);
            }
            13 => {
                let s = view.read(cx);
                let effect = &s.project.as_ref().unwrap().snapshot.channels[0].effect_chain[0];
                assert!((effect.mix.unwrap() - 0.7).abs() < 0.001);
                assert!(!effect.parameters.contains_key("mix"));
                self.at = panel.read(cx).parameter_bounds.borrow()["host:bypass"].center();
                pointer(window, self.at, 0, cx);
            }
            14 => {
                pointer(window, self.at, 2, cx);
            }
            15 => {
                assert_eq!(
                    view.read(cx).project.as_ref().unwrap().snapshot.channels[0].effect_chain[0]
                        .bypass,
                    Some(true)
                );
                view.update(cx, |s, cx| {
                    s.document_request(DocumentOperation::Save);
                    cx.notify();
                });
            }
            16 => {
                assert!(!view.read(cx).document.view.as_ref().unwrap().modified);
                assert!(!panel.read(cx).plots.is_empty());
                self.revision = view.read(cx).document.view.as_ref().unwrap().revision;
                self.at = panel.read(cx).parameter_bounds.borrow()["plugin:cutoffHz"].center();
                pointer(window, self.at, 0, cx);
            }
            17 => {
                assert!(view.read(cx).document.plugin.gesture.is_some());
                pointer(window, self.at, 2, cx);
            }
            18 => {
                let s = view.read(cx);
                assert_eq!(s.document.view.as_ref().unwrap().revision, self.revision);
                assert!(!s.document.view.as_ref().unwrap().modified);
                eprintln!("Builtin panel editing passed: hit-tested knob, projected graph, instance isolation, Undo/Redo, Escape, Mix, bypass, Save and no-op click");
                self.stage = 24;
                return;
            }
            _ => {}
        }
        self.stage += 1;
    }
}
fn pointer(window: &Window, at: Point<Pixels>, kind: u8, cx: &App) {
    crate::capture_pointer::dispatch(
        window,
        match kind {
            0 => PlatformInput::MouseDown(MouseDownEvent {
                position: at,
                ..Default::default()
            }),
            1 => PlatformInput::MouseMove(MouseMoveEvent {
                position: at,
                pressed_button: Some(MouseButton::Left),
                ..Default::default()
            }),
            _ => PlatformInput::MouseUp(MouseUpEvent {
                position: at,
                ..Default::default()
            }),
        },
        cx,
    );
}
