//! Channel-part selection shared by painting, gestures and source transactions.
use crate::ui::Preview;
use gpui::{prelude::*, *};
use oxitone_core::wire::PatternSpec;

impl Preview {
    pub fn piano_period(&self) -> Option<f64> {
        let snapshot = &self.project.as_ref()?.snapshot;
        let clip = snapshot
            .pattern_clips
            .iter()
            .find(|c| Some(&c.id) == self.selected_clip.as_ref())?;
        Some(
            snapshot
                .patterns
                .iter()
                .find(|p| p.id == clip.pattern_id)?
                .length_beats
                .to_f64(),
        )
    }
    pub fn composite_selected(&self) -> bool {
        self.project.as_ref().is_some_and(|project| {
            project
                .snapshot
                .pattern_clips
                .iter()
                .find(|c| Some(&c.id) == self.selected_clip.as_ref())
                .is_some_and(|c| self.piano_pattern().is_some_and(|p| p.id != c.pattern_id))
        })
    }
    pub fn piano_pattern(&self) -> Option<&PatternSpec> {
        let snapshot = &self.project.as_ref()?.snapshot;
        let clip = snapshot
            .pattern_clips
            .iter()
            .find(|c| Some(&c.id) == self.selected_clip.as_ref())?;
        let root = snapshot.patterns.iter().find(|p| p.id == clip.pattern_id)?;
        let Some(parts) = &root.parts else {
            return Some(root);
        };
        let part = parts
            .iter()
            .find(|p| Some(&p.channel_id) == self.piano.channel.as_ref())
            .or_else(|| parts.first())?;
        snapshot.patterns.iter().find(|p| p.id == part.pattern_id)
    }
}
pub fn selector(this: &Preview, cx: &mut Context<Preview>) -> Option<impl IntoElement> {
    let snapshot = &this.project.as_ref()?.snapshot;
    let clip = snapshot
        .pattern_clips
        .iter()
        .find(|c| Some(&c.id) == this.selected_clip.as_ref())?;
    let parts = snapshot
        .patterns
        .iter()
        .find(|p| p.id == clip.pattern_id)?
        .parts
        .as_ref()?;
    let selected = this
        .piano
        .channel
        .as_ref()
        .filter(|id| parts.iter().any(|p| &p.channel_id == *id))
        .or_else(|| parts.first().map(|p| &p.channel_id));
    let mut row = div()
        .id("pattern-channels")
        .flex_shrink_0()
        .h(px(30.))
        .flex()
        .items_center()
        .gap_1()
        .px_2()
        .overflow_x_scroll();
    for (i, part) in parts.iter().enumerate() {
        let id = part.channel_id.clone();
        let channel = snapshot.channels.iter().find(|c| c.id == id)?;
        let name = channel
            .name
            .clone()
            .unwrap_or_else(|| format!("Channel {}", i + 1));
        row = row.child(
            this.theme
                .tool(
                    SharedString::from(format!("part-{i}")),
                    name,
                    selected == Some(&id),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.piano.channel = Some(id.clone());
                    this.piano.fit();
                    this.document.notes.clear();
                    this.document.gesture = None;
                    cx.notify();
                })),
        );
    }
    Some(row)
}
