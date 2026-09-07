//! Channel rack and routing are presentation only; there are no authoring controls.
use crate::ui::{label, Preview, ACCENT, BORDER, GOLD, MUTED};
use gpui::{prelude::*, *};

pub fn db(value: f32) -> String {
    if value <= 0.000001 {
        "−∞".into()
    } else {
        format!("{:.1}", 20. * value.log10())
    }
}

pub fn view(this: &Preview, cx: &mut Context<Preview>) -> impl IntoElement {
    let project = this.project.as_ref().unwrap();
    let mut strips = div().flex().h_full();
    for channel in &project.snapshot.channels {
        strips = strips.child(strip(
            this,
            cx,
            &channel.id,
            channel.name.as_deref().unwrap_or(&channel.id),
            channel.level,
            channel.pan,
            channel.mute.unwrap_or(false),
            channel.solo.unwrap_or(false),
            &channel.instrument.plugin_id,
            channel
                .effect_chain
                .iter()
                .map(|e| e.plugin_id.clone())
                .collect(),
            vec![format!("→ {}", channel.mixer_channel_id)],
        ));
    }
    let implicit_master = (!project
        .snapshot
        .mixer_channels
        .iter()
        .any(|b| b.id == "mix_master"))
    .then(|| oxitone_core::wire::MixerChannelSpec {
        id: "mix_master".into(),
        name: Some("Master".into()),
        level: 1.,
        balance: 0.,
        master_send_ratio: None,
        inserts: vec![],
        sends: vec![],
        mute: None,
        solo: None,
    });
    for bus in project
        .snapshot
        .mixer_channels
        .iter()
        .chain(implicit_master.as_ref())
    {
        let routes = bus
            .sends
            .iter()
            .map(|s| {
                format!(
                    "{} {} {:.0}%{}",
                    if s.sidechain == Some(true) {
                        "SC →"
                    } else {
                        "→"
                    },
                    s.destination_id,
                    s.ratio * 100.,
                    if s.pre_fader == Some(true) {
                        " pre"
                    } else {
                        ""
                    }
                )
            })
            .chain((bus.id != "mix_master").then(|| {
                format!(
                    "→ Master {:.0}%",
                    bus.master_send_ratio.unwrap_or(1.) * 100.
                )
            }))
            .collect();
        strips = strips.child(strip(
            this,
            cx,
            &bus.id,
            bus.name.as_deref().unwrap_or(&bus.id),
            bus.level,
            bus.balance,
            bus.mute.unwrap_or(false),
            bus.solo.unwrap_or(false),
            if bus.id == "mix_master" {
                "MASTER"
            } else {
                "BUS"
            },
            bus.inserts.iter().map(|e| e.plugin_id.clone()).collect(),
            routes,
        ));
    }
    div()
        .w(relative(0.46))
        .min_w(px(280.))
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(34.))
                .px_3()
                .flex()
                .items_center()
                .child(label("CHANNEL RACK / MIXER · Click to inspect scope")),
        )
        .child(
            div()
                .id("mixer-scroll")
                .flex_1()
                .min_h_0()
                .overflow_scroll()
                .child(strips),
        )
}

#[allow(clippy::too_many_arguments)]
fn strip(
    this: &Preview,
    cx: &mut Context<Preview>,
    id: &str,
    name: &str,
    level: f64,
    pan: f64,
    mute: bool,
    solo: bool,
    kind: &str,
    effects: Vec<String>,
    routes: Vec<String>,
) -> impl IntoElement {
    let analysis = this.analysis.get(id);
    let peak = analysis.map_or(0., |a| a.peak);
    let rms = analysis.map_or(0., |a| a.rms);
    let selected = this.selected_scope == id;
    let select = id.to_owned();
    let mut strip = div()
        .id(SharedString::from(format!("mixer-{id}")))
        .w(px(125.))
        .flex_shrink_0()
        .p_2()
        .flex()
        .flex_col()
        .gap_1()
        .border_r_1()
        .border_color(rgb(BORDER))
        .bg(rgb(if selected { 0x202f3c } else { 0x151b25 }))
        .cursor_pointer()
        .on_click(cx.listener(move |this, _, _, cx| {
            this.selected_scope = select.clone();
            cx.notify();
        }))
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .truncate()
                .child(name.to_owned()),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(MUTED))
                .truncate()
                .child(kind.to_owned()),
        );
    let height = |v: f32| ((20. * v.max(0.000001).log10() + 60.) / 60.).clamp(0., 1.) * 67.;
    strip = strip
        .child(
            div()
                .flex()
                .gap_2()
                .items_end()
                .h(px(69.))
                .child(
                    div()
                        .relative()
                        .w(px(10.))
                        .h_full()
                        .bg(rgb(0x080f16))
                        .child(
                            div()
                                .absolute()
                                .bottom_0()
                                .w_full()
                                .h(px(height(peak)))
                                .bg(rgb(if peak >= 1. { 0xe88b79 } else { ACCENT })),
                        ),
                )
                .child(
                    div()
                        .relative()
                        .w(px(10.))
                        .h_full()
                        .bg(rgb(0x080f16))
                        .child(
                            div()
                                .absolute()
                                .bottom_0()
                                .w_full()
                                .h(px(height(rms)))
                                .bg(rgb(0x457f90)),
                        ),
                )
                .child(div().text_xs().text_color(rgb(MUTED)).child(format!(
                    "{} peak\n{} RMS",
                    db(peak),
                    db(rms)
                ))),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(if mute || solo { GOLD } else { MUTED }))
                .child(format!(
                    "{:.1} dB · {:+.2}{}{}",
                    20. * level.max(1e-6).log10(),
                    pan,
                    if mute { " M" } else { "" },
                    if solo { " S" } else { "" }
                )),
        );
    for effect in effects {
        strip = strip.child(
            div()
                .text_xs()
                .truncate()
                .text_color(rgb(ACCENT))
                .child(effect),
        );
    }
    if id == "mix_master" {
        strip = strip.child(div().text_xs().text_color(rgb(GOLD)).child(format!(
            "{} dBTP · scope",
            db(analysis.map_or(0., |a| a.true_peak))
        )));
    }
    if let Some(a) = analysis.filter(|a| a.dropped > 0) {
        strip = strip.child(
            div()
                .text_xs()
                .text_color(rgb(GOLD))
                .child(format!("{} analysis drops", a.dropped)),
        );
    }
    for route in routes {
        strip = strip.child(
            div()
                .text_xs()
                .truncate()
                .text_color(rgb(MUTED))
                .child(route),
        );
    }
    strip
}
