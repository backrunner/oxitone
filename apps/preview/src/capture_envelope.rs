//! Release must move with the native pointer, not stay pinned to an auto-scaled chart edge.
use crate::{capture_builtin::pointer, plugin_window::PluginWindow, ui::Preview};
use gpui::*;

#[derive(Default)]
pub struct Smoke {
    stage: u8,
    before: [f64; 4],
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
        if panel.read(cx).target.slot().is_some() {
            return true;
        }
        let key = "graph:amp.release:";
        let values = |cx: &App| {
            let details = crate::plugin_details::resolve(
                view.read(cx).project.as_ref().unwrap(),
                &panel.read(cx).target,
            )
            .unwrap();
            ["amp.attack", "amp.decay", "amp.sustain", "amp.release"].map(|id| {
                details
                    .parameters
                    .iter()
                    .find(|p| !p.host && p.spec.id == id)
                    .unwrap()
                    .value
            })
        };
        let revision = |cx: &App| view.read(cx).document.view.as_ref().unwrap().revision;
        match self.stage {
            0 => {
                self.before = values(cx);
                self.revision = revision(cx);
                self.pointer = panel.read(cx).parameter_bounds.borrow()[key].center();
                pointer(window, self.pointer, 0, cx);
            }
            1 => {
                assert_eq!(
                    view.read(cx).document.plugin.changes()[0].parameter,
                    "amp.release"
                );
                self.pointer.x += px(16.);
                pointer(window, self.pointer, 1, cx);
            }
            2 => {
                let center = panel.read(cx).parameter_bounds.borrow()[key].center();
                assert!(
                    f32::from(center.x - self.pointer.x).abs() < 1.,
                    "Release node must follow horizontal pointer movement"
                );
                assert_eq!(revision(cx), self.revision);
                pointer(window, self.pointer, 2, cx);
            }
            3 => {
                let accepted = values(cx);
                assert_eq!(&accepted[..3], &self.before[..3]);
                self.after = accepted[3];
                assert!(self.after > self.before[3]);
                assert_eq!(revision(cx), self.revision + 1);
                window.dispatch_keystroke(Keystroke::parse("cmd-z").unwrap(), cx);
            }
            4 => {
                assert_eq!(values(cx), self.before);
                window.dispatch_keystroke(Keystroke::parse("cmd-shift-z").unwrap(), cx);
            }
            5 => {
                assert_eq!(values(cx)[3], self.after);
                return true;
            }
            _ => unreachable!(),
        }
        self.stage += 1;
        false
    }
}
