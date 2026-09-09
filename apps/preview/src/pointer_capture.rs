//! Window capture keeps gestures alive across sibling hitboxes and outside their starting panel.
use crate::ui::Preview;
use gpui::{prelude::*, *};

pub fn cancel(this: &mut Preview) {
    this.document.plugin.cancel();
    this.document.gesture = None;
    this.document.notes.marquee = None;
    this.document.playlist.drag = None;
    this.document.mixer.gesture = None;
    this.document.automation.gesture = None;
    if let Some(crate::workspace::Gesture::Resize { region, .. }) = this.workspace.gesture.take() {
        crate::workspace_resize::apply(this, region, point(px(0.), px(0.)));
    }
    this.document.windows.cancel();
}

pub fn view(cx: &Context<Preview>) -> impl IntoElement {
    let entity = cx.entity().downgrade();
    canvas(
        |_, _, _| {},
        move |_, _, window, _| {
            let focused = entity.clone();
            window.on_mouse_event(move |event: &MouseDownEvent, phase, _, cx| {
                if phase != DispatchPhase::Capture {
                    return;
                }
                let _ = focused.update(cx, |this, cx| {
                    if this.document.tempo.input.is_some()
                        && !this.document.tempo.bounds.get().contains(&event.position)
                    {
                        this.document.tempo.input = None;
                        cx.notify();
                    }
                    if this.close.open || this.show_shortcuts {
                        return;
                    }
                    if let Some(id) = this.document.windows.hit(event.position) {
                        if this.document.windows.state(id).z != this.document.windows.next_z {
                            this.document.windows.focus(id);
                            cx.notify();
                        }
                    }
                });
            });
            let moving = entity.clone();
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                if phase != DispatchPhase::Capture {
                    return;
                }
                let _ = moving.update(cx, |this, cx| {
                    if this.close.open || this.show_shortcuts {
                        return;
                    }
                    let active = this.workspace.gesture.is_some()
                        || this.document.plugin.gesture.is_some()
                        || this.document.windows.drag.is_some()
                        || this.document.playlist.drag.is_some()
                        || this.document.mixer.gesture.is_some()
                        || this.document.gesture.is_some()
                        || this.document.notes.marquee.is_some()
                        || this.document.automation.gesture.is_some();
                    if !active {
                        return;
                    }
                    if event.pressed_button.is_some() {
                        this.document.windows.move_drag(event.position);
                        this.move_playlist(event);
                        this.move_mix(event);
                        if event.pressed_button == Some(MouseButton::Left) {
                            this.move_plugin_parameter(event);
                        }
                        if this.document.automation.gesture.is_some() {
                            this.move_curve(event.position, event.modifiers.shift);
                        }
                        if this.document.gesture.is_some() || this.document.notes.marquee.is_some()
                        {
                            this.move_note(event);
                        }
                        if this.workspace.gesture.is_some() {
                            this.move_gesture(event, window);
                        }
                    }
                    cx.notify();
                    cx.stop_propagation();
                });
            });
            let released = entity.clone();
            window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                if phase != DispatchPhase::Capture
                    || !matches!(
                        event.button,
                        MouseButton::Left | MouseButton::Right | MouseButton::Middle
                    )
                {
                    return;
                }
                let _ = released.update(cx, |this, cx| {
                    if this.close.open || this.show_shortcuts {
                        return;
                    }
                    if this.workspace.gesture.is_some()
                        || this.document.plugin.gesture.is_some()
                        || this.document.windows.drag.is_some()
                        || this.document.playlist.drag.is_some()
                        || this.document.mixer.gesture.is_some()
                        || this.document.gesture.is_some()
                        || this.document.notes.marquee.is_some()
                        || this.document.automation.gesture.is_some()
                    {
                        this.workspace.gesture = None;
                        if event.button == MouseButton::Left {
                            this.move_plugin_parameter(&MouseMoveEvent {
                                position: event.position,
                                modifiers: event.modifiers,
                                ..Default::default()
                            });
                            this.finish_plugin_parameter();
                        }
                        this.document.windows.end_drag();
                        this.move_playlist(&MouseMoveEvent {
                            position: event.position,
                            modifiers: event.modifiers,
                            pressed_button: Some(event.button),
                            ..Default::default()
                        });
                        this.finish_playlist();
                        this.move_mix(&MouseMoveEvent {
                            position: event.position,
                            modifiers: event.modifiers,
                            ..Default::default()
                        });
                        this.finish_mix();
                        this.move_note(&MouseMoveEvent {
                            position: event.position,
                            modifiers: event.modifiers,
                            pressed_button: Some(event.button),
                            ..Default::default()
                        });
                        this.finish_note();
                        this.finish_automation();
                        cx.notify();
                        cx.stop_propagation();
                    }
                });
            });
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size(px(0.))
}
