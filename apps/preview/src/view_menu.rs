//! Dropdown layout and keyboard capture, independent from musical document operations.
use crate::{
    ui::Preview,
    ui_icons::{icon, Icon},
    window_navigation::View,
};
use gpui::{prelude::*, *};
use std::{cell::Cell, rc::Rc};

#[derive(Default)]
pub struct ViewMenu {
    pub selected: Option<usize>,
    pub button: Rc<Cell<Bounds<Pixels>>>,
    pub bounds: Rc<Cell<Bounds<Pixels>>>,
}

pub fn menu(this: &Preview, cx: &mut Context<Preview>) -> impl IntoElement {
    let t = this.theme;
    let bounds = this.view_menu.bounds.clone();
    let mut menu = t
        .surface()
        .id("view-menu")
        .occlude()
        .w(px(192.))
        .p_1()
        .relative()
        .flex()
        .flex_col()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            canvas(move |area, _, _| bounds.set(area), |_, _, _, _| {})
                .absolute()
                .size_full(),
        );
    for (index, view) in this.view_options().into_iter().enumerate() {
        if index == 4 || matches!(view, View::About) {
            menu = menu.child(div().h(px(1.)).my_1().mx_2().bg(rgb(t.border)));
        }
        let active = view.active(this);
        menu = menu.child(
            t.tool(
                format!("view-menu-{index}"),
                "",
                this.view_menu.selected == Some(index),
            )
            .h(px(30.))
            .gap_2()
            .justify_start()
            .child(icon(view.icon(), if active { t.accent } else { t.muted }))
            .child(div().flex_1().child(view.label()))
            .when(active, |d| d.child(icon(Icon::Check, t.accent)))
            .on_mouse_move(cx.listener(move |this, _, _, cx| {
                if this.view_menu.selected != Some(index) {
                    this.view_menu.selected = Some(index);
                    cx.notify();
                }
            }))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.open_view(view, window);
                cx.stop_propagation();
                cx.notify();
            })),
        );
    }
    let button = this.view_menu.button.get();
    // The shield dismisses without activating controls or notes underneath it.
    deferred(
        div()
            .absolute()
            .inset_0()
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.view_menu.selected = None;
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .child(
                anchored()
                    .anchor(Corner::TopRight)
                    .position(point(button.right(), button.bottom() + px(6.)))
                    .snap_to_window_with_margin(px(8.))
                    .child(menu),
            ),
    )
    .with_priority(1)
}

impl Preview {
    pub fn view_menu_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(selected) = self.view_menu.selected else {
            return;
        };
        let options = self.view_options();
        let count = options.len();
        let key = &event.keystroke;
        if !key.modifiers.platform && !key.modifiers.control && !key.modifiers.alt {
            match key.key.as_str() {
                "escape" | "tab" => self.view_menu.selected = None,
                "up" => self.view_menu.selected = Some((selected + count - 1) % count),
                "down" => self.view_menu.selected = Some((selected + 1) % count),
                "home" => self.view_menu.selected = Some(0),
                "end" => self.view_menu.selected = Some(count - 1),
                "enter" | "space" if !event.is_held => {
                    self.open_view(options[selected.min(count - 1)], window)
                }
                "f5" | "f7" | "f9" => {
                    self.view_menu.selected = None;
                    self.workspace_key(event, window, cx);
                }
                _ => {}
            }
        }
        cx.stop_propagation();
        cx.notify();
    }
}
