//! Interactive wrappers around mixer readouts and measured control bounds.
use crate::{mixer_model::Strip, ui::Preview};
use gpui::{prelude::*, *};
fn badge(label: &'static str, active: bool, theme: crate::theme::Theme) -> Div {
    div()
        .w(px(19.))
        .h(px(17.))
        .rounded_sm()
        .text_center()
        .text_size(px(9.))
        .bg(rgb(if active { theme.gold } else { theme.button }))
        .text_color(rgb(if active { theme.bg } else { theme.muted }))
        .child(label)
}
pub fn control(
    this: &Preview,
    strip: &Strip,
    kind: crate::mixer_edit::Control,
    value: f64,
    cx: &Context<Preview>,
) -> Stateful<Div> {
    let id = strip.id.clone();
    let instrument = strip.instrument;
    let bounds = this.document.mixer.bounds.clone();
    let key = format!(
        "{}-{}",
        id,
        if kind == crate::mixer_edit::Control::Pan {
            "pan"
        } else {
            "level"
        }
    );
    div()
        .id(SharedString::from(format!(
            "mix-control-{}-{}",
            id,
            kind == crate::mixer_edit::Control::Pan
        )))
        .relative()
        .child(
            canvas(
                move |area, _, _| {
                    bounds.borrow_mut().insert(key.clone(), area);
                },
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0()
            .size_full(),
        )
        .when(this.document_ready(), |div| {
            div.cursor(CursorStyle::ResizeUpDown)
        })
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, event, window, cx| {
                window.focus(&this.workspace_focus);
                this.begin_mix(id.clone(), instrument, kind, value, event);
                cx.stop_propagation();
                cx.notify();
            }),
        )
        .on_click(|_, _, cx| cx.stop_propagation())
}
pub fn toggle(this: &Preview, strip: &Strip, solo: bool, cx: &Context<Preview>) -> Stateful<Div> {
    let id = strip.id.clone();
    let instrument = strip.instrument;
    let active = if solo { strip.solo } else { strip.mute };
    let bounds = this.document.mixer.bounds.clone();
    let key = format!("{}-{}", id, if solo { "solo" } else { "mute" });
    badge(if solo { "S" } else { "M" }, active, this.theme)
        .relative()
        .child(
            canvas(
                move |area, _, _| {
                    bounds.borrow_mut().insert(key.clone(), area);
                },
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0()
            .size_full(),
        )
        .id(SharedString::from(format!("mix-toggle-{id}-{solo}")))
        .when(this.document_ready(), |div| div.cursor_pointer())
        .on_click(cx.listener(move |this, _, _, cx| {
            let mut values = crate::project_edit::MixValues::default();
            if solo {
                values.solo = Some(!active);
            } else {
                values.mute = Some(!active);
            }
            this.commit_mix(&id, instrument, values);
            cx.stop_propagation();
            cx.notify();
        }))
}
