//! Branded About dialog and application/help actions.
use crate::{ui::Preview, ui_icons::Icon};
use gpui::{prelude::*, *};
use std::borrow::Cow;

actions!(oxitone, [ShowAbout, ShowShortcuts, Quit]);

pub struct BrandAssets;
impl AssetSource for BrandAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(
            (path == "oxitone-mark.svg").then_some(Cow::Borrowed(include_bytes!(
                "../../../assets/brand/oxitone-mark.svg"
            ))),
        )
    }
    fn list(&self, _: &str) -> Result<Vec<SharedString>> {
        Ok(vec!["oxitone-mark.svg".into()])
    }
}

impl Preview {
    pub fn about_action(&mut self, _: &ShowAbout, window: &mut Window, cx: &mut Context<Self>) {
        if !self.close.open {
            self.show_shortcuts = false;
            self.open_view(crate::window_navigation::View::About, window);
            cx.notify();
        }
    }
    pub fn help_action(&mut self, _: &ShowShortcuts, window: &mut Window, cx: &mut Context<Self>) {
        if !self.close.open {
            self.show_about = false;
            self.show_shortcuts = true;
            self.workspace_focus.focus(window);
            cx.notify();
        }
    }
    pub fn quit_action(&mut self, _: &Quit, window: &mut Window, cx: &mut Context<Self>) {
        self.request_close(window, cx);
    }
}

pub fn view(this: &Preview, cx: &mut Context<Preview>) -> impl IntoElement {
    let t = this.theme;
    let header = div()
        .h(px(34.))
        .flex()
        .items_center()
        .justify_end()
        .px_2()
        .child(
            t.icon_button("about-close", Icon::Close, "Close")
                .on_click(cx.listener(|this, _, window, cx| {
                    this.show_about = false;
                    this.workspace_focus.focus(window);
                    cx.notify();
                })),
        );
    let links = div()
        .mt_6()
        .flex()
        .gap_2()
        .child(
            t.button("about-project", "Project & documentation")
                .on_click(|_, _, cx| {
                    cx.open_url("https://github.com/backrunner/oxitone");
                }),
        )
        .child(t.button("about-license", "MPL 2.0").on_click(|_, _, cx| {
            cx.open_url("https://github.com/backrunner/oxitone/blob/main/LICENSE");
        }));
    let content = div()
        .px_8()
        .pb_8()
        .flex()
        .flex_col()
        .items_center()
        .child(
            svg()
                .path("oxitone-mark.svg")
                .size(px(88.))
                .text_color(rgb(t.accent)),
        )
        .child(
            div()
                .mt_2()
                .text_size(px(34.))
                .font_weight(FontWeight::SEMIBOLD)
                .child("oxitone"),
        )
        .child(
            div()
                .mt_2()
                .text_size(px(15.))
                .text_color(rgb(t.muted))
                .child("Code your sound."),
        )
        .child(
            div()
                .mt_4()
                .px_3()
                .py_1()
                .rounded_full()
                .bg(rgb(t.raised))
                .text_size(px(11.))
                .child(format!(
                    "Version {} · {}",
                    env!("CARGO_PKG_VERSION"),
                    std::env::consts::ARCH
                )),
        )
        .child(
            div()
                .mt_5()
                .text_size(px(12.))
                .text_color(rgb(t.muted))
                .text_center()
                .child("Compose in TypeScript. Shape every sound."),
        )
        .child(
            div()
                .mt_1()
                .text_size(px(12.))
                .text_color(rgb(t.muted))
                .text_center()
                .child("A native workspace for patterns, instruments and audio."),
        )
        .child(links);
    let footer = div()
        .border_t_1()
        .border_color(rgb(t.border))
        .px_5()
        .py_3()
        .flex()
        .justify_between()
        .text_size(px(10.))
        .text_color(rgb(t.muted))
        .child("Built with Rust, TypeScript & GPUI")
        .child("Open source");
    div()
        .id("about-overlay")
        .absolute()
        .inset_0()
        .occlude()
        .bg(crate::ui::alpha(0, 0.4))
        .flex()
        .items_center()
        .justify_center()
        .child(
            t.surface()
                .w(px(480.))
                .overflow_hidden()
                .flex()
                .flex_col()
                .child(header)
                .child(content)
                .child(footer),
        )
}
