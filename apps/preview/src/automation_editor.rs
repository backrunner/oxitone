use crate::{
    automation_curve::{self, CurveTool},
    ui::Preview,
    ui_icons::Icon,
};
use gpui::{prelude::*, *};
impl Preview {
    pub fn automation_editor(&self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let site = self.automation_site().cloned();
        let lane_id = self.automation_lane().map(|lane| &lane.id);
        let state = &self.document.automation;
        let mut tools = t.tool_group();
        for (id, glyph, label, tool) in [
            (
                "curve-points",
                Icon::Select,
                "Edit points · Alt drag to bend",
                CurveTool::Points,
            ),
            ("curve-draw", Icon::Draw, "Draw curve", CurveTool::Draw),
            ("curve-line", Icon::Curve, "Draw line", CurveTool::Line),
        ] {
            tools = tools.child(t.icon_tool(id, glyph, label, state.tool == tool).on_click(
                cx.listener(move |this, _, _, cx| {
                    this.document.automation.tool = tool;
                    this.document.automation.gesture = None;
                    cx.notify();
                }),
            ));
        }
        tools = tools
            .child(div().mx_1().w(px(1.)).h(px(16.)).bg(rgb(t.border)))
            .child(
                t.ghost("curve-interpolation", state.interpolation.label())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.document.automation.interpolation =
                            this.document.automation.interpolation.next();
                        cx.notify();
                    })),
            )
            .child(
                t.ghost("curve-snap", state.snap.label())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.document.automation.snap = this.document.automation.snap.next();
                        cx.notify();
                    })),
            );
        let compiled = state.compiled.as_ref().map(|(_, source)| source.clone());
        let gesture = state.gesture.as_ref().or(state.pending.as_ref()).cloned();
        let handles = site
            .as_ref()
            .and_then(|s| automation_curve::editable(&s.source))
            .map(|c| c.points)
            .unwrap_or_default();
        let bounds = state.bounds.clone();
        let point_count = handles.len();
        let (beats, offset) = (state.beats, state.offset);
        let canvas = div()
            .flex_1()
            .min_h(px(60.))
            .overflow_hidden()
            .on_scroll_wheel(cx.listener(|this, event, _, cx| {
                this.scroll_automation(event);
                cx.stop_propagation();
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event, window, cx| {
                    this.workspace_focus.focus(window);
                    this.press_curve(event);
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event, window, cx| {
                    this.workspace_focus.focus(window);
                    this.press_curve(event);
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .child(
                canvas(
                    move |area, _, _| bounds.set(area),
                    move |area, _, window, cx| {
                        crate::automation_paint::paint(
                            area,
                            t,
                            beats,
                            offset,
                            compiled.as_deref(),
                            &handles,
                            gesture.as_ref(),
                            window,
                            cx,
                        );
                    },
                )
                .size_full(),
            );
        let mut editor = div()
            .bg(rgb(t.panel))
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(
                div()
                    .p_1()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .border_b_1()
                    .border_color(rgb(t.border))
                    .child(tools)
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .child(
                                t.icon_button("automation-fit", Icon::Fit, "Reset view")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.document.automation.offset = 0.;
                                        this.document.automation.beats = 8.;
                                        this.document.automation.gesture = None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                t.icon_button("automation-zoom-in", Icon::Plus, "Zoom in")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.document.automation.beats =
                                            (this.document.automation.beats / 2.).max(0.25);
                                        this.document.automation.gesture = None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                t.icon_button("automation-zoom-out", Icon::Minus, "Zoom out")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.document.automation.beats =
                                            (this.document.automation.beats * 2.).min(1024.);
                                        this.document.automation.gesture = None;
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .h(px(28.))
                    .px_4()
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_size(px(10.))
                    .text_color(rgb(t.muted))
                    .child(format!("{offset:.2} beats"))
                    .child("Value")
                    .child(format!("{:.2} beats", offset + beats)),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .px_3()
                    .pb_3()
                    .child(
                        div()
                            .w(px(30.))
                            .flex_shrink_0()
                            .flex()
                            .flex_col()
                            .justify_between()
                            .text_size(px(9.))
                            .text_color(rgb(t.muted))
                            .child("1.00")
                            .child("0.50")
                            .child("0.00"),
                    )
                    .child(canvas),
            )
            .when(self.document.show_code, |d| {
                d.child(
                    div()
                        .px_4()
                        .py_2()
                        .text_size(px(10.))
                        .font_family("Menlo")
                        .truncate()
                        .text_color(rgb(t.muted))
                        .child(
                            site.as_ref()
                                .map_or_else(String::new, |s| s.expression.clone()),
                        ),
                )
            })
            .child(
                div()
                    .h(px(26.))
                    .px_4()
                    .flex()
                    .items_center()
                    .border_t_1()
                    .border_color(rgb(t.border))
                    .text_size(px(10.))
                    .text_color(rgb(t.muted))
                    .child(format!(
                        "{} points",
                        state
                            .gesture
                            .as_ref()
                            .map_or(point_count, |g| g.curve.points.len())
                    )),
            );
        if site.is_none() {
            editor = editor.child(div().p_3().text_xs().text_color(rgb(t.gold)).child(
                if lane_id.is_none() {
                    "No automation lanes"
                } else {
                    "Source is shared. Switch to Shared source to edit."
                },
            ));
        }
        editor
    }
}
