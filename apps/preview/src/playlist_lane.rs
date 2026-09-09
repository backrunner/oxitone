use crate::ui::{alpha, Preview};
use gpui::{prelude::*, *};
const ROW: f32 = 52.;

pub fn rows(
    this: &Preview,
    width: f32,
    cursor: f32,
    grid_ticks: &[(f32, bool)],
    cx: &mut Context<Preview>,
) -> (Div, Div) {
    let theme = this.theme;
    let project = this.project.as_ref().unwrap();
    let zoom = this.zoom;
    let offset = this.workspace.arrangement.offset();
    // Bounds are refreshed on every layout pass so a resize or scroll cannot
    // leave stale drag targets behind.
    this.document.playlist.rows.borrow_mut().clear();
    this.document.playlist.controls.borrow_mut().clear();
    let drag = this.document.playlist.drag.as_ref().or_else(|| {
        this.document
            .playlist
            .pending
            .as_ref()
            .filter(|_| this.presentation_active())
    });
    let clips =
        crate::playlist_projection::project(crate::playlist_projection::placements(project), drag);
    let mut lanes = div().w(px(width)).flex().flex_col();
    let mut headers = div().absolute().top(offset.y).w_full().flex().flex_col();
    for (index, track) in project.snapshot.tracks.iter().enumerate() {
        let tint = theme.track(index);
        let mut lane = div()
            .relative()
            .w(px(width))
            .h(px(ROW))
            .flex_shrink_0()
            .overflow_hidden()
            .bg(rgb(theme.lanes[index % 2]))
            .border_b_1()
            .border_color(alpha(theme.border, 0.6));
        let bounds = this.document.playlist.rows.clone();
        let track_key = track.id.clone();
        lane = lane.child(
            canvas(
                move |area, _, _| {
                    bounds.borrow_mut().insert(track_key.clone(), area);
                },
                |_, _, _, _| {},
            )
            .absolute()
            .size_full(),
        );
        for (x, downbeat) in grid_ticks {
            lane = lane.child(
                div()
                    .absolute()
                    .left(px(*x))
                    .w(px(1.))
                    .h_full()
                    .bg(alpha(theme.border, if *downbeat { 0.8 } else { 0.25 })),
            );
        }
        lane = crate::playlist_clips::view(this, lane, track, tint, &clips, cx);
        if this.loop_enabled {
            lane = lane.child(
                div()
                    .absolute()
                    .left(px(this.loop_start as f32 * zoom))
                    .top_0()
                    .w(px(((this.loop_end - this.loop_start) as f32 * zoom).max(0.)))
                    .h(px(2.))
                    .bg(rgb(theme.gold)),
            );
        }
        lane = lane
            .child(
                div()
                    .absolute()
                    .left(px(project.beat(this.cue_frame) as f32 * zoom))
                    .top_0()
                    .w(px(2.))
                    .h(px(10.))
                    .bg(rgb(theme.accent)),
            )
            .child(
                div()
                    .absolute()
                    .left(px(cursor))
                    .top_0()
                    .w(px(1.))
                    .h_full()
                    .bg(rgb(theme.gold)),
            );
        lanes = lanes.child(lane);
        headers = headers.child(crate::channel_header::view(track, index, tint, this, cx));
    }
    (lanes, headers)
}
