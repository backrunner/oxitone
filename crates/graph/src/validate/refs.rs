//! Dangling-reference checks: clips → patterns/tracks/samples, channels →
//! mixer buses, track membership lists, sends → destinations, and Master
//! routing rules (02-domain-spec.md §Mixer 与 routing).

use std::collections::HashSet;

use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::wire::ProjectSnapshot;

use crate::topology::MASTER_MIXER_CHANNEL_ID;

fn dangling(path: &str, what: &str, id: &str) -> OxitoneError {
    OxitoneError::with_path(
        codes::INVALID_PROJECT,
        format!("reference to unknown {what} {id:?}"),
        path,
    )
}

pub(super) fn validate_refs(snapshot: &ProjectSnapshot) -> Result<(), OxitoneError> {
    let track_ids: HashSet<&str> = snapshot.tracks.iter().map(|t| t.id.as_str()).collect();
    let pattern_ids: HashSet<&str> = snapshot.patterns.iter().map(|p| p.id.as_str()).collect();
    let sample_ids: HashSet<&str> = snapshot.samples.iter().map(|s| s.id.as_str()).collect();
    let channel_ids: HashSet<&str> = snapshot.channels.iter().map(|c| c.id.as_str()).collect();
    let mixer_ids: HashSet<&str> = snapshot
        .mixer_channels
        .iter()
        .map(|m| m.id.as_str())
        .collect();
    let pattern_clip_ids: HashSet<&str> = snapshot
        .pattern_clips
        .iter()
        .map(|c| c.id.as_str())
        .collect();
    let sample_clip_ids: HashSet<&str> = snapshot
        .sample_clips
        .iter()
        .map(|c| c.id.as_str())
        .collect();
    let automation_lane_ids: HashSet<&str> = snapshot
        .automation
        .iter()
        .map(|lane| lane.id.as_str())
        .collect();

    super::playlist::validate_playlist(snapshot)?;

    for (i, clip) in snapshot.pattern_clips.iter().enumerate() {
        if !pattern_ids.contains(clip.pattern_id.as_str()) {
            return Err(dangling(
                &format!("$.patternClips[{i}].patternId"),
                "pattern",
                &clip.pattern_id,
            ));
        }
        if !track_ids.contains(clip.track_id.as_str()) {
            return Err(dangling(
                &format!("$.patternClips[{i}].trackId"),
                "track",
                &clip.track_id,
            ));
        }
    }
    for (i, clip) in snapshot.sample_clips.iter().enumerate() {
        if !sample_ids.contains(clip.sample_id.as_str()) {
            return Err(dangling(
                &format!("$.sampleClips[{i}].sampleId"),
                "sample",
                &clip.sample_id,
            ));
        }
        if !track_ids.contains(clip.track_id.as_str()) {
            return Err(dangling(
                &format!("$.sampleClips[{i}].trackId"),
                "track",
                &clip.track_id,
            ));
        }
    }
    for (i, clip) in snapshot
        .automation_clips
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .enumerate()
    {
        if !automation_lane_ids.contains(clip.lane_id.as_str()) {
            return Err(dangling(
                &format!("$.automationClips[{i}].laneId"),
                "automation lane",
                &clip.lane_id,
            ));
        }
        if !track_ids.contains(clip.track_id.as_str()) {
            return Err(dangling(
                &format!("$.automationClips[{i}].trackId"),
                "track",
                &clip.track_id,
            ));
        }
    }
    for (i, track) in snapshot.tracks.iter().enumerate() {
        for (j, id) in track.channel_ids.iter().enumerate() {
            if !channel_ids.contains(id.as_str()) {
                return Err(dangling(
                    &format!("$.tracks[{i}].channelIds[{j}]"),
                    "channel",
                    id,
                ));
            }
        }
        for (j, id) in track.pattern_clip_ids.iter().enumerate() {
            if !pattern_clip_ids.contains(id.as_str()) {
                return Err(dangling(
                    &format!("$.tracks[{i}].patternClipIds[{j}]"),
                    "pattern clip",
                    id,
                ));
            }
        }
        for (j, id) in track.sample_clip_ids.iter().enumerate() {
            if !sample_clip_ids.contains(id.as_str()) {
                return Err(dangling(
                    &format!("$.tracks[{i}].sampleClipIds[{j}]"),
                    "sample clip",
                    id,
                ));
            }
        }
    }
    for (i, channel) in snapshot.channels.iter().enumerate() {
        let target = channel.mixer_channel_id.as_str();
        if target != MASTER_MIXER_CHANNEL_ID && !mixer_ids.contains(target) {
            return Err(dangling(
                &format!("$.channels[{i}].mixerChannelId"),
                "mixer channel",
                &channel.mixer_channel_id,
            ));
        }
    }
    for (i, bus) in snapshot.mixer_channels.iter().enumerate() {
        let path = format!("$.mixerChannels[{i}]");
        if bus.id == MASTER_MIXER_CHANNEL_ID {
            if !bus.sends.is_empty() {
                return Err(OxitoneError::with_path(
                    codes::INVALID_PROJECT,
                    "Master must not be a send source; its output cannot be re-routed",
                    format!("{path}.sends"),
                ));
            }
            if bus.master_send_ratio.is_some() {
                return Err(OxitoneError::with_path(
                    codes::INVALID_PROJECT,
                    "masterSendRatio does not apply to Master itself",
                    format!("{path}.masterSendRatio"),
                ));
            }
        }
        let mut destinations: HashSet<&str> = HashSet::new();
        for (s, send) in bus.sends.iter().enumerate() {
            let send_path = format!("{path}.sends[{s}].destinationId");
            let destination = send.destination_id.as_str();
            if destination == MASTER_MIXER_CHANNEL_ID {
                return Err(OxitoneError::with_path(
                    codes::INVALID_PROJECT,
                    "send destination must not be Master; use masterSendRatio",
                    send_path,
                ));
            }
            if !mixer_ids.contains(destination) {
                return Err(dangling(&send_path, "mixer channel", &send.destination_id));
            }
            if !destinations.insert(destination) {
                return Err(OxitoneError::with_path(
                    codes::INVALID_PROJECT,
                    format!("duplicate send to {destination:?} on the same mixer channel"),
                    send_path,
                ));
            }
        }
    }
    Ok(())
}
