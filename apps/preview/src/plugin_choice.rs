//! Compact enum picker. Only the open picker reveals alternatives and waveform previews.
use crate::{
    plugin_details::ParameterDetail,
    plugin_layout::{Choice, Control},
    plugin_window::PluginWindow,
    ui_icons::{icon, Icon},
};
use gpui::{prelude::*, *};

pub fn view(
    control: &Control,
    options: &[Choice],
    parameter: &ParameterDetail,
    this: &PluginWindow,
    enabled: bool,
    cx: &mut Context<PluginWindow>,
) -> Div {
    let t = this.theme;
    let id = parameter.spec.id.clone();
    let open = this.choice_open.as_ref() == Some(&id);
    let selected = options
        .iter()
        .find(|o| o.value == parameter.value)
        .map_or_else(
            || crate::plugin_controls::value(parameter),
            |o| o.label.clone(),
        );
    let mut button = t
        .ghost(format!("select-{id}"), "")
        .w_full()
        .h(px(32.))
        .px_2()
        .gap_2()
        .justify_start()
        .bg(rgb(t.bg))
        .rounded(px(5.))
        .child(
            div()
                .text_color(rgb(t.muted))
                .text_size(px(10.))
                .child(control.label().unwrap_or(&parameter.spec.label).to_owned()),
        );
    button = button.child(div().flex_1());
    if let Some(preview) = preview(this, &id, parameter.value) {
        button = button.child(preview.w(px(40.)).h(px(20.)));
    }
    button = button
        .child(div().truncate().child(selected))
        .child(icon(
            if open {
                Icon::ChevronUp
            } else {
                Icon::ChevronDown
            },
            t.muted,
        ))
        .on_click(cx.listener(move |this, _, window, cx| {
            this.focus(window);
            this.choice_open = if open { None } else { Some(id.clone()) };
            cx.notify();
        }));
    let mut root = div()
        .w_full()
        .py_1()
        .child(crate::plugin_control_input::record(button, parameter, this));
    if !open {
        return root;
    }
    let mut choices = div()
        .mt_1()
        .p_1()
        .flex()
        .flex_wrap()
        .gap_1()
        .rounded(px(5.))
        .bg(rgb(t.bg));
    for option in options {
        let p = parameter.clone();
        let target = this.target.clone();
        let identity = this.identity.clone();
        let value = option.value;
        let bounds = this.parameter_bounds.clone();
        let key = format!("choice:{}:{value}", p.spec.id);
        let mut tile = t
            .tool(
                format!("choice-{}-{value}", p.spec.id),
                "",
                value == p.value,
            )
            .flex_1()
            .min_w(px(84.))
            .h(px(30.))
            .gap_2()
            .px_2()
            .relative()
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
            );
        if let Some(preview) = preview(this, &p.spec.id, value) {
            tile = tile
                .h(px(58.))
                .flex_col()
                .gap_1()
                .py_1()
                .child(preview.w_full().h(px(26.)));
        }
        choices = choices.child(
            tile.child(option.label.clone())
                .when(!enabled, |d| d.cursor_default())
                .on_click(cx.listener(move |this, _, window, cx| {
                    if !enabled {
                        return;
                    }
                    this.focus(window);
                    this.choice_open = None;
                    let _ = this.owner.update(cx, |owner, cx| {
                        owner.set_plugin_parameter(&target, &identity, &p, value);
                        cx.notify();
                    });
                    cx.notify();
                })),
        );
    }
    root = root.child(choices);
    root
}

fn preview(this: &PluginWindow, id: &str, value: f64) -> Option<Div> {
    let details = this.details.as_ref()?;
    if !crate::plugin_synth_visibility::bundled(details) {
        return None;
    }
    let samples: Vec<f32> = if id.ends_with(".wavetable") || id.ends_with(".morphTo") {
        oxitone_instruments::wavetable::preview_cycle(
            0,
            value as usize,
            value as usize,
            0.,
            0.,
            0,
            0.,
        )
        .to_vec()
    } else if id == "sub.wave" {
        oxitone_instruments::wavetable::preview_sub_cycle(value as usize).to_vec()
    } else if id.ends_with(".shape") {
        (0..=64)
            .map(|i| {
                oxitone_instruments::wavetable::lfo_value(value as usize, i as f64 / 64.) as f32
            })
            .collect()
    } else {
        return None;
    };
    let color = this.theme.accent;
    Some(
        div().child(
            canvas(
                |_, _, _| {},
                move |at, _, window, _| {
                    crate::plugin_visuals::trace(
                        at,
                        samples.iter().enumerate().map(|(i, v)| {
                            (
                                0.04 + i as f32 / (samples.len() - 1) as f32 * 0.92,
                                0.5 - v * 0.38,
                            )
                        }),
                        rgb(color).into(),
                        1.5,
                        window,
                    );
                },
            )
            .size_full(),
        ),
    )
}
