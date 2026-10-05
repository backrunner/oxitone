use crate::{ui::*, ui_icons::Icon};
use gpui::{prelude::*, *};

impl Preview {
    pub(crate) fn title_transport(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
        let bounds = self.transport_bounds.clone();
        div()
            .id("transport-controls")
            .relative()
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap_2()
            .child(
                canvas(move |area, _, _| bounds.set(area), |_, _, _, _| {})
                    .absolute()
                    .size_full(),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
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
                        .size(px(28.))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.workspace_focus.focus(window);
                            this.toggle_playback();
                            cx.notify();
                        })),
                    )
                    .child(
                        t.icon_button("stop", Icon::Stop, "Stop · Shift Space")
                            .size(px(28.))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.workspace_focus.focus(window);
                                this.stop_at_cue();
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .px_2()
                    .min_w(px(108.))
                    .h(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.))
                    .bg(rgb(t.bg))
                    .font_family("Menlo")
                    .text_size(px(14.))
                    .child(format!(
                        "{:03}.{:02}.{:03}",
                        bar,
                        within.to_f64().floor() as u32 + 1,
                        (within.to_f64().fract() * 960.) as u32
                    )),
            )
    }

    pub(crate) fn playlist_zoom(&self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        div()
            .flex()
            .items_center()
            .gap_1()
            .flex_shrink_0()
            .child(
                t.icon_button("playlist-fit", Icon::Fit, "Fit arrangement")
                    .size(px(24.))
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
                    .size(px(24.))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.zoom = (this.zoom / 1.25).max(0.01);
                        cx.notify();
                    })),
            )
            .child(
                t.icon_button("zoom-in", Icon::Plus, "Zoom in")
                    .size(px(24.))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.zoom = (this.zoom * 1.25).min(120.);
                        cx.notify();
                    })),
            )
    }
}
