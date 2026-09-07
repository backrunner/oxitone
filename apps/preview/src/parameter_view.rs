use crate::{parameter_format::number, plugin_details::ParameterDetail, theme::Theme};
use gpui::{prelude::*, *};

pub fn row(parameter: &ParameterDetail, theme: Theme, specs: bool) -> impl IntoElement {
    let spec = &parameter.spec;
    div()
        .py_2()
        .border_b_1()
        .border_color(rgb(theme.border))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(px(11.))
                        .child(spec.label.clone()),
                )
                .child(div().text_size(px(9.)).text_color(rgb(theme.muted)).child(
                    if parameter.explicit {
                        "Source"
                    } else {
                        "Default"
                    },
                ))
                .when(!parameter.automation.is_empty(), |d| {
                    d.child(
                        div()
                            .text_size(px(9.))
                            .text_color(rgb(theme.gold))
                            .child("Auto"),
                    )
                })
                .child(
                    div()
                        .min_w(px(78.))
                        .text_right()
                        .text_size(px(12.))
                        .text_color(rgb(theme.accent))
                        .child(crate::plugin_controls::value(parameter)),
                ),
        )
        .when(specs, |d| {
            d.child(
                div()
                    .mt_1()
                    .text_size(px(9.))
                    .text_color(rgb(theme.muted))
                    .child(format!(
                        "{}{} · {}…{} · Default {} · {:?} / {:?} / {:?}",
                        if parameter.host { "host." } else { "plugin." },
                        spec.id,
                        number(spec.min),
                        number(spec.max),
                        number(spec.default),
                        spec.mapping
                            .unwrap_or(oxitone_core::wire::ParameterMapping::Linear),
                        spec.rate,
                        spec.smoothing
                    ))
                    .when(!parameter.automation.is_empty(), |d| {
                        d.child(div().mt_1().child(parameter.automation.join(", ")))
                    }),
            )
        })
}
