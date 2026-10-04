//! Large response surfaces with calibrated axes and direct parameter handles.
use crate::{plugin_window::PluginWindow, ui::alpha};
use gpui::{prelude::*, *};

pub fn view(this: &PluginWindow, width: f32, cx: &mut Context<PluginWindow>) -> Div {
    let theme = this.theme;
    let plots = &this.plots;
    let editable = this.owner.upgrade().is_some_and(|owner| {
        let owner = owner.read(cx);
        owner.document_ready() && owner.plugin_configuration_target(&this.target).is_some()
    });
    let columns = if plots.len() > 1 && width >= 720. {
        2
    } else {
        1
    };
    let mut row = div().w_full().flex().flex_wrap().gap_3();
    for (index, plot) in plots.iter().enumerate() {
        let data = plots.clone();
        let mut legend = div().flex().flex_wrap().gap_3().items_center();
        for trace in plot.traces.iter().filter(|t| !t.label.is_empty()) {
            legend = legend.child(
                div()
                    .flex()
                    .gap_1()
                    .items_center()
                    .child(
                        div()
                            .size(px(5.))
                            .rounded_full()
                            .bg(rgb(theme.track(trace.color))),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(rgb(theme.muted))
                            .child(trace.label.clone()),
                    ),
            );
        }
        let chart = div()
            .relative()
            .w_full()
            .h(px(if columns == 1 { 252. } else { 208. }))
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, cx| {
                        crate::plugin_graph_paint::plot(&data[index], bounds, theme, window, cx);
                    },
                )
                .size_full(),
            )
            .child(
                div()
                    .absolute()
                    .left(px(40.))
                    .right(px(18.))
                    .top(px(18.))
                    .bottom(px(30.))
                    .child(crate::plugin_graph_handle::overlay(&plot.handles, this, cx)),
            );
        row = row.child(
            div()
                .flex_1()
                .min_w_0()
                .flex_basis(px((width - 36.) / columns as f32 - 12.))
                .rounded_lg()
                .bg(rgb(theme.scope))
                .border_1()
                .border_color(alpha(theme.border, 0.7))
                .overflow_hidden()
                .child(
                    div()
                        .px_4()
                        .pt_3()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(plot.title.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(9.))
                                .text_color(rgb(theme.muted))
                                .child("SOURCE VIEW"),
                        ),
                )
                .child(
                    div()
                        .px_4()
                        .pt_1()
                        .text_size(px(10.))
                        .text_color(rgb(theme.muted))
                        .child(plot.detail.clone()),
                )
                .child(chart)
                .when(!plot.handles.is_empty(), |d| d.child(
                        div().px_4().pb_2().child(crate::plugin_graph_readout::view(&plot.handles, this, editable))
                ))
                .child(
                    div()
                        .px_4()
                        .py_2()
                        .border_t_1()
                        .border_color(alpha(theme.border, 0.5))
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(legend)
                        .when(editable && !plot.handles.is_empty(), |d| {
                            d.child(
                                div()
                                    .text_size(px(9.))
                                    .text_color(rgb(theme.muted))
                                    .child(if plot.handles.iter().any(|h| h.auxiliary.is_some()) {
                                        if plot.handles.iter().any(|h| h.auxiliary.as_ref().is_some_and(|a| a.parameter == "kneeDb")) {
                                            "Drag graph · Alt/Option knee · Shift fine · Double-click reset"
                                        } else {
                                            "Drag graph · Alt/Option Q · Shift fine · Double-click reset"
                                        }
                                    } else {
                                        "Drag graph · Shift fine · Double-click reset"
                                    }),
                            )
                        }),
                ),
        );
    }
    row
}
