//! Cached native curve evaluation for Playlist thumbnails.
use crate::model::ViewProject;
use gpui::{prelude::*, *};
use oxitone_core::wire::AutomationCombine;
use oxitone_graph::compile::{lane_beat, CompiledLane};
use oxitone_transport::{CompiledAutomation, EvalContext};
use std::sync::Arc;

impl ViewProject {
    fn curve_preview(&self, id: &str) -> Option<Arc<CompiledLane>> {
        self.automation_previews
            .get_or_init(|| {
                self.snapshot
                    .automation
                    .iter()
                    .filter_map(|lane| {
                        let automation =
                            CompiledAutomation::compile(&lane.source, self.snapshot.seed).ok()?;
                        let start = lane
                            .loop_spec
                            .as_ref()
                            .and_then(|l| l.start_beat)
                            .map_or(0., |b| b.to_f64());
                        let length = lane.loop_spec.as_ref().map(|l| l.length_beats.to_f64());
                        let end = lane.loop_spec.as_ref().and_then(|l| {
                            l.last_beat.map(|b| b.to_f64()).or_else(|| {
                                l.count
                                    .map(|n| start + f64::from(n) * l.length_beats.to_f64())
                            })
                        });
                        Some((
                            lane.id.clone(),
                            Arc::new(CompiledLane {
                                automation,
                                placements: None,
                                combine: AutomationCombine::Replace,
                                loop_start: start,
                                loop_length: length,
                                loop_end: end,
                                last_beat: lane.last_beat.map(|b| b.to_f64()),
                            }),
                        ))
                    })
                    .collect()
            })
            .get(id)
            .cloned()
    }
}
pub fn view(project: &ViewProject, lane: &str, length: f64, color: u32) -> impl IntoElement {
    let source = project.curve_preview(lane);
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let Some(source) = &source else { return };
            let width = f32::from(bounds.size.width);
            let height = f32::from(bounds.size.height);
            let count = (width.ceil() as usize).clamp(2, 1024);
            let mut path = PathBuilder::stroke(px(1.5));
            for i in 0..=count {
                let fraction = i as f64 / count as f64;
                let value = source.automation.value_at(
                    lane_beat(source, fraction * length),
                    &EvalContext::default(),
                );
                let p = bounds.origin
                    + point(
                        px(fraction as f32 * width),
                        px(2. + (1. - value as f32) * (height - 4.).max(0.)),
                    );
                if i == 0 {
                    path.move_to(p);
                } else {
                    path.line_to(p);
                }
            }
            window.with_content_mask(Some(ContentMask { bounds }), |window| {
                if let Ok(path) = path.build() {
                    window.paint_path(path, rgb(color));
                }
            });
        },
    )
    .size_full()
}
