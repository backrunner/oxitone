//! Opt-in smoke: real keyboard dispatch plus scroll/drag controllers using rendered bounds.
use crate::{
    piano_layout::{KEY_WIDTH, SCROLLBAR},
    ui::Preview,
};
use gpui::*;

pub fn step(frame: usize, view: &Entity<Preview>, window: &mut Window, cx: &mut App) {
    match frame {
        3 => view.read(cx).piano_focus.focus(window),
        4 => {
            window.dispatch_keystroke(Keystroke::parse("+").unwrap(), cx);
        }
        5 => {
            assert!(view.read(cx).piano.zoom > 1., "piano keyboard focus/zoom");
            view.update(cx, |state, cx| {
                state.scroll_piano(&ScrollWheelEvent {
                    delta: ScrollDelta::Pixels(point(px(-40.), px(0.))),
                    ..Default::default()
                });
                cx.notify();
            });
        }
        6 => {
            view.update(cx, |state, cx| {
                let l = state.piano_layout().unwrap();
                assert!(l.scroll_x > 0., "piano horizontal scrolling");
                let (start, length) =
                    crate::workspace::thumb(l.grid_width, l.grid_width, l.max_x, -l.scroll_x);
                let at = state.piano.origin.get()
                    + point(
                        px(KEY_WIDTH + start + length * 0.5),
                        px(l.height - SCROLLBAR * 0.5),
                    );
                state.press_piano(&MouseDownEvent {
                    position: at,
                    ..Default::default()
                });
                assert!(
                    state.workspace.gesture.is_some(),
                    "piano scrollbar coordinates"
                );
                state.move_gesture(
                    &MouseMoveEvent {
                        position: at + point(px(-30.), px(0.)),
                        pressed_button: Some(MouseButton::Left),
                        ..Default::default()
                    },
                    window,
                );
                state.workspace.gesture = None;
                assert!(
                    state.piano_layout().unwrap().scroll_x < l.scroll_x,
                    "piano scrollbar dragging"
                );
                cx.notify();
            });
        }
        7 => view.read(cx).mixer_focus.focus(window),
        8 => {
            window.dispatch_keystroke(Keystroke::parse("end").unwrap(), cx);
        }
        9 => {
            let state = view.read(cx);
            let strips = crate::mixer_model::strips(state.project.as_ref().unwrap());
            let last = strips.iter().rev().find(|s| s.id != "mix_master").unwrap();
            assert_eq!(state.selected_scope, last.id, "mixer keyboard selection");
            if state.workspace.mixer.max_offset().width > px(0.) {
                assert!(
                    state.workspace.mixer.offset().x < px(0.),
                    "selected strip is revealed"
                );
            }
            window.dispatch_keystroke(Keystroke::parse("down").unwrap(), cx);
        }
        10 => {
            let state = view.read(cx);
            if state.workspace.mixer.max_offset().height > px(0.) {
                assert!(
                    state.workspace.mixer.offset().y < px(0.),
                    "mixer vertical scrolling"
                );
            }
            assert!(
                !state.playback.playing,
                "navigation must not start playback"
            );
            eprintln!("Preview navigation smoke passed");
        }
        _ => {}
    }
}
