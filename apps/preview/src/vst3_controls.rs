//! Reusable text fields and paged normalized VST3 parameter controls.
use crate::{plugin_manager::CatalogEntry, ui::Preview, vst3_model::Field};
use gpui::{prelude::*, *};

pub fn bounds(this: &Preview, key: &str) -> impl IntoElement {
    let bounds = this.document.manager.vst3.bounds.clone();
    let key = key.to_owned();
    canvas(
        move |area, _, _| {
            bounds.borrow_mut().insert(key.clone(), area);
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
    .size_full()
}

pub fn field(this: &Preview, label: &str, id: Field, value: String, cx: &Context<Preview>) -> Div {
    let active = this
        .document
        .manager
        .vst3
        .input
        .as_ref()
        .filter(|(key, _)| *key == id);
    let text = active.map_or(value.clone(), |(_, text)| text.clone());
    let t = this.theme;
    div()
        .flex()
        .flex_shrink_0()
        .items_center()
        .gap_2()
        .min_w_0()
        .child(
            div()
                .w(px(90.))
                .flex_shrink_0()
                .text_size(px(11.))
                .text_color(rgb(t.muted))
                .child(label.to_owned()),
        )
        .child(
            div()
                .id(SharedString::from(format!("vst3-field-{label}")))
                .relative()
                .child(bounds(this, &format!("field-{id:?}")))
                .flex_1()
                .min_w_0()
                .h(px(27.))
                .px_2()
                .bg(rgb(t.bg))
                .border_b_1()
                .border_color(rgb(if active.is_some() { t.accent } else { t.border }))
                .cursor(CursorStyle::IBeam)
                .text_size(px(12.))
                .child(div().truncate().child(text))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.start_vst3_field(id, value.clone());
                    this.document.manager.searching = false;
                    this.workspace_focus.focus(window);
                    cx.notify();
                })),
        )
}

pub fn parameters(this: &Preview, entry: &CatalogEntry, cx: &Context<Preview>) -> Div {
    let catalog = entry.vst3.as_ref().unwrap();
    let t = this.theme;
    let state = &this.document.manager.vst3;
    let draft = state.drafts.get(&entry.handle).cloned().unwrap_or_default();
    let mut body = div().flex().flex_col().gap_2().flex_shrink_0();
    let pages = catalog.parameters.len().div_ceil(32).max(1);
    let page = state.page.min(pages - 1);
    let mut header = div()
        .flex()
        .items_center()
        .gap_2()
        .text_size(px(11.))
        .child(format!(
            "Parameters · {} · page {}/{}",
            catalog.parameters.len(),
            page + 1,
            pages
        ));
    for (label, next) in [
        ("Previous", page.saturating_sub(1)),
        ("Next", (page + 1).min(pages - 1)),
    ] {
        header = header.child(
            t.ghost(format!("vst3-page-{label}"), label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.document.manager.vst3.page = next;
                    cx.notify();
                })),
        );
    }
    body = body.child(header);
    for parameter in catalog.parameters.iter().skip(page * 32).take(32) {
        let value = draft
            .parameters
            .get(&parameter.id.to_string())
            .copied()
            .unwrap_or(parameter.value);
        let label = format!("{} ({})", parameter.name, parameter.id);
        let mut row = div().flex().items_center().gap_2().min_w_0();
        if parameter.read_only {
            row = row.child(
                div()
                    .flex_1()
                    .text_size(px(11.))
                    .child(format!("{label}: {value:.5} · read-only")),
            );
        } else {
            row = row.child(
                field(
                    this,
                    &label,
                    Field::Parameter(parameter.id),
                    value.to_string(),
                    cx,
                )
                .flex_1(),
            );
            let id = parameter.id.to_string();
            let default = parameter.default;
            let handle = entry.handle.clone();
            row = row.child(
                t.ghost(format!("vst3-default-{id}"), "Reset")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.document
                            .manager
                            .vst3
                            .drafts
                            .entry(handle.clone())
                            .or_default()
                            .parameters
                            .insert(id.clone(), default);
                        this.document.manager.vst3.input = None;
                        cx.notify();
                    })),
            );
        }
        let detail = if parameter.step_count > 0 {
            format!("{} values", i64::from(parameter.step_count) + 1)
        } else {
            "continuous".into()
        };
        body = body
            .child(row)
            .child(
                div()
                    .text_size(px(10.))
                    .text_color(rgb(t.muted))
                    .child(format!(
                        "{detail}{}",
                        if parameter.can_automate {
                            " · automatable"
                        } else {
                            ""
                        }
                    )),
            );
    }

    body
}
