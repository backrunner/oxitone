use crate::{
    mixer_model::Strip,
    mixer_routing_view::{card, empty, section, Direction},
    plugin_details::DetailTarget,
    ui::{alpha, Preview},
    ui_icons::{icon, Icon},
    workspace::Axis,
};
use gpui::{prelude::*, *};
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InspectorTab {
    Chain,
    Routing,
}

pub fn view(
    this: &Preview,
    strip: &Strip,
    width: f32,
    cx: &mut Context<Preview>,
) -> impl IntoElement {
    let theme = this.theme;
    let tint = if strip.id == "mix_master" {
        theme.gold
    } else {
        theme.track(strip.color_index)
    };
    let mut tabs = div()
        .flex()
        .px_3()
        .gap_4()
        .h(px(33.))
        .flex_shrink_0()
        .border_b_1()
        .border_color(rgb(theme.border));
    for (tab, label, count) in [
        (InspectorTab::Chain, "Chain", strip.effects.len()),
        (
            InspectorTab::Routing,
            "Routing",
            strip.outputs.len() + strip.inputs.len(),
        ),
    ] {
        tabs = tabs.child(
            theme
                .tab(
                    label,
                    format!("{label}  {count}"),
                    this.workspace.inspector_tab == tab,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.workspace.inspector_tab = tab;
                    this.workspace.inspector.set_offset(point(px(0.), px(0.)));
                    cx.notify();
                })),
        );
    }
    let content = match this.workspace.inspector_tab {
        InspectorTab::Routing => crate::mixer_routing_view::view(this, strip, cx),
        InspectorTab::Chain => chain(this, strip, cx),
    };
    div()
        .id("mixer-detail-panel")
        .track_focus(&this.inspector_focus)
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, window, cx| {
                this.inspector_focus.focus(window);
                cx.stop_propagation();
            }),
        )
        .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
            let m = event.keystroke.modifiers;
            if !m.alt
                && !m.platform
                && !m.control
                && crate::mixer_actions::scroll_details(
                    &this.workspace.inspector,
                    &event.keystroke.key,
                )
            {
                cx.stop_propagation();
                cx.notify();
            }
        }))
        .w(px(width))
        .h_full()
        .flex_shrink_0()
        .flex()
        .flex_col()
        .border_l_1()
        .border_color(rgb(theme.border))
        .bg(rgb(theme.bg))
        .child(
            div()
                .px_3()
                .py_3()
                .flex_shrink_0()
                .flex()
                .gap_3()
                .items_center()
                .bg(rgb(theme.panel))
                .child(
                    div()
                        .size(px(32.))
                        .rounded_md()
                        .bg(alpha(tint, 0.12))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon(
                            if strip.instrument {
                                Icon::Wave
                            } else {
                                Icon::Route
                            },
                            tint,
                        )),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .text_size(px(12.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(strip.name.clone()),
                        )
                        .child(
                            div()
                                .mt_1()
                                .text_size(px(9.))
                                .text_color(rgb(theme.muted))
                                .child(if strip.id == "mix_master" {
                                    format!(
                                        "Stereo output · {} dBTP scope",
                                        crate::mixer_model::db(
                                            this.analysis
                                                .get(&strip.id)
                                                .map_or(0., |a| a.true_peak)
                                        )
                                    )
                                } else {
                                    strip.kind.clone()
                                }),
                        ),
                ),
        )
        .child(tabs)
        .child(
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .child(
                    div()
                        .id("mixer-inspector-scroll")
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .overflow_y_scroll()
                        .track_scroll(&this.workspace.inspector)
                        .child(content),
                )
                .child(crate::scrollbar::view(
                    "inspector-scrollbar",
                    Axis::Vertical,
                    &this.workspace.inspector,
                    this,
                    cx,
                )),
        )
}
fn chain(this: &Preview, strip: &Strip, cx: &mut Context<Preview>) -> Div {
    let theme = this.theme;
    let mut content = div().p_3().flex().flex_col().gap_2();
    if strip.instrument {
        let target = DetailTarget::Instrument(strip.id.clone());
        content = content.child(section(theme, "INSTRUMENT", 1)).child(
            div()
                .id("inspect-instrument")
                .p_3()
                .rounded_md()
                .bg(rgb(theme.panel))
                .border_1()
                .border_color(alpha(theme.accent, 0.45))
                .cursor_pointer()
                .hover(move |s| s.bg(rgb(theme.button)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.open_plugin(target.clone(), cx);
                }))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .items_center()
                        .child(icon(Icon::Wave, theme.accent))
                        .child(
                            div()
                                .flex_1()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .child(strip.kind.clone()),
                        )
                        .child(div().text_color(rgb(theme.muted)).text_xs().child("↗")),
                )
                .child(
                    div()
                        .mt_1()
                        .text_size(px(9.))
                        .text_color(rgb(theme.muted))
                        .child("Open instrument details"),
                ),
        );
    }
    content = content.child(section(theme, "EFFECT CHAIN", strip.effects.len()));
    for (i, effect) in strip.effects.iter().enumerate() {
        let target = if strip.instrument {
            DetailTarget::ChannelInsert(strip.id.clone(), i)
        } else {
            DetailTarget::BusInsert(strip.id.clone(), i)
        };
        let tone = if effect.bypass {
            theme.muted
        } else {
            theme.accent
        };
        content = content.child(
            div()
                .id(("inspect-effect", i))
                .p_2()
                .rounded_md()
                .bg(rgb(theme.panel))
                .border_1()
                .border_color(alpha(theme.border, 0.6))
                .cursor_pointer()
                .hover(move |s| s.border_color(rgb(tone)).bg(rgb(theme.button)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.open_plugin(target.clone(), cx);
                }))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .size(px(24.))
                                .rounded_sm()
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(rgb(theme.button))
                                .text_size(px(9.))
                                .text_color(rgb(theme.muted))
                                .child(format!("{:02}", i + 1)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_size(px(11.))
                                .font_weight(FontWeight::MEDIUM)
                                .child(effect.name.clone()),
                        )
                        .child(icon(Icon::Effect, tone)),
                )
                .child(
                    div()
                        .mt_2()
                        .flex()
                        .justify_between()
                        .text_size(px(9.))
                        .text_color(rgb(theme.muted))
                        .child(if effect.bypass { "Bypassed" } else { "Active" })
                        .child(format!("Mix {:.0}%", effect.mix * 100.)),
                )
                .child(
                    div()
                        .mt_1()
                        .h(px(2.))
                        .rounded_full()
                        .bg(rgb(theme.border))
                        .child(div().h_full().w(relative(effect.mix as f32)).bg(rgb(tone))),
                ),
        );
    }
    if strip.effects.is_empty() {
        content = content.child(empty(theme, "Clean signal · no insert effects"));
    }
    content = content.child(section(theme, "OUTPUT", 1));
    if let Some(route) = strip.output() {
        content = content.child(card(this, route, Direction::Outgoing, cx));
    } else {
        content = content.child(empty(theme, "Master → Stereo device"));
    }
    content
}
