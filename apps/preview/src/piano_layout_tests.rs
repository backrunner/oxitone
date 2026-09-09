use crate::piano_layout::*;

#[test]
fn fits_notes_and_short_patterns_without_a_dead_canvas() {
    let l = PianoLayout::new(800., 320., 4., [36, 42, 46].into_iter(), 1., 1., None);
    assert!(l.max_x > l.grid_width * 2.);
    assert!(l.x(4.) < 800. - SCROLLBAR);
    for pitch in [36, 42, 46] {
        assert!(l.visible_keys().any(|p| p == pitch));
    }
    for beat in [0., 0.125, 3.9, 4., 256.] {
        assert!((l.beat(l.x(beat)) - beat).abs() < 0.0001);
    }
}

#[test]
fn scroll_clamps_and_all_midi_keys_are_reachable() {
    let l = PianoLayout::new(
        400.,
        250.,
        128.,
        [0, 127].into_iter(),
        8.,
        1.,
        Some((1e6, 1e6)),
    );
    assert_eq!(l.scroll_x, 1e6);
    assert!(l.max_x > l.scroll_x + l.grid_width);
    assert!(l.visible_keys().any(|p| p == 0));
    let top = PianoLayout::new(
        400.,
        250.,
        128.,
        [0, 127].into_iter(),
        8.,
        1.,
        Some((-1., -1.)),
    );
    assert_eq!(top.scroll_x, 0.);
    assert!(top.visible_keys().any(|p| p == 127));
    assert_eq!(note_name(0), "C-1");
    assert_eq!(note_name(127), "G9");
}

#[test]
fn short_patterns_keep_a_reasonable_default_beat_width() {
    let l = PianoLayout::new(1200., 400., 1., [60].into_iter(), 1., 1., None);
    assert!(l.beat_width <= 180.);
    let zoomed = PianoLayout::new(1200., 400., 1., [60].into_iter(), 2., 1., None);
    assert!(zoomed.beat_width > l.beat_width);
}

#[test]
fn growing_a_pattern_keeps_scale_and_scroll_and_keeps_more_grid_ahead() {
    let mut state = PianoState::default();
    state.set_offset((9600., 100.));
    let old = state.layout(4., [60].into_iter());
    let new = state.layout(128., [60].into_iter());
    assert_eq!(old.beat_width, new.beat_width);
    assert_eq!(old.scroll_x, new.scroll_x);
    assert!(new.max_x > new.scroll_x + new.grid_width);
    state.set_offset((new.max_x, new.scroll_y));
    assert!(state.layout(128., [60].into_iter()).max_x > new.max_x);
}

#[test]
fn fit_uses_the_new_pattern_length_and_supports_long_phrases() {
    let mut state = PianoState::default();
    state.fit_length(256.);
    let long = state.layout(256., [60].into_iter());
    assert!((long.beat_width * 256. - long.grid_width).abs() < 0.01);
    state.fit_length(4.);
    let short = state.layout(4., [60].into_iter());
    assert!((short.beat_width * 8. - short.grid_width).abs() < 0.01);
}

#[test]
fn magnet_can_release_the_selected_snap_grid() {
    let mut state = PianoState::default();
    assert_eq!(state.effective_snap(false), Snap::Quarter);
    state.magnet = false;
    assert_eq!(state.effective_snap(false), Snap::Free);
    assert_eq!(state.effective_snap(true), Snap::Free);
}

#[test]
fn resizing_after_editing_keeps_the_same_musical_center() {
    let mut state = PianoState::default();
    state
        .viewport
        .set(gpui::size(gpui::px(900.), gpui::px(400.)));
    let old = state.layout(4., [60, 64, 67].into_iter());
    state.set_offset((old.scroll_x, old.scroll_y));
    let center = (old.scroll_y + old.grid_height * 0.5) / old.key_height;
    for height in [190., 500., 270., 400.] {
        state
            .viewport
            .set(gpui::size(gpui::px(650.), gpui::px(height)));
        let next = state.layout(4., [60, 64, 67].into_iter());
        assert_eq!(
            next.scroll_x, 0.,
            "left edge stays visible when floating or shrinking an editor"
        );
        assert!(
            ((next.scroll_y + next.grid_height * 0.5) / next.key_height - center).abs() < 0.001
        );
        assert!(next.visible_keys().any(|p| p == 64));
    }
}
