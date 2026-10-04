//! Clipboard retains placement settings, including after the source is removed.
use crate::{
    playlist_actions::Clip,
    playlist_edit::{ArrangementEdit, ResourceKind},
    ui::Preview,
};
use serde_json::Value;
#[derive(Clone)]
pub struct CopiedClip {
    pub clip: Clip,
    pub settings: Value,
    pub channels: Vec<String>,
}
impl Preview {
    pub fn copy_placements(&self, clips: &[Clip]) -> Vec<CopiedClip> {
        let Some(p) = &self.project else {
            return vec![];
        };
        clips
            .iter()
            .filter_map(|clip| {
                let mut settings = match clip.kind {
                    ResourceKind::Pattern => serde_json::to_value(
                        p.snapshot.pattern_clips.iter().find(|c| c.id == clip.id)?,
                    )
                    .ok()?,
                    ResourceKind::Sample => serde_json::to_value(
                        p.snapshot.sample_clips.iter().find(|c| c.id == clip.id)?,
                    )
                    .ok()?,
                    ResourceKind::Automation => serde_json::to_value(
                        p.snapshot
                            .automation_clips
                            .as_ref()?
                            .iter()
                            .find(|c| c.id == clip.id)?,
                    )
                    .ok()?,
                };
                for key in ["id", "patternId", "sampleId", "laneId", "trackId"] {
                    settings.as_object_mut()?.remove(key);
                }
                settings.as_object_mut()?.retain(|_, v| !v.is_null());
                let channels = p
                    .snapshot
                    .tracks
                    .iter()
                    .find(|t| t.id == clip.track)?
                    .channel_ids
                    .clone();
                Some(CopiedClip {
                    clip: clip.clone(),
                    settings,
                    channels,
                })
            })
            .collect()
    }
    pub fn paste_placement(
        &self,
        copied: &CopiedClip,
        track: &str,
        start: f64,
    ) -> Option<ArrangementEdit> {
        let order = self.document.view.as_ref()?.arrangement_order.as_ref()?;
        Some(ArrangementEdit::Paste {
            kind: copied.clip.kind,
            resource: order.resource(copied.clip.kind, &copied.clip.resource)?,
            track: order.tracks.iter().position(|id| id == track)?,
            start_beat: start,
            settings: copied.settings.clone(),
            channels: copied
                .channels
                .iter()
                .map(|id| order.channels.iter().position(|v| v == id))
                .collect::<Option<Vec<_>>>()?,
        })
    }
}
