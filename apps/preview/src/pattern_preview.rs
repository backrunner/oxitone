//! Miniature note geometry shares the compiled clip clock with the playlist.
use crate::{model::ViewProject, playlist_actions::Clip};
use gpui::{prelude::*, *};
use std::sync::Arc;

pub fn thumbnail(project: Arc<ViewProject>, clip: Clip, tint: u32) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let Some(pattern) = project
                .snapshot
                .patterns
                .iter()
                .find(|p| p.id == clip.resource)
            else {
                return;
            };
            let notes: Vec<_> = if let Some(parts) = &pattern.parts {
                parts
                    .iter()
                    .filter_map(|part| {
                        project
                            .snapshot
                            .patterns
                            .iter()
                            .find(|p| p.id == part.pattern_id)
                    })
                    .flat_map(|p| p.notes.iter())
                    .collect()
            } else {
                pattern.notes.iter().collect()
            };
            let low = notes.iter().map(|n| n.pitch).min().unwrap_or(60);
            let high = notes.iter().map(|n| n.pitch).max().unwrap_or(72);
            let (start, end) = (clip.start, clip.start + clip.length);
            let local_start = project.global_to_local(&clip.track, start);
            let local_end = project.global_to_local(&clip.track, end);
            let length = pattern.length_beats.to_f64();
            let repeats = ((local_end - local_start) / length).ceil().max(0.) as usize;
            let width = f32::from(bounds.size.width);
            let height = f32::from(bounds.size.height);
            window.with_content_mask(Some(ContentMask { bounds }), |window| {
                for repeat in 0..repeats.min(4096) {
                    for note in &notes {
                        let local = local_start + repeat as f64 * length + note.start.to_f64();
                        if local >= local_end {
                            continue;
                        }
                        let a = project.local_to_global(&clip.track, local);
                        let b = project.local_to_global(
                            &clip.track,
                            (local + note.duration.to_f64()).min(local_end),
                        );
                        let x = ((a - start) / (end - start).max(1e-9)) as f32 * width;
                        let w = (((b - a) / (end - start).max(1e-9)) as f32 * width).max(1.5);
                        let y = 3.
                            + f32::from(high - note.pitch) / f32::from((high - low).max(12))
                                * (height - 7.).max(1.);
                        window.paint_quad(fill(
                            Bounds::new(bounds.origin + point(px(x), px(y)), size(px(w), px(2.))),
                            rgb(tint),
                        ));
                    }
                }
            });
        },
    )
    .size_full()
}
