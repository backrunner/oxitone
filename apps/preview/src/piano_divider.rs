//! Measure the velocity boundary in the same frame as the piano canvas, including editor switches.
use crate::{
    piano_layout::{velocity_height, RULER, SCROLLBAR},
    ui::Preview,
    workspace::Gesture,
    workspace_layout::DIVIDER,
    workspace_resize::Resize,
};
use gpui::{prelude::*, *};

pub fn view(this: &Preview, cx: &Context<Preview>) -> impl IntoElement {
    let theme = this.theme;
    let requested = this.piano.velocity_height;
    let dividers = this.workspace.dividers.clone();
    let owner = cx.entity().downgrade();
    canvas(
        move |bounds, window, _| {
            let height = f32::from(bounds.size.height);
            let velocity = velocity_height(height, requested);
            let top = RULER + (height - RULER - velocity - SCROLLBAR).max(1.) - DIVIDER * 0.5;
            let area = Bounds::new(
                bounds.origin + point(px(0.), px(top)),
                size(bounds.size.width, px(DIVIDER)),
            );
            dividers.borrow_mut().insert("piano-velocity-divider", area);
            (
                window.insert_hitbox(area, HitboxBehavior::Normal),
                area,
                Resize::Velocity {
                    height: velocity,
                    max: height * 0.4,
                },
            )
        },
        move |_, (hitbox, area, region), window, _| {
            window.set_cursor_style(CursorStyle::ResizeUpDown, &hitbox);
            window.paint_quad(fill(
                area,
                rgb(if hitbox.is_hovered(window) {
                    theme.selected
                } else {
                    theme.bg
                }),
            ));
            window.paint_quad(fill(
                Bounds::new(
                    area.origin + point(px(0.), px(3.)),
                    size(area.size.width, px(1.)),
                ),
                rgb(theme.border),
            ));
            window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                if phase != DispatchPhase::Bubble
                    || event.button != MouseButton::Left
                    || !hitbox.is_hovered(window)
                {
                    return;
                }
                let _ = owner.update(cx, |this, cx| {
                    this.workspace.gesture = Some(Gesture::Resize {
                        pointer: event.position,
                        region,
                    });
                    cx.stop_propagation();
                    cx.notify();
                });
            });
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}
