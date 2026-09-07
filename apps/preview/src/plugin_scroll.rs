use crate::{plugin_window::PluginWindow, workspace::thumb};
use gpui::{prelude::*, *};
use std::{cell::Cell, rc::Rc};

pub fn view(this: &PluginWindow, cx: &mut Context<PluginWindow>) -> impl IntoElement {
    let theme = this.theme;
    let bounds = Rc::new(Cell::new(Bounds::default()));
    let measured = bounds.clone();
    let scroll = this.scroll.clone();
    div()
        .id("plugin-detail-scrollbar")
        .w(px(12.))
        .h_full()
        .flex_shrink_0()
        .cursor_pointer()
        .child(
            canvas(
                |_, _, _| {},
                move |at, _, window, _| {
                    measured.set(at);
                    let max = f32::from(scroll.max_offset().height);
                    let (start, length) = thumb(
                        f32::from(at.size.height),
                        f32::from(scroll.bounds().size.height),
                        max,
                        f32::from(scroll.offset().y),
                    );
                    window.paint_quad(
                        fill(
                            Bounds::new(
                                at.origin + point(px(4.), px(start)),
                                size(px(4.), px(length)),
                            ),
                            rgb(if max > 0. { theme.muted } else { theme.border }),
                        )
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
                let max = f32::from(this.scroll.max_offset().height);
                let track = f32::from(at.size.height);
                let (start, length) = thumb(
                    track,
                    f32::from(this.scroll.bounds().size.height),
                    max,
                    f32::from(this.scroll.offset().y),
                );
                if max <= 0. || track <= length {
                    return;
                }
                let local = f32::from(event.position.y - at.origin.y);
                if local < start || local > start + length {
                    this.scroll.set_offset(point(
                        px(0.),
                        px(-((local - length * 0.5) / (track - length)).clamp(0., 1.) * max),
                    ));
                }
                this.scroll_drag = Some((
                    f32::from(event.position.y),
                    f32::from(this.scroll.offset().y),
                    max / (track - length),
                ));
                cx.stop_propagation();
                cx.notify();
            }),
        )
}
