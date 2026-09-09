//! Real list hit testing and keyboard navigation without a document or audio mutation.
use crate::{
    plugin_details::DetailTarget, plugin_manager::LibrarySection, ui::Preview,
    window_manager::WindowId,
};
use gpui::*;
use std::sync::Arc;

#[derive(Default)]
pub struct Smoke {
    stage: u8,
    revision: u64,
    code: Vec<String>,
    project: Option<Arc<crate::model::ViewProject>>,
    configuration: Option<String>,
    button: Point<Pixels>,
}
impl Smoke {
    pub fn complete(&self) -> bool {
        std::env::var_os("OXITONE_PREVIEW_CAPTURE_LIBRARY").is_none() || self.stage == 15
    }
    pub fn step(&mut self, view: &Entity<Preview>, window: &mut Window, cx: &mut App) {
        if self.complete() || !view.read(cx).document_ready() {
            return;
        }
        match self.stage {
            0 => {
                view.update(cx, |s, cx| {
                    let document = s.document.view.as_ref().unwrap();
                    self.revision = document.revision;
                    self.code = document.files.iter().map(|f| f.text.clone()).collect();
                    self.project = s.project.clone();
                    let owner = s.project.as_ref().unwrap().snapshot.channels[0].id.clone();
                    s.edit_plugin_source(&DetailTarget::Instrument(owner), window, cx);
                    self.configuration = s.document.configuration.selected.clone();
                    assert!(self.configuration.is_some());
                    s.document.configuration.open = false;
                    s.document.manager.open = true;
                    s.document.show_code = false;
                    s.document.windows.focus(WindowId::Plugins);
                    s.workspace_focus.focus(window);
                    cx.notify();
                });
            }
            1 | 4 => {
                let index = if self.stage == 1 { 2 } else { 1 };
                let bounds = view.read(cx).document.manager.filter_bounds.get()[index];
                assert!(bounds.size.width > px(30.));
                self.button = bounds.center();
                pointer(window, self.button, false, cx);
            }
            2 | 5 => pointer(window, self.button, true, cx),
            3 | 6 => {
                let s = view.read(cx);
                let kind = if self.stage == 3 {
                    "effect"
                } else {
                    "instrument"
                };
                assert_eq!(
                    s.document.manager.category,
                    if self.stage == 3 { 2 } else { 1 }
                );
                assert!(!s.library_entries().is_empty());
                assert!(s.library_entries().iter().all(|entry| entry.kind == kind));
            }
            7 => {
                key(window, "cmd-f", cx);
                // String input is captured before document undo or global transport.
                key(window, "cmd-z", cx);
                assert!(view.read(cx).document.pending.is_none());
                for text in ["w", "a", "v", "e", "t", "a", "b", "l", "e"] {
                    key(window, text, cx);
                }
                assert_eq!(view.read(cx).document.manager.query, "wavetable");
                assert!(!view.read(cx).library_entries().is_empty());
                key(window, "enter", cx);
                key(window, "down", cx);
            }
            8 => {
                let s = view.read(cx);
                assert_eq!(
                    s.document.manager.selected.as_deref(),
                    Some(s.library_entries()[0].handle.as_str())
                );
                assert!(s.document.manager.section.is_none());
                assert_eq!(s.document.configuration.selected, self.configuration);
                key(window, "enter", cx);
                assert!(view.read(cx).document.manager.section == Some(LibrarySection::Uses));
            }
            9 => {
                key(window, "escape", cx);
                assert!(view.read(cx).document.manager.section.is_none());
                key(window, "cmd-f", cx);
                key(window, "backspace", cx);
                key(window, "enter", cx);
                assert!(view.read(cx).document.manager.query.is_empty());
                let bounds = view.read(cx).document.manager.filter_bounds.get()[0];
                self.button = bounds.center();
                pointer(window, self.button, false, cx);
            }
            10 => pointer(window, self.button, true, cx),
            11 => {
                assert_eq!(view.read(cx).document.manager.category, 0);
                key(window, "end", cx);
                let s = view.read(cx);
                assert_eq!(
                    s.document.manager.selected.as_deref(),
                    Some(s.library_entries().last().unwrap().handle.as_str())
                );
                key(window, "home", cx);
            }
            12 => {
                let original = view.read(cx).document.windows.bounds(WindowId::Plugins);
                view.update(cx, |s, _| {
                    s.document.windows.begin(
                        WindowId::Plugins,
                        point(px(0.), px(0.)),
                        crate::window_manager::GestureKind::Move,
                    );
                    s.document.windows.move_drag(point(px(10.), px(10.)));
                    assert_ne!(s.document.windows.bounds(WindowId::Plugins), original);
                });
                key(window, "escape", cx);
                let s = view.read(cx);
                assert!(s.document.windows.drag.is_none());
                assert_eq!(s.document.windows.bounds(WindowId::Plugins), original);
                assert!(s.document.pending.is_none());
                assert_eq!(s.document.view.as_ref().unwrap().revision, self.revision);
                assert_eq!(
                    s.document
                        .view
                        .as_ref()
                        .unwrap()
                        .files
                        .iter()
                        .map(|f| f.text.clone())
                        .collect::<Vec<_>>(),
                    self.code
                );
                assert!(Arc::ptr_eq(
                    s.project.as_ref().unwrap(),
                    self.project.as_ref().unwrap()
                ));
                assert_eq!(s.document.configuration.selected, self.configuration);
                assert!(s.plugin_windows.is_empty() && !s.document.configuration.open);
                assert!(!s.is_playing());
            }
            13 => {
                if std::env::var("OXITONE_PREVIEW_CAPTURE_LIBRARY").is_ok_and(|v| v == "details") {
                    view.update(cx, |s, cx| {
                        let handle = s
                            .document
                            .view
                            .as_ref()
                            .unwrap()
                            .plugins
                            .iter()
                            .find(|p| p.plugin_id == "fixture.gain")
                            .unwrap()
                            .handle
                            .clone();
                        s.select_library_plugin(handle);
                        s.toggle_library_section(LibrarySection::Details);
                        let mut bounds = s.document.windows.state(WindowId::Plugins).bounds;
                        bounds.width = 340.;
                        bounds.height = 300.;
                        s.document.windows.set_bounds(WindowId::Plugins, bounds);
                        cx.notify();
                    });
                }
            }
            14 => {
                let s = view.read(cx);
                let index = s
                    .library_entries()
                    .iter()
                    .position(|entry| s.document.manager.selected.as_ref() == Some(&entry.handle))
                    .unwrap();
                let scroll = &s.document.manager.list_scroll;
                let row = scroll.bounds_for_item(index).unwrap();
                assert!(row.top() + scroll.offset().y >= scroll.bounds().top() - px(1.));
                assert!(row.bottom() + scroll.offset().y <= scroll.bounds().bottom() + px(1.));
                eprintln!("Plugin library smoke passed: actual type filters, search input isolation, keyboard list selection and usage drawer, stable instance selection, unchanged source and native projection, selected row remains visible");
            }
            _ => {}
        }
        self.stage += 1;
    }
}
fn key(window: &mut Window, value: &str, cx: &mut App) {
    window.dispatch_keystroke(Keystroke::parse(value).unwrap(), cx);
}
fn pointer(window: &Window, position: Point<Pixels>, up: bool, cx: &App) {
    let event = if up {
        PlatformInput::MouseUp(MouseUpEvent {
            position,
            ..Default::default()
        })
    } else {
        PlatformInput::MouseDown(MouseDownEvent {
            position,
            ..Default::default()
        })
    };
    crate::capture_pointer::dispatch(window, event, cx);
}
