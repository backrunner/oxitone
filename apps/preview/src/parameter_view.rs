use crate::{
    parameter_format::{number, unit},
    plugin_details::ParameterDetail,
    theme::Theme,
};
use gpui::{prelude::*, *};

pub fn row(parameter: &ParameterDetail, theme: Theme) -> impl IntoElement {
    let spec = &parameter.spec;
    let origin = if parameter.explicit {
        "Source"
    } else {
        "Default"
    };
    let metadata = format!(
        "{} … {} {}  ·  default {}  ·  {:?} / {:?} / {:?}  ·  automatable {}",
        number(spec.min),
        number(spec.max),
        unit(spec.unit),
        number(spec.default),
        spec.mapping
            .unwrap_or(oxitone_core::wire::ParameterMapping::Linear),
        spec.rate,
        spec.smoothing,
        spec.automation.unwrap_or(false)
    );
    div()
        .w_full()
        .p_3()
        .border_b_1()
        .border_color(rgb(theme.border))
        .child(
            div()
                .flex()
                .gap_3()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(spec.label.clone()),
                        )
                        .child(div().text_size(px(10.)).text_color(rgb(theme.muted)).child(
                            format!("{}{}", if parameter.host { "Host · " } else { "" }, spec.id),
                        )),
                )
                .child(
                    div().flex_shrink_0().text_right().child(
                        div()
                            .text_sm()
                            .font_family("Menlo")
                            .text_color(rgb(theme.accent))
                            .child(if parameter.host && spec.id == "bypass" {
                                if parameter.value >= 0.5 {
                                    "On".into()
                                } else {
                                    "Off".into()
                                }
                            } else {
                                format!("{} {}", number(parameter.value), unit(spec.unit))
                            }),
                    ),
                )
                .child(
                    div()
                        .w(px(52.))
                        .text_right()
                        .text_size(px(10.))
                        .text_color(rgb(theme.muted))
                        .child(origin),
                ),
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
                .mt_1()
                .text_size(px(10.))
                .text_color(rgb(theme.muted))
                .child(metadata),
        )
        .when(!parameter.automation.is_empty(), |d| {
            d.child(
                div()
                    .mt_1()
                    .text_size(px(10.))
                    .text_color(rgb(theme.gold))
                    .child(format!("Automation · {}", parameter.automation.join(", "))),
            )
        })
}
