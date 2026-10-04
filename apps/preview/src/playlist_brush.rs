//! One brush stroke or marquee owns pointer capture and commits at mouse-up.
use crate::{
    piano_state::NoteTool,
    playlist_actions::Clip,
    playlist_clipboard::CopiedClip,
    playlist_edit::{ArrangementEdit, ResourceKind},
    ui::Preview,
};
use gpui::{prelude::*, *};
use std::collections::BTreeSet;

pub struct Stroke {
    pub anchor: Point<Pixels>,
    pub position: Point<Pixels>,
    pub select: bool,
    pub template: Option<Clip>,
    pub copied: Option<CopiedClip>,
    pub erase: bool,
    pub cells: Vec<(String, f64)>,
    pub removed: BTreeSet<String>,
    pub initial: BTreeSet<String>,
    pub previous: Option<(String, f64)>,
}
fn rectangle(a: Point<Pixels>, b: Point<Pixels>) -> Bounds<Pixels> {
    Bounds::from_corners(
        point(a.x.min(b.x), a.y.min(b.y)),
        point(a.x.max(b.x), a.y.max(b.y)),
    )
}
impl Preview {
    fn playlist_cell(&self, point: Point<Pixels>, alt: bool) -> Option<(String, f64)> {
        if !self.document.playlist.viewport.get().contains(&point)
            || self.document.windows.hit(point).is_some()
        {
            return None;
        }
        let rows = self.document.playlist.rows.borrow();
        let (track, bounds) = rows.iter().find(|(_, bounds)| bounds.contains(&point))?;
        let step = if alt { 1. / 960. } else { 0.25 };
        Some((
            track.clone(),
            (f64::from(f32::from(point.x - bounds.origin.x)) / f64::from(self.zoom) / step)
                .round()
                .max(0.)
                * step,
        ))
    }
    pub fn start_playlist_tool(&mut self, event: &MouseDownEvent) {
        self.document.playlist.focused = true;
        let cell = self.playlist_cell(event.position, event.modifiers.alt);
        if let Some((track, beat)) = &cell {
            self.document.playlist.cursor = *beat;
            self.document.playlist.cursor_track = Some(track.clone());
        }
        if !self.document_ready() {
            return;
        }
        let select = event.button != MouseButton::Right
            && (self.document.playlist.tool == NoteTool::Select
                || event.modifiers.platform
                || event.modifiers.control);
        let template = self.document.playlist.brush_clip.clone().or_else(|| {
            let pattern = self.active_pattern()?;
            Some(Clip {
                kind: ResourceKind::Pattern,
                id: String::new(),
                resource: pattern.id.clone(),
                track: String::new(),
                start: 0.,
                length: pattern.length_beats.to_f64(),
                enabled: true,
            })
        });
        if !select && template.is_none() && event.button != MouseButton::Right {
            self.press_playlist(event);
            return;
        }
        let copied = template.as_ref().and_then(|c| {
            self.copy_placements(std::slice::from_ref(c))
                .into_iter()
                .next()
        });
        let initial = if event.modifiers.shift {
            self.document.playlist.selection.clone()
        } else {
            BTreeSet::new()
        };
        self.document.playlist.brush = Some(Stroke {
            anchor: event.position,
            position: event.position,
            select,
            template,
            copied,
            erase: event.button == MouseButton::Right,
            cells: vec![],
            removed: BTreeSet::new(),
            initial,
            previous: None,
        });
        self.move_playlist_tool(&MouseMoveEvent {
            position: event.position,
            modifiers: event.modifiers,
            pressed_button: Some(event.button),
            ..Default::default()
        });
    }
    pub fn move_playlist_tool(&mut self, event: &MouseMoveEvent) {
        let cell = self.playlist_cell(event.position, event.modifiers.alt);
        let Some(stroke) = &mut self.document.playlist.brush else {
            return;
        };
        stroke.position = event.position;
        let Some(project) = &self.project else {
            return;
        };
        let clips = crate::playlist_projection::placements(project);
        if stroke.select {
            let bounds = rectangle(stroke.anchor, stroke.position);
            let mut selected = stroke.initial.clone();
            for c in clips {
                if let Some(row) = self.document.playlist.rows.borrow().get(&c.track) {
                    let area = Bounds::new(
                        row.origin + point(px(c.start as f32 * self.zoom), px(0.)),
                        size(px(c.length as f32 * self.zoom), row.size.height),
                    );
                    if area.intersects(&bounds) {
                        selected.insert(c.id);
                    }
                }
            }
            self.document.playlist.selected = None;
            self.document.playlist.selection = selected;
            return;
        }
        let Some((track, beat)) = cell else {
            stroke.previous = None;
            return;
        };
        if stroke.erase {
            let from = stroke
                .previous
                .as_ref()
                .filter(|(t, _)| t == &track)
                .map_or(beat, |(_, b)| *b);
            for c in clips {
                if c.track == track
                    && c.start <= beat.max(from)
                    && c.start + c.length > beat.min(from)
                {
                    stroke.removed.insert(c.id);
                }
            }
            stroke.previous = Some((track, beat));
            return;
        }
        let Some(template) = &stroke.template else {
            return;
        };
        let length = template.length.max(0.25);
        let cell = if self.document.playlist.tool == NoteTool::Draw {
            beat
        } else {
            (beat / length).floor() * length
        };
        let previous = if self.document.playlist.tool == NoteTool::Paint {
            stroke
                .previous
                .as_ref()
                .filter(|(t, _)| t == &track)
                .map_or(cell, |(_, b)| *b)
        } else {
            cell
        };
        if self.document.playlist.tool == NoteTool::Draw && !stroke.cells.is_empty() {
            return;
        }
        let count = (((cell - previous).abs() / length).round() as usize).min(512);
        for i in 0..=count {
            if stroke.cells.len() >= 512 {
                break;
            }
            let start = previous.min(cell) + i as f64 * length;
            if !stroke
                .cells
                .iter()
                .any(|(t, b)| t == &track && (*b - start).abs() < 1e-7)
                && !clips.iter().any(|c| {
                    c.track == track
                        && c.resource == template.resource
                        && (c.start - start).abs() < 1e-7
                })
            {
                stroke.cells.push((track.clone(), start));
            }
        }
        stroke.previous = Some((track, cell));
    }
    pub fn finish_playlist_tool(&mut self) {
        let Some(stroke) = self.document.playlist.brush.take() else {
            return;
        };
        if stroke.select {
            return;
        }
        let mut edits = Vec::new();
        let mut selected = Vec::new();
        if stroke.erase {
            if let Some(project) = &self.project {
                for c in crate::playlist_projection::placements(project)
                    .into_iter()
                    .filter(|c| stroke.removed.contains(&c.id))
                {
                    if let Some((clip, resource, _)) = self.placement_address(&c) {
                        edits.push(ArrangementEdit::Remove {
                            kind: c.kind,
                            resource,
                            clip,
                        });
                    }
                }
            }
        } else if let Some(template) = stroke.template {
            let Some(o) = self
                .document
                .view
                .as_ref()
                .and_then(|v| v.arrangement_order.as_ref())
            else {
                return;
            };
            for (track, start) in stroke.cells {
                let edit = if let Some(copied) = &stroke.copied {
                    self.paste_placement(copied, &track, start)
                } else {
                    o.resource(template.kind, &template.resource)
                        .zip(o.tracks.iter().position(|t| t == &track))
                        .map(|(resource, track)| ArrangementEdit::Place {
                            kind: template.kind,
                            resource,
                            track,
                            start_beat: start,
                            duration_beats: template.length,
                        })
                };
                let Some(edit) = edit else {
                    return;
                };
                let mut c = template.clone();
                c.track = track;
                c.start = start;
                selected.push(c);
                edits.push(edit);
            }
        }
        self.submit_placements(edits, selected);
    }
}
pub fn project(this: &Preview, mut clips: Vec<Clip>) -> Vec<Clip> {
    if let Some(stroke) = &this.document.playlist.brush {
        clips.retain(|c| !stroke.removed.contains(&c.id));
        if let Some(template) = &stroke.template {
            for (i, (track, start)) in stroke.cells.iter().enumerate() {
                let mut clip = template.clone();
                clip.id = format!("brush-{i}");
                clip.track = track.clone();
                clip.start = *start;
                clips.push(clip);
            }
        }
    }
    clips
}
pub fn marquee(this: &Preview) -> Div {
    let mut root = div();
    if let Some(stroke) = this.document.playlist.brush.as_ref().filter(|s| s.select) {
        let bounds = rectangle(stroke.anchor, stroke.position);
        root = root
            .absolute()
            .left(bounds.origin.x)
            .top(bounds.origin.y)
            .w(bounds.size.width)
            .h(bounds.size.height)
            .border_1()
            .border_color(rgb(this.theme.accent))
            .bg(crate::ui::alpha(this.theme.accent, 0.12));
    }
    root
}
