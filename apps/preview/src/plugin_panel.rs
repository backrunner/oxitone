use crate::plugin_window::PluginWindow;
use gpui::{prelude::*, *};

pub fn view(this: &PluginWindow, width: f32, cx: &mut Context<PluginWindow>) -> Div {
    let theme = this.theme;
    let Some(layout) = &this.panel else {
        return div();
    };
    let Some(details) = &this.details else {
        return div();
    };
    let parameter = |id: &str| {
        details
            .parameters
            .iter()
            .find(|p| !p.host && p.spec.id == id)
    };
    let mut content = div().w_full().p_3().flex().flex_col().gap_2();
    let key = (
        details.info.descriptor.plugin_id.clone(),
        details.info.descriptor.plugin_version.clone(),
    );
    if let Some(error) = this.project.panels.error(&key) {
        content = content.child(
            div()
                .px_2()
                .py_1()
                .text_size(px(10.))
                .bg(rgb(theme.diagnostic_bg))
                .text_color(rgb(theme.diagnostic_text))
                .child(format!(
                    "{error} · {}",
                    if this.project.panels.layouts.contains_key(&key) {
                        "Last valid layout"
                    } else {
                        "Default layout"
                    }
                )),
        );
    }
    let mut toolbar = div().flex().flex_wrap().items_center().gap_2();
    if layout.pages.len() > 1 {
        for page in &layout.pages {
            let id = page.id.clone();
            toolbar = toolbar.child(
                theme
                    .tool(
                        format!("panel-page-{id}"),
                        page.title.clone(),
                        this.page == id,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.page = id.clone();
                        this.scroll.set_offset(point(px(0.), px(0.)));
                        cx.notify();
                    })),
            );
        }
    }
    let Some(page) = layout
        .pages
        .iter()
        .find(|p| p.id == this.page)
        .or(layout.pages.first())
    else {
        return content;
    };
    let oscillator = page
        .groups
        .iter()
        .flat_map(|g| &g.controls)
        .any(|c| matches!(c, crate::plugin_layout::Control::Oscillator { .. }));
    if oscillator {
        toolbar = toolbar.child(
            div().flex().flex_1().justify_end().child(
                theme
                    .button(
                        "wave-view",
                        if this.stacked_waveforms {
                            "3D waveform"
                        } else {
                            "2D waveform"
                        },
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.stacked_waveforms = !this.stacked_waveforms;
                        cx.notify();
                    })),
            ),
        );
    }
    if oscillator || layout.pages.len() > 1 {
        content = content.child(toolbar);
    }
    if !this.project.panels.layouts.contains_key(&key) && !this.plots.is_empty() {
        content = content.child(crate::plugin_plot_view::view(
            this.plots.clone(),
            theme,
            width,
        ));
    }
    let mut groups = div().flex().flex_wrap().gap_2().items_start();
    let default_layout = !this.project.panels.layouts.contains_key(&key);
    let group_width = if default_layout && width >= 720. && page.groups.len() > 1 {
        (width - 48.) * 0.5
    } else {
        width - 40.
    };
    for group in &page.groups {
        let cols = group.columns.min(
            ((if default_layout {
                group_width - 18.
            } else {
                width - 52.
            }) / 82.)
                .floor()
                .max(1.) as u32,
        );
        let mut rows = Vec::new();
        let mut cells = Vec::new();
        for control in &group.controls {
            if default_layout
                && !this.plots.is_empty()
                && !control.visual()
                && !crate::plugin_time_plot::active_parameter(details, control.bindings()[0])
            {
                continue;
            }
            if control.visual() {
                flush_row(&mut rows, &mut cells, cols);
                rows.push(crate::plugin_visuals::view(
                    control,
                    details,
                    theme,
                    this.project.snapshot.sample_rate as f64,
                    this.stacked_waveforms,
                ));
            } else if let Some(p) = parameter(control.bindings()[0]) {
                cells.push(crate::plugin_control_input::view(control, p, this, cx));
                if matches!(control, crate::plugin_layout::Control::Choice { .. }) {
                    let choice = cells.pop().unwrap();
                    flush_row(&mut rows, &mut cells, cols);
                    rows.push(choice);
                } else if cells.len() == cols as usize {
                    flush_row(&mut rows, &mut cells, cols);
                }
            }
        }
        flush_row(&mut rows, &mut cells, cols);
        let controls = div().px_2().py_2().flex().flex_col().gap_1().children(rows);
        groups = groups.child(
            div()
                .flex_1()
                .flex_basis(px((cols as f32 * 82. + 18.).max((width - 48.) * 0.5)))
                .min_w(px(cols as f32 * 82. + 18.))
                .when(default_layout, |d| {
                    d.flex_none()
                        .flex_basis(px(group_width))
                        .w(px(group_width))
                        .min_w_0()
                })
                .rounded(px(6.))
                .bg(rgb(theme.panel))
                .border_1()
                .border_color(crate::ui::alpha(theme.border, 0.65))
                .child(
                    div()
                        .h(px(30.))
                        .px_3()
                        .flex()
                        .items_center()
                        .rounded_t(px(5.))
                        .bg(rgb(theme.raised))
                        .border_b_1()
                        .border_color(crate::ui::alpha(theme.border, 0.5))
                        .text_size(px(11.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(theme.text))
                        .child(group.title.clone()),
                )
                .child(controls),
        );
    }
    content
        .child(groups)
        .when(details.parameters.is_empty(), |d| {
            d.child(theme.label("No parameter controls"))
        })
}

/// Explicit equal-width rows avoid percentage rounding wrapping the last dial.
fn flush_row(rows: &mut Vec<Div>, cells: &mut Vec<Div>, columns: u32) {
    if cells.is_empty() {
        return;
    }
    let count = cells.len();
    let mut row = div().w_full().flex();
    for cell in cells.drain(..) {
        row = row.child(div().flex_1().min_w_0().child(cell));
    }
    for _ in count..columns as usize {
        row = row.child(div().flex_1().min_w_0());
    }
    rows.push(row);
}
