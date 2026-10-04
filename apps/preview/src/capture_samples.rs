//! Real tree hit-testing, file drops, source round trips and grouped editing with silent audio.
use crate::{plugin_details::DetailTarget, ui::Preview};
use gpui::*;
#[derive(Default)]
pub struct Smoke {
    stage: u8,
    at: Point<Pixels>,
    target: Point<Pixels>,
    samples: usize,
    clips: usize,
    brush_count: usize,
    instance: String,
    resource: String,
}
impl Smoke {
    pub fn complete(&self) -> bool {
        std::env::var_os("OXITONE_PREVIEW_CAPTURE_SAMPLES").is_none() || self.stage == 40
    }
    pub fn step(&mut self, view: &Entity<Preview>, window: &mut Window, cx: &mut App) {
        if self.complete() {
            return;
        }
        match self.stage {
            0 => view.update(cx, |s, cx| {
                s.document.windows.visible.clear();
                s.document.patterns_open = false;
                s.document.show_code = false;
                s.workspace.dock_open = false;
                s.dock_browser();
                s.zoom = 32.;
                s.workspace.arrangement.set_offset(point(px(0.), px(0.)));
                self.samples = s.project.as_ref().unwrap().snapshot.samples.len();
                self.clips = s.project.as_ref().unwrap().snapshot.pattern_clips.len();
                s.workspace_focus.focus(window);
                cx.notify();
            }),
            1 | 3 => {
                let tree = &view.read(cx).document.browser;
                let path = if self.stage == 1 {
                    tree.roots[0].clone()
                } else {
                    tree.roots[0].join("Samples")
                };
                let Some(bounds) = tree.row_bounds.borrow().get(&path).copied() else {
                    return;
                };
                self.at = bounds.center();
                pointer(window, self.at, 0, cx);
            }
            2 | 4 => pointer(window, self.at, 2, cx),
            5 | 10 => {
                let s = view.read(cx);
                let tree = &s.document.browser;
                let Some(bounds) = tree
                    .row_bounds
                    .borrow()
                    .get(&tree.roots[0].join("Samples/kick.wav"))
                    .copied()
                else {
                    return;
                };
                self.at = bounds.center();
                self.target = if self.stage == 5 {
                    let id = &s.project.as_ref().unwrap().snapshot.tracks[0].id;
                    let row = s.document.playlist.rows.borrow()[id];
                    point(row.left() + px(192.), row.center().y)
                } else {
                    let plugin = s.plugin_windows.values().next().unwrap().read(cx);
                    let bounds = plugin.sample_slots.borrow();
                    bounds["sample"].center()
                };
                pointer(window, self.at, 0, cx);
            }
            6 | 11 => pointer(window, self.target, 1, cx),
            7 | 12 => pointer(window, self.target, 2, cx),
            8 => {
                let s = view.read(cx);
                if !s.document_ready() {
                    return;
                }
                assert_eq!(
                    s.project.as_ref().unwrap().snapshot.samples.len(),
                    self.samples + 1
                );
                assert_eq!(s.project.as_ref().unwrap().snapshot.sample_clips.len(), 1);
            }
            9 => view.update(cx, |s, cx| {
                let ch = s
                    .project
                    .as_ref()
                    .unwrap()
                    .snapshot
                    .channels
                    .iter()
                    .find(|c| c.instrument.plugin_id == "oxitone.sampler")
                    .unwrap();
                self.instance = ch.instrument.instance_id.clone().unwrap();
                self.resource = ch.instrument.resources.as_ref().unwrap()["sample"].clone();
                s.open_plugin(DetailTarget::Instrument(ch.id.clone()), cx);
                cx.notify();
            }),
            13 => {
                let s = view.read(cx);
                if !s.document_ready() {
                    return;
                }
                let p = s.project.as_ref().unwrap();
                let ch = p
                    .snapshot
                    .channels
                    .iter()
                    .find(|c| c.instrument.plugin_id == "oxitone.sampler")
                    .unwrap();
                assert_eq!(
                    ch.instrument.instance_id.as_deref(),
                    Some(self.instance.as_str())
                );
                assert_ne!(
                    ch.instrument.resources.as_ref().unwrap()["sample"],
                    self.resource
                );
                assert_eq!(p.snapshot.samples.len(), self.samples + 2);
                press("cmd-z", window, cx);
            }
            14 => {
                let s = view.read(cx);
                if !s.document_ready() {
                    return;
                }
                assert_eq!(
                    s.project.as_ref().unwrap().snapshot.samples.len(),
                    self.samples + 1
                );
                press("cmd-z", window, cx);
            }
            15 => {
                if !view.read(cx).document_ready() {
                    return;
                }
                view.update(cx, |s, cx| {
                    assert_eq!(
                        s.project.as_ref().unwrap().snapshot.samples.len(),
                        self.samples
                    );
                    s.plugin_windows.clear();
                    s.document.windows.visible.clear();
                    s.workspace_focus.focus(window);
                    s.document.playlist.focused = true;
                    s.document.playlist.brush_clip = Some(
                        crate::playlist_projection::placements(s.project.as_ref().unwrap())[0]
                            .clone(),
                    );
                    cx.notify();
                });
                press("b", window, cx);
            }
            16 => {
                let s = view.read(cx);
                let length = s.document.playlist.brush_clip.as_ref().unwrap().length;
                self.brush_count = ((16. / length).floor() - (8. / length).floor()) as usize + 1;
                let track = &s.project.as_ref().unwrap().snapshot.tracks[0].id;
                let row = s.document.playlist.rows.borrow()[track];
                self.at = point(row.left() + px(8. * s.zoom), row.center().y);
                self.target = point(row.left() + px(16. * s.zoom), row.center().y);
                pointer(window, self.at, 0, cx);
            }
            17 => pointer(window, self.target, 1, cx),
            18 => pointer(window, self.target, 2, cx),
            19 => {
                let s = view.read(cx);
                if !s.document_ready() {
                    return;
                }
                assert_eq!(
                    s.project.as_ref().unwrap().snapshot.pattern_clips.len(),
                    self.clips + self.brush_count
                );
                press("cmd-z", window, cx);
            }
            20 => {
                if !view.read(cx).document_ready() {
                    return;
                }
                press("cmd-a", window, cx);
                press("cmd-d", window, cx);
            }
            21 => {
                let s = view.read(cx);
                if !s.document_ready() {
                    return;
                }
                assert_eq!(
                    s.project.as_ref().unwrap().snapshot.pattern_clips.len(),
                    self.clips * 2
                );
                assert_eq!(s.document.playlist.selection.len(), self.clips);
                press("cmd-c", window, cx);
                press("cmd-x", window, cx);
            }
            22 => {
                let s = view.read(cx);
                if !s.document_ready() {
                    return;
                }
                assert_eq!(
                    s.project.as_ref().unwrap().snapshot.pattern_clips.len(),
                    self.clips
                );
                press("cmd-shift-v", window, cx);
            }
            23 => {
                let s = view.read(cx);
                if !s.document_ready() {
                    return;
                }
                assert_eq!(
                    s.project.as_ref().unwrap().snapshot.pattern_clips.len(),
                    self.clips * 2
                );
                press("cmd-z", window, cx);
            }
            24 | 25 => {
                if !view.read(cx).document_ready() {
                    return;
                }
                press("cmd-z", window, cx);
            }
            26 => {
                let s = view.read(cx);
                if !s.document_ready() {
                    return;
                }
                assert_eq!(
                    s.project.as_ref().unwrap().snapshot.pattern_clips.len(),
                    self.clips
                );
                press("cmd-s", window, cx);
            }
            27 => {
                let s = view.read(cx);
                if !s.document_ready() || s.document.view.as_ref().unwrap().modified {
                    return;
                }
                eprintln!("Sample browser smoke passed: tree expansion, file-to-lane and file-to-sampler drops, stable instance, Undo, interpolated brush, group duplicate, cut/paste and Save");
                self.stage = 40;
                return;
            }
            _ => unreachable!(),
        }
        self.stage += 1;
    }
}
fn press(key: &str, window: &mut Window, cx: &mut App) {
    window.dispatch_keystroke(Keystroke::parse(key).unwrap(), cx);
}
fn pointer(window: &Window, position: Point<Pixels>, kind: u8, cx: &App) {
    crate::capture_pointer::dispatch(
        window,
        match kind {
            0 => PlatformInput::MouseDown(MouseDownEvent {
                position,
                ..Default::default()
            }),
            1 => PlatformInput::MouseMove(MouseMoveEvent {
                position,
                pressed_button: Some(MouseButton::Left),
                ..Default::default()
            }),
            _ => PlatformInput::MouseUp(MouseUpEvent {
                position,
                ..Default::default()
            }),
        },
        cx,
    );
}
