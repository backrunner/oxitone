//! Responsive panel cards and progressive disclosure, separate from panel navigation.
use crate::{
    plugin_layout::{Control, Group},
    plugin_synth_visibility as synth,
    plugin_window::PluginWindow,
};
use gpui::{prelude::*, *};

pub fn view(
    this: &PluginWindow,
    groups: &[Group],
    width: f32,
    cx: &mut Context<PluginWindow>,
) -> Div {
    let t = this.theme;
    let details = this.details.as_ref().unwrap();
    let key = (
        details.info.descriptor.plugin_id.clone(),
        details.info.descriptor.plugin_version.clone(),
    );
    let builtin = !this.project.panels.layouts.contains_key(&key);
    let synth = builtin && synth::bundled(details);
    let group_width = if builtin && width >= 720. && groups.len() > 1 {
        (width - 48.) * 0.5
    } else {
        width - 40.
    };
    let mut cards = div().flex().flex_wrap().gap_2().items_start();
    for group in groups {
        if synth && !synth::group_visible(details, &group.id) {
            continue;
        }
        let cols = group
            .columns
            .min(((group_width - 18.) / 82.).floor().max(1.) as u32);
        let expanded = this.expanded_groups.contains(&group.id);
        let mut rows = Vec::new();
        let mut cells = Vec::new();
        let mut extras = Vec::new();
        let mut choices = Vec::new();
        for original in &group.controls {
            if !original.visual() {
                let id = original.bindings()[0];
                if builtin
                    && ((!this.plots.is_empty()
                        && !crate::plugin_time_plot::active_parameter(details, id))
                        || (synth && !synth::active(details, id)))
                {
                    continue;
                }
                if synth && synth::advanced(id) {
                    extras.push(original);
                    continue;
                }
            }
            let control = if synth && !original.visual() {
                synth::control(original)
            } else {
                original.clone()
            };
            if synth && group.id.starts_with("osc") && matches!(control, Control::Choice { .. }) {
                flush_row(&mut rows, &mut cells, cols);
                if let Some(p) = details
                    .parameters
                    .iter()
                    .find(|p| !p.host && p.spec.id == control.bindings()[0])
                {
                    choices.push(crate::plugin_control_input::view(&control, p, this, cx));
                    if choices.len() == 2 {
                        flush_row(&mut rows, &mut choices, 2);
                    }
                }
                continue;
            }
            flush_row(&mut rows, &mut choices, 2);
            append(&control, this, cols, &mut rows, &mut cells, cx);
        }
        flush_row(&mut rows, &mut choices, 2);
        flush_row(&mut rows, &mut cells, cols);
        if !extras.is_empty() {
            let id = group.id.clone();
            let label = if id.starts_with("osc") {
                "Tuning & phase"
            } else if id.starts_with("route") {
                "Response curve"
            } else {
                "Curve & timing"
            };
            rows.push(
                div()
                    .mt_1()
                    .border_t_1()
                    .border_color(crate::ui::alpha(t.border, 0.5))
                    .child(
                        t.ghost(format!("details-{id}"), label)
                            .w_full()
                            .justify_between()
                            .h(px(28.))
                            .text_color(rgb(t.muted))
                            .child(crate::ui_icons::icon(
                                if expanded {
                                    crate::ui_icons::Icon::ChevronUp
                                } else {
                                    crate::ui_icons::Icon::ChevronDown
                                },
                                t.muted,
                            ))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if !this.expanded_groups.remove(&id) {
                                    this.expanded_groups.insert(id.clone());
                                }
                                cx.notify();
                            })),
                    ),
            );
            if expanded {
                for control in extras {
                    append(control, this, cols, &mut rows, &mut cells, cx);
                }
                flush_row(&mut rows, &mut cells, cols);
            }
        }
        cards = cards.child(
            div()
                .flex_1()
                .flex_basis(px((cols as f32 * 82. + 18.).max((width - 48.) * 0.5)))
                .min_w(px(cols as f32 * 82. + 18.))
                .when(builtin, |d| {
                    d.flex_none()
                        .flex_basis(px(group_width))
                        .w(px(group_width))
                        .min_w_0()
                })
                .rounded(px(7.))
                .bg(rgb(t.panel))
                .border_1()
                .border_color(crate::ui::alpha(t.border, 0.65))
                .child(
                    div()
                        .h(px(32.))
                        .px_3()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_size(px(11.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(div().w(px(3.)).h(px(12.)).rounded_full().bg(rgb(
                            if group.id == "oscB" {
                                t.secondary
                            } else {
                                t.accent
                            },
                        )))
                        .child(group.title.clone()),
                )
                .child(div().px_2().pb_2().flex().flex_col().gap_1().children(rows)),
        );
    }
    cards
}

fn append(
    control: &Control,
    this: &PluginWindow,
    cols: u32,
    rows: &mut Vec<Div>,
    cells: &mut Vec<Div>,
    cx: &mut Context<PluginWindow>,
) {
    let details = this.details.as_ref().unwrap();
    if control.visual() {
        flush_row(rows, cells, cols);
        let mut visual_theme = this.theme;
        if matches!(control, Control::Oscillator { position, .. } if position.starts_with("oscB."))
        {
            visual_theme.accent = this.theme.secondary;
            visual_theme.secondary = this.theme.accent;
        }
        let visual = crate::plugin_visuals::view(
            control,
            details,
            visual_theme,
            this.project.snapshot.sample_rate as f64,
            this.stacked_waveforms,
        );
        rows.push(if let Control::Oscillator { position, .. } = control {
            crate::plugin_control_input::waveform(visual, position, this, cx)
        } else {
            visual
        });
    } else if let Some(parameter) = details
        .parameters
        .iter()
        .find(|p| !p.host && p.spec.id == control.bindings()[0])
    {
        let cell = crate::plugin_control_input::view(control, parameter, this, cx);
        if matches!(control, Control::Choice { .. } | Control::Fader { .. }) {
            flush_row(rows, cells, cols);
            rows.push(cell);
        } else {
            cells.push(cell);
            if cells.len() == cols as usize {
                flush_row(rows, cells, cols);
            }
        }
    }
}

fn flush_row(rows: &mut Vec<Div>, cells: &mut Vec<Div>, _columns: u32) {
    if cells.is_empty() {
        return;
    }
    let mut row = div().w_full().flex().gap_2();
    for cell in cells.drain(..) {
        row = row.child(div().flex_1().min_w_0().child(cell));
    }
    rows.push(row);
}
