//! Cross-resource Playlist contracts; runs before compiling any events.
use oxitone_core::{
    error::{codes, OxitoneError},
    wire::{AutomationPlayback, ProjectSnapshot},
};
use std::collections::HashSet;

pub(super) fn validate_playlist(snapshot: &ProjectSnapshot) -> Result<(), OxitoneError> {
    let invalid =
        |path: String, message| OxitoneError::with_path(codes::INVALID_PROJECT, message, path);
    for (i, pattern) in snapshot.patterns.iter().enumerate() {
        let Some(parts) = &pattern.parts else {
            continue;
        };
        let path = format!("$.patterns[{i}].parts");
        if parts.is_empty() || parts.len() > 256 || !pattern.notes.is_empty() {
            return Err(invalid(
                path,
                "composite Patterns require 1..256 parts and no root notes",
            ));
        }
        let mut channels = HashSet::new();
        for part in parts {
            let leaf = snapshot.patterns.iter().find(|p| p.id == part.pattern_id);
            if !channels.insert(&part.channel_id)
                || !snapshot.channels.iter().any(|c| c.id == part.channel_id)
                || leaf.is_none_or(|p| {
                    p.parts.is_some()
                        || i128::from(p.length_beats.numerator())
                            * i128::from(pattern.length_beats.denominator())
                            > i128::from(pattern.length_beats.numerator())
                                * i128::from(p.length_beats.denominator())
                })
            {
                return Err(invalid(
                    path,
                    "parts require unique Channels and existing leaves within the root length",
                ));
            }
        }
    }
    let clips = snapshot.automation_clips.as_deref().unwrap_or(&[]);
    if clips.len() > 65536 {
        return Err(invalid(
            "$.automationClips".into(),
            "too many automation clips",
        ));
    }
    for lane in &snapshot.automation {
        if lane.playback == Some(AutomationPlayback::Playlist)
            && lane.target.entity_id == snapshot.id
        {
            return Err(invalid(
                "$.automation.playback".into(),
                "tempo automation must use global time",
            ));
        }
        if clips.iter().filter(|c| c.lane_id == lane.id).count() > 1024 {
            return Err(invalid(
                "$.automationClips".into(),
                "a lane supports at most 1024 placements",
            ));
        }
    }
    for (i, clip) in clips.iter().enumerate() {
        let lane = snapshot
            .automation
            .iter()
            .find(|lane| lane.id == clip.lane_id);
        let track = snapshot
            .tracks
            .iter()
            .find(|track| track.id == clip.track_id);
        if lane.is_none_or(|lane| lane.playback != Some(AutomationPlayback::Playlist))
            || track.is_none_or(|track| track.tempo.is_some())
            || clip.start_beat.to_f64() < 0.
            || clip.duration_beats.is_none_or(|d| d.to_f64() <= 0.)
        {
            return Err(invalid(
                format!("$.automationClips[{i}]"),
                "invalid Playlist automation lane, Track or duration",
            ));
        }
        clip.start_beat.checked_add(clip.duration_beats.unwrap())?;
    }
    Ok(())
}
