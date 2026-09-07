//! Playlist with pinned track headers/ruler and note thumbnails inside clips.
use crate::{
    ui::{alpha, Preview},
    workspace::Axis,
};
use gpui::{prelude::*, *};

const SIDEBAR: f32 = 176.;
pub fn view(this: &mut Preview, window_width: f32, cx: &mut Context<Preview>) -> impl IntoElement {
    let theme = this.theme;
    let project = this.project.as_ref().unwrap();
    let viewport = (window_width - SIDEBAR - 10.).max(1.);
    if !this.workspace.playlist_fitted {
        this.zoom = (viewport / project.end() as f32).clamp(0.01, 120.);
        this.workspace.playlist_fitted = true;
    }
    let zoom = this.zoom;
    let width = (project.end() as f32 * zoom).max(viewport);
    let cursor = project.beat(this.playback.audible) as f32 * zoom;
    let offset = this.workspace.arrangement.offset();
    let mut ruler = div().relative().h(px(30.)).w(px(width));
    let mut grid_ticks = Vec::new();
    let step = ((20. / zoom).ceil() as usize).max(1);
    let first = ((-f32::from(offset.x) / zoom).max(0.) as usize / step) * step;
    let last = (((-f32::from(offset.x) + viewport) / zoom).ceil() as usize + step)
        .min(project.end().ceil() as usize);
    for beat in (first..last).step_by(step) {
        let (bar, within) = project
            .plan
            .time_signatures
            .beat_to_bar_beat(oxitone_core::Beat::new(beat as i64, 1).unwrap());
        let downbeat = within == oxitone_core::Beat::ZERO;
        grid_ticks.push((beat as f32 * zoom, downbeat));
        ruler = ruler.child(
            div()
                .id(("beat", beat))
                .absolute()
                .left(px(beat as f32 * zoom))
                .top_0()
                .w(px(zoom * step as f32))
                .h_full()
                .border_l_1()
                .border_color(alpha(theme.border, if downbeat { 1. } else { 0.4 }))
                .px_1()
                .text_size(px(10.))
                .text_color(rgb(theme.muted))
                .cursor_pointer()
                .child(if downbeat {
                    format!("{bar}")
                } else {
                    "·".into()
                })
                .on_click(cx.listener(move |this, _, _, _| this.seek(beat as f64))),
        );
    }
    let (lanes, headers) = crate::playlist_lane::rows(this, width, cursor, &grid_ticks, cx);
    let mut markers = div()
        .id("marker-navigation")
        .flex_1()
        .min_w_0()
        .flex()
        .gap_2()
        .overflow_x_scroll();
    for (index, marker) in project.snapshot.markers.iter().enumerate() {
        let beat = marker.start_beat.to_f64();
        markers = markers.child(
            theme
                .button(
                    format!("marker-{index}"),
                    marker
                        .name
                        .clone()
                        .unwrap_or_else(|| format!("Marker {}", index + 1)),
                )
                .text_size(px(10.))
                .flex_shrink_0()
                .on_click(cx.listener(move |this, _, _, _| this.seek(beat))),
        );
    }
    div()
        .flex_1()
        .min_h(px(120.))
        .flex()
        .flex_col()
        .overflow_hidden()
        .child(
            div()
                .h(px(34.))
                .flex_shrink_0()
                .px_3()
                .flex()
                .items_center()
                .gap_3()
                .bg(rgb(theme.panel))
                .border_b_1()
                .border_color(rgb(theme.border))
                .child(
                    div()
                        .w(px(SIDEBAR - 24.))
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .child("PLAYLIST"),
                )
                .child(markers)
                .child(
                    div()
                        .text_size(px(10.))
                        .text_color(rgb(theme.muted))
                        .child(format!("{} tracks", project.snapshot.tracks.len())),
                ),
        )
        .child(
            div()
                .h(px(30.))
                .flex_shrink_0()
                .flex()
                .bg(rgb(theme.bg))
                .child(
                    div()
                        .w(px(SIDEBAR))
                        .flex_shrink_0()
                        .px_3()
                        .text_size(px(9.))
                        .text_color(rgb(theme.muted))
                        .child("TRACKS / BARS"),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .relative()
                        .overflow_hidden()
                        .child(div().absolute().left(offset.x).child(ruler)),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .child(
                    div()
                        .relative()
                        .w(px(SIDEBAR))
                        .flex_shrink_0()
                        .overflow_hidden()
                        .bg(rgb(theme.panel))
                        .child(headers),
                )
                .child(
                    div()
                        .id("arrangement-scroll")
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .overflow_scroll()
                        .track_scroll(&this.workspace.arrangement)
                        // A descendant handles the wheel before GPUI's scroll container
                        // bubbles it, so Shift/zoom never also perform a native scroll.
                        .child(lanes.id("playlist-lanes").min_h_full().on_scroll_wheel(
                            cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                                let scroll = &this.workspace.arrangement;
                                let delta = event.delta.pixel_delta(px(24.));
                                if event.modifiers.platform || event.modifiers.control {
                                    let old = this.zoom;
                                    this.zoom =
                                        (old * (f32::from(delta.y) / 150.).exp()).clamp(0.01, 120.);
                                    scroll.set_offset(point(
                                        scroll.offset().x * (this.zoom / old),
                                        scroll.offset().y,
                                    ));
                                } else {
                                    let delta = if event.modifiers.shift {
                                        point(delta.x + delta.y, px(0.))
                                    } else {
                                        delta
                                    };
                                    let max = scroll.max_offset();
                                    scroll.set_offset(point(
                                        (scroll.offset().x + delta.x).clamp(-max.width, px(0.)),
                                        (scroll.offset().y + delta.y).clamp(-max.height, px(0.)),
                                    ));
                                }
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        )),
                )
                .child(crate::scrollbar::view(
                    "playlist-vertical",
                    Axis::Vertical,
                    &this.workspace.arrangement,
                    this,
                    cx,
                )),
        )
        .child(
            div()
                .pl(px(SIDEBAR))
                .pr(px(10.))
                .child(crate::scrollbar::view(
                    "playlist-horizontal",
                    Axis::Horizontal,
                    &this.workspace.arrangement,
                    this,
                    cx,
                )),
        )
}
