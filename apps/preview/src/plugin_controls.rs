use crate::{
    plugin_details::ParameterDetail, plugin_layout::Control, plugin_window::PluginWindow,
    theme::Theme,
};
use gpui::{prelude::*, *};
use oxitone_core::wire::{ParameterMapping, ParameterUnit};

pub fn control_view(control: &Control, parameter: &ParameterDetail, theme: Theme) -> Div {
    let label = control.label().unwrap_or(&parameter.spec.label);
    let mut root = div().h(px(94.)).px_1().flex().flex_col().items_center();
    root = root.child(
        div()
            .w_full()
            .truncate()
            .text_center()
            .text_size(px(10.))
            .text_color(rgb(theme.muted))
            .child(format!(
                "{label}{}",
                if parameter.automation.is_empty() {
                    ""
                } else {
                    " •"
                }
            )),
    );
    let mut text = value(parameter);
    match control {
        Control::Knob { .. } => {
            root = root.child(crate::plugin_dial::dial(
                parameter.fraction(),
                parameter.spec.mapping == Some(ParameterMapping::Bipolar),
                theme,
            ))
        }
        Control::Fader { .. } => {
            root = root.child(
                div()
                    .h(px(58.))
                    .w_full()
                    .flex()
                    .items_center()
                    .px_2()
                    .child(
                        div()
                            .relative()
                            .h(px(6.))
                            .w_full()
                            .rounded_full()
                            .bg(rgb(theme.border))
                            .child(
                                div()
                                    .h_full()
                                    .w(relative(parameter.fraction()))
                                    .rounded_full()
                                    .bg(rgb(theme.accent)),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .left(relative(parameter.fraction()))
                                    .ml(px(-5.))
                                    .top(px(-5.))
                                    .w(px(10.))
                                    .h(px(16.))
                                    .rounded_sm()
                                    .bg(rgb(theme.button_hover))
                                    .border_1()
                                    .border_color(rgb(theme.muted)),
                            ),
                    ),
            );
        }
        Control::Toggle { .. } => {
            text = if parameter.value >= 0.5 { "On" } else { "Off" }.into();
            root = root.child(
                div().h(px(58.)).flex().items_center().child(
                    div()
                        .w(px(38.))
                        .h(px(18.))
                        .p(px(3.))
                        .flex()
                        .rounded_full()
                        .bg(rgb(if parameter.value >= 0.5 {
                            theme.accent
                        } else {
                            theme.button
                        }))
                        .when(parameter.value >= 0.5, |d| d.justify_end())
                        .child(div().size(px(12.)).rounded_full().bg(rgb(theme.panel))),
                ),
            );
        }
        Control::Choice { options, .. } => {
            text = options
                .iter()
                .find(|o| o.value == parameter.value)
                .map_or_else(|| value(parameter), |o| o.label.clone());
            root = root.child(
                div()
                    .h(px(58.))
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .px_2()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(theme.button))
                            .text_size(px(11.))
                            .child(text.clone()),
                    ),
            );
            text = String::new();
        }
        _ => {
            root = root.child(
                div()
                    .h(px(58.))
                    .flex()
                    .items_center()
                    .text_size(px(21.))
                    .child(text.clone()),
            );
            text = String::new();
        }
    }
    root.child(
        div()
            .text_size(px(11.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(if parameter.automation.is_empty() {
                theme.text
            } else {
                theme.gold
            }))
            .child(text),
    )
}

pub fn value(parameter: &ParameterDetail) -> String {
    let v = parameter.value;
    match parameter.spec.unit {
        ParameterUnit::Hz if v >= 1000. => format!("{:.2} kHz", v / 1000.),
        ParameterUnit::Hz if v < 10. => format!("{v:.2} Hz"),
        ParameterUnit::Hz => format!("{v:.0} Hz"),
        ParameterUnit::Seconds if v < 1. => format!("{:.1} ms", v * 1000.),
        ParameterUnit::Seconds => format!("{v:.2} s"),
        ParameterUnit::Normalized if parameter.spec.min == 0. && parameter.spec.max == 1. => {
            format!("{:.0}%", v * 100.)
        }
        ParameterUnit::Normalized | ParameterUnit::Enum => crate::parameter_format::number(v),
        _ => format!(
            "{} {}",
            crate::parameter_format::number(v),
            crate::parameter_format::unit(parameter.spec.unit)
        ),
    }
}

/// Host wet/dry and bypass remain visible on every effect, outside custom pages and scrolling.
pub fn mix(this: &PluginWindow) -> Div {
    let theme = this.theme;
    let Some(details) = &this.details else {
        return div();
    };
    let Some(mix) = details
        .parameters
        .iter()
        .find(|p| p.host && p.spec.id == "mix")
    else {
        return div();
    };
    let bypass = details
        .parameters
        .iter()
        .find(|p| p.host && p.spec.id == "bypass")
        .is_some_and(|p| p.value >= 0.5);
    div()
        .h(px(42.))
        .flex_shrink_0()
        .px_4()
        .flex()
        .items_center()
        .gap_3()
        .border_b_1()
        .border_color(rgb(theme.border))
        .child(div().size(px(6.)).rounded_full().bg(rgb(if bypass {
            theme.muted
        } else {
            theme.accent
        })))
        .child(div().w(px(60.)).text_size(px(10.)).child(if bypass {
            "Bypassed"
        } else {
            "Enabled"
        }))
        .child(
            div()
                .text_size(px(11.))
                .font_weight(FontWeight::SEMIBOLD)
                .child("Mix"),
        )
        .child(
            div()
                .w(px(45.))
                .text_size(px(13.))
                .text_color(rgb(theme.accent))
                .child(format!("{:.0}%", mix.value * 100.)),
        )
        .child(
            div()
                .flex_1()
                .h(px(4.))
                .rounded_full()
                .bg(rgb(theme.border))
                .child(
                    div()
                        .h_full()
                        .w(relative(mix.fraction()))
                        .rounded_full()
                        .bg(rgb(theme.accent)),
                ),
        )
        .child(
            div()
                .text_size(px(9.))
                .text_color(rgb(theme.muted))
                .child(format!(
                    "{:.0} dry / {:.0} wet{}",
                    (1. - mix.value) * 100.,
                    mix.value * 100.,
                    if mix.automation.is_empty() {
                        ""
                    } else {
                        " • Auto"
                    }
                )),
        )
}
