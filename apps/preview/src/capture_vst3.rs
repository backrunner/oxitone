//! Opt-in local fixture: real field/button hit testing, keyboard input and helper WAV render.
use crate::{
    document_wire::DocumentOperation, plugin_manager::LibrarySection, ui::Preview,
    window_manager::WindowId,
};
use gpui::*;

#[derive(Default)]
pub struct Smoke {
    stage: usize,
    up: Option<Point<Pixels>>,
    revision: u64,
    code: Vec<String>,
    handle: String,
    files: crate::capture_vst3_files::Smoke,
}
impl Smoke {
    pub fn complete(&self) -> bool {
        std::env::var_os("OXITONE_PREVIEW_CAPTURE_VST3").is_none()
            || (self.stage == 16 && self.files.complete())
    }
    pub fn step(&mut self, view: &Entity<Preview>, window: &mut Window, cx: &mut App) {
        if self.complete() {
            return;
        }
        if self.stage == 16 {
            self.files.step(view, window, cx);
            return;
        }
        if let Some(at) = self.up.take() {
            pointer(window, at, true, cx);
            return;
        }
        if !view.read(cx).document_ready() {
            return;
        }
        match self.stage {
            0 => view.update(cx, |s, cx| {
                let doc = s.document.view.as_ref().unwrap();
                self.revision = doc.revision;
                self.code = doc.files.iter().map(|f| f.text.clone()).collect();
                s.document.manager.open = true;
                s.document.manager.vst3.adding = true;
                s.document.manager.assignment = None;
                s.document.manager.category = 0;
                s.document.manager.query.clear();
                s.document.configuration.open = false;
                s.document.show_code = false;
                s.document.windows.focus(WindowId::Plugins);
                let mut bounds = s.document.windows.state(WindowId::Plugins).bounds;
                bounds.width = 630.;
                bounds.height = 520.;
                s.document.windows.set_bounds(WindowId::Plugins, bounds);
                s.workspace_focus.focus(window);
                cx.notify();
            }),
            1 => self.click("field-Bundle", view, window, cx),
            2 => paste(window, &std::env::var("OXITONE_VST3_FIXTURE").unwrap(), cx),
            3 => self.click("field-Class", view, window, cx),
            4 => paste(window, "56455354494741494e30303030303031", cx),
            5 => self.click("add", view, window, cx),
            6 => view.update(cx, |s, cx| {
                self.handle = s
                    .document
                    .view
                    .as_ref()
                    .unwrap()
                    .plugins
                    .iter()
                    .find(|entry| entry.source == "vst3")
                    .expect("local VST3 entry")
                    .handle
                    .clone();
                s.select_library_plugin(self.handle.clone());
                s.toggle_library_section(LibrarySection::Vst3);
                cx.notify();
            }),
            7 => self.click("inspect", view, window, cx),
            8 => {
                let s = view.read(cx);
                let entry = s
                    .document
                    .view
                    .as_ref()
                    .unwrap()
                    .plugins
                    .iter()
                    .find(|p| p.handle == self.handle)
                    .unwrap();
                assert_eq!(entry.validation, "verified", "{:?}", entry.diagnostic);
                assert_eq!(entry.vst3.as_ref().unwrap().parameters.len(), 2);
                self.click("field-Input", view, window, cx);
            }
            9 => {
                paste(window, &std::env::var("OXITONE_VST3_INPUT").unwrap(), cx);
                self.click("field-Output", view, window, cx);
            }
            10 => {
                paste(window, &std::env::var("OXITONE_VST3_OUTPUT").unwrap(), cx);
                self.click("field-Parameter(1)", view, window, cx);
            }
            11 => {
                paste(window, "1", cx);
                self.click("render", view, window, cx);
            }
            12 => {
                let s = view.read(cx);
                let doc = s.document.view.as_ref().unwrap();
                let catalog = doc
                    .plugins
                    .iter()
                    .find(|p| p.handle == self.handle)
                    .unwrap()
                    .vst3
                    .as_ref()
                    .unwrap();
                assert_eq!(
                    catalog.render.as_ref().expect("render report").path,
                    std::env::var("OXITONE_VST3_OUTPUT").unwrap()
                );
                assert_eq!(doc.revision, self.revision);
                assert_eq!(
                    doc.files.iter().map(|f| f.text.clone()).collect::<Vec<_>>(),
                    self.code
                );
                assert!(!s.is_playing());
                assert!(s.plugin_windows.is_empty());
            }
            13 => view.update(cx, |s, _| {
                // Refresh must retain the local selection but expire the inspected state.
                s.document_request(DocumentOperation::RefreshPlugins);
            }),
            14 => {
                let entry = view
                    .read(cx)
                    .document
                    .view
                    .as_ref()
                    .unwrap()
                    .plugins
                    .iter()
                    .find(|p| p.handle == self.handle)
                    .unwrap();
                assert_eq!(entry.validation, "unverified");
                assert!(entry.vst3.as_ref().unwrap().render.is_none());
                assert!(view
                    .read(cx)
                    .document
                    .manager
                    .vst3
                    .drafts
                    .get(&self.handle)
                    .unwrap()
                    .parameters
                    .is_empty());
                // Retain a populated panel for the capture after proving refresh invalidation.
                view.update(cx, |s, _| {
                    s.document_request(DocumentOperation::VerifyPlugin {
                        plugin: self.handle.clone(),
                    })
                });
            }
            15 => {
                let entry = view
                    .read(cx)
                    .document
                    .view
                    .as_ref()
                    .unwrap()
                    .plugins
                    .iter()
                    .find(|p| p.handle == self.handle)
                    .unwrap();
                assert_eq!(entry.validation, "verified");
                eprintln!("VST3 GUI smoke passed: real field/button pointer input, keyboard paste, exact-class Inspect, normalized bypass parameter, offline WAV, unchanged source/revision, refresh expires configuration; simulated audio only");
            }
            _ => {}
        }
        self.stage += 1;
    }
    fn click(&mut self, key: &str, view: &Entity<Preview>, window: &Window, cx: &App) {
        let bounds = view.read(cx).document.manager.vst3.bounds.borrow()[key];
        assert!(
            bounds.size.width > px(10.) && bounds.size.height > px(10.),
            "{key}"
        );
        let at = bounds.center();
        pointer(window, at, false, cx);
        self.up = Some(at);
    }
}
pub(super) fn paste(window: &mut Window, text: &str, cx: &mut App) {
    cx.write_to_clipboard(ClipboardItem::new_string(text.to_owned()));
    for key in ["cmd-a", "cmd-v", "enter"] {
        window.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx);
    }
}
pub(super) fn pointer(window: &Window, at: Point<Pixels>, up: bool, cx: &App) {
    let event = if up {
        PlatformInput::MouseUp(MouseUpEvent {
            position: at,
            ..Default::default()
        })
    } else {
        PlatformInput::MouseDown(MouseDownEvent {
            position: at,
            ..Default::default()
        })
    };
    crate::capture_pointer::dispatch(window, event, cx);
}
