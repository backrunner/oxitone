use crate::ui::*;
use gpui::{prelude::*, *};

impl Preview {
    pub(crate) fn transport_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        let beat = self
            .project
            .as_ref()
            .map_or(0.0, |p| p.beat(self.position_frame()));
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
            self.position_frame() as f64 / f64::from(p.snapshot.sample_rate)
        });
        let mut controls =
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    theme
                        .button(
                            "play",
                            if self.is_playing() {
                                "Pause"
                            } else {
                                "▶ Play"
                            },
                        )
                        .bg(rgb(theme.selected))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.workspace_focus.focus(window);
                            this.toggle_playback();
                            cx.notify();
                        })),
                )
                .child(theme.button("stop", "■ Stop").on_click(cx.listener(
                    |this, _, window, cx| {
                        this.workspace_focus.focus(window);
                        this.stop_at_cue();
                        cx.notify();
                    },
                )))
                .child(
                    theme
                        .button(
                            "loop",
                            if self.loop_enabled {
                                "Loop on"
                            } else {
                                "Loop off"
                            },
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.workspace_focus.focus(window);
                            this.toggle_loop();
                            cx.notify();
                        })),
                )
                .child(div().ml_4().text_lg().font_family("Menlo").child(format!(
                    "{:03}.{:02}.{:03}",
                    bar,
                    within.to_f64().floor() as u32 + 1,
                    (within.to_f64().fract() * 960.) as u32
                )))
                .child(
                    div()
                        .ml_3()
                        .text_xs()
                        .text_color(rgb(theme.muted))
                        .child(format!(
                            "{:02}:{:05.2}  /  {:.1} BPM",
                            (seconds / 60.) as u32,
                            seconds % 60.,
                            bpm
                        )),
                );
        controls = controls.child(crate::position::view(self, cx));
        div()
            .h(px(48.))
            .flex_shrink_0()
            .px_5()
            .flex()
            .items_center()
            .justify_between()
            .bg(rgb(theme.panel))
            .border_b_1()
            .border_color(rgb(theme.border))
            .child(controls)
            .child(
                div()
                    .flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(rgb(theme.muted))
                            .child("PLAYLIST ZOOM"),
                    )
                    .child(theme.button("playlist-fit", "Fit").on_click(cx.listener(
                        |this, _, window, cx| {
                            if let Some(project) = &this.project {
                                this.zoom = ((f32::from(window.viewport_size().width) - 190.)
                                    / project.end() as f32)
                                    .clamp(0.01, 120.);
                                this.workspace.arrangement.set_offset(point(px(0.), px(0.)));
                                cx.notify();
                            }
                        },
                    )))
                    .child(
                        theme
                            .button("zoom-out", "−")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.zoom = (this.zoom / 1.25).max(0.01);
                                cx.notify();
                            })),
                    )
                    .child(
                        theme
                            .button("zoom-in", "+")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.zoom = (this.zoom * 1.25).min(120.);
                                cx.notify();
                            })),
                    ),
            )
    }

    pub(crate) fn footer(&self) -> impl IntoElement {
        let theme = self.theme;
        let revision = self.project.as_ref().map_or(0, |p| p.snapshot.revision);
        div()
            .h(px(28.))
            .flex_shrink_0()
            .px_5()
            .flex()
            .items_center()
            .justify_between()
            .text_xs()
            .text_color(rgb(theme.muted))
            .border_t_1()
            .border_color(rgb(theme.border))
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
