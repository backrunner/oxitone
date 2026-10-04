//! Editable source ADSR schematic; the sustain duration is an illustrative hold.
use crate::{
    plugin_envelope_geometry::TimeAxis,
    plugin_graph_handle::{Axis, Handle},
    plugin_parameter_drag::Range,
    plugin_window::PluginWindow,
    theme::Theme,
    ui::alpha,
};
use gpui::{prelude::*, *};

pub fn view(
    ids: [&str; 4],
    this: &PluginWindow,
    theme: Theme,
    cx: &mut Context<PluginWindow>,
) -> Div {
    let Some(details) = &this.details else {
        return div();
    };
    let parameters: Vec<_> = ids
        .iter()
        .filter_map(|id| {
            details
                .parameters
                .iter()
                .find(|p| !p.host && p.spec.id == *id)
        })
        .collect();
    if parameters.len() != 4 {
        return div();
    }
    let values = std::array::from_fn(|i| parameters[i].value);
    let mut basis = values;
    if let Some(owner) = this.owner.upgrade() {
        let edit = &owner.read(cx).document.plugin;
        if edit.gesture.is_some() {
            for change in edit
                .changes()
                .iter()
                .filter(|c| c.identity == this.identity && !c.host)
            {
                if let Some(index) = ids.iter().position(|id| *id == change.parameter) {
                    basis[index] = change.before;
                }
            }
        }
    }
    let axis = TimeAxis::new(basis);
    let total = axis.end;
    let points = axis.points(values);
    let handles = vec![
        Handle {
            auxiliary: None,
            label: "A".into(),
            position: points[1],
            color: 0,
            x: Some(Axis::new(ids[0], Range::linear(0., total))),
            y: None,
        },
        Handle {
            auxiliary: None,
            label: "D".into(),
            position: points[2],
            color: 0,
            x: Some(Axis::new(ids[1], Range::linear(0., total))),
            y: Some(Axis::new(ids[2], Range::linear(0., 1.))),
        },
        Handle {
            auxiliary: None,
            label: "S".into(),
            position: points[3],
            color: 0,
            x: None,
            y: Some(Axis::new(ids[2], Range::linear(0., 1.))),
        },
        Handle {
            auxiliary: None,
            label: "R".into(),
            position: points[4],
            color: 0,
            x: Some(Axis::new(ids[3], Range::linear(0., total))),
            y: None,
        },
    ];
    let mut values = div().px_3().py_2().flex().justify_between().gap_2();
    for (p, label) in parameters.iter().zip(["A", "D", "S", "R"]) {
        values = values.child(
            div()
                .flex()
                .gap_1()
                .text_size(px(10.))
                .child(div().text_color(rgb(theme.accent)).child(label))
                .child(crate::plugin_controls::value(p)),
        );
    }
    div()
        .w_full()
        .rounded_md()
        .bg(rgb(theme.scope))
        .border_1()
        .border_color(alpha(theme.border, 0.5))
        .mb_2()
        .child(values)
        .child(
            div()
                .h(px(136.))
                .relative()
                .overflow_hidden()
                .child(
                    canvas(
                        |_, _, _| {},
                        move |bounds, _, window, cx| {
                            let at = Bounds::new(
                                bounds.origin + point(px(18.), px(16.)),
                                size(
                                    (bounds.size.width - px(36.)).max(px(1.)),
                                    bounds.size.height - px(34.),
                                ),
                            );
                            crate::plugin_visuals::grid(at, theme, window);
                            crate::plugin_graph_paint::area(
                                at,
                                &points,
                                1.,
                                alpha(theme.accent, 0.13).into(),
                                window,
                            );
                            crate::plugin_visuals::trace(
                                at,
                                points.iter().copied(),
                                alpha(theme.accent, 0.12).into(),
                                6.,
                                window,
                            );
                            crate::plugin_visuals::trace(
                                at,
                                points.iter().copied(),
                                rgb(theme.accent).into(),
                                2.,
                                window,
                            );
                            crate::plugin_graph_paint::text(
                                "0",
                                at.origin + point(px(0.), at.size.height + px(7.)),
                                theme.muted,
                                None,
                                window,
                                cx,
                            );
                        },
                    )
                    .size_full(),
                )
                .child(
                    div()
                        .absolute()
                        .left(px(18.))
                        .right(px(18.))
                        .top(px(16.))
                        .bottom(px(18.))
                        .child(crate::plugin_graph_handle::overlay(&handles, this, cx)),
                ),
        )
        .child(crate::plugin_visuals::caption(
            "ADSR · illustrative hold".into(),
            format!("{total:.2} s"),
            theme,
        ))
}
