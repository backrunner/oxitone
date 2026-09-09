//! Presentation-only placements. Moving and pending items use the ordinary content renderer.
use crate::{
    model::ViewProject,
    playlist_actions::Clip,
    playlist_edit::{Drag, DragMode, ResourceKind},
};

pub fn placements(project: &ViewProject) -> Vec<Clip> {
    let mut clips: Vec<_> = project
        .snapshot
        .pattern_clips
        .iter()
        .map(|c| {
            let (start, end) = project.clip_bounds(c);
            Clip {
                kind: ResourceKind::Pattern,
                id: c.id.clone(),
                resource: c.pattern_id.clone(),
                track: c.track_id.clone(),
                start,
                length: end - start,
                enabled: c.enabled != Some(false),
            }
        })
        .collect();
    clips.extend(project.snapshot.sample_clips.iter().filter_map(|c| {
        let (_, a, b) = project.plan.samples.iter().find(|p| p.0 == c.id)?;
        let start = project.beat(*a);
        Some(Clip {
            kind: ResourceKind::Sample,
            id: c.id.clone(),
            resource: c.sample_id.clone(),
            track: c.track_id.clone(),
            start,
            length: project.beat(*b) - start,
            enabled: c.enabled != Some(false),
        })
    }));
    clips.extend(
        project
            .snapshot
            .automation_clips
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|c| Clip {
                kind: ResourceKind::Automation,
                id: c.id.clone(),
                resource: c.lane_id.clone(),
                track: c.track_id.clone(),
                start: c.start_beat.to_f64(),
                length: c.duration_beats.map_or(4., |b| b.to_f64()),
                enabled: c.enabled != Some(false),
            }),
    );
    clips
}

/// A move replaces its original; copy retains it. Invalid drops keep the source visible.
pub fn project(mut clips: Vec<Clip>, drag: Option<&Drag>) -> Vec<Clip> {
    let Some(drag) = drag.filter(|d| d.moved) else {
        return clips;
    };
    let Some((track, start)) = &drag.target else {
        return clips;
    };
    let original = drag
        .clip
        .as_ref()
        .and_then(|(id, _)| clips.iter().position(|c| &c.id == id));
    let mut live = original.map(|i| clips[i].clone()).unwrap_or_else(|| Clip {
        kind: drag.kind,
        id: "playlist-insertion".into(),
        resource: drag.resource.clone(),
        track: track.clone(),
        start: *start,
        length: drag.length,
        enabled: true,
    });
    live.track = track.clone();
    live.start = *start;
    live.length = drag.length;
    if let Some(index) = original {
        if drag.mode != DragMode::Copy {
            clips.remove(index);
        } else {
            live.id = format!("playlist-copy-{}", live.id);
        }
    }
    // Last in paint order, so the item under the pointer remains visible over overlaps.
    clips.push(live);
    clips
}
