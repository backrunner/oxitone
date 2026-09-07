//! Display/transport controls only. No note or authoring mutation is permitted.
use crate::ui::Preview;
use gpui::{prelude::*, *};

pub fn view(this: &mut Preview, cx: &mut Context<Preview>) -> impl IntoElement {
    let theme = this.theme;
    if this.piano.selection != this.selected_clip {
        this.piano.selection = this.selected_clip.clone();
        this.piano.fit();
    }
    let project = this.project.as_ref().unwrap().clone();
    let mut root = div()
        .flex_1()
        .min_w_0()
        .h_full()
        .flex()
        .flex_col()
        .border_r_1()
        .border_color(rgb(theme.border));
    let Some(clip) = project
        .snapshot
        .pattern_clips
        .iter()
        .find(|c| Some(&c.id) == this.selected_clip.as_ref())
    else {
        return root.child(
            div()
                .p_4()
                .child(theme.label("PIANO ROLL · Select a pattern clip")),
        );
    };
    let pattern = project
        .snapshot
        .patterns
        .iter()
        .find(|p| p.id == clip.pattern_id)
        .unwrap();
    let (start, end) = project.clip_bounds(clip);
    let name = project.pattern_label(&pattern.id);
    let local_length = pattern.length_beats.to_f64();
    root = root.child(
        div()
            .h(px(46.))
            .flex_shrink_0()
            .px_3()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(div().text_sm().truncate().child(name))
                    .child(theme.label(format!(
                        "PIANO ROLL · {} notes · {local_length:.1} beats",
                        pattern.notes.len()
                    ))),
            )
            .child(
                theme
                    .button("piano-fit", "Fit")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.piano.fit();
                        cx.notify();
                    })),
            )
            .child(
                theme
                    .button("piano-out", "−")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.zoom_piano(0.8, 1.);
                        cx.notify();
                    })),
            )
            .child(
                theme
                    .button("piano-in", "+")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.zoom_piano(1.25, 1.);
                        cx.notify();
                    })),
            )
            .child(
                theme
                    .button("piano-keys-out", "Keys −")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.zoom_piano(1., 0.8);
                        cx.notify();
                    })),
            )
            .child(
                theme
                    .button("piano-keys-in", "Keys +")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.zoom_piano(1., 1.25);
                        cx.notify();
                    })),
            )
            .child(
                theme
                    .button("loop-selection", "Loop clip")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.loop_start = start;
                        this.loop_end = end;
                        this.loop_enabled = true;
                        this.seek(start);
                        if this.playback.playing {
                            this.play();
                        }
                        cx.notify();
                    })),
            ),
    );
    let channel_ids = project
        .snapshot
        .tracks
        .iter()
        .find(|t| t.id == clip.track_id)
        .map(|t| t.channel_ids.as_slice())
        .unwrap_or(&[]);
    let mut sounding = [false; 128];
    for pitch in 0..128 {
        let actual = pitch as i32 + clip.transpose.unwrap_or(0);
        sounding[pitch] = (0..128).contains(&actual)
            && channel_ids.iter().any(|id| {
                this.analysis
                    .get(id)
                    .is_some_and(|a| a.sounding(actual as u8))
            });
    }
    let global = project.beat(this.playback.audible);
    let phase = (global >= start && global < end && clip.enabled != Some(false)).then(|| {
        (project.global_to_local(&clip.track_id, global) - clip.start_beat.to_f64())
            .rem_euclid(local_length)
    });
    let state = this.piano.clone();
    let pattern_id = pattern.id.clone();
    let source = project.clone();
    let surface = canvas(
        |_, _, _| {},
        move |bounds, _, window, cx| {
            state.viewport.set(bounds.size);
            state.origin.set(bounds.origin);
            let pattern = source
                .snapshot
                .patterns
                .iter()
                .find(|p| p.id == pattern_id)
                .unwrap();
            let layout = state.layout(local_length, pattern.notes.iter().map(|n| n.pitch));
            window.with_content_mask(Some(ContentMask { bounds }), |window| {
                crate::piano_paint::paint(
                    bounds, layout, theme, pattern, &sounding, phase, window, cx,
                );
            });
        },
    )
    .size_full();
    root.child(
        div()
            .id("piano-viewport")
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .child(surface)
            .track_focus(&this.piano_focus)
            .on_scroll_wheel(cx.listener(|this, event, _, cx| {
                this.scroll_piano(event);
                cx.stop_propagation();
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event, window, cx| {
                    this.piano_focus.focus(window);
                    this.press_piano(event);
                    cx.notify();
                }),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if !event.keystroke.modifiers.platform
                    && !event.keystroke.modifiers.control
                    && !event.keystroke.modifiers.alt
                    && this.piano_key(&event.keystroke.key)
                {
                    cx.stop_propagation();
                    cx.notify();
                }
            })),
    )
    .child(
        div()
            .h(px(24.))
            .flex_shrink_0()
            .px_3()
            .flex()
            .items_center()
            .text_size(px(10.))
            .text_color(rgb(theme.muted))
            .child("Pattern beats · Scroll keys / Shift-scroll time · ⌘ scroll zoom · Read only"),
    )
}
