//! A compact map uses spare bank space to make the selected channel's routes visible.
use crate::{
    mixer_model::Strip,
    mixer_routes::RouteKind,
    ui::{alpha, Preview},
    ui_icons::{icon, Icon},
};
use gpui::{prelude::*, *};

pub fn view(
    this: &Preview,
    strip: &Strip,
    width: f32,
    cx: &mut Context<Preview>,
) -> impl IntoElement {
    let theme = this.theme;
    let incoming = strip.outputs.is_empty();
    let routes = if incoming {
        &strip.inputs
    } else {
        &strip.outputs
    };
    let shown = routes.len().min(3);
    let tint = theme.track(strip.color_index);
    let mut graph = div().relative().mt_4().h(px(236.)).w_full().child(
        canvas(
            |_, _, _| {},
            move |at, _, window, _| {
                let mut path = PathBuilder::stroke(px(1.5));
                for i in 0..shown {
                    let y = 36. + i as f32 * 72.;
                    path.move_to(at.origin + point(px(12.), px(0.)));
                    path.line_to(at.origin + point(px(12.), px(y - 8.)));
                    path.line_to(at.origin + point(px(20.), px(y)));
                    path.line_to(at.origin + point(px(34.), px(y)));
                }
                if let Ok(path) = path.build() {
                    window.paint_path(path, alpha(tint, 0.65));
                }
            },
        )
        .size_full(),
    );
    for (i, r) in routes.iter().take(3).enumerate() {
        let target = if incoming {
            r.source.clone()
        } else {
            r.destination.clone()
        };
        let name = if incoming {
            &r.source_name
        } else {
            &r.destination_name
        };
        let tone = if r.kind == RouteKind::Sidechain {
            theme.gold
        } else {
            theme.accent
        };
        graph = graph.child(
            div()
                .id(("flow-node", i))
                .absolute()
                .top(px(i as f32 * 72. + 9.))
                .left(px(34.))
                .w(px(width - 74.))
                .h(px(54.))
                .px_3()
                .py_2()
                .rounded_md()
                .bg(rgb(theme.panel))
                .border_1()
                .border_color(alpha(theme.border, 0.7))
                .cursor_pointer()
                .hover(move |s| s.border_color(rgb(tone)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.select_mixer(&target);
                    cx.notify();
                }))
                .child(
                    div()
                        .text_size(px(11.))
                        .font_weight(FontWeight::MEDIUM)
                        .truncate()
                        .child(name.clone()),
                )
                .child(
                    div()
                        .mt_1()
                        .text_size(px(9.))
                        .text_color(rgb(theme.muted))
                        .child(if r.kind == RouteKind::Output {
                            "Direct output".into()
                        } else {
                            format!("{} · {:.0}%", r.tap(), r.ratio * 100.)
                        }),
                ),
        );
    }
    div()
        .w(px(width))
        .flex_shrink_0()
        .h_full()
        .px_5()
        .py_4()
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(icon(Icon::Route, theme.muted))
                .child(
                    div()
                        .text_size(px(9.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(theme.muted))
                        .child(if incoming {
                            "INCOMING SIGNAL"
                        } else {
                            "SIGNAL FLOW"
                        }),
                ),
        )
        .child(
            div()
                .mt_4()
                .px_3()
                .py_2()
                .rounded_md()
                .border_1()
                .border_color(alpha(tint, 0.55))
                .bg(rgb(theme.selected))
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .truncate()
                        .child(strip.name.clone()),
                ),
        )
        .child(graph)
        .when(routes.len() > 3, |d| {
            d.child(
                div()
                    .text_size(px(9.))
                    .text_color(rgb(theme.muted))
                    .child(format!("+ {} connections in Routing", routes.len() - 3)),
            )
        })
}
