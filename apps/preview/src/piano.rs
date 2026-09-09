//! Pattern edits are submitted to the Node document owner; this view never writes source or DSP state.
use crate::ui::Preview;
use gpui::{prelude::*, *};

pub fn view(this: &mut Preview, width: f32, cx: &mut Context<Preview>) -> impl IntoElement {
    let theme = this.theme;
    if this.piano.selection != this.selected_clip {
        this.piano.selection = this.selected_clip.clone();
        this.piano.fit_length(this.piano_period().unwrap_or(8.));
        this.document.notes.clear();
        this.document.gesture = None;
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
        return root.child(div().p_4().child(theme.label("No clip selected")));
    };
    let pattern = this.piano_pattern().unwrap().clone();
    if let Some(selector) = crate::pattern_parts::selector(this, cx) {
        root = root.child(selector);
    }
    let (start, end) = project.clip_bounds(clip);
    let name = project.pattern_label(&pattern.id);
    let local_length = this.piano_period().unwrap();
    root = root.child(crate::piano_toolbar::view(
        this,
        width,
        name,
        (start, end),
        cx,
    ));
    let channel_ids = project
        .snapshot
        .tracks
        .iter()
        .find(|t| t.id == clip.track_id)
        .map(|t| t.channel_ids.as_slice())
        .unwrap_or(&[]);
    let part_channels: Vec<_> = project
        .snapshot
        .patterns
        .iter()
        .find(|p| p.id == clip.pattern_id)
        .and_then(|p| p.parts.as_ref())
        .into_iter()
        .flatten()
        .filter(|part| part.pattern_id == pattern.id)
        .map(|part| part.channel_id.clone())
        .collect();
    let channel_ids = if part_channels.is_empty() {
        channel_ids
    } else {
        &part_channels
    };
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
    let global = project.beat(this.position_frame());
    let phase = (global >= start && global < end && clip.enabled != Some(false)).then(|| {
        (project.global_to_local(&clip.track_id, global) - clip.start_beat.to_f64())
            .rem_euclid(local_length)
    });
    let state = this.piano.clone();
    let pattern_id = pattern.id.clone();
    let source = project.clone();
    let site = this.pattern_site();
    let selected: std::collections::BTreeSet<_> = site
        .filter(|s| this.document.notes.site.as_ref() == Some(&s.handle))
        .map(|_| this.document.notes.indices.clone())
        .unwrap_or_default();
    let pending = this.document.pending_notes.as_ref().filter(|g| {
        this.presentation_active()
            && this.document.pending_note_pattern.as_ref() == Some(&pattern.id)
            && g.placement
                .as_ref()
                .is_none_or(|p| Some(p) == this.selected_clip.as_ref())
    });
    let gesture = this
        .document
        .gesture
        .as_ref()
        .filter(|g| site.is_some_and(|s| s.handle == g.site))
        .or(pending);
    // Hit testing and paint use the same source output order, including duplicate notes.
    let source_notes: Vec<_> = if pending.is_some() {
        this.document.pending_note_source.clone()
    } else if let Some(site) = site.filter(|_| {
        this.document.view.as_ref().is_some_and(|v| {
            v.accepted_revision == v.revision as i64 && v.revision + 1 == project.snapshot.revision
        })
    }) {
        site.outputs.iter().map(|o| o.note.clone()).collect()
    } else {
        pattern
            .notes
            .iter()
            .map(|n| crate::document_wire::SourceNote {
                pitch: n.pitch,
                start: n.start.to_f64(),
                duration: n.duration.to_f64(),
                velocity: n.velocity,
            })
            .collect()
    };
    let painted = crate::piano_note_paint::visible_notes(&source_notes, &selected, gesture);
    let marquee = this.document.notes.marquee.clone();
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
                    bounds, layout, theme, &painted, &sounding, phase, state.snap, window, cx,
                );
                if let Some(marquee) = &marquee {
                    let left = layout.x(marquee.start.0.min(marquee.end.0)).clamp(
                        crate::piano_layout::KEY_WIDTH,
                        layout.width - crate::piano_layout::SCROLLBAR,
                    );
                    let right = layout
                        .x(marquee.start.0.max(marquee.end.0))
                        .clamp(left, layout.width - crate::piano_layout::SCROLLBAR);
                    let top = crate::piano_layout::RULER
                        + marquee.start.1.min(marquee.end.1) * layout.key_height
                        - layout.scroll_y;
                    let bottom = crate::piano_layout::RULER
                        + marquee.start.1.max(marquee.end.1) * layout.key_height
                        - layout.scroll_y;
                    let top = top.clamp(
                        crate::piano_layout::RULER,
                        crate::piano_layout::RULER + layout.grid_height,
                    );
                    let bottom = bottom.clamp(top, crate::piano_layout::RULER + layout.grid_height);
                    let area = Bounds::new(
                        bounds.origin + point(px(left), px(top)),
                        size(px(right - left), px(bottom - top)),
                    );
                    window.paint_quad(quad(
                        area,
                        px(2.),
                        crate::ui::alpha(theme.accent, 0.12),
                        px(1.),
                        rgb(theme.accent),
                        BorderStyle::default(),
                    ));
                }
            });
        },
    )
    .size_full();
    root.child(
        div()
            .id("piano-viewport")
            .relative()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .cursor(crate::piano_feedback::cursor(this))
            .child(surface)
            .child(crate::piano_divider::view(this, cx))
            .track_focus(&this.piano_focus)
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                this.piano.pointer = Some(event.position);
                cx.notify();
            }))
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                if !hovered {
                    this.piano.pointer = None;
                    cx.notify();
                }
            }))
            .on_scroll_wheel(cx.listener(|this, event, _, cx| {
                this.scroll_piano(event);
                cx.stop_propagation();
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event, window, cx| {
                    this.piano_focus.focus(window);
                    if !this.pan_piano(event) && !this.press_note(event) {
                        this.press_piano(event);
                    }
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Middle,
                cx.listener(|this, event, window, cx| {
                    this.piano_focus.focus(window);
                    this.pan_piano(event);
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event, window, cx| {
                    this.piano_focus.focus(window);
                    this.press_note(event);
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if this.edit_note_key(&event.keystroke)
                    || (!event.keystroke.modifiers.platform
                        && !event.keystroke.modifiers.control
                        && !event.keystroke.modifiers.alt
                        && this.piano_key(&event.keystroke.key))
                {
                    cx.stop_propagation();
                    cx.notify();
                }
            })),
    )
    .child(crate::piano_feedback::status(this, pattern.notes.len()))
}
