//! Scrollable mixer bank, pinned master and a separate insert/routing inspector.
use crate::{
    mixer_model::{self, STRIP_WIDTH},
    ui::{alpha, Preview},
    ui_icons::{icon, Icon},
    workspace::Axis,
};
use gpui::{prelude::*, *};

pub fn view(
    this: &Preview,
    height: f32,
    width: f32,
    cx: &mut Context<Preview>,
) -> impl IntoElement {
    let theme = this.theme;
    let strips = mixer_model::strips(this.project.as_ref().unwrap());
    let master = strips.iter().find(|s| s.id == "mix_master").unwrap();
    let selected = strips
        .iter()
        .find(|s| s.id == this.selected_scope)
        .unwrap_or(master);
    let strip_height = (height - 52.).max(356.);
    let inspector_width = if width < 650. { 228. } else { 260. };
    let spare = width
        - strips.len() as f32 * STRIP_WIDTH
        - 11.
        - if this.workspace.inspector_open {
            inspector_width
        } else {
            0.
        };
    let mut bank = div()
        .flex()
        .h(px(strip_height))
        .w(px((strips.len() - 1) as f32 * STRIP_WIDTH));
    for strip in strips.iter().filter(|s| s.id != "mix_master") {
        bank = bank.child(crate::mixer_strip::view(
            this,
            strip,
            selected.relation(&strip.id),
            cx,
        ));
    }
    if spare >= 280. {
        bank = bank
            .w(px((strips.len() - 1) as f32 * STRIP_WIDTH + spare))
            .child(crate::mixer_flow::view(this, selected, spare, cx));
    }
    let mut body = div()
        .flex_1()
        .min_h_0()
        .flex()
        .overflow_hidden()
        .child(
            div()
                .relative()
                .w(px(STRIP_WIDTH + 1.))
                .h_full()
                .flex_shrink_0()
                .pb(px(10.))
                .border_r_1()
                .border_color(alpha(theme.gold, 0.4))
                .overflow_hidden()
                .child(
                    div()
                        .absolute()
                        .top(this.workspace.mixer.offset().y)
                        .h(px(strip_height))
                        .child(crate::mixer_strip::view(
                            this,
                            master,
                            selected.relation(&master.id),
                            cx,
                        )),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .h_full()
                .flex()
                .flex_col()
                .child(
                    div()
                        .id("mixer-scroll")
                        .flex_1()
                        .min_h_0()
                        .overflow_scroll()
                        .track_scroll(&this.workspace.mixer)
                        // Intercept in the content before the native parent scroll handler.
                        .child(
                            bank.id("mixer-bank")
                                .min_w_full()
                                .min_h_full()
                                .on_scroll_wheel(cx.listener(
                                    |this, event: &ScrollWheelEvent, _, cx| {
                                        let delta = event.delta.pixel_delta(px(28.));
                                        let scroll = &this.workspace.mixer;
                                        let movement = if delta.x.abs() > px(0.) {
                                            delta.x
                                        } else {
                                            delta.y
                                        };
                                        let old = scroll.offset();
                                        scroll.set_offset(if event.modifiers.alt {
                                            point(
                                                old.x,
                                                (old.y + delta.y)
                                                    .clamp(-scroll.max_offset().height, px(0.)),
                                            )
                                        } else {
                                            point(
                                                (old.x + movement)
                                                    .clamp(-scroll.max_offset().width, px(0.)),
                                                old.y,
                                            )
                                        });
                                        cx.stop_propagation();
                                        cx.notify();
                                    },
                                )),
                        ),
                )
                .child(crate::scrollbar::view(
                    "mixer-scrollbar",
                    Axis::Horizontal,
                    &this.workspace.mixer,
                    this,
                    cx,
                )),
        )
        .child(crate::scrollbar::view(
            "mixer-vertical",
            Axis::Vertical,
            &this.workspace.mixer,
            this,
            cx,
        ));
    if this.workspace.inspector_open {
        body = body.child(crate::mixer_inspector::view(
            this,
            selected,
            inspector_width,
            cx,
        ));
    }
    div()
        .id("mixer-panel")
        .track_focus(&this.mixer_focus)
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, window, _| this.mixer_focus.focus(window)),
        )
        .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
            if !event.keystroke.modifiers.platform
                && !event.keystroke.modifiers.control
                && !event.keystroke.modifiers.alt
                && this.mixer_key(&event.keystroke.key)
            {
                cx.stop_propagation();
                cx.notify();
            }
        }))
        .flex_1()
        .min_w_0()
        .h_full()
        .flex()
        .flex_col()
        .bg(rgb(theme.bg))
        .child(
            div()
                .h(px(42.))
                .flex_shrink_0()
                .px_3()
                .flex()
                .items_center()
                .gap_2()
                .border_b_1()
                .border_color(rgb(theme.border))
                .child(icon(Icon::Effect, theme.accent))
                .child(
                    div()
                        .text_size(px(12.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Mixer"),
                )
                .child(
                    div()
                        .flex_1()
                        .text_size(px(10.))
                        .text_color(rgb(theme.muted))
                        .min_w_0()
                        .truncate()
                        .child(if width > 600. {
                            format!("{} channels · Source levels", strips.len())
                        } else {
                            String::new()
                        }),
                )
                .child(
                    theme
                        .button("mixer-prev", "‹")
                        .on_click(cx.listener(|this, _, _, cx| {
                            nudge(&this.workspace.mixer, STRIP_WIDTH * 2.);
                            cx.notify();
                        })),
                )
                .child(
                    theme
                        .button("mixer-next", "›")
                        .on_click(cx.listener(|this, _, _, cx| {
                            nudge(&this.workspace.mixer, -STRIP_WIDTH * 2.);
                            cx.notify();
                        })),
                )
                .child(
                    theme
                        .button("mixer-inspector", "Details")
                        .when(this.workspace.inspector_open, |d| d.bg(rgb(theme.selected)))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.workspace.inspector_open = !this.workspace.inspector_open;
                            cx.notify();
                        })),
                )
                .child(
                    theme
                        .button(
                            "mixer-expand",
                            if this.workspace.mixer_expanded {
                                "Split"
                            } else {
                                "Expand"
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.workspace.mixer_expanded = !this.workspace.mixer_expanded;
                            cx.notify();
                        })),
                ),
        )
        .child(body)
}
fn nudge(scroll: &ScrollHandle, amount: f32) {
    scroll.set_offset(point(
        (scroll.offset().x + px(amount)).clamp(-scroll.max_offset().width, px(0.)),
        scroll.offset().y,
    ));
}
