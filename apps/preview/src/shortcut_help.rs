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
                        .child("Keyboard & gestures"),
                )
                .child(
                    t.icon_button(
                        "close-shortcuts",
                        crate::ui_icons::Icon::Close,
                        "Close · Esc",
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.show_shortcuts = false;
                        cx.notify();
                    })),
                ),
        );
    for (key, action) in [
        ("Click timeline / piano ruler", "Set cue position"),
        ("Double-click / ⌥ click", "Play from pointer position"),
        ("Space", "Play / pause"),
        ("Enter", "Replay from the cue"),
        ("Shift Space / Stop", "Stop and return to cue"),
        ("⌥ ← / →", "Move one beat · add Shift for one bar"),
        ("⌘ Home / End", "Project start / end"),
        ("[ / ]", "Previous / next marker"),
        ("L", "Toggle loop"),
        ("Scroll / Shift scroll", "Browse vertically / horizontally"),
        (
            "⌘ scroll / ⌘ Shift scroll",
            "Zoom time / zoom piano key height",
        ),
        ("Middle drag / ⌘ ⌥ drag", "Pan piano roll"),
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
    if this.document.view.is_some() {
        panel = panel.child(
            div()
                .pt_2()
                .border_t_1()
                .border_color(rgb(t.border))
                .child(t.label("PIANO & AUTOMATION")),
        );
        for (key, action) in [
            ("P / B / E", "Draw / paint / select notes"),
            (
                "Draw drag / Shift draw drag",
                "Place note / draw note length",
            ),
            ("F5 / F7 / F9", "Arrange / full piano roll / full mixer"),
            (
                "⌘ click / drag · ⌘ A",
                "Toggle note / box selection · select all",
            ),
            (
                "Shift drag · ⌘ D",
                "Copy notes · duplicate selection after itself",
            ),
            (
                "Arrows · Shift arrows",
                "Move notes · octave / change duration",
            ),
            (
                "Q · Delete · Right-click",
                "Quantize · remove selected notes · erase",
            ),
            (
                "Magnet / ⌥ during note drag",
                "Toggle grid snap or temporarily release it",
            ),
            (
                "⌥ drag curve segment",
                "Bend a curve · Shift for fine point movement",
            ),
            ("Escape · ⌘ Z / Shift Z", "Cancel gesture · undo / redo"),
            (
                "Playlist: Shift drag / ⌘ D",
                "Copy clip / repeat at its end",
            ),
            (
                "Playlist: edge / M / Delete",
                "Change duration / toggle / remove",
            ),
            (
                "Mixer: drag / Shift drag",
                "Adjust level or pan / fine adjustment",
            ),
            ("Track: M / S", "Mute or solo the selected track"),
            ("Mixer: double-click", "Reset level to 0 dB or center pan"),
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
    }
    panel = panel.child(div().pt_3().border_t_1().border_color(rgb(t.border)).text_size(px(10.))
        .text_color(rgb(t.muted)).child("⌥ = Option / Alt · ⌘ = Command / Ctrl. Playback keys also work in plugin windows. Locating outside a loop turns it off."));
    let panel = panel
        .id("shortcut-list")
        .max_h(relative(0.9))
        .overflow_y_scroll();
    div()
        .id("shortcuts-overlay")
        .absolute()
        .inset_0()
        .occlude()
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
