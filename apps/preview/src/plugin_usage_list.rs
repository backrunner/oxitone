//! Instance navigation is revealed only when requested from the library.
use crate::{plugin_manager::CatalogEntry, ui::Preview};
use gpui::{prelude::*, *};

pub fn view(this: &Preview, entry: &CatalogEntry, cx: &mut Context<Preview>) -> Div {
    let t = this.theme;
    let mut list = div().flex().flex_col();
    for (index, usage) in entry.usages.iter().enumerate() {
        let target = usage.target();
        let edit_target = target.clone();
        list = list.child(
            div()
                .h(px(30.))
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(12.))
                        .child(usage.label.clone()),
                )
                .when(usage.kind != "instrument", |d| {
                    d.child(
                        div()
                            .text_size(px(10.))
                            .text_color(rgb(t.muted))
                            .child(format!("Insert {}", usage.index + 1)),
                    )
                })
                .child(
                    t.ghost(format!("plugin-use-{index}"), "Edit")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.edit_plugin_source(&edit_target, window, cx);
                        })),
                )
                .child(
                    t.ghost(format!("plugin-window-{index}"), "Open")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_plugin(target.clone(), cx);
                            cx.notify();
                        })),
                ),
        );
    }
    if entry.usages.is_empty() {
        list = list.child(
            div()
                .py_2()
                .text_size(px(12.))
                .text_color(rgb(t.muted))
                .child("Not used in this project"),
        );
    }
    list
}
