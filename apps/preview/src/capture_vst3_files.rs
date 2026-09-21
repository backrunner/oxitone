//! Opt-in preset and frozen Playlist integration, with native pointer/keyboard input.
use crate::{
    capture_vst3::{paste, pointer},
    plugin_manager::LibrarySection,
    ui::Preview,
    window_manager::WindowId,
};
use gpui::*;
#[derive(Default)]
pub struct Smoke {
    stage: usize,
    up: Option<Point<Pixels>>,
    revision: u64,
    samples: usize,
    handle: String,
}
impl Smoke {
    pub fn complete(&self) -> bool {
        std::env::var_os("OXITONE_VST3_PRESET").is_none() || self.stage == 25
    }
    pub fn step(&mut self, view: &Entity<Preview>, window: &mut Window, cx: &mut App) {
        if self.complete() {
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
            0 => {
                view.update(cx, |s, cx| {
                    self.revision = s.document.view.as_ref().unwrap().revision;
                    self.samples = s.project.as_ref().unwrap().snapshot.samples.len();
                    let mut bounds = s.document.windows.state(WindowId::Plugins).bounds;
                    bounds.width = 680.;
                    bounds.height = 650.;
                    s.document.windows.set_bounds(WindowId::Plugins, bounds);
                    cx.notify();
                });
            }
            1 => self.click("field-Parameter(1)", view, window, cx),
            2 => {
                paste(window, "1", cx);
                self.click("field-Output", view, window, cx);
            }
            3 => {
                paste(window, &env("OXITONE_VST3_OUTPUT_2"), cx);
                self.click("render", view, window, cx);
            }
            4 => self.click("files", view, window, cx),
            5 => self.click("field-Preset", view, window, cx),
            6 => {
                paste(window, &env("OXITONE_VST3_PRESET"), cx);
                self.click("save-preset", view, window, cx);
            }
            7 => self.click("load-preset", view, window, cx),
            8 => view.update(cx, |s, cx| {
                let entry = s
                    .document
                    .view
                    .as_ref()
                    .unwrap()
                    .plugins
                    .iter()
                    .find(|p| {
                        p.vst3.as_ref().is_some_and(|v| {
                            v.preset_path.as_deref() == Some(env("OXITONE_VST3_PRESET").as_str())
                        }) && p.validation == "unverified"
                    })
                    .expect("loaded preset");
                self.handle = entry.handle.clone();
                s.select_library_plugin(self.handle.clone());
                s.toggle_library_section(LibrarySection::Vst3);
                cx.notify();
            }),
            9 => self.click("inspect", view, window, cx),
            10 => {
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
                assert_eq!(
                    entry
                        .vst3
                        .as_ref()
                        .unwrap()
                        .parameters
                        .iter()
                        .find(|p| p.id == 1)
                        .unwrap()
                        .value,
                    1.
                );
                self.click("files", view, window, cx);
            }
            11 => self.click("field-Input", view, window, cx),
            12 => {
                paste(window, &env("OXITONE_VST3_INPUT"), cx);
                self.click("field-Output", view, window, cx);
            }
            13 => {
                paste(window, &env("OXITONE_VST3_OUTPUT_3"), cx);
                self.click("render", view, window, cx);
            }
            14 => self.click("files", view, window, cx),
            15 => self.click("field-TrackName", view, window, cx),
            16 => {
                paste(window, "VST3 print", cx);
                self.click("field-StartBeat", view, window, cx);
            }
            17 => {
                paste(window, "4", cx);
                self.click("attach-render", view, window, cx);
            }
            18 => {
                self.assert_samples(view, cx, self.samples + 1, self.revision + 1);
                key(window, "cmd-z", cx);
            }
            19 => {
                self.assert_samples(view, cx, self.samples, self.revision + 2);
                key(window, "cmd-shift-z", cx);
            }
            20 => {
                self.assert_samples(view, cx, self.samples + 1, self.revision + 3);
                key(window, "cmd-s", cx);
            }
            21 => {
                if view.read(cx).document.view.as_ref().unwrap().modified {
                    return;
                }
                let s = view.read(cx);
                assert!(s
                    .document
                    .view
                    .as_ref()
                    .unwrap()
                    .files
                    .iter()
                    .any(|f| f.text.contains(".importAudio(")));
                assert!(!s.is_playing());
            }
            22..=24 => {} // Let layout and accepted-revision presentation settle before capture.
            _ => unreachable!(),
        }
        self.stage += 1;
        if self.complete() {
            eprintln!("VST3 files smoke passed: preset save/load/Inspect/render, frozen Playlist import, Undo/Redo/Save; simulated audio only");
        }
    }
    fn assert_samples(&self, view: &Entity<Preview>, cx: &App, count: usize, revision: u64) {
        assert_eq!(
            view.read(cx)
                .project
                .as_ref()
                .unwrap()
                .snapshot
                .samples
                .len(),
            count
        );
        assert_eq!(
            view.read(cx).document.view.as_ref().unwrap().revision,
            revision
        );
    }
    fn click(&mut self, key: &str, view: &Entity<Preview>, window: &Window, cx: &App) {
        let area = view.read(cx).document.manager.vst3.bounds.borrow()[key];
        assert!(
            area.size.width > px(10.) && area.size.height > px(10.),
            "{key}"
        );
        pointer(window, area.center(), false, cx);
        self.up = Some(area.center());
    }
}
fn env(name: &str) -> String {
    std::env::var(name).unwrap()
}
fn key(window: &mut Window, text: &str, cx: &mut App) {
    window.dispatch_keystroke(Keystroke::parse(text).unwrap(), cx);
}
