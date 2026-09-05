//! Entity ID structure, documented prefixes, and global uniqueness
//! (01-architecture.md §IDs: opaque, globally unique, never array indexes).

use std::collections::HashSet;

use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::id::{prefixes, validate_id};
use oxitone_core::wire::ProjectSnapshot;

fn check_id<'a>(seen: &mut HashSet<&'a str>, path: &str, id: &'a str) -> Result<(), OxitoneError> {
    validate_id(id).map_err(|mut err| {
        err.path = Some(path.to_string());
        err
    })?;
    if !seen.insert(id) {
        return Err(OxitoneError::with_path(
            codes::INVALID_PROJECT,
            format!("duplicate entity id {id:?}"),
            path,
        ));
    }
    Ok(())
}

fn check_prefixed<'a>(
    seen: &mut HashSet<&'a str>,
    path: &str,
    id: &'a str,
    prefix: &str,
    kind: &str,
) -> Result<(), OxitoneError> {
    check_id(seen, path, id)?;
    if !id.starts_with(prefix) {
        return Err(OxitoneError::with_path(
            codes::INVALID_PROJECT,
            format!("{kind} id {id:?} must use the {prefix:?} prefix"),
            path,
        ));
    }
    Ok(())
}

/// Structural and prefix checks plus global duplicate detection.
pub(super) fn validate_ids(snapshot: &ProjectSnapshot) -> Result<(), OxitoneError> {
    let mut seen: HashSet<&str> = HashSet::new();
    check_id(&mut seen, "$.id", &snapshot.id)?;

    for (i, marker) in snapshot.markers.iter().enumerate() {
        check_id(&mut seen, &format!("$.markers[{i}].id"), &marker.id)?;
    }
    for (i, track) in snapshot.tracks.iter().enumerate() {
        check_prefixed(
            &mut seen,
            &format!("$.tracks[{i}].id"),
            &track.id,
            prefixes::TRACK,
            "track",
        )?;
    }
    for (i, pattern) in snapshot.patterns.iter().enumerate() {
        check_prefixed(
            &mut seen,
            &format!("$.patterns[{i}].id"),
            &pattern.id,
            prefixes::PATTERN,
            "pattern",
        )?;
        for (n, note) in pattern.notes.iter().enumerate() {
            if let Some(id) = &note.id {
                check_id(&mut seen, &format!("$.patterns[{i}].notes[{n}].id"), id)?;
            }
        }
    }
    for (i, clip) in snapshot.pattern_clips.iter().enumerate() {
        check_id(&mut seen, &format!("$.patternClips[{i}].id"), &clip.id)?;
    }
    for (i, clip) in snapshot.sample_clips.iter().enumerate() {
        check_id(&mut seen, &format!("$.sampleClips[{i}].id"), &clip.id)?;
    }
    for (i, sample) in snapshot.samples.iter().enumerate() {
        check_prefixed(
            &mut seen,
            &format!("$.samples[{i}].id"),
            &sample.id,
            prefixes::SAMPLE,
            "sample",
        )?;
    }
    for (i, channel) in snapshot.channels.iter().enumerate() {
        check_prefixed(
            &mut seen,
            &format!("$.channels[{i}].id"),
            &channel.id,
            prefixes::CHANNEL,
            "channel",
        )?;
    }
    for (i, bus) in snapshot.mixer_channels.iter().enumerate() {
        check_prefixed(
            &mut seen,
            &format!("$.mixerChannels[{i}].id"),
            &bus.id,
            prefixes::MIXER_CHANNEL,
            "mixer channel",
        )?;
    }
    for (i, lane) in snapshot.automation.iter().enumerate() {
        check_prefixed(
            &mut seen,
            &format!("$.automation[{i}].id"),
            &lane.id,
            prefixes::AUTOMATION,
            "automation lane",
        )?;
    }
    Ok(())
}
