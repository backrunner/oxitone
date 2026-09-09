//! Track controls gate placements, independently of shared Channel routing.
use crate::{
    document_wire::DocumentOperation,
    project_edit::ProjectEdit,
    ui::{alpha, Preview},
};
use gpui::{prelude::*, *};
use oxitone_core::wire::TrackSpec;

#[derive(Clone, Copy)]
enum Control {
    Enabled,
    Mute,
    Solo,
}

pub fn view(
    track: &TrackSpec,
    index: usize,
    tint: u32,
    this: &Preview,
    cx: &Context<Preview>,
) -> Div {
    let theme = this.theme;
    let enabled = track.enabled != Some(false);
    let mut controls = div().flex().items_center().gap_1();
    for (control, key, label, active, tip) in [
        (
            Control::Mute,
            "mute",
            "M",
            track.mute == Some(true),
            "Mute track",
        ),
        (
            Control::Solo,
            "solo",
            "S",
            track.solo == Some(true),
            "Solo track",
        ),
    ] {
        let id = track.id.clone();
        let bounds = this.document.playlist.controls.clone();
        let key = format!("track-{key}-{id}");
        let measure = key.clone();
        controls = controls.child(
            theme
                .tool(key, label, active)
                .relative()
                .px_0()
                .w(px(26.))
                .h(px(20.))
                .opacity(if this.document_ready() { 1. } else { 0.45 })
                .child(
                    canvas(
                        move |area, _, _| {
                            bounds.borrow_mut().insert(measure.clone(), area);
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .size_full(),
                )
                .tooltip(move |_, cx| cx.new(|_| TrackTip(tip)).into())
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.workspace_focus.focus(window);
                    toggle(this, &id, control, !active);
                    cx.stop_propagation();
                    cx.notify();
                })),
        );
    }
    let id = track.id.clone();
    div()
        .h(px(52.))
        .flex_shrink_0()
        .px_2()
        .flex()
        .items_center()
        .gap_2()
        .border_b_1()
        .border_color(rgb(theme.border))
        .child(
            div()
                .id(SharedString::from(format!("track-enable-{id}")))
                .w(px(5.))
                .h(px(34.))
                .flex_shrink_0()
                .bg(alpha(tint, if enabled { 1. } else { 0.2 }))
                .tooltip(|_, cx| cx.new(|_| TrackTip("Enable track")).into())
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.workspace_focus.focus(window);
                    toggle(this, &id, Control::Enabled, !enabled);
                    cx.stop_propagation();
                    cx.notify();
                })),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div().text_size(px(11.)).truncate().child(
                        track
                            .name
                            .clone()
                            .unwrap_or_else(|| format!("Track {}", index + 1)),
                    ),
                )
                .child(controls),
        )
}
fn toggle(this: &mut Preview, id: &str, control: Control, value: bool) {
    let Some(index) = this
        .document
        .view
        .as_ref()
        .and_then(|v| v.arrangement_order.as_ref())
        .and_then(|o| o.tracks.iter().position(|t| t == id))
    else {
        return;
    };
    this.document_request(DocumentOperation::Project {
        edit: ProjectEdit::Track {
            index,
            enabled: matches!(control, Control::Enabled).then_some(value),
            mute: matches!(control, Control::Mute).then_some(value),
            solo: matches!(control, Control::Solo).then_some(value),
        },
    });
}
struct TrackTip(&'static str);
impl Render for TrackTip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().px_2().py_1().text_size(px(11.)).child(self.0)
    }
}
