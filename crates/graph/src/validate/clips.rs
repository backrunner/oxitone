//! Numeric domains of patterns, notes, clips, and samples.

use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::wire::ProjectSnapshot;

use super::numeric::{check_clip_length, check_loop_spec, range};

pub(super) fn validate_clips(snapshot: &ProjectSnapshot) -> Result<(), OxitoneError> {
    for (i, pattern) in snapshot.patterns.iter().enumerate() {
        if pattern.length_beats.numerator() == 0 {
            return Err(OxitoneError::with_path(
                codes::INVALID_PROJECT,
                "pattern lengthBeats must be > 0",
                format!("$.patterns[{i}].lengthBeats"),
            ));
        }
        for (n, note) in pattern.notes.iter().enumerate() {
            let path = format!("$.patterns[{i}].notes[{n}]");
            if note.pitch > 127 {
                return Err(OxitoneError::with_path(
                    codes::INVALID_PROJECT,
                    format!("pitch must be 0..=127, got {}", note.pitch),
                    format!("{path}.pitch"),
                ));
            }
            if note.duration.numerator() == 0 {
                return Err(OxitoneError::with_path(
                    codes::INVALID_PROJECT,
                    "note duration must be > 0",
                    format!("{path}.duration"),
                ));
            }
            range(
                &format!("{path}.velocity"),
                "velocity",
                note.velocity,
                0.0,
                1.0,
            )?;
            if let Some(off) = note.off_velocity {
                range(&format!("{path}.offVelocity"), "offVelocity", off, 0.0, 1.0)?;
            }
            if let Some(chance) = note.chance {
                range(&format!("{path}.chance"), "chance", chance, 0.0, 1.0)?;
            }
        }
    }
    for (i, clip) in snapshot.pattern_clips.iter().enumerate() {
        let path = format!("$.patternClips[{i}]");
        check_clip_length(
            &path,
            clip.duration_beats,
            clip.loop_count,
            clip.last_beat,
            clip.start_beat,
        )?;
        if let Some(scale) = clip.velocity_scale {
            range(
                &format!("{path}.velocityScale"),
                "velocityScale",
                scale,
                0.0,
                2.0,
            )?;
        }
        if let Some(probability) = clip.probability {
            range(
                &format!("{path}.probability"),
                "probability",
                probability,
                0.0,
                1.0,
            )?;
        }
    }
    for (i, clip) in snapshot.sample_clips.iter().enumerate() {
        let path = format!("$.sampleClips[{i}]");
        if let Some(duration) = clip.duration_beats {
            if duration.numerator() == 0 {
                return Err(OxitoneError::with_path(
                    codes::INVALID_PROJECT,
                    "durationBeats must be > 0",
                    format!("{path}.durationBeats"),
                ));
            }
        }
        if let Some(gain) = clip.gain {
            range(&format!("{path}.gain"), "gain", gain, 0.0, 2.0)?;
        }
        if let Some(pan) = clip.pan {
            range(&format!("{path}.pan"), "pan", pan, -1.0, 1.0)?;
        }
        if let Some(rate) = clip.rate {
            range(&format!("{path}.rate"), "rate", rate, 0.25, 4.0)?;
        }
        if let Some(loop_spec) = &clip.loop_spec {
            check_loop_spec(&format!("{path}.loop"), loop_spec)?;
        }
    }
    for (i, sample) in snapshot.samples.iter().enumerate() {
        let path = format!("$.samples[{i}]");
        if let Some(provenance) = &sample.provenance {
            if provenance.source_sha256.len() != 64
                || !provenance
                    .source_sha256
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
                || provenance.source_sample_rate == 0
                || provenance.source_channels == 0
                || provenance.source_bit_depth == Some(0)
                || provenance.decoder.is_empty()
            {
                return Err(OxitoneError::with_path(
                    codes::INVALID_PROJECT,
                    "invalid sample provenance",
                    format!("{path}.provenance"),
                ));
            }
        }
        if sample.frames == 0 || sample.sample_rate == 0 || ![1, 2].contains(&sample.channels) {
            return Err(OxitoneError::with_path(
                codes::INVALID_PROJECT,
                "sample needs frames > 0, sampleRate > 0 and 1|2 channels",
                path,
            ));
        }
        if let Some(length) = sample.musical_length_beats {
            if length.numerator() == 0 {
                return Err(OxitoneError::with_path(
                    codes::INVALID_PROJECT,
                    "musicalLengthBeats must be > 0",
                    format!("{path}.musicalLengthBeats"),
                ));
            }
        }
        if let Some(edits) = &sample.edits {
            let (start, end) = (
                edits.start_frame.unwrap_or(0),
                edits.end_frame.unwrap_or(sample.frames),
            );
            if start >= end || end > sample.frames {
                return Err(OxitoneError::with_path(
                    codes::INVALID_PROJECT,
                    "sample edits need startFrame < endFrame <= frames",
                    format!("{path}.edits"),
                ));
            }
            if let Some(level) = edits.level {
                range(&format!("{path}.edits.level"), "level", level, 0.0, 2.0)?;
            }
            if let Some(tone) = edits.tone {
                range(&format!("{path}.edits.tone"), "tone", tone, -1.0, 1.0)?;
            }
            if let Some(normalize) = &edits.normalize {
                if !normalize.peak_db.is_finite() || normalize.peak_db > 0.0 {
                    return Err(OxitoneError::with_path(
                        codes::INVALID_PROJECT,
                        "normalize peakDb must be finite and <= 0 dBFS",
                        format!("{path}.edits.normalize.peakDb"),
                    ));
                }
            }
        }
    }
    Ok(())
}
