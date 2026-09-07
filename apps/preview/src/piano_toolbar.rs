use crate::ui::Preview;
use gpui::{prelude::*, *};
pub fn view(
    this: &Preview,
    width: f32,
    name: String,
    notes: usize,
    length: f64,
    clip: (f64, f64),
    cx: &mut Context<Preview>,
) -> impl IntoElement {
    let theme = this.theme;
    let compact = width < 620.;
    let title = div()
        .min_w_0()
        .map(|d| {
            if compact {
                d.w(px(width - 24.)).flex_shrink_0()
            } else {
                d.flex_1()
            }
        })
        .child(
            div()
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .truncate()
                .child(name),
        )
        .child(
            div()
                .text_size(px(9.))
                .text_color(rgb(theme.muted))
                .truncate()
                .child(format!("Piano roll · {notes} notes · {length:.1} beats")),
        );
    let mut controls = div()
        .flex()
        .items_center()
        .gap_2()
        .flex_shrink_0()
        .when(compact, |d| d.w_full());
    controls = controls.child(theme.button("piano-fit", "Fit").on_click(cx.listener(
        |this, _, _, cx| {
            this.piano.fit();
            cx.notify();
        },
    )));
    for (id, label, horizontal, vertical) in [
        ("piano-out", "−", 0.8, 1.),
        ("piano-in", "+", 1.25, 1.),
        ("piano-keys-out", "Keys −", 1., 0.8),
        ("piano-keys-in", "Keys +", 1., 1.25),
    ] {
        controls = controls.child(theme.button(id, label).on_click(cx.listener(
            move |this, _, _, cx| {
                this.zoom_piano(horizontal, vertical);
                cx.notify();
            },
        )));
    }
    controls =
        controls
            .child(div().flex_1())
            .child(
                theme
                    .button("loop-selection", "Loop clip")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.loop_start = clip.0;
                        this.loop_end = clip.1;
                        this.loop_enabled = true;
                        this.seek(clip.0);
                        if this.playback.playing {
                            this.play();
                        }
                        cx.notify();
                    })),
            );
    div()
        .w(px(width))
        .h(px(if compact { 76. } else { 46. }))
        .flex_shrink_0()
        .px_3()
        .py_1()
        .flex()
        .items_center()
        .gap_2()
        .when(compact, |d| d.flex_col())
        .child(title)
        .child(controls)
}
