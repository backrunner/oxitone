//! MIDI channel allocation (`.agents/docs/06-format-and-export.md` §MIDI 导出).
//!
//! Explicit `Track.midiChannel` (1..=16, sharing allowed) always wins.
//! Remaining note tracks are auto-assigned the lowest free channels in
//! stable track-ID order. With more than 16 note tracks, every note track
//! must carry an explicit channel; any unassigned track fails the export
//! with `MidiChannelLimit` listing all unassigned track IDs.

use serde::Serialize;

use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::wire::{EntityId, TrackSpec};

use super::MidiExportError;

/// Where a track's channel came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ChannelSource {
    Explicit,
    Auto,
}

/// One channel assignment, reported in the export diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelAssignment {
    pub track_id: EntityId,
    pub channel: u8,
    pub source: ChannelSource,
}

/// Allocate channels for the note tracks (already sorted by stable track
/// ID). The returned vector is in the same order.
pub(crate) fn allocate(
    note_tracks: &[&TrackSpec],
) -> Result<Vec<ChannelAssignment>, MidiExportError> {
    for track in note_tracks {
        if let Some(channel) = track.midi_channel {
            if !(1..=16).contains(&channel) {
                return Err(OxitoneError::with_path(
                    codes::INVALID_PROJECT,
                    format!("midiChannel must be in 1..=16, got {channel}"),
                    format!("$.tracks[{}].midiChannel", track.id),
                )
                .into());
            }
        }
    }

    if note_tracks.len() > 16 {
        let unassigned: Vec<EntityId> = note_tracks
            .iter()
            .filter(|t| t.midi_channel.is_none())
            .map(|t| t.id.clone())
            .collect();
        if !unassigned.is_empty() {
            return Err(MidiExportError::channel_limit(
                format!(
                    "MIDI supports 16 channels but the project has {} note tracks; \
                     set an explicit midiChannel on every note track so tracks can share channels",
                    note_tracks.len()
                ),
                unassigned,
            ));
        }
        return Ok(note_tracks
            .iter()
            .map(|t| ChannelAssignment {
                track_id: t.id.clone(),
                channel: t.midi_channel.unwrap_or(1),
                source: ChannelSource::Explicit,
            })
            .collect());
    }

    let mut used = [false; 16];
    for track in note_tracks {
        if let Some(channel) = track.midi_channel {
            used[usize::from(channel - 1)] = true;
        }
    }
    let mut assignments = Vec::with_capacity(note_tracks.len());
    for track in note_tracks {
        if let Some(channel) = track.midi_channel {
            assignments.push(ChannelAssignment {
                track_id: track.id.clone(),
                channel,
                source: ChannelSource::Explicit,
            });
        } else {
            let channel = used
                .iter()
                .position(|taken| !taken)
                .map(|i| i as u8 + 1)
                .ok_or_else(|| {
                    MidiExportError::channel_limit(
                        "MIDI supports 16 channels; no free channel remains for auto assignment",
                        vec![track.id.clone()],
                    )
                })?;
            used[usize::from(channel - 1)] = true;
            assignments.push(ChannelAssignment {
                track_id: track.id.clone(),
                channel,
                source: ChannelSource::Auto,
            });
        }
    }
    Ok(assignments)
}
