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
    let mut content = div().p_3().flex().flex_col().gap_2();
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
                    .button(format!("panel-page-{id}"), page.title.clone())
                    .when(this.page == id, |d| d.bg(rgb(theme.selected)))
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
    if page
        .groups
        .iter()
        .flat_map(|g| &g.controls)
        .any(|c| matches!(c, crate::plugin_layout::Control::Oscillator { .. }))
    {
        toolbar = toolbar.child(
            div().flex().flex_1().justify_end().child(
                theme
                    .button(
                        "wave-view",
                        if this.stacked_waveforms {
                            "Wave view · 3D"
                        } else {
                            "Wave view · 2D"
                        },
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.stacked_waveforms = !this.stacked_waveforms;
                        cx.notify();
                    })),
            ),
        );
    }
    content = content.child(toolbar);
    let mut groups = div().flex().flex_wrap().gap_2().items_start();
    for group in &page.groups {
        let cols = group
            .columns
            .min(((width - 52.) / 82.).floor().max(1.) as u32);
        let mut rows = Vec::new();
        let mut cells = Vec::new();
        for control in &group.controls {
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
                cells.push(crate::plugin_controls::control_view(control, p, theme));
                if cells.len() == cols as usize {
                    flush_row(&mut rows, &mut cells, cols);
                }
            }
        }
        flush_row(&mut rows, &mut cells, cols);
        let controls = div().flex().flex_col().children(rows);
        groups = groups.child(
            div()
                .flex_1()
                .flex_basis(px(cols as f32 * 82. + 18.))
                .min_w(px(cols as f32 * 82. + 18.))
                .px_2()
                .py_2()
                .rounded_lg()
                .bg(rgb(theme.panel))
                .border_1()
                .border_color(crate::ui::alpha(theme.border, 0.65))
                .child(
                    div()
                        .mb_2()
                        .px_1()
                        .text_size(px(11.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(theme.muted))
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
