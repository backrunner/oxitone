use crate::plugin_window::{ParameterFilter, PluginWindow};
use gpui::{prelude::*, *};
pub fn view(this: &PluginWindow, cx: &mut Context<PluginWindow>) -> Div {
    let theme = this.theme;
    let details = this.details.as_ref().unwrap();
    let mut filters = div().flex().items_center().gap_2();
    for (filter, label) in [
        (ParameterFilter::All, "All"),
        (ParameterFilter::Source, "In source"),
        (ParameterFilter::Automated, "Automated"),
    ] {
        filters = filters.child(
            theme
                .button(label, label)
                .when(this.filter == filter, |d| d.bg(rgb(theme.selected)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.filter = filter;
                    this.scroll.set_offset(point(px(0.), px(0.)));
                    cx.notify();
                })),
        );
    }
    filters = filters.child(div().flex_1()).child(
        theme
            .button("parameter-specs", "Specs")
            .when(this.parameter_specs, |d| d.bg(rgb(theme.selected)))
            .on_click(cx.listener(|this, _, _, cx| {
                this.parameter_specs = !this.parameter_specs;
                cx.notify();
            })),
    );
    let mut grid = div().flex().flex_col();
    let mut count = 0;
    for p in details.parameters.iter().filter(|p| match this.filter {
        ParameterFilter::All => true,
        ParameterFilter::Source => p.explicit,
        ParameterFilter::Automated => !p.automation.is_empty(),
    }) {
        count += 1;
        grid = grid.child(div().w_full().child(crate::parameter_view::row(
            p,
            theme,
            this.parameter_specs,
        )));
    }
    div()
        .p_3()
        .flex()
        .flex_col()
        .gap_3()
        .child(filters)
        .child(grid)
        .when(count == 0, |d| {
            d.child(
                div()
                    .p_4()
                    .text_sm()
                    .text_color(rgb(theme.muted))
                    .child("No parameters in this view"),
            )
        })
}
