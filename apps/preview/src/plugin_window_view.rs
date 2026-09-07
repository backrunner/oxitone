use crate::{
    plugin_window::{DetailTab, ParameterFilter, PluginWindow},
    theme::Theme,
};
use gpui::{prelude::*, *};

pub fn header(this: &PluginWindow, window: &Window) -> impl IntoElement {
    let theme = this.theme;
    div()
        .id("plugin-window-drag")
        .window_control_area(WindowControlArea::Drag)
        .h(px(56.))
        .flex_shrink_0()
        .pl(px(if window.is_fullscreen() { 20. } else { 100. }))
        .pr_4()
        .flex()
        .items_center()
        .gap_3()
        .border_b_1()
        .border_color(rgb(theme.border))
        .on_mouse_down(MouseButton::Left, |event, window, cx| {
            if window.is_fullscreen() {
                return;
            }
            if event.click_count == 2 {
                window.titlebar_double_click();
            } else {
                crate::window_chrome::start_drag(window, cx);
            }
        })
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(theme.accent))
                .child("OXITONE"),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_xs()
                .truncate()
                .child(this.title()),
        )
        .child(
            div()
                .text_size(px(9.))
                .text_color(rgb(theme.muted))
                .child("READ ONLY"),
        )
}

pub fn view(this: &PluginWindow, cx: &mut Context<PluginWindow>) -> impl IntoElement {
    let theme = this.theme;
    let Some(details) = &this.details else {
        return div()
            .flex_1()
            .p_5()
            .child(theme.label("This slot is no longer attached"))
            .child(
                div().mt_3().text_sm().text_color(rgb(theme.muted)).child(
                    "The window will update if this slot returns in a successful code build.",
                ),
            );
    };
    let descriptor = &details.info.descriptor;
    let mut tabs = div()
        .flex()
        .gap_2()
        .items_center()
        .px_4()
        .py_2()
        .flex_shrink_0()
        .border_b_1()
        .border_color(rgb(theme.border));
    for (tab, title) in [
        (DetailTab::Parameters, "Parameters"),
        (DetailTab::Resources, "Resources / State"),
        (DetailTab::Plugin, "Plugin info"),
    ] {
        tabs = tabs.child(
            theme
                .button(title, title)
                .when(this.tab == tab, |d| d.bg(rgb(theme.selected)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.tab = tab;
                    this.scroll.set_offset(point(px(0.), px(0.)));
                    cx.notify();
                })),
        );
    }
    tabs = tabs.child(div().flex_1()).child(
        theme
            .button(
                "copy-plugin-reference",
                if this.copied { "Copied" } else { "Copy JSON" },
            )
            .on_click(cx.listener(|this, _, _, cx| {
                if let Some(details) = &this.details {
                    cx.write_to_clipboard(ClipboardItem::new_string(
                        serde_json::to_string_pretty(&details.source).unwrap(),
                    ));
                    this.copied = true;
                    cx.notify();
                }
            })),
    );
    let mut content = div().w_full().flex().flex_col();
    match this.tab {
        DetailTab::Parameters => {
            content = content.child(div().px_4().py_3().text_xs().text_color(rgb(theme.muted))
                .child("Initial values from code and plugin defaults. Automation can change playback values."));
            let mut filters = div().px_4().pb_2().flex().gap_2();
            for (filter, label) in [
                (ParameterFilter::All, "All"),
                (ParameterFilter::Source, "In source"),
                (ParameterFilter::Automated, "Automated"),
            ] {
                filters = filters.child(
                    theme
                        .button(label, label)
                        .when(this.filter == filter, |d| d.bg(rgb(theme.selected)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.filter = filter;
                            this.scroll.set_offset(point(px(0.), px(0.)));
                            cx.notify();
                        })),
                );
            }
            content = content.child(filters);
            let mut count = 0;
            for parameter in details.parameters.iter().filter(|p| match this.filter {
                ParameterFilter::All => true,
                ParameterFilter::Source => p.explicit,
                ParameterFilter::Automated => !p.automation.is_empty(),
            }) {
                count += 1;
                content = content.child(crate::parameter_view::row(parameter, theme));
            }
            if count == 0 {
                content = content.child(
                    div()
                        .p_4()
                        .text_sm()
                        .text_color(rgb(theme.muted))
                        .child("No parameters in this view"),
                );
            }
        }
        DetailTab::Resources => content = crate::plugin_resources::view(this),
        DetailTab::Plugin => {
            content = content
                .p_4()
                .gap_2()
                .child(field(theme, "Plugin ID", descriptor.plugin_id.clone()))
                .child(field(theme, "Version", descriptor.plugin_version.clone()))
                .child(field(theme, "Type", format!("{:?}", descriptor.kind)))
                .child(field(
                    theme,
                    "Audio layout",
                    format!(
                        "{:?} → {:?}",
                        descriptor.input_layout, descriptor.output_layout
                    ),
                ))
                .child(field(
                    theme,
                    "Sidechain / Tail",
                    format!(
                        "{} / {}",
                        descriptor.capabilities.sidechain_input,
                        descriptor.capabilities.reports_tail
                    ),
                ))
                .child(field(
                    theme,
                    "Polyphony limit",
                    descriptor
                        .max_polyphony
                        .map_or_else(|| "Not declared".into(), |v| v.to_string()),
                ))
                .child(field(
                    theme,
                    "State schema",
                    descriptor
                        .state_schema
                        .map_or_else(|| "None".into(), |v| v.0.into()),
                ))
                .child(field(theme, "View", "Host parameter view".into()));
            if let Some(library) = &details.info.library {
                content = content
                    .child(field(
                        theme,
                        "Source",
                        "Dynamic library · Audio ABI v1".into(),
                    ))
                    .child(field(theme, "Library", library.path.clone()))
                    .child(field(theme, "SHA-256", library.sha256.clone()));
            } else {
                content = content.child(field(theme, "Source", "Built-in Rust plugin".into()));
            }
            for (label, value) in &details.settings {
                content = content.child(field(theme, label, value.clone()));
            }
        }
    }
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .child(
            div()
                .px_4()
                .py_3()
                .flex_shrink_0()
                .bg(rgb(theme.panel))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .text_lg()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(details.name.clone()),
                        )
                        .child(div().text_xs().text_color(rgb(theme.muted)).child(format!(
                            "v{} · {}",
                            descriptor.plugin_version,
                            if details.info.library.is_some() {
                                "Dynamic"
                            } else {
                                "Built-in"
                            }
                        ))),
                )
                .child(
                    div()
                        .mt_1()
                        .text_xs()
                        .text_color(rgb(theme.muted))
                        .child(format!(
                            "{} · {} · {} parameters",
                            details.owner_name,
                            details.target.label(),
                            details.parameters.len()
                        )),
                ),
        )
        .child(tabs)
        .child(
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .child(
                    div()
                        .id("plugin-detail-scroll")
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .overflow_y_scroll()
                        .track_scroll(&this.scroll)
                        .child(content),
                )
                .child(crate::plugin_scroll::view(this, cx)),
        )
}

pub fn field(theme: Theme, label: &str, value: String) -> Div {
    div()
        .flex()
        .gap_3()
        .py_1()
        .border_b_1()
        .border_color(rgb(theme.border))
        .child(
            div()
                .w(px(118.))
                .flex_shrink_0()
                .text_size(px(11.))
                .text_color(rgb(theme.muted))
                .child(label.to_owned()),
        )
        .child(div().flex_1().min_w_0().text_size(px(11.)).child(value))
}
