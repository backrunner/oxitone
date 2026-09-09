//! Real-window UI regression, restricted to the disposable smoke-daw fixture.
use crate::{
    document_wire::DocumentOperation, plugin_details::DetailTarget, ui::Preview,
    window_manager::WindowId,
};
use gpui::*;

#[derive(Default)]
pub struct Smoke {
    stage: u8,
    revision: u64,
    file: String,
    code: String,
    button: Point<Pixels>,
    lock: Option<std::path::PathBuf>,
    install_prepared: bool,
}
impl Smoke {
    pub fn complete(&self) -> bool {
        std::env::var_os("OXITONE_PREVIEW_CAPTURE_UI_REVIEW").is_none() || self.stage == 12
    }
    pub fn step(&mut self, view: &Entity<Preview>, window: &mut Window, cx: &mut App) {
        if self.complete() {
            return;
        }
        match self.stage {
            0 => {
                if !view.read(cx).document_ready() {
                    return;
                }
                view.update(cx, |s, cx| {
                    let document = s.document.view.as_ref().unwrap();
                    self.revision = document.revision;
                    self.file = document.files[0].path.clone();
                    self.code = document.files[0].text.clone();
                    s.document.show_code = false;
                    s.show_shortcuts = true;
                    // Deliberately retain child focus: a modal must capture before piano handlers.
                    s.piano_focus.focus(window);
                    cx.notify();
                });
            }
            1 => {
                let before = view.read(cx).workspace.mode;
                for key in ["cmd-z", "delete", "space", "f9", "cmd-w"] {
                    window.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx);
                }
                let s = view.read(cx);
                assert!(s.show_shortcuts && s.document.pending.is_none());
                assert_eq!(s.workspace.mode, before);
                assert_eq!(s.document.view.as_ref().unwrap().revision, self.revision);
                assert!(!s.is_playing());
                window.dispatch_keystroke(Keystroke::parse("escape").unwrap(), cx);
                view.update(cx, |s, cx| {
                    assert!(!s.show_shortcuts);
                    let owner = s.project.as_ref().unwrap().snapshot.channels[0].id.clone();
                    s.open_plugin(DetailTarget::Instrument(owner), cx).unwrap();
                    cx.notify();
                });
            }
            2 => {
                let plugin = view
                    .read(cx)
                    .plugin_windows
                    .values()
                    .next()
                    .unwrap()
                    .read(cx);
                let bounds = plugin.source_button.get();
                assert!(bounds.size.width > px(40.));
                self.button = bounds.center();
                pointer(window, self.button, false, cx);
            }
            3 => pointer(window, self.button, true, cx),
            4 => {
                view.update(cx, |s, _| {
                    assert_eq!(s.document.windows.front(), Some(WindowId::Configuration));
                    let site = s
                        .configuration_site()
                        .expect("configuration navigation must isolate the selected use");
                    assert_eq!(site.kind, "instrument");
                    assert_eq!(
                        site.usages[0].handle,
                        s.project.as_ref().unwrap().snapshot.channels[0]
                            .instrument
                            .instance_id
                            .clone()
                            .unwrap()
                    );
                    s.document_request(DocumentOperation::Code {
                        file_name: self.file.clone(),
                        text: "export default !!!".into(),
                    });
                });
            }
            5 => {
                let s = view.read(cx);
                if s.document.pending.is_some()
                    || s.document.view.as_ref().unwrap().status != "invalid"
                {
                    return;
                }
                assert!(s.active_diagnostic().is_some());
                assert_eq!(
                    s.project.as_ref().unwrap().snapshot.revision,
                    self.revision + 1
                );
                if !self.install_prepared {
                    crate::capture_plugin_install::prepare(view, window, cx);
                    self.install_prepared = true;
                    return;
                }
                crate::capture_plugin_install::invalid_project(view, window, cx);
                view.update(cx, |s, _| {
                    s.document_request(DocumentOperation::Code {
                        file_name: self.file.clone(),
                        text: format!("{}\n// UI close review\n", self.code),
                    })
                });
            }
            6 => {
                if !view.read(cx).document_ready() {
                    return;
                }
                assert!(view.read(cx).active_diagnostic().is_none());
                assert!(view.read(cx).document.view.as_ref().unwrap().modified);
                native_close(window, cx);
            }
            7 => {
                assert!(
                    view.read(cx).close.open,
                    "native close must retain the dirty project"
                );
                window.dispatch_keystroke(Keystroke::parse("cmd-z").unwrap(), cx);
                assert!(view.read(cx).document.pending.is_none());
                window.dispatch_keystroke(Keystroke::parse("escape").unwrap(), cx);
                assert!(!view.read(cx).close.open);
                let root = std::path::Path::new(&self.file).parent().unwrap();
                assert!(root
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("oxitone-daw-smoke-"));
                let lock = root.join(".oxitone-source-save/lock");
                std::fs::create_dir(&lock).unwrap();
                std::fs::write(
                    lock.join("owner.json"),
                    format!("{{\"pid\":{}}}", std::process::id()),
                )
                .unwrap();
                self.lock = Some(lock);
                native_close(window, cx);
            }
            8 => {
                assert!(view.read(cx).close.open);
                window.dispatch_keystroke(Keystroke::parse("enter").unwrap(), cx);
                assert!(view.read(cx).close.saving());
            }
            9 => {
                if view.read(cx).close.saving() || view.read(cx).document.pending.is_some() {
                    return;
                }
                let s = view.read(cx);
                assert!(s.close.open && s.close.error.is_some());
                assert!(s.document.view.as_ref().unwrap().modified);
                assert_eq!(cx.windows().len(), 1);
                std::fs::remove_dir_all(self.lock.take().unwrap()).unwrap();
            }
            11 => {
                eprintln!("UI review smoke passed: modal key isolation, plugin configuration hit test, invalid-code recovery, native dirty close, cancel, failed Save retained; retry Save & close follows capture");
            }
            _ => {}
        }
        self.stage += 1;
    }
}

fn pointer(window: &Window, position: Point<Pixels>, up: bool, cx: &App) {
    let input = if up {
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
    crate::capture_pointer::dispatch(window, input, cx);
}
#[cfg(target_os = "macos")]
fn native_close(window: &Window, cx: &App) {
    let surface = crate::capture_surface::Surface::new(window).unwrap();
    cx.foreground_executor()
        .spawn(async move {
            surface.close();
        })
        .detach();
}
#[cfg(not(target_os = "macos"))]
fn native_close(_: &Window, _: &App) {
    panic!("UI review capture requires macOS");
}
