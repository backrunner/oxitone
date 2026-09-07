//! GPUI keyboard dispatch and pointer controllers using rendered bounds; simulated native sink.
use crate::{playback_controls::frame, plugin_window::PluginWindow, ui::Preview};
use gpui::*;

pub fn enabled() -> bool {
    std::env::var_os("OXITONE_PREVIEW_CAPTURE").is_some()
        && std::env::var("OXITONE_PREVIEW_CAPTURE_TRANSPORT").is_ok_and(|v| v == "1")
}
#[derive(Default)]
pub struct Smoke {
    source: Option<String>,
    cue: u64,
    detail: Option<WindowHandle<PluginWindow>>,
}
fn key(window: &mut Window, cx: &mut App, text: &str) {
    window.dispatch_keystroke(Keystroke::parse(text).unwrap(), cx);
}
fn click(
    view: &Entity<Preview>,
    window: &mut Window,
    cx: &mut App,
    at: Point<Pixels>,
    count: usize,
    piano: bool,
) {
    let event = MouseDownEvent {
        position: at,
        click_count: count,
        modifiers: Modifiers {
            alt: piano,
            ..Default::default()
        },
        ..Default::default()
    };
    view.update(cx, |state, cx| {
        if piano {
            state.piano_focus.focus(window);
            state.press_piano(&event);
        } else {
            state.workspace_focus.focus(window);
            state.press_playlist(&event);
        }
        cx.notify();
    });
}
impl Smoke {
    pub fn step(&mut self, tick: usize, view: &Entity<Preview>, window: &mut Window, cx: &mut App) {
        match tick {
            2 => {
                let state = view.read(cx);
                let project = state.project.as_ref().unwrap();
                self.source = Some(serde_json::to_string(&project.snapshot).unwrap());
                self.cue = frame(project, 5.375);
                let scroll = &state.workspace.arrangement;
                let at = scroll.bounds().origin + point(px(5.375 * state.zoom), px(-15.));
                click(view, window, cx, at, 1, false);
            }
            4 => {
                let state = view.read(cx);
                assert!(
                    state.cue_frame.abs_diff(self.cue) <= 1,
                    "ruler locates fractional beats"
                );
                assert!(!state.playback.playing);
                assert!(state.playback.cursor.abs_diff(self.cue) <= 1);
                let at = state.workspace.arrangement.bounds().origin
                    + point(px(5.375 * state.zoom), px(20.));
                click(view, window, cx, at, 2, false);
            }
            7 => {
                assert!(
                    view.read(cx).playback.playing,
                    "double-click starts native playback"
                );
                key(window, cx, "space");
                view.update(cx, |state, cx| {
                    state.workspace_key(
                        &KeyDownEvent {
                            keystroke: Keystroke::parse("space").unwrap(),
                            is_held: true,
                        },
                        window,
                        cx,
                    )
                });
            }
            9 => {
                let state = view.read(cx);
                assert!(!state.playback.playing, "held Space must not toggle again");
                assert_eq!(state.playback.audible, state.playback.cursor);
                key(window, cx, "enter");
            }
            11 => {
                assert!(view.read(cx).playback.playing);
                key(window, cx, "shift-space");
            }
            13 => {
                let state = view.read(cx);
                assert!(!state.playback.playing);
                assert!(
                    state.playback.cursor.abs_diff(self.cue) <= 1,
                    "Stop returns to cue"
                );
                view.update(cx, |state, _| {
                    state.loop_start = 4.;
                    state.loop_end = 8.;
                });
                key(window, cx, "l");
                key(window, cx, "g");
            }
            14 => {
                key(window, cx, "space");
                assert!(
                    !view.read(cx).is_playing(),
                    "position field blocks playback shortcut"
                );
                for ch in "12.2.480".chars() {
                    key(window, cx, &ch.to_string());
                }
                key(window, cx, "enter");
            }
            16 => {
                let state = view.read(cx);
                let project = state.project.as_ref().unwrap();
                assert!(state.cue_frame.abs_diff(frame(project, 45.5)) <= 1);
                assert!(!state.loop_enabled, "locating outside the loop disables it");
                key(window, cx, "alt-right");
                key(window, cx, "alt-right");
            }
            18 => {
                let state = view.read(cx);
                assert!(
                    state
                        .cue_frame
                        .abs_diff(frame(state.project.as_ref().unwrap(), 47.5))
                        <= 1
                );
                key(window, cx, "l");
                assert_eq!(
                    view.read(cx).cue_frame,
                    frame(view.read(cx).project.as_ref().unwrap(), 4.),
                    "enabling a loop outside its region locates its start"
                );
                key(window, cx, "cmd-home");
            }
            20 => {
                let state = view.read(cx);
                assert_eq!(state.playback.cursor, 0);
                let layout = state.piano_layout().unwrap();
                let at = state.piano.origin.get()
                    + point(px(layout.x(1.25)), px(crate::piano_layout::RULER + 20.));
                click(view, window, cx, at, 1, true);
            }
            23 => {
                assert!(
                    view.read(cx).playback.playing,
                    "Alt-click in piano starts playback"
                );
                key(window, cx, "shift-space");
                self.cue = view.read(cx).cue_frame;
                let target = crate::plugin_details::DetailTarget::Instrument(
                    view.read(cx).project.as_ref().unwrap().snapshot.channels[0]
                        .id
                        .clone(),
                );
                self.detail = view.update(cx, |state, cx| state.open_plugin(target, cx));
            }
            25 => {
                cx.update_window(self.detail.unwrap().into(), |_, window, cx| {
                    key(window, cx, "space")
                })
                .unwrap();
            }
            28 => {
                assert!(
                    view.read(cx).playback.playing,
                    "plugin window forwards transport keys"
                );
                cx.update_window(self.detail.unwrap().into(), |_, window, cx| {
                    key(window, cx, "shift-space")
                })
                .unwrap();
            }
            30 => {
                assert!(!view.read(cx).playback.playing);
                assert_eq!(view.read(cx).playback.cursor, self.cue);
                self.detail
                    .take()
                    .unwrap()
                    .update(cx, |_, window, _| window.remove_window())
                    .unwrap();
            }
            32 => {
                view.read(cx).workspace_focus.focus(window);
                key(window, cx, "?");
            }
            34 => {
                let state = view.read(cx);
                assert!(state.show_shortcuts);
                assert_eq!(
                    self.source.as_ref().unwrap(),
                    &serde_json::to_string(&state.project.as_ref().unwrap().snapshot).unwrap()
                );
                eprintln!("Preview transport mouse/keyboard smoke passed (simulated audio)");
            }
            _ => {}
        }
    }
}
