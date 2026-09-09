//! Standard MIDI File (SMF) Type 1 export
//! (`.agents/docs/06-format-and-export.md` §MIDI 导出).
//!
//! Deterministic by construction: every collection that reaches the output
//! is sorted (stable track/clip/note IDs, `BTreeMap` lookups), and all
//! float-to-integer conversions use round-half-up, the same rounding rule
//! the transport applies to frames.
//!
//! Interpretation notes where the spec is silent:
//!
//! - Clips are expanded in the beat domain with rules mirroring the
//!   transport scheduler (`loopCount` / exclusive `lastBeat` clipping,
//!   `transpose`, `velocityScale`, `enabled`, and `probability` drawn from
//!   the same seeded `hash64`/`Pcg32` sequence). Swing is NOT applied:
//!   swing is a scheduling-layer behavior and MIDI export carries the
//!   arrangement data at its unswung beat positions.
//! - A "note track" is an enabled track with at least one expanded note;
//!   only note tracks get an SMF track or consume a channel.
//! - Note ticks are `round_half_up(beat * PPQ)` computed in exact rational
//!   arithmetic after any independent Track clock is mapped to Project beats.

mod channel;
mod expand;
mod smf;
mod tempo;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use oxitone_core::beat::Beat;
use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::wire::{EntityId, ProjectSnapshot};

pub use channel::{ChannelAssignment, ChannelSource};

/// Default pulses (ticks) per quarter note (`.agents/docs/06-format-and-export.md`).
pub const DEFAULT_PPQ: u16 = 960;

/// Largest PPQ encodable in the SMF division field (SMPTE bit must be clear).
pub const MAX_PPQ: u16 = 0x7FFF;

/// Export options; wire form is camelCase JSON.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MidiExportOptions {
    /// When set, the N-API layer writes the bytes to this path and the
    /// report carries `path`/`bytes` instead of `bytesBase64`.
    #[serde(default)]
    pub path: Option<String>,
    /// Ticks per quarter note; default 960. Recorded in diagnostics.
    #[serde(default)]
    pub ppq: Option<u16>,
    /// Resampling step for continuous (`linear`/`exponential`) tempo
    /// segments; default `max(1, ppq / 8)`. Recorded in diagnostics.
    #[serde(default)]
    pub tempo_event_resolution_ticks: Option<u32>,
}

/// One skipped automation lane; Phase 1 exports no CC automation because no
/// parameter declares a MIDI CC mapping.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedAutomation {
    pub lane_id: EntityId,
    pub target_entity_id: EntityId,
    pub target_parameter_id: String,
    pub reason: String,
}

/// Deterministic export diagnostics (`.agents/docs/06-format-and-export.md`
/// requires the channel assignment and tempo resolution to be reported).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MidiDiagnostics {
    pub ppq: u16,
    pub tempo_event_resolution_ticks: u32,
    /// Tempo meta events in the conductor track after resampling and dedup.
    pub tempo_event_count: u32,
    pub note_track_count: u32,
    /// One entry per note track, sorted by track ID.
    pub channel_assignments: Vec<ChannelAssignment>,
    /// Automation lanes that have no MIDI representation, sorted by lane ID.
    pub skipped_automation: Vec<SkippedAutomation>,
}

/// Successful export: SMF bytes plus the diagnostics report.
#[derive(Debug, Clone)]
pub struct MidiExportResult {
    pub bytes: Vec<u8>,
    pub diagnostics: MidiDiagnostics,
}

/// Export failure. `unassigned_track_ids` is populated only for
/// `MidiChannelLimit` so the N-API layer can attach them as structured
/// error details (`.agents/docs/06-format-and-export.md`).
#[derive(Debug)]
pub struct MidiExportError {
    pub error: OxitoneError,
    pub unassigned_track_ids: Vec<EntityId>,
}

impl MidiExportError {
    pub fn channel_limit(message: impl Into<String>, unassigned: Vec<EntityId>) -> Self {
        Self {
            error: OxitoneError::with_path(codes::MIDI_CHANNEL_LIMIT, message, "$.tracks"),
            unassigned_track_ids: unassigned,
        }
    }
}

impl From<OxitoneError> for MidiExportError {
    fn from(error: OxitoneError) -> Self {
        Self {
            error,
            unassigned_track_ids: Vec::new(),
        }
    }
}

/// Beat to MIDI ticks: exact rational `beat * ppq`, rounded half-up (the
/// same deterministic rounding the transport uses for frames).
pub(crate) fn beat_to_ticks(beat: Beat, ppq: u32) -> u64 {
    let num = i128::from(beat.numerator()) * i128::from(ppq);
    let den = i128::from(beat.denominator());
    ((2 * num + den) / (2 * den)) as u64
}

/// Velocity 0..1 to the MIDI range 1..=127 (round-half-up).
pub(crate) fn velocity_to_midi(velocity: f64) -> u8 {
    1 + (velocity.clamp(0.0, 1.0) * 126.0 + 0.5).floor() as u8
}

/// Export `snapshot` as an SMF Type 1 file. Two calls with the same
/// snapshot and options produce byte-identical output.
pub fn export_midi(
    snapshot: &ProjectSnapshot,
    options: &MidiExportOptions,
) -> Result<MidiExportResult, MidiExportError> {
    let ppq = options.ppq.unwrap_or(DEFAULT_PPQ);
    if ppq == 0 || ppq > MAX_PPQ {
        return Err(OxitoneError::with_path(
            codes::INVALID_PROJECT,
            format!("ppq must be in 1..={MAX_PPQ}, got {ppq}"),
            "ppq",
        )
        .into());
    }
    let resolution = options
        .tempo_event_resolution_ticks
        .unwrap_or_else(|| u32::from(ppq / 8).max(1));
    if resolution == 0 {
        return Err(OxitoneError::with_path(
            codes::INVALID_PROJECT,
            "tempoEventResolutionTicks must be >= 1",
            "tempoEventResolutionTicks",
        )
        .into());
    }

    let patterns: BTreeMap<&str, &_> = snapshot
        .patterns
        .iter()
        .map(|p| (p.id.as_str(), p))
        .collect();
    let clips: BTreeMap<&str, &_> = snapshot
        .pattern_clips
        .iter()
        .map(|c| (c.id.as_str(), c))
        .collect();

    let mut tracks: Vec<_> = snapshot
        .tracks
        .iter()
        .filter(|t| t.audible(false))
        .collect();
    tracks.sort_by(|a, b| a.id.cmp(&b.id));

    let mut note_tracks: Vec<(&oxitone_core::wire::TrackSpec, Vec<expand::ExpandedNote>)> =
        Vec::new();
    let mut sequence = 0_u64;
    let clock = oxitone_transport::TempoMap::compile(
        &oxitone_graph::compile::effective_tempo_table(
            snapshot,
            snapshot.sample_rate.max(1),
            snapshot.seed,
            0.0,
        )?,
        snapshot.sample_rate.max(1),
    )?;
    for track in tracks {
        let mut track_clips = Vec::with_capacity(track.pattern_clip_ids.len());
        let mut clip_ids: Vec<&str> = track.pattern_clip_ids.iter().map(String::as_str).collect();
        clip_ids.sort_unstable();
        for clip_id in clip_ids {
            let clip = clips.get(clip_id).ok_or_else(|| {
                OxitoneError::with_path(
                    codes::INVALID_PROJECT,
                    format!("unknown pattern clip id: {clip_id}"),
                    format!("$.tracks[{}].patternClipIds", track.id),
                )
            })?;
            track_clips.push(*clip);
        }
        let notes = expand::expand_track(
            &track_clips,
            &patterns,
            ppq,
            snapshot.seed,
            &mut sequence,
            oxitone_transport::TrackClock::new(&clock, track.tempo)?,
        )?;
        if !notes.is_empty() {
            note_tracks.push((track, notes));
        }
    }

    let assignments = channel::allocate(&note_tracks.iter().map(|(t, _)| *t).collect::<Vec<_>>())?;

    let (conductor, tempo_event_count) =
        tempo::conductor_events(snapshot, u32::from(ppq), resolution)?;

    let mut smf_tracks: Vec<(String, Vec<smf::TrackEvent>)> = vec![(
        snapshot.name.clone().unwrap_or_else(|| "conductor".into()),
        conductor,
    )];
    for ((track, notes), assignment) in note_tracks.iter().zip(assignments.iter()) {
        let events = notes
            .iter()
            .flat_map(|note| smf::note_events(note, assignment.channel))
            .collect();
        smf_tracks.push((
            track.name.clone().unwrap_or_else(|| track.id.clone()),
            events,
        ));
    }

    let bytes = smf::write_smf(ppq, &smf_tracks);

    let mut lanes: Vec<_> = snapshot.automation.iter().collect();
    lanes.sort_by(|a, b| a.id.cmp(&b.id));
    let skipped_automation = lanes
        .into_iter()
        .filter(|lane| lane.target.entity_id != snapshot.id || lane.target.parameter_id != "tempo")
        .map(|lane| SkippedAutomation {
            lane_id: lane.id.clone(),
            target_entity_id: lane.target.entity_id.clone(),
            target_parameter_id: lane.target.parameter_id.clone(),
            reason: if lane.target.parameter_id == "tempo" {
                "tempo automation lanes are not baked into MIDI export; the snapshot tempoMap is exported as-is".into()
            } else {
                "parameter declares no MIDI CC mapping; CC automation is not exported in Phase 1".into()
            },
        })
        .collect();

    Ok(MidiExportResult {
        bytes,
        diagnostics: MidiDiagnostics {
            ppq,
            tempo_event_resolution_ticks: resolution,
            tempo_event_count,
            note_track_count: note_tracks.len() as u32,
            channel_assignments: assignments,
            skipped_automation,
        },
    })
}
