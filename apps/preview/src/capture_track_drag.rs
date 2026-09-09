//! Real pointer regression for Track controls and live cross-track item presentation.
use crate::{playlist_edit::DragMode, playlist_projection, ui::Preview};
use gpui::*;
#[derive(Default)]
pub struct Smoke {
    stage: u8,
    at: Point<Pixels>,
    original: String,
    target: String,
    clip: String,
    revision: u64,
}
impl Smoke {
    pub fn step(&mut self, view: &Entity<Preview>, window: &mut Window, cx: &mut App) -> bool {
        if !view.read(cx).document_ready() {
            return false;
        }
        match self.stage {
            0 => view.update(cx, |s, cx| {
                s.document.windows.hidden = true;
                let p = s.project.as_ref().unwrap();
                let clip = &p.snapshot.pattern_clips[0];
                self.clip = clip.id.clone();
                self.original = clip.track_id.clone();
                self.target = p
                    .snapshot
                    .tracks
                    .iter()
                    .find(|t| t.id != self.original)
                    .unwrap()
                    .id
                    .clone();
                cx.notify();
            }),
            1 | 4 => {
                let key = if self.stage == 1 { "mute" } else { "solo" };
                let area = view.read(cx).document.playlist.controls.borrow()
                    [&format!("track-{key}-{}", self.original)];
                self.at = area.center();
                pointer(window, self.at, 0, cx);
            }
            2 | 5 => pointer(window, self.at, 2, cx),
            3 | 6 => {
                let p = view.read(cx).project.as_ref().unwrap();
                let track = p
                    .snapshot
                    .tracks
                    .iter()
                    .find(|t| t.id == self.original)
                    .unwrap();
                assert_eq!(track.mute, Some(true));
                if self.stage == 6 {
                    assert_eq!(track.solo, Some(true));
                    assert!(p
                        .snapshot
                        .tracks
                        .iter()
                        .filter(|t| t.id != self.original)
                        .all(|t| t.solo != Some(true) && t.mute != Some(true)));
                    assert!(
                        p.snapshot.channels.iter().all(|c| c.solo != Some(true)),
                        "Track solo leaves shared Channels alone"
                    );
                }
            }
            7 => {
                let s = view.read(cx);
                let p = s.project.as_ref().unwrap();
                let clip = p
                    .snapshot
                    .pattern_clips
                    .iter()
                    .find(|c| c.id == self.clip)
                    .unwrap();
                let row = s.document.playlist.rows.borrow()[&self.original];
                self.at =
                    row.origin + point(px((p.clip_bounds(clip).0 as f32 + 0.25) * s.zoom), px(20.));
                self.revision = s.document.view.as_ref().unwrap().revision;
                pointer(window, self.at, 0, cx);
            }
            8 => {
                let s = view.read(cx);
                assert!(s.document.playlist.drag.is_some());
                let row = s.document.playlist.rows.borrow()[&self.target];
                self.at = row.origin + point(px(8.25 * s.zoom), px(20.));
                pointer(window, self.at, 1, cx);
            }
            9 => {
                let s = view.read(cx);
                let drag = s.document.playlist.drag.as_ref().unwrap();
                assert!(drag.mode == DragMode::Move);
                self.assert_projection(s, drag);
                assert_eq!(
                    s.document.view.as_ref().unwrap().revision,
                    self.revision,
                    "move does not submit per frame"
                );
                let clip = self.clip.clone();
                let target = self.target.clone();
                release(window, self.at, view, cx, move |s| {
                    assert!(s.document.pending.is_some());
                    let drag = s
                        .document
                        .playlist
                        .pending
                        .as_ref()
                        .expect("released item stays in place until acceptance");
                    let items = playlist_projection::project(
                        playlist_projection::placements(s.project.as_ref().unwrap()),
                        Some(drag),
                    );
                    let live: Vec<_> = items.iter().filter(|c| c.id == clip).collect();
                    assert_eq!(live.len(), 1);
                    assert_eq!(live[0].track, target);
                    assert_eq!(live[0].start, 8.);
                });
            }
            10 => {
                let s = view.read(cx);
                let p = s.project.as_ref().unwrap();
                assert_eq!(
                    s.document.view.as_ref().unwrap().revision,
                    self.revision + 1
                );
                let clip = p
                    .snapshot
                    .pattern_clips
                    .iter()
                    .find(|c| c.id == self.clip)
                    .unwrap();
                assert_eq!(clip.track_id, self.target);
                assert_eq!(clip.start_beat.to_f64(), 8.);
                assert!(s.document.playlist.pending.is_none());
                window.dispatch_keystroke(Keystroke::parse("cmd-z").unwrap(), cx);
            }
            11 => {
                let s = view.read(cx);
                assert_eq!(
                    s.project
                        .as_ref()
                        .unwrap()
                        .snapshot
                        .pattern_clips
                        .iter()
                        .find(|c| c.id == self.clip)
                        .unwrap()
                        .track_id,
                    self.original
                );
                window.dispatch_keystroke(Keystroke::parse("cmd-s").unwrap(), cx);
            }
            12 => {
                if view.read(cx).document.view.as_ref().unwrap().modified {
                    return false;
                }
                view.update(cx, |s, cx| {
                    s.document.windows.hidden = false;
                    s.float_editor(crate::workspace_layout::EditorMode::Piano);
                    s.document.notes.clear();
                    self.revision = s.document.view.as_ref().unwrap().revision;
                    cx.notify();
                });
            }
            13 => {
                let s = view.read(cx);
                let l = s.piano_layout().unwrap();
                let note = &s.pattern_site().unwrap().outputs[0].note;
                self.at = s.piano.origin.get()
                    + point(
                        px(l.x(note.start + note.duration * 0.4)),
                        px(l.y(note.pitch) + l.key_height * 0.5),
                    );
                pointer(window, self.at, 0, cx);
            }
            14 => {
                let s = view.read(cx);
                assert!(
                    s.document.gesture.is_some(),
                    "Piano note hit starts direct drag: at={:?}, origin={:?}, layout={:?}, window={:?}, top={:?}, ready={}",
                    self.at, s.piano.origin.get(), s.piano_layout(),
                    s.document.windows.bounds(crate::window_manager::WindowId::Piano),
                    s.document.windows.hit(self.at), s.document_ready()
                );
                let l = s.piano_layout().unwrap();
                self.at.x += px(l.beat_width * 0.5);
                self.at.y -= px(l.key_height * 2.);
                pointer(window, self.at, 1, cx);
            }
            15 => {
                let s = view.read(cx);
                let g = s.document.gesture.as_ref().unwrap();
                assert_eq!(g.changes.len(), 1);
                assert_ne!(g.changes[0].before, g.changes[0].after);
                let notes: Vec<_> = s
                    .pattern_site()
                    .unwrap()
                    .outputs
                    .iter()
                    .map(|o| o.note.clone())
                    .collect();
                let painted = crate::piano_note_paint::visible_notes(
                    &notes,
                    &s.document.notes.indices,
                    Some(g),
                );
                assert_eq!(painted.len(), notes.len());
                assert_eq!(painted.last().unwrap().note, g.changes[0].after);
                assert_eq!(s.document.view.as_ref().unwrap().revision, self.revision);
                release(window, self.at, view, cx, |s| {
                    assert!(s.document.pending_notes.is_some() && s.presentation_active());
                });
            }
            16 => {
                let s = view.read(cx);
                assert_eq!(
                    s.document.view.as_ref().unwrap().revision,
                    self.revision + 1
                );
                assert!(s.document.pending_notes.is_none());
                assert_eq!(s.document.notes.indices.len(), 1);
                window.dispatch_keystroke(Keystroke::parse("cmd-z").unwrap(), cx);
            }
            17 => {
                window.dispatch_keystroke(Keystroke::parse("cmd-s").unwrap(), cx);
            }
            18 => {
                if view.read(cx).document.view.as_ref().unwrap().modified {
                    return false;
                }
                eprintln!("Track/drag smoke passed: real M/S hits, shared Channel, live cross-track content, pending position, one revision, Undo and Save");
                return true;
            }
            _ => unreachable!(),
        }
        self.stage += 1;
        false
    }
    fn assert_projection(&self, s: &Preview, drag: &crate::playlist_edit::Drag) {
        let items = playlist_projection::project(
            playlist_projection::placements(s.project.as_ref().unwrap()),
            Some(drag),
        );
        let live: Vec<_> = items.iter().filter(|c| c.id == self.clip).collect();
        assert_eq!(live.len(), 1);
        assert_eq!(live[0].track, self.target);
        assert_eq!(live[0].start, 8.);
        assert_eq!(live[0].resource, drag.resource);
    }
}
fn pointer(window: &Window, position: Point<Pixels>, phase: u8, cx: &App) {
    let input = match phase {
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
    };
    crate::capture_pointer::dispatch(window, input, cx);
}
fn release(
    window: &Window,
    position: Point<Pixels>,
    view: &Entity<Preview>,
    cx: &App,
    check: impl FnOnce(&Preview) + 'static,
) {
    let view = view.clone();
    crate::capture_pointer::dispatch_checked(
        window,
        PlatformInput::MouseUp(MouseUpEvent {
            position,
            ..Default::default()
        }),
        cx,
        move |cx| check(view.read(cx)),
    );
}
