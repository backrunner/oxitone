//! Titlebar authoring controls share the existing document and transport paths.
use crate::{backend::Command, ui::Preview, ui_icons::Icon};
use gpui::{prelude::*, *};

pub fn view(this: &Preview, cx: &mut Context<Preview>) -> impl IntoElement {
    let t = this.theme;
    let bpm = this.project.as_ref().map_or(120., |p| {
        p.plan.tempo.bpm_at_beat(p.beat(this.position_frame()))
    });
    let metronome = this.metronome_pending.unwrap_or(this.metronome);
    let bounds = this.workflow_bounds.clone();
    div()
        .relative()
        .flex()
        .items_center()
        .gap_3()
        .flex_shrink_0()
        .child(
            canvas(move |area, _, _| bounds.set(area), |_, _, _, _| {})
                .absolute()
                .size_full(),
        )
        .child(crate::pattern_navigation::selector(this, cx))
        .child(crate::tempo_edit::view(this, bpm, cx))
        .child(
            t.icon_tool("metronome", Icon::Metronome, "Metronome", metronome)
                .opacity(if this.project.is_some() { 1. } else { 0.4 })
                .on_click(cx.listener(|this, _, window, cx| {
                    if this.project.is_some() && this.metronome_pending.is_none() {
                        let enabled = !this.metronome;
                        if this
                            .backend
                            .commands
                            .send(Command::Metronome(enabled))
                            .is_ok()
                        {
                            this.metronome_pending = Some(enabled);
                        }
                    }
                    this.workspace_focus.focus(window);
                    cx.notify();
                })),
        )
}
