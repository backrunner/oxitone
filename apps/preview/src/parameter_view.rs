use crate::{
    parameter_format::{number, unit},
    plugin_details::ParameterDetail,
    theme::Theme,
    ui::alpha,
};
use gpui::{prelude::*, *};

pub fn row(parameter: &ParameterDetail, theme: Theme, specs: bool) -> impl IntoElement {
    let spec = &parameter.spec;
    let origin = if parameter.explicit {
        "In source"
    } else {
        "Default"
    };
    div()
        .w_full()
        .min_h(px(126.))
        .p_3()
        .rounded_lg()
        .bg(rgb(theme.panel))
        .border_1()
        .border_color(alpha(theme.border, 0.6))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(px(11.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(spec.label.clone()),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .px_2()
                        .py(px(2.))
                        .rounded_full()
                        .bg(rgb(theme.button))
                        .text_size(px(8.))
                        .text_color(rgb(theme.muted))
                        .child(origin),
                ),
        )
        .child(
            div()
                .mt_3()
                .flex()
                .items_baseline()
                .gap_2()
                .child(
                    div()
                        .text_size(px(22.))
                        .font_family("Menlo")
                        .text_color(rgb(theme.accent))
                        .child(if parameter.host && spec.id == "bypass" {
                            if parameter.value >= 0.5 {
                                "On".into()
                            } else {
                                "Off".into()
                            }
                        } else {
                            number(parameter.value)
                        }),
                )
                .child(div().text_size(px(10.)).text_color(rgb(theme.muted)).child(
                    if parameter.host && spec.id == "bypass" {
                        ""
                    } else {
                        unit(spec.unit)
                    },
                )),
        )
        .child(
            div()
                .mt_2()
                .h(px(3.))
                .rounded_full()
                .bg(rgb(theme.border))
                .child(
                    div()
                        .h_full()
                        .w(relative(parameter.fraction()))
                        .rounded_full()
                        .bg(rgb(theme.accent)),
                ),
        )
        .child(
            div()
                .mt_2()
                .flex()
                .justify_between()
                .gap_2()
                .text_size(px(9.))
                .text_color(rgb(theme.muted))
                .child(format!("{} … {}", number(spec.min), number(spec.max)))
                .child(format!("Default {}", number(spec.default))),
        )
        .when(!parameter.automation.is_empty(), |d| {
            d.child(
                div()
                    .mt_2()
                    .text_size(px(9.))
                    .text_color(rgb(theme.gold))
                    .child(if specs {
                        format!("Automation · {}", parameter.automation.join(", "))
                    } else {
                        "Automated".into()
                    }),
            )
        })
        .when(specs, |d| {
            d.child(
                div()
                    .mt_2()
                    .pt_2()
                    .border_t_1()
                    .border_color(alpha(theme.border, 0.5))
                    .text_size(px(9.))
                    .text_color(rgb(theme.muted))
                    .child(format!(
                        "{}{}",
                        if parameter.host { "Host · " } else { "" },
                        spec.id
                    ))
                    .child(div().mt_1().child(format!(
                            "Mapping {:?} · Rate {:?} · Smoothing {:?} · Automatable {}",
                            spec.mapping
                                .unwrap_or(oxitone_core::wire::ParameterMapping::Linear),
                            spec.rate,
                            spec.smoothing,
                            spec.automation.unwrap_or(false)
                        ))),
            )
        })
}
