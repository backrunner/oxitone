use crate::ui::{alpha, Preview};
use gpui::{prelude::*, *};

pub fn view(this: &Preview, cx: &mut Context<Preview>) -> impl IntoElement {
    let t = this.theme;
    let mut panel = div()
        .w(px(600.))
        .p_5()
        .rounded_lg()
        .bg(rgb(t.panel))
        .border_1()
        .border_color(rgb(t.border))
        .flex()
        .flex_col()
        .gap_3()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Playback & navigation"),
                )
                .child(
                    t.button("close-shortcuts", "Close · Esc")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.show_shortcuts = false;
                            cx.notify();
                        })),
                ),
        );
    for (key, action) in [
        (
            "Click timeline / piano",
            "Locate precisely · keep playback state",
        ),
        ("Double-click / ⌥ click", "Play from pointer position"),
        ("Space", "Play / pause"),
        ("Enter", "Replay from the cue"),
        ("Shift Space / Stop", "Stop and return to cue"),
        ("⌥ ← / →", "Move one beat · add Shift for one bar"),
        ("⌘ Home / End", "Locate project start / end"),
        ("[ / ]", "Previous / next marker"),
        ("L", "Toggle loop"),
        ("G", "Go to bar.beat.tick, seconds or mm:ss"),
        ("Enter / Shift Enter in Go", "Locate / locate and play"),
        ("Scroll / Shift scroll", "Browse vertically / horizontally"),
        ("⌘ scroll", "Zoom timeline · piano Keys ± zoom pitch"),
    ] {
        panel = panel.child(
            div()
                .flex()
                .gap_4()
                .text_size(px(11.))
                .child(
                    div()
                        .w(px(205.))
                        .flex_shrink_0()
                        .text_color(rgb(t.accent))
                        .child(key),
                )
                .child(div().flex_1().child(action)),
        );
    }
    panel = panel.child(div().pt_3().border_t_1().border_color(rgb(t.border)).text_size(px(10.))
        .text_color(rgb(t.muted)).child("⌥ = Option / Alt · ⌘ = Command / Ctrl. Playback keys also work in plugin windows. Locating outside a loop turns it off. Music stays read only."));
    div()
        .id("shortcuts-overlay")
        .absolute()
        .inset_0()
        .bg(alpha(0, 0.4))
        .flex()
        .items_center()
        .justify_center()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, _, cx| {
                this.show_shortcuts = false;
                cx.stop_propagation();
                cx.notify();
            }),
        )
        .child(panel)
}
