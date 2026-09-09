use crate::{piano_layout::NoteTool, ui::Preview, ui_icons::Icon, workspace_layout::EditorMode};
use gpui::{prelude::*, *};

pub fn view(
    this: &Preview,
    width: f32,
    name: String,
    clip: (f64, f64),
    cx: &mut Context<Preview>,
) -> impl IntoElement {
    let t = this.theme;
    let expanded = this.workspace.mode == EditorMode::Piano;
    let header = div()
        .h(px(32.))
        .flex_shrink_0()
        .px_3()
        .flex()
        .items_center()
        .gap_2()
        .child(crate::ui_icons::icon(Icon::Piano, t.muted))
        .child(
            div()
                .text_size(px(11.))
                .font_weight(FontWeight::SEMIBOLD)
                .child("Piano roll"),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(11.))
                .text_color(rgb(t.muted))
                .child(name),
        )
        .child(
            t.icon_button("loop-selection", Icon::Loop, "Loop clip")
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.loop_start = clip.0;
                    this.loop_end = clip.1;
                    this.loop_enabled = true;
                    this.seek(clip.0);
                    if this.is_playing() {
                        this.play();
                    }
                    cx.notify();
                })),
        )
        .child(
            t.icon_button("piano-float", Icon::Restore, "Float / dock piano roll")
                .on_click(cx.listener(|this, _, window, cx| {
                    if this.document.windows.piano_open {
                        this.dock_editor(crate::window_manager::WindowId::Piano, window);
                    } else {
                        this.float_editor(EditorMode::Piano);
                    }
                    cx.notify();
                })),
        )
        .child(
            t.icon_button(
                "piano-maximize",
                if expanded {
                    Icon::Restore
                } else {
                    Icon::Maximize
                },
                "Maximize / restore · F7",
            )
            .on_click(cx.listener(|this, _, window, cx| {
                this.toggle_editor(EditorMode::Piano, window);
                cx.notify();
            })),
        );
    let mut tools = div()
        .id("piano-tools")
        .h(px(36.))
        .flex_shrink_0()
        .px_2()
        .flex()
        .items_center()
        .gap_2()
        .overflow_x_scroll()
        .border_b_1()
        .border_color(crate::ui::alpha(t.border, 0.6));
    if this.document.view.is_some() {
        let mut group = t.tool_group();
        for (id, icon, tip, tool) in [
            ("note-draw", Icon::Draw, "Draw · P", NoteTool::Draw),
            ("note-paint", Icon::Paint, "Paint · B", NoteTool::Paint),
            ("note-select", Icon::Select, "Select · E", NoteTool::Select),
        ] {
            group = group.child(
                t.icon_tool(id, icon, tip, this.piano.tool == tool)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.piano.tool = tool;
                        this.piano_focus.focus(window);
                        cx.notify();
                    })),
            );
        }
        tools = tools
            .child(group)
            .child(
                t.icon_tool(
                    "note-magnet",
                    Icon::Magnet,
                    if this.piano.magnet {
                        "Magnet on · grid snap"
                    } else {
                        "Magnet off · free note placement"
                    },
                    this.piano.magnet,
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    this.piano.magnet = !this.piano.magnet;
                    this.piano_focus.focus(window);
                    cx.notify();
                })),
            )
            .child(
                t.ghost(
                    "note-snap",
                    if this.piano.magnet {
                        this.piano.snap.label()
                    } else {
                        "Free"
                    },
                )
                .child(crate::ui_icons::icon(Icon::ChevronDown, t.muted))
                .gap_1()
                .on_click(cx.listener(|this, event: &ClickEvent, window, cx| {
                    this.piano.snap_menu = Some(event.position() + point(px(0.), px(16.)));
                    this.piano_focus.focus(window);
                    cx.notify();
                })),
            );
    }
    if this.document.view.is_some() {
        tools = tools.child(this.pattern_controls(cx));
    }
    let mut zoom = t.tool_group().child(
        t.icon_button("piano-fit", Icon::Fit, "Fit notes · F")
            .on_click(cx.listener(|this, _, _, cx| {
                this.piano.fit();
                cx.notify();
            })),
    );
    for (id, icon, tip, horizontal, vertical) in [
        ("piano-out", Icon::Minus, "Zoom out · −", 0.8, 1.),
        ("piano-in", Icon::Plus, "Zoom in · +", 1.25, 1.),
        (
            "piano-keys-out",
            Icon::PitchOut,
            "Reduce key height",
            1.,
            0.8,
        ),
        (
            "piano-keys-in",
            Icon::PitchIn,
            "Increase key height",
            1.,
            1.25,
        ),
    ] {
        zoom = zoom.child(t.icon_button(id, icon, tip).on_click(cx.listener(
            move |this, _, _, cx| {
                this.zoom_piano(horizontal, vertical);
                cx.notify();
            },
        )));
    }
    div()
        .w(px(width))
        .flex_shrink_0()
        .flex()
        .flex_col()
        .bg(rgb(t.panel))
        .when(!this.document.windows.piano_open, |d| d.child(header))
        .child(
            tools
                .when(this.document.windows.piano_open, |d| {
                    d.child(
                        t.icon_button("loop-selection-floating", Icon::Loop, "Loop clip")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.loop_start = clip.0;
                                this.loop_end = clip.1;
                                this.loop_enabled = true;
                                this.seek(clip.0);
                                if this.is_playing() {
                                    this.play();
                                }
                                cx.notify();
                            })),
                    )
                })
                .child(div().flex_1())
                .child(zoom),
        )
}
