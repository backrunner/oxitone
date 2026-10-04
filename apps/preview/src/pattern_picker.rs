//! A scrollable pattern dropdown anchored to the titlebar selector.
use crate::{
    pattern_navigation::patterns,
    ui::Preview,
    ui_icons::{icon, Icon},
};
use gpui::{prelude::*, *};

pub fn menu(this: &Preview, cx: &mut Context<Preview>) -> impl IntoElement {
    let t = this.theme;
    let mut list = div()
        .id("pattern-picker-list")
        .max_h(px(320.))
        .overflow_y_scroll()
        .track_scroll(&this.pattern_picker.scroll)
        .flex()
        .flex_col();
    if let Some(project) = &this.project {
        for (index, pattern) in patterns(project).into_iter().enumerate() {
            let id = pattern.id.clone();
            let active = this.active_pattern().is_some_and(|p| p.id == id);
            list = list.child(
                t.tool(
                    format!("pick-pattern-{index}"),
                    "",
                    this.pattern_picker.selected == Some(index),
                )
                .h(px(30.))
                .flex_shrink_0()
                .justify_start()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .child(project.pattern_label(&id)),
                )
                .when(active, |d| d.child(icon(Icon::Check, t.accent)))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.select_pattern(&id);
                    this.pattern_picker.selected = None;
                    this.workspace_focus.focus(window);
                    cx.stop_propagation();
                    cx.notify();
                })),
            );
        }
    }
    let button = this.pattern_picker.button.get();
    deferred(
        div()
            .absolute()
            .inset_0()
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.pattern_picker.selected = None;
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .child(
                anchored()
                    .position(point(button.left(), button.bottom() + px(6.)))
                    .snap_to_window_with_margin(px(8.))
                    .child(
                        t.surface()
                            .w(px(240.))
                            .p_1()
                            .occlude()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .child(list),
                    ),
            ),
    )
    .with_priority(1)
}
