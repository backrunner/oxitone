//! Persistent host mix/bypass controls, separate from the plugin parameter namespace.
use crate::plugin_window::PluginWindow;
use gpui::{prelude::*, *};

pub fn view(this: &PluginWindow, cx: &mut Context<PluginWindow>) -> Div {
    let t = this.theme;
    let Some(details) = &this.details else {
        return div();
    };
    let Some(mix) = details
        .parameters
        .iter()
        .find(|p| p.host && p.spec.id == "mix")
        .cloned()
    else {
        return div();
    };
    let Some(bypass) = details
        .parameters
        .iter()
        .find(|p| p.host && p.spec.id == "bypass")
        .cloned()
    else {
        return div();
    };
    let target = this.target.clone();
    let mix_target = this.target.clone();
    let identity = this.identity.clone();
    let mix_identity = this.identity.clone();
    let mix_readout = mix.clone();
    let bypass_readout = bypass.clone();
    let enabled = this.owner.upgrade().is_some_and(|owner| {
        owner.read(cx).document_ready()
            && owner
                .read(cx)
                .plugin_configuration_target(&this.target)
                .is_some()
    });
    div()
        .h(px(40.))
        .flex_shrink_0()
        .px_3()
        .flex()
        .items_center()
        .gap_3()
        .border_b_1()
        .border_color(rgb(t.border))
        .child(crate::plugin_control_input::record(
            t.ghost(
                "host-bypass",
                if bypass.value >= 0.5 {
                    "Bypassed"
                } else {
                    "Enabled"
                },
            )
            .text_color(rgb(if bypass.value >= 0.5 {
                t.muted
            } else {
                t.accent
            }))
            .when(!enabled, |d| d.cursor_default())
            .on_click(cx.listener(move |this, _, window, cx| {
                if !enabled {
                    return;
                }
                this.focus(window);
                let _ = this.owner.update(cx, |owner, cx| {
                    owner.set_plugin_parameter(
                        &target,
                        &identity,
                        &bypass,
                        if bypass.value >= 0.5 { 0. } else { 1. },
                    );
                    cx.notify();
                });
            })),
            &bypass_readout,
            this,
        ))
        .child(
            div()
                .text_size(px(10.))
                .text_color(rgb(t.muted))
                .child("Mix"),
        )
        .child(
            div()
                .text_size(px(12.))
                .w(px(42.))
                .child(format!("{:.0}%", mix.value * 100.)),
        )
        .child(
            crate::plugin_control_input::record(
                div()
                    .h(px(24.))
                    .flex()
                    .items_center()
                    .when(enabled, |d| d.cursor(CursorStyle::ResizeLeftRight))
                    .child(
                        div()
                            .h(px(4.))
                            .w_full()
                            .rounded_full()
                            .bg(rgb(t.border))
                            .child(
                                div()
                                    .h_full()
                                    .w(relative(mix.fraction()))
                                    .rounded_full()
                                    .bg(rgb(t.accent)),
                            ),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                            if !enabled {
                                return;
                            }
                            this.focus(window);
                            let _ = this.owner.update(cx, |owner, cx| {
                                if event.click_count == 2 {
                                    owner.set_plugin_parameter(
                                        &mix_target,
                                        &mix_identity,
                                        &mix,
                                        1.,
                                    );
                                } else {
                                    owner.begin_plugin_parameter(
                                        &mix_target,
                                        &mix_identity,
                                        &mix,
                                        event,
                                        true,
                                    );
                                }
                                cx.notify();
                            });
                            cx.stop_propagation();
                        }),
                    ),
                &mix_readout,
                this,
            )
            .flex_1(),
        )
}
