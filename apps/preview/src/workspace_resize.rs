use crate::{
    ui::Preview,
    workspace::{Axis, Gesture},
    workspace_layout::{PanelLayout, DIVIDER},
};
use gpui::{prelude::*, *};

#[derive(Clone, Copy)]
pub enum Resize {
    Editor { height: f32, max: f32 },
    Split { width: f32, piano: f32 },
    Scopes { height: f32, max: f32 },
    Inspector { width: f32, max: f32 },
    Velocity { height: f32, max: f32 },
}
pub fn apply(this: &mut Preview, region: Resize, delta: Point<Pixels>) {
    let (x, y) = (f32::from(delta.x), f32::from(delta.y));
    match region {
        Resize::Editor { height, max } => {
            this.workspace.editor_height = (height - y).clamp(220_f32.min(max), max)
        }
        Resize::Split { width, piano } => {
            let available = (width - DIVIDER).max(1.);
            this.workspace.piano_fraction =
                PanelLayout::new(width, 1000., 400., (piano + x) / available, None).piano
                    / available;
        }
        Resize::Scopes { height, max } => {
            this.workspace.scopes_height = (height - y).clamp(60., max.max(60.))
        }
        Resize::Inspector { width, max } => {
            this.workspace.inspector_width = (width - x).clamp(190., max.max(190.))
        }
        Resize::Velocity { height, max } => {
            this.piano.velocity_height = (height - y).clamp(36., max.max(36.))
        }
    }
}
pub fn divider(
    id: &'static str,
    axis: Axis,
    region: Resize,
    this: &Preview,
    cx: &mut Context<Preview>,
) -> Stateful<Div> {
    let theme = this.theme;
    let bounds = this.workspace.dividers.clone();
    div()
        .id(id)
        .relative()
        .flex_shrink_0()
        .bg(rgb(theme.bg))
        .map(|d| match axis {
            Axis::Vertical => d.h(px(DIVIDER)).w_full().cursor(CursorStyle::ResizeUpDown),
            Axis::Horizontal => d
                .w(px(DIVIDER))
                .h_full()
                .cursor(CursorStyle::ResizeLeftRight),
        })
        .hover(move |d| d.bg(rgb(theme.selected)))
        .child(
            canvas(
                move |area, _, _| {
                    bounds.borrow_mut().insert(id, area);
                },
                move |area, _, window, _| {
                    let (origin, size) = match axis {
                        Axis::Vertical => (
                            area.origin + point(px(0.), px(3.)),
                            size(area.size.width, px(1.)),
                        ),
                        Axis::Horizontal => (
                            area.origin + point(px(3.), px(0.)),
                            size(px(1.), area.size.height),
                        ),
                    };
                    window.paint_quad(fill(Bounds::new(origin, size), rgb(theme.border)));
                },
            )
            .absolute()
            .size_full(),
        )
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                this.workspace.gesture = Some(Gesture::Resize {
                    pointer: event.position,
                    region,
                });
                cx.stop_propagation();
                cx.notify();
            }),
        )
}
