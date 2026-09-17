use crate::{
    plugin_window::{DetailTab, PluginWindow},
    theme::Theme,
};
use gpui::{prelude::*, *};

pub fn view(this: &PluginWindow, width: f32, cx: &mut Context<PluginWindow>) -> impl IntoElement {
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
    let tabs = crate::plugin_panel_navigation::view(this, cx);
    let mut content = div().w_full().flex().flex_col();
    match this.tab {
        DetailTab::Panel => content = crate::plugin_panel::view(this, width, cx),
        DetailTab::Parameters => content = crate::plugin_parameters::view(this, cx),
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
                .child(field(theme, "View", "Native GPUI · Source values".into()));
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
        .when(this.sync_status != "Synced", |d| {
            d.child(
                div()
                    .px_4()
                    .py_1()
                    .text_size(px(10.))
                    .text_color(rgb(theme.muted))
                    .child(this.sync_status.clone()),
            )
        })
        .child(crate::plugin_host_controls::view(this, cx))
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
