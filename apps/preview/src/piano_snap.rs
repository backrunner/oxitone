//! Direct grid selection stays out of the piano toolbar's layout and scroll clipping.
use crate::{
    piano_layout::Snap,
    ui::Preview,
    ui_icons::{icon, Icon},
};
use gpui::{prelude::*, *};

pub fn menu(this: &Preview, cx: &mut Context<Preview>) -> impl IntoElement {
    let t = this.theme;
    let mut menu = t
        .surface()
        .id("piano-snap-menu")
        .occlude()
        .w(px(148.))
        .p_1()
        .flex()
        .flex_col()
        .on_mouse_down_out(cx.listener(|this, _, _, cx| {
            this.piano.snap_menu = None;
            cx.notify();
            cx.stop_propagation();
        }));
    for (index, snap) in [
        Snap::Bar,
        Snap::Beat,
        Snap::Half,
        Snap::Quarter,
        Snap::Eighth,
        Snap::Free,
    ]
    .into_iter()
    .enumerate()
    {
        let selected = this.piano.snap == snap;
        menu = menu.child(
            t.tool(format!("snap-value-{index}"), snap.label(), selected)
                .justify_between()
                .when(selected, |d| d.child(icon(Icon::Check, t.accent)))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.piano.snap = snap;
                    this.piano.snap_menu = None;
                    this.piano_focus.focus(window);
                    cx.notify();
                    cx.stop_propagation();
                })),
        );
    }
    deferred(
        anchored()
            .position(this.piano.snap_menu.unwrap_or_default())
            .snap_to_window_with_margin(px(8.))
            .child(menu),
    )
    .with_priority(1)
}
