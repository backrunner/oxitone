//! Interactive wrappers shared by bundled and third-party declarative controls.
use crate::{plugin_details::ParameterDetail, plugin_layout::Control, plugin_window::PluginWindow};
use gpui::{prelude::*, *};

pub fn view(
    control: &Control,
    parameter: &ParameterDetail,
    this: &PluginWindow,
    cx: &mut Context<PluginWindow>,
) -> Div {
    let theme = this.theme;
    let enabled = this.owner.upgrade().is_some_and(|owner| {
        let owner = owner.read(cx);
        owner.document_ready() && owner.plugin_configuration_target(&this.target).is_some()
    });
    if let Control::Choice { options, .. } = control {
        let mut choices = div()
            .flex_1()
            .min_w_0()
            .p(px(2.))
            .rounded(px(4.))
            .bg(rgb(theme.bg))
            .flex()
            .flex_wrap()
            .gap(px(2.));
        for option in options {
            let p = parameter.clone();
            let target = this.target.clone();
            let identity = this.identity.clone();
            let value = option.value;
            choices = choices.child(
                theme
                    .ghost(
                        format!("choice-{}-{value}", parameter.spec.id),
                        option.label.clone(),
                    )
                    .flex_1()
                    .min_w(px(48.))
                    .text_size(px(10.))
                    .px_1()
                    .h(px(23.))
                    .when(value == parameter.value, |d| {
                        d.bg(rgb(theme.button_hover)).text_color(rgb(theme.text))
                    })
                    .when(!enabled, |d| d.cursor_default())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if !enabled {
                            return;
                        }
                        this.focus(window);
                        let _ = this.owner.update(cx, |owner, cx| {
                            owner.set_plugin_parameter(&target, &identity, &p, value);
                            cx.notify();
                        });
                    })),
            );
        }
        let root = div()
            .min_h(px(34.))
            .px_1()
            .py_1()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .text_size(px(10.))
                    .w(px(76.))
                    .flex_shrink_0()
                    .truncate()
                    .text_color(rgb(theme.muted))
                    .child(control.label().unwrap_or(&parameter.spec.label).to_owned()),
            )
            .child(choices)
            .when(!options.iter().any(|o| o.value == parameter.value), |d| {
                d.child(
                    div()
                        .text_center()
                        .text_size(px(11.))
                        .child(crate::plugin_controls::value(parameter)),
                )
            });
        return record(root, parameter, this);
    }
    let p = parameter.clone();
    let target = this.target.clone();
    let identity = this.identity.clone();
    let horizontal = matches!(control, Control::Fader { .. });
    let toggle = matches!(control, Control::Toggle { .. });
    let editable = enabled && !matches!(control, Control::Readout { .. });
    let root = crate::plugin_controls::control_view(control, parameter, theme)
        .when(editable, |d| {
            d.cursor(if toggle {
                CursorStyle::PointingHand
            } else {
                if horizontal {
                    CursorStyle::ResizeLeftRight
                } else {
                    CursorStyle::ResizeUpDown
                }
            })
        })
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                if !editable {
                    return;
                }
                this.focus(window);
                let _ = this.owner.update(cx, |owner, cx| {
                    if event.click_count == 2 {
                        owner.set_plugin_parameter(&target, &identity, &p, p.spec.default);
                    } else if toggle {
                        owner.set_plugin_parameter(
                            &target,
                            &identity,
                            &p,
                            if p.value >= 0.5 { 0. } else { 1. },
                        );
                    } else {
                        owner.begin_plugin_parameter(&target, &identity, &p, event, horizontal);
                    }
                    cx.notify();
                });
                cx.stop_propagation();
            }),
        );
    record(root, parameter, this)
}

pub fn record(element: impl IntoElement, parameter: &ParameterDetail, this: &PluginWindow) -> Div {
    let bounds = this.parameter_bounds.clone();
    let id = format!(
        "{}:{}",
        if parameter.host { "host" } else { "plugin" },
        parameter.spec.id
    );
    div().relative().child(element).child(
        canvas(
            move |area, _, _| {
                bounds.borrow_mut().insert(id.clone(), area);
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0()
        .size_full(),
    )
}
