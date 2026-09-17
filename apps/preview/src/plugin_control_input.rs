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
        return crate::plugin_choice::view(control, options, parameter, this, enabled, cx);
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
                this.choice_open = None;
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

/// Drag the plotted wavetable to scan its position using the same source transaction as a fader.
pub fn waveform(
    element: Div,
    position: &str,
    this: &PluginWindow,
    cx: &mut Context<PluginWindow>,
) -> Div {
    let Some(p) = this
        .details
        .as_ref()
        .and_then(|d| {
            d.parameters
                .iter()
                .find(|p| !p.host && p.spec.id == position)
        })
        .cloned()
    else {
        return element;
    };
    let enabled = this.owner.upgrade().is_some_and(|owner| {
        owner.read(cx).document_ready()
            && owner
                .read(cx)
                .plugin_configuration_target(&this.target)
                .is_some()
    });
    let target = this.target.clone();
    let identity = this.identity.clone();
    let bounds = this.parameter_bounds.clone();
    let key = format!("wave:{position}");
    element
        .relative()
        .when(enabled, |d| d.cursor(CursorStyle::ResizeLeftRight))
        .child(
            canvas(
                move |area, _, _| {
                    bounds.borrow_mut().insert(key.clone(), area);
                },
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0()
            .size_full(),
        )
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                if !enabled {
                    return;
                }
                this.focus(window);
                this.choice_open = None;
                let _ = this.owner.update(cx, |owner, cx| {
                    if event.click_count == 2 {
                        owner.set_plugin_parameter(&target, &identity, &p, p.spec.default);
                    } else {
                        owner.begin_plugin_parameter(&target, &identity, &p, event, true);
                    }
                    cx.notify();
                });
                cx.stop_propagation();
            }),
        )
}
