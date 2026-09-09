//! Search, direct type filters and single-line plugin rows.
use crate::{
    plugin_manager::CatalogEntry,
    ui::Preview,
    ui_icons::{icon, Icon},
};
use gpui::{prelude::*, *};

pub fn search(this: &Preview, cx: &mut Context<Preview>) -> impl IntoElement {
    let t = this.theme;
    let state = &this.document.manager;
    div()
        .id("plugin-search")
        .h(px(28.))
        .flex_1()
        .min_w_0()
        .px_2()
        .flex()
        .items_center()
        .gap_2()
        .border_b_1()
        .border_color(rgb(if state.searching && !state.adding {
            t.accent
        } else {
            t.border
        }))
        .cursor(CursorStyle::IBeam)
        .child(icon(Icon::Search, t.muted))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(12.))
                .text_color(rgb(if state.query.is_empty() {
                    t.muted
                } else {
                    t.text
                }))
                .child(if state.query.is_empty() {
                    "Search".into()
                } else {
                    state.query.clone()
                }),
        )
        .when(!state.query.is_empty(), |d| {
            d.child(
                t.icon_button("plugin-search-clear", Icon::Close, "Clear search")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.document.manager.query.clear();
                        cx.stop_propagation();
                        cx.notify();
                    })),
            )
        })
        .on_click(cx.listener(|this, _, window, cx| {
            this.document.manager.adding = false;
            this.document.manager.searching = true;
            this.document.manager.select_all = true;
            this.workspace_focus.focus(window);
            cx.notify();
        }))
}

pub fn install(this: &Preview, cx: &mut Context<Preview>) -> Div {
    let t = this.theme;
    let state = &this.document.manager;
    div()
        .px_2()
        .pb_2()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .id("plugin-package-input")
                .flex_1()
                .min_w_0()
                .h(px(28.))
                .px_2()
                .flex()
                .items_center()
                .bg(rgb(t.bg))
                .border_b_1()
                .border_color(rgb(t.accent))
                .text_size(px(12.))
                .cursor(CursorStyle::IBeam)
                .child(div().truncate().child(if state.package_input.is_empty() {
                    "package@version".into()
                } else {
                    state.package_input.clone()
                }))
                .on_click(cx.listener(|this, _, window, cx| {
                    this.document.manager.searching = true;
                    this.workspace_focus.focus(window);
                    cx.notify();
                })),
        )
        .child(
            t.button("plugin-install-submit", "Install")
                .on_click(cx.listener(|this, _, _, cx| {
                    this.submit_plugin_package();
                    cx.notify();
                })),
        )
        .child(
            t.icon_button(
                "plugin-install-cancel",
                Icon::Close,
                "Cancel installation input",
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.document.manager.adding = false;
                this.document.manager.searching = false;
                this.document.manager.input_error = None;
                cx.notify();
            })),
        )
}

pub fn filters(this: &Preview, cx: &mut Context<Preview>) -> Div {
    let t = this.theme;
    let mut row = div()
        .h(px(34.))
        .flex_shrink_0()
        .px_2()
        .flex()
        .items_center()
        .gap_1()
        .border_b_1()
        .border_color(rgb(t.border));
    for (index, label) in [(0, "All"), (1, "Instruments"), (2, "Effects")] {
        row = row.child(
            t.tool(
                format!("plugin-type-{index}"),
                label,
                this.document.manager.category == index,
            )
            .relative()
            .child({
                let bounds = this.document.manager.filter_bounds.clone();
                canvas(
                    move |area, _, _| {
                        let mut values = bounds.get();
                        values[index] = area;
                        bounds.set(values);
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full()
            })
            .on_click(cx.listener(move |this, _, window, cx| {
                let state = &mut this.document.manager;
                state.category = index;
                state.section = None;
                state.searching = false;
                state.list_scroll.set_offset(point(px(0.), px(0.)));
                this.workspace_focus.focus(window);
                cx.notify();
            })),
        );
    }
    row
}

pub fn list(this: &Preview, entries: &[&CatalogEntry], cx: &mut Context<Preview>) -> Stateful<Div> {
    let t = this.theme;
    let mut names = std::collections::HashMap::new();
    for entry in entries {
        *names.entry(&entry.display_name).or_insert(0) += 1;
    }
    let mut list = div()
        .id("plugin-list")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .track_scroll(&this.document.manager.list_scroll);
    for entry in entries {
        let handle = entry.handle.clone();
        let selected = this.document.manager.selected.as_ref() == Some(&handle);
        let unavailable = entry.availability != "available";
        list = list.child(
            div()
                .id(SharedString::from(handle.clone()))
                .h(px(30.))
                .flex_shrink_0()
                .px_3()
                .flex()
                .items_center()
                .gap_3()
                .cursor_pointer()
                .bg(rgb(if selected { t.selected } else { t.panel }))
                .hover(move |d| d.bg(rgb(if selected { t.selected } else { t.button })))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(12.))
                        .child(if names[&entry.display_name] > 1 {
                            format!(
                                "{} · {} {}",
                                entry.display_name, entry.vendor, entry.plugin_version
                            )
                        } else {
                            entry.display_name.clone()
                        }),
                )
                .when(unavailable, |d| {
                    d.child(
                        div()
                            .text_size(px(10.))
                            .text_color(rgb(t.danger))
                            .child("Unavailable"),
                    )
                })
                .child(
                    div()
                        .w(px(82.))
                        .flex_shrink_0()
                        .text_size(px(11.))
                        .text_color(rgb(t.muted))
                        .child(if entry.kind == "instrument" {
                            "Instrument"
                        } else {
                            "Effect"
                        }),
                )
                .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                    this.select_library_plugin(handle.clone());
                    if event.click_count() == 2 {
                        this.toggle_library_section(crate::plugin_manager::LibrarySection::Uses);
                    }
                    this.workspace_focus.focus(window);
                    cx.notify();
                })),
        );
    }
    if entries.is_empty() {
        list = list.child(
            div()
                .px_3()
                .py_4()
                .text_size(px(12.))
                .text_color(rgb(t.muted))
                .child("No matching plugins"),
        );
    }
    list
}
