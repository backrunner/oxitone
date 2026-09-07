use crate::{
    mixer_model::{self, Strip},
    mixer_routes::{Route, RouteKind},
    theme::Theme,
    ui::{alpha, Preview},
    ui_icons::{icon, Icon},
};
use gpui::{prelude::*, *};

pub fn view(this: &Preview, strip: &Strip, cx: &mut Context<Preview>) -> Div {
    let theme = this.theme;
    let mut content = div().p_3().flex().flex_col().gap_2();
    let mut sends: Vec<_> = strip.sends().collect();
    let bus = strip
        .instrument
        .then(|| strip.output())
        .flatten()
        .and_then(|r| {
            mixer_model::strips(this.project.as_ref().unwrap())
                .iter()
                .find(|s| s.id == r.destination)
        });
    let label = if let Some(bus) = bus.filter(|b| b.id != "mix_master") {
        sends = bus.sends().collect();
        format!("SENDS VIA {}", bus.name)
    } else {
        "SENDS".into()
    };
    if strip.id != "mix_master" {
        content = content.child(section(theme, &label, sends.len()));
        if sends.is_empty() {
            content = content.child(empty(theme, "No auxiliary sends in this route"));
        }
        for route in sends {
            content = content.child(card(this, route, Direction::Outgoing, cx));
        }
    }
    content = content.child(section(theme, "OUTPUT", 1));
    if let Some(route) = strip.output() {
        content = content.child(card(this, route, Direction::Outgoing, cx));
    } else {
        content = content.child(
            div()
                .p_3()
                .rounded_md()
                .bg(rgb(theme.panel))
                .flex()
                .items_center()
                .gap_2()
                .child(icon(Icon::Output, theme.gold))
                .child(div().text_xs().child("Stereo device output")),
        );
    }
    if !strip.instrument {
        content = content.child(section(theme, "INPUTS", strip.inputs.len()));
        if strip.inputs.is_empty() {
            content = content.child(empty(theme, "No incoming connections"));
        }
        for route in &strip.inputs {
            content = content.child(card(this, route, Direction::Incoming, cx));
        }
    }
    content.child(
        div()
            .mt_2()
            .text_size(px(9.))
            .text_color(rgb(theme.muted))
            .child("Initial routing from code · Select a connection to follow it"),
    )
}
#[derive(Clone, Copy)]
pub enum Direction {
    Incoming,
    Outgoing,
}
pub fn card(
    this: &Preview,
    route: &Route,
    direction: Direction,
    cx: &mut Context<Preview>,
) -> impl IntoElement {
    let theme = this.theme;
    let (id, name, glyph) = match direction {
        Direction::Incoming => (&route.source, &route.source_name, Icon::Input),
        Direction::Outgoing => (&route.destination, &route.destination_name, Icon::Output),
    };
    let target = id.clone();
    let tint = if route.kind == RouteKind::Sidechain {
        theme.gold
    } else {
        theme.accent
    };
    let direct = route.kind == RouteKind::Output;
    div()
        .id(SharedString::from(format!(
            "route-{}-{}-{id}",
            route.source, route.destination
        )))
        .p_2()
        .rounded_md()
        .border_1()
        .border_color(alpha(theme.border, 0.6))
        .bg(rgb(theme.panel))
        .cursor_pointer()
        .hover(move |s| s.border_color(rgb(tint)).bg(rgb(theme.button)))
        .on_click(cx.listener(move |this, _, _, cx| {
            this.select_mixer(&target);
            cx.notify();
        }))
        .child(
            div()
                .flex()
                .gap_2()
                .items_center()
                .child(icon(
                    if route.kind == RouteKind::Sidechain {
                        Icon::Route
                    } else {
                        glyph
                    },
                    tint,
                ))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(px(11.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(name.clone()),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .text_size(px(10.))
                        .font_family("Menlo")
                        .text_color(rgb(tint))
                        .child(if direct {
                            "→".into()
                        } else {
                            format!("{:.0}%", route.ratio * 100.)
                        }),
                ),
        )
        .child(
            div()
                .mt_1()
                .flex()
                .justify_between()
                .gap_2()
                .text_size(px(9.))
                .text_color(rgb(theme.muted))
                .child(format!(
                    "{}{}",
                    route.tap(),
                    if route.automated { " · Auto" } else { "" }
                ))
                .when(!direct, |d| {
                    d.child(format!("{} dB", mixer_model::db(route.ratio as f32)))
                }),
        )
        .when(!direct, |d| {
            d.child(
                div()
                    .mt_2()
                    .h(px(3.))
                    .rounded_full()
                    .bg(rgb(theme.border))
                    .child(
                        div()
                            .h_full()
                            .w(relative(route.ratio as f32))
                            .rounded_full()
                            .bg(rgb(tint)),
                    ),
            )
        })
}
pub fn section(theme: Theme, title: &str, count: usize) -> Div {
    div()
        .mt_2()
        .mb_1()
        .flex()
        .justify_between()
        .items_center()
        .gap_2()
        .text_size(px(9.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(theme.muted))
        .child(div().flex_1().min_w_0().child(title.to_owned()))
        .child(div().text_size(px(9.)).child(count.to_string()))
}
pub fn empty(theme: Theme, title: &str) -> Div {
    div()
        .py_2()
        .text_size(px(10.))
        .text_color(rgb(theme.muted))
        .child(title.to_owned())
}
