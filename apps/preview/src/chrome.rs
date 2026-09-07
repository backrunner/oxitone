use crate::ui::*;
use gpui::{prelude::*, *};
use serde_json::json;

impl Preview {
    pub(crate) fn header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let name = self
            .project
            .as_ref()
            .and_then(|p| p.snapshot.name.clone())
            .unwrap_or_else(|| "Open an Oxitone project".into());
        div()
            .h(px(60.))
            .px_5()
            .flex()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(rgb(BORDER))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(ACCENT))
                            .child("OXITONE"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MUTED))
                            .child("PROJECT PREVIEW"),
                    ),
            )
            .child(div().text_sm().child(name))
            .child(
                div()
                    .flex()
                    .gap_3()
                    .items_center()
                    .child(div().size(px(6.)).rounded_full().bg(rgb(
                        if self.diagnostic.is_some() {
                            0xe88b79
                        } else {
                            ACCENT
                        },
                    )))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MUTED))
                            .child(self.status.clone()),
                    )
                    .child(
                        button(
                            "scopes",
                            if self.show_scopes {
                                "Scopes on"
                            } else {
                                "Scopes off"
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.show_scopes = !this.show_scopes;
                            cx.notify();
                        })),
                    ),
            )
    }

    pub(crate) fn transport_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let beat = self
            .project
            .as_ref()
            .map_or(0.0, |p| p.beat(self.playback.audible));
        let (bar, within) = self
            .project
            .as_ref()
            .map(|p| {
                p.plan
                    .time_signatures
                    .beat_to_bar_beat(oxitone_core::Beat::from_f64(beat).unwrap())
            })
            .unwrap_or((1, oxitone_core::Beat::ZERO));
        let bpm = self
            .project
            .as_ref()
            .map_or(120.0, |p| p.plan.tempo.bpm_at_beat(beat));
        let seconds = self.project.as_ref().map_or(0.0, |p| {
            self.playback.audible as f64 / f64::from(p.snapshot.sample_rate)
        });
        let mut controls =
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    button(
                        "play",
                        if self.playback.playing {
                            "Pause"
                        } else {
                            "▶ Play"
                        },
                    )
                    .bg(rgb(if self.playback.playing {
                        0x315d59
                    } else {
                        0x24443f
                    }))
                    .on_click(cx.listener(|this, _, _, _| {
                        if this.playback.playing {
                            this.transport(json!({"command":"pause"}));
                        } else {
                            this.play();
                        }
                    })),
                )
                .child(button("stop", "■ Stop").on_click(
                    cx.listener(|this, _, _, _| this.transport(json!({"command":"stop"}))),
                ))
                .child(
                    button(
                        "loop",
                        if self.loop_enabled {
                            "Loop on"
                        } else {
                            "Loop off"
                        },
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.loop_enabled = !this.loop_enabled;
                        if this.playback.playing {
                            this.play();
                        }
                        cx.notify();
                    })),
                )
                .child(div().ml_4().text_lg().font_family("Menlo").child(format!(
                    "{:03}.{:02}.{:03}",
                    bar,
                    within.to_f64().floor() as u32 + 1,
                    (within.to_f64().fract() * 960.) as u32
                )))
                .child(div().ml_3().text_xs().text_color(rgb(MUTED)).child(format!(
                    "{:02}:{:05.2}  /  {:.1} BPM",
                    (seconds / 60.) as u32,
                    seconds % 60.,
                    bpm
                )));
        controls = controls.child(crate::position::view(self, cx));
        if let Some(project) = &self.project {
            for (index, marker) in project.snapshot.markers.iter().take(8).enumerate() {
                let beat = marker.start_beat.to_f64();
                controls = controls.child(
                    button(
                        format!("marker-{index}"),
                        marker
                            .name
                            .clone()
                            .unwrap_or_else(|| format!("M{}", index + 1)),
                    )
                    .on_click(cx.listener(move |this, _, _, _| this.seek(beat))),
                );
            }
        }
        div()
            .h(px(56.))
            .px_5()
            .flex()
            .items_center()
            .justify_between()
            .bg(rgb(PANEL))
            .border_b_1()
            .border_color(rgb(BORDER))
            .child(controls)
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        button("zoom-out", "−").on_click(cx.listener(|this, _, _, cx| {
                            this.zoom = (this.zoom / 1.25).max(5.);
                            cx.notify();
                        })),
                    )
                    .child(
                        button("zoom-in", "+").on_click(cx.listener(|this, _, _, cx| {
                            this.zoom = (this.zoom * 1.25).min(120.);
                            cx.notify();
                        })),
                    ),
            )
    }

    pub(crate) fn footer(&self) -> impl IntoElement {
        let revision = self.project.as_ref().map_or(0, |p| p.snapshot.revision);
        div()
            .h(px(28.))
            .px_5()
            .flex()
            .items_center()
            .justify_between()
            .text_xs()
            .text_color(rgb(MUTED))
            .border_t_1()
            .border_color(rgb(BORDER))
            .child(format!(
                "READ ONLY  ·  Source code controls this project  ·  Revision {revision}"
            ))
            .child(format!(
                "Load {:.1}%  ·  Latency {} frames  ·  Xruns {}  ·  Plugin faults {} {}",
                self.playback.load * 100.,
                self.playback.latency,
                self.playback.xruns,
                self.playback.faults,
                self.playback.fault_nodes
            ))
    }
}
