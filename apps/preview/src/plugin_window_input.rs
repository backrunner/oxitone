//! Focus, keyboard navigation and pointer handling for an embedded plugin panel.
use crate::plugin_window::PluginWindow;
use gpui::{prelude::*, *};

impl Render for PluginWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme;
        self.parameter_bounds.borrow_mut().clear();
        if self.request_focus {
            self.focus.focus(window);
            self.request_focus = false;
        }
        div()
            .id("plugin-details")
            .size_full()
            .flex()
            .flex_col()
            .font_family("Helvetica Neue")
            .bg(rgb(theme.bg))
            .text_color(rgb(theme.text))
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "escape"
                    && this
                        .owner
                        .upgrade()
                        .is_some_and(|owner| owner.read(cx).document.plugin.gesture.is_some())
                {
                    let _ = this.owner.update(cx, |owner, cx| {
                        owner.document.plugin.cancel();
                        cx.notify();
                    });
                    cx.stop_propagation();
                    return;
                }
                if key == "escape" && this.choice_open.take().is_some() {
                    cx.stop_propagation();
                    cx.notify();
                    return;
                }
                if key == "escape" || (key == "w" && event.keystroke.modifiers.platform) {
                    let id = crate::window_manager::WindowId::Plugin(cx.entity_id().as_u64());
                    let _ = this.owner.update(cx, |owner, cx| {
                        owner.close_internal(id, window);
                        cx.notify();
                    });
                    cx.stop_propagation();
                    return;
                }
                if let Some(shortcut) = crate::shortcuts::playback(&event.keystroke) {
                    if !event.is_held || shortcut.repeats() {
                        let _ = this.owner.update(cx, |owner, cx| {
                            owner.playback_shortcut(shortcut);
                            cx.notify();
                        });
                    }
                    cx.stop_propagation();
                    return;
                }
                let delta = match key {
                    "up" => Some(32.),
                    "down" => Some(-32.),
                    "pageup" => Some(f32::from(this.scroll.bounds().size.height)),
                    "pagedown" => Some(-f32::from(this.scroll.bounds().size.height)),
                    "home" => Some(f32::from(this.scroll.max_offset().height)),
                    "end" => Some(-f32::from(this.scroll.max_offset().height)),
                    _ => None,
                };
                if let Some(delta) = delta {
                    this.scroll.set_offset(point(
                        px(0.),
                        (this.scroll.offset().y + px(delta))
                            .clamp(-this.scroll.max_offset().height, px(0.)),
                    ));
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if event.pressed_button != Some(MouseButton::Left) {
                    this.scroll_drag = None;
                }
                if let Some((pointer, offset, scale)) = this.scroll_drag {
                    let next = (offset - (f32::from(event.position.y) - pointer) * scale)
                        .clamp(-f32::from(this.scroll.max_offset().height), 0.);
                    this.scroll.set_offset(point(px(0.), px(next)));
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.scroll_drag = None),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.scroll_drag = None),
            )
            .child(crate::plugin_window_view::view(self, self.width, cx))
    }
}
