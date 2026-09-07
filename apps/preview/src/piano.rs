use crate::ui::{alpha, Preview};
use gpui::{prelude::*, *};

pub fn view(this: &Preview, cx: &mut Context<Preview>) -> impl IntoElement {
    let theme = this.theme;
    let project = this.project.as_ref().unwrap();
    let mut root = div()
        .flex_1()
        .min_w_0()
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
    let name = pattern.name.clone().unwrap_or_else(|| pattern.id.clone());
    root = root.child(
        div()
            .h(px(34.))
            .px_3()
            .flex()
            .items_center()
            .justify_between()
            .child(theme.label(format!("PIANO ROLL · {name}")))
            .child(
                theme
                    .button("loop-selection", "Loop selection")
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
    let low = pattern
        .notes
        .iter()
        .map(|n| n.pitch)
        .min()
        .unwrap_or(48)
        .saturating_sub(2);
    let high = pattern
        .notes
        .iter()
        .map(|n| n.pitch)
        .max()
        .unwrap_or(72)
        .saturating_add(2)
        .max(low.saturating_add(12))
        .min(127);
    let zoom = this.zoom.max(45.);
    let width = (pattern.length_beats.to_f64() as f32 * zoom).max(600.);
    let height = f32::from(high - low + 1) * 13.;
    let channel_ids = project
        .snapshot
        .tracks
        .iter()
        .find(|t| t.id == clip.track_id)
        .map(|t| t.channel_ids.as_slice())
        .unwrap_or(&[]);
    let mut grid = div().relative().w(px(width + 40.)).h(px(height));
    for pitch in low..=high {
        let y = f32::from(high - pitch) * 13.;
        let black = [1, 3, 6, 8, 10].contains(&(pitch % 12));
        let on = channel_ids.iter().any(|id| {
            this.analysis.get(id).is_some_and(|a| {
                let actual = i32::from(pitch) + clip.transpose.unwrap_or(0);
                (0..128).contains(&actual) && a.sounding(actual as u8)
            })
        });
        grid = grid.child(
            div()
                .absolute()
                .top(px(y))
                .left_0()
                .w_full()
                .h(px(13.))
                .bg(rgb(theme.piano_rows[usize::from(black)]))
                .border_b_1()
                .border_color(rgb(theme.border))
                .child(
                    div()
                        .w(px(40.))
                        .h_full()
                        .text_xs()
                        .text_color(rgb(if on {
                            theme.on_accent
                        } else {
                            theme.key_text[usize::from(black)]
                        }))
                        .bg(rgb(if on {
                            theme.accent
                        } else {
                            theme.keys[usize::from(black)]
                        }))
                        .child(format!(
                            "{}{}",
                            [
                                "C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B"
                            ][usize::from(pitch % 12)],
                            i32::from(pitch) / 12 - 1
                        )),
                ),
        );
    }
    for beat in 0..=pattern.length_beats.to_f64().ceil().min(10_000.) as usize {
        grid = grid.child(
            div()
                .absolute()
                .left(px(40. + beat as f32 * zoom))
                .top_0()
                .h_full()
                .w(px(1.))
                .bg(rgb(theme.border)),
        );
    }
    for note in &pattern.notes {
        grid = grid.child(
            div()
                .absolute()
                .left(px(40. + note.start.to_f64() as f32 * zoom))
                .top(px(f32::from(high - note.pitch) * 13. + 1.))
                .w(px((note.duration.to_f64() as f32 * zoom - 1.).max(2.)))
                .h(px(10.))
                .rounded_sm()
                .bg(alpha(theme.accent, 0.3 + note.velocity as f32 * 0.7)),
        );
    }
    let global = project.beat(this.playback.audible);
    if global >= start && global < end {
        let phase = (project.global_to_local(&clip.track_id, global) - clip.start_beat.to_f64())
            .rem_euclid(pattern.length_beats.to_f64());
        grid = grid.child(
            div()
                .absolute()
                .left(px(40. + phase as f32 * zoom))
                .top_0()
                .h_full()
                .w(px(1.))
                .bg(rgb(theme.gold)),
        );
    }
    root.child(
        div()
            .id("piano-scroll")
            .flex_1()
            .min_h_0()
            .overflow_scroll()
            .child(grid),
    )
}
