//! Shared native controls; semantic palette stays in theme.rs.
use crate::theme::Theme;
use gpui::{prelude::*, *};
impl Theme {
    fn control(self, id: impl Into<SharedString>, label: impl Into<SharedString>) -> Stateful<Div> {
        div()
            .id(ElementId::Name(id.into()))
            .px(px(10.))
            .h(px(26.))
            .flex()
            .items_center()
            .justify_center()
            .flex_shrink_0()
            .rounded(px(4.))
            .bg(rgb(self.button))
            .text_color(rgb(self.text))
            .text_size(px(11.))
            .font_weight(FontWeight::MEDIUM)
            .cursor_pointer()
            .child(label.into())
    }

    pub fn button(
        self,
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
    ) -> Stateful<Div> {
        self.control(id, label)
            .hover(move |style| style.bg(rgb(self.button_hover)))
            .active(move |style| style.bg(rgb(self.selected)))
    }

    pub fn ghost(
        self,
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
    ) -> Stateful<Div> {
        self.button(id, label).bg(crate::ui::alpha(self.bg, 0.))
    }

    pub fn primary(
        self,
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
    ) -> Stateful<Div> {
        self.control(id, label)
            .bg(rgb(self.accent))
            .text_color(rgb(self.on_accent))
            .hover(move |s| s.bg(crate::ui::alpha(self.accent, 0.85)))
            .active(move |s| s.bg(crate::ui::alpha(self.accent, 0.7)))
    }

    pub fn icon_button(
        self,
        id: impl Into<SharedString>,
        kind: crate::ui_icons::Icon,
        tip: &'static str,
    ) -> Stateful<Div> {
        self.ghost(id, "")
            .px_0()
            .w(px(28.))
            .child(crate::ui_icons::icon(kind, self.muted))
            .tooltip(move |_, cx| {
                cx.new(|_| ButtonTip {
                    text: tip.into(),
                    theme: self,
                })
                .into()
            })
    }

    pub fn icon_tool(
        self,
        id: impl Into<SharedString>,
        kind: crate::ui_icons::Icon,
        tip: &'static str,
        selected: bool,
    ) -> Stateful<Div> {
        self.tool(id, "", selected)
            .px_0()
            .w(px(28.))
            .child(crate::ui_icons::icon(
                kind,
                if selected { self.accent } else { self.muted },
            ))
            .tooltip(move |_, cx| {
                cx.new(|_| ButtonTip {
                    text: tip.into(),
                    theme: self,
                })
                .into()
            })
    }

    pub fn tool_group(self) -> Div {
        div()
            .flex()
            .items_center()
            .gap(px(2.))
            .p(px(2.))
            .rounded(px(5.))
            .bg(rgb(self.bg))
            .flex_shrink_0()
    }

    pub fn surface(self) -> Div {
        div()
            .bg(rgb(self.panel))
            .rounded_lg()
            .border_1()
            .border_color(rgb(self.border))
            .shadow(vec![BoxShadow {
                color: crate::ui::alpha(0x000000, 0.10).into(),
                offset: point(px(0.), px(3.)),
                blur_radius: px(12.),
                spread_radius: px(0.),
            }])
    }

    pub fn tool(
        self,
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        selected: bool,
    ) -> Stateful<Div> {
        self.ghost(id, label).when(selected, |d| {
            d.bg(rgb(self.raised)).text_color(rgb(self.accent))
        })
    }

    pub fn label(self, text: impl Into<SharedString>) -> Div {
        div()
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(self.muted))
            .child(text.into())
    }

    pub fn tab(self, id: &'static str, label: String, selected: bool) -> Stateful<Div> {
        div()
            .id(id)
            .h_full()
            .flex()
            .items_center()
            .px_1()
            .border_b_2()
            .border_color(if selected {
                rgb(self.accent)
            } else {
                crate::ui::alpha(self.border, 0.)
            })
            .text_size(px(11.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(if selected { self.text } else { self.muted }))
            .cursor_pointer()
            .hover(move |s| s.text_color(rgb(self.text)))
            .child(label)
    }
}

struct ButtonTip {
    text: SharedString,
    theme: Theme,
}
impl Render for ButtonTip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.theme
            .surface()
            .px_2()
            .py_1()
            .text_size(px(11.))
            .text_color(rgb(self.theme.text))
            .child(self.text.clone())
    }
}
