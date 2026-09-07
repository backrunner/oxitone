use crate::{
    ui::Preview,
    workspace::{thumb, Axis, Gesture},
};
use gpui::{prelude::*, *};
use std::{cell::Cell, rc::Rc};

pub fn view(
    id: &'static str,
    axis: Axis,
    handle: &ScrollHandle,
    this: &Preview,
    cx: &mut Context<Preview>,
) -> impl IntoElement {
    let theme = this.theme;
    let bounds = Rc::new(Cell::new(Bounds::default()));
    let measured = bounds.clone();
    let paint_handle = handle.clone();
    let input_handle = handle.clone();
    div()
        .id(id)
        .flex_shrink_0()
        .cursor_pointer()
        .map(|d| match axis {
            Axis::Horizontal => d.h(px(10.)).w_full(),
            Axis::Vertical => d.w(px(10.)).h_full(),
        })
        .bg(rgb(theme.bg))
        .child(
            canvas(
                |_, _, _| {},
                move |at, _, window, _| {
                    measured.set(at);
                    let max = axis.extent(paint_handle.max_offset());
                    let (start, length) = thumb(
                        axis.extent(at.size),
                        axis.extent(paint_handle.bounds().size),
                        max,
                        axis.coordinate(paint_handle.offset()),
                    );
                    let rect = match axis {
                        Axis::Horizontal => Bounds::new(
                            at.origin + point(px(start), px(3.)),
                            size(px(length), px(4.)),
                        ),
                        Axis::Vertical => Bounds::new(
                            at.origin + point(px(3.), px(start)),
                            size(px(4.), px(length)),
                        ),
                    };
                    window.paint_quad(
                        fill(rect, rgb(if max > 0. { theme.muted } else { theme.border }))
                            .corner_radii(px(2.)),
                    );
                },
            )
            .size_full(),
        )
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                let at = bounds.get();
                let max = axis.extent(input_handle.max_offset());
                let track = axis.extent(at.size);
                let (start, length) = thumb(
                    track,
                    axis.extent(input_handle.bounds().size),
                    max,
                    axis.coordinate(input_handle.offset()),
                );
                if max <= 0. || track <= length {
                    return;
                }
                let local = axis.coordinate(event.position - at.origin);
                if local < start || local > start + length {
                    let value = -((local - length * 0.5) / (track - length)).clamp(0., 1.) * max;
                    let old = input_handle.offset();
                    input_handle.set_offset(match axis {
                        Axis::Horizontal => point(px(value), old.y),
                        Axis::Vertical => point(old.x, px(value)),
                    });
                }
                this.workspace.gesture = Some(Gesture::Scroll {
                    handle: input_handle.clone(),
                    axis,
                    pointer: axis.coordinate(event.position),
                    offset: input_handle.offset(),
                    scale: max / (track - length),
                });
                cx.stop_propagation();
                cx.notify();
            }),
        )
}
