use crate::{ui::*, ui_icons::Icon};
use gpui::{prelude::*, *};

impl Preview {
    pub(crate) fn transport_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let beat = self
            .project
            .as_ref()
            .map_or(0., |p| p.beat(self.position_frame()));
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
            .map_or(120., |p| p.plan.tempo.bpm_at_beat(beat));
        let controls = div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap_3()
            .child(
                t.tool_group()
                    .child(
                        t.icon_tool(
                            "play",
                            if self.is_playing() {
                                Icon::Pause
                            } else {
                                Icon::Play
                            },
                            "Play / pause · Space",
                            self.is_playing(),
                        )
                        .w(px(36.))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.workspace_focus.focus(window);
                            this.toggle_playback();
                            cx.notify();
                        })),
                    )
                    .child(
                        t.icon_button("stop", Icon::Stop, "Stop · Shift Space")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.workspace_focus.focus(window);
                                this.stop_at_cue();
                                cx.notify();
                            })),
                    )
                    .child(
                        t.icon_tool("loop", Icon::Loop, "Loop · L", self.loop_enabled)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.workspace_focus.focus(window);
                                this.toggle_loop();
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .px_3()
                    .h(px(32.))
                    .flex()
                    .items_center()
                    .rounded(px(4.))
                    .bg(rgb(t.bg))
                    .font_family("Menlo")
                    .text_size(px(17.))
                    .child(format!(
                        "{:03}.{:02}.{:03}",
                        bar,
                        within.to_f64().floor() as u32 + 1,
                        (within.to_f64().fract() * 960.) as u32
                    )),
            )
            .child(crate::tempo_edit::view(self, bpm, cx));
        div()
            .id("transport-controls")
            .h(px(48.))
            .flex_shrink_0()
            .px_3()
            .flex()
            .items_center()
            .gap_4()
            .overflow_x_scroll()
            .bg(rgb(t.panel))
            .border_b_1()
            .border_color(rgb(t.border))
            .child(controls)
            .child(div().flex_1())
            .child(self.document_controls(cx))
    }

    pub(crate) fn playlist_zoom(&self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        t.tool_group()
            .child(
                t.icon_button("playlist-fit", Icon::Fit, "Fit arrangement")
                    .on_click(cx.listener(|this, _, window, cx| {
                        if let Some(project) = &this.project {
                            this.zoom = ((f32::from(window.viewport_size().width) - 190.)
                                / project.end() as f32)
                                .clamp(0.01, 120.);
                            this.workspace.arrangement.set_offset(point(px(0.), px(0.)));
                            cx.notify();
                        }
                    })),
            )
            .child(
                t.icon_button("zoom-out", Icon::Minus, "Zoom out")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.zoom = (this.zoom / 1.25).max(0.01);
                        cx.notify();
                    })),
            )
            .child(
                t.icon_button("zoom-in", Icon::Plus, "Zoom in")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.zoom = (this.zoom * 1.25).min(120.);
                        cx.notify();
                    })),
            )
    }
}
