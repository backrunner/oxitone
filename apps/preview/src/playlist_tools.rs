use crate::{piano_state::NoteTool, ui::Preview, ui_icons::Icon};
use gpui::{prelude::*, *};
pub fn view(this: &Preview, cx: &Context<Preview>) -> Div {
    let mut row = div().flex().gap_1();
    for (id, tool, icon, tip) in [
        ("arrange-draw", NoteTool::Draw, Icon::Draw, "Draw · P"),
        ("arrange-paint", NoteTool::Paint, Icon::Paint, "Paint · B"),
        (
            "arrange-select",
            NoteTool::Select,
            Icon::Select,
            "Select · E / ⌘ drag",
        ),
    ] {
        row = row.child(
            this.theme
                .icon_button(id, icon, tip)
                .size(px(24.))
                .when(this.document.playlist.tool == tool, |d| {
                    d.bg(rgb(this.theme.selected))
                })
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.document.playlist.tool = tool;
                    this.document.playlist.focused = true;
                    this.workspace_focus.focus(window);
                    cx.notify();
                })),
        );
    }
    row
}
