//! Compact editable signal chain; detailed parameters belong to the instance editor.
use crate::{
    mixer_model::Strip, plugin_details::DetailTarget, plugin_picker::Slot, ui::Preview,
    ui_icons::Icon,
};
use gpui::{prelude::*, *};

pub fn view(this: &Preview, strip: &Strip, cx: &Context<Preview>) -> Div {
    let t = this.theme;
    let mut rows = div().p_2().flex().flex_col().gap_1();
    if strip.instrument {
        let slot = Slot {
            owner: strip.id.clone(),
            target: "instrument".into(),
            instance: None,
            label: format!("{} · Instrument", strip.name),
        };
        rows = rows.child(row(
            this,
            &strip.kind,
            DetailTarget::Instrument(strip.id.clone()),
            slot,
            "Instrument",
            cx,
        ));
    }
    for (index, effect) in strip.effects.iter().enumerate() {
        let slot = Slot {
            owner: strip.id.clone(),
            target: if strip.instrument {
                "channelInsert"
            } else {
                "busInsert"
            }
            .into(),
            instance: effect.instance.clone(),
            label: format!("{} · Insert {}", strip.name, index + 1),
        };
        let target = if strip.instrument {
            DetailTarget::ChannelInsert(strip.id.clone(), index)
        } else {
            DetailTarget::BusInsert(strip.id.clone(), index)
        };
        rows = rows.child(row(
            this,
            &effect.name,
            target,
            slot,
            if effect.bypass { "Bypassed" } else { "Effect" },
            cx,
        ));
    }
    let slot = Slot {
        owner: strip.id.clone(),
        target: if strip.instrument {
            "channelInsert"
        } else {
            "busInsert"
        }
        .into(),
        instance: None,
        label: format!("{} · Add effect", strip.name),
    };
    rows.when(this.document.view.is_some(), |rows| {
        rows.child(
            t.ghost("chain-add-effect", "+ Add effect")
                .opacity(if this.document_ready() { 1. } else { 0.4 })
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.choose_plugin(slot.clone(), window);
                    cx.notify();
                })),
        )
    })
}
fn row(
    this: &Preview,
    name: &str,
    target: DetailTarget,
    slot: Slot,
    kind: &str,
    cx: &Context<Preview>,
) -> Div {
    let t = this.theme;
    let replace = slot.clone();
    let key = slot.instance.clone().unwrap_or_else(|| slot.owner.clone());
    div()
        .h(px(38.))
        .flex()
        .items_center()
        .gap_1()
        .border_b_1()
        .border_color(rgb(t.border))
        .child(
            div()
                .id(SharedString::from(format!("chain-open-{key}")))
                .flex_1()
                .min_w_0()
                .px_1()
                .cursor_pointer()
                .child(div().text_size(px(11.)).truncate().child(name.to_owned()))
                .child(
                    div()
                        .text_size(px(9.))
                        .text_color(rgb(t.muted))
                        .child(kind.to_owned()),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.open_plugin(target.clone(), cx);
                })),
        )
        .when(this.document.view.is_some(), |row| {
            row.child(
                t.icon_button(format!("chain-replace-{key}"), Icon::Loop, "Replace plugin")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.choose_plugin(replace.clone(), window);
                        cx.notify();
                    })),
            )
            .when(!slot.instrument(), |row| {
                row.child(
                    t.icon_button(format!("chain-remove-{key}"), Icon::Close, "Remove effect")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.remove_plugin_slot(&slot);
                            cx.notify();
                        })),
                )
            })
        })
}
