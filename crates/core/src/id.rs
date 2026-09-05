//! Opaque, globally unique string entity IDs (01-architecture.md).
//! Prefixes are recognized for the documented kinds; unknown prefixes stay
//! valid because IDs are opaque to the engine.

use crate::error::{codes, OxitoneError};

/// Documented ID prefixes.
pub mod prefixes {
    pub const TRACK: &str = "trk_";
    pub const PATTERN: &str = "pat_";
    pub const CHANNEL: &str = "chn_";
    pub const MIXER_CHANNEL: &str = "mix_";
    pub const SAMPLE: &str = "smp_";
    pub const AUTOMATION: &str = "auto_";
}

/// Entity kind implied by a documented ID prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdKind {
    Track,
    Pattern,
    Channel,
    MixerChannel,
    Sample,
    Automation,
}

/// Kind for a documented prefix, or `None` for an unknown (still valid) one.
pub fn id_kind(id: &str) -> Option<IdKind> {
    use prefixes::*;
    [
        (TRACK, IdKind::Track),
        (PATTERN, IdKind::Pattern),
        (CHANNEL, IdKind::Channel),
        (MIXER_CHANNEL, IdKind::MixerChannel),
        (SAMPLE, IdKind::Sample),
        (AUTOMATION, IdKind::Automation),
    ]
    .into_iter()
    .find(|(prefix, _)| id.starts_with(prefix))
    .map(|(_, kind)| kind)
}

/// Structural validation: non-empty, bounded length, wire-safe charset.
pub fn validate_id(id: &str) -> Result<(), OxitoneError> {
    let valid = !id.is_empty()
        && id.len() <= 128
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if valid {
        Ok(())
    } else {
        Err(OxitoneError::new(
            codes::INVALID_PROJECT,
            format!("invalid entity id: {id:?}"),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_documented_prefixes() {
        assert_eq!(id_kind("trk_0001"), Some(IdKind::Track));
        assert_eq!(id_kind("mix_0002"), Some(IdKind::MixerChannel));
        assert_eq!(id_kind("auto_0003"), Some(IdKind::Automation));
        assert_eq!(id_kind("prj_0001"), None);
    }

    #[test]
    fn validates_charset_and_length() {
        assert!(validate_id("chn_0001-x").is_ok());
        assert!(validate_id("").is_err());
        assert!(validate_id("has space").is_err());
        assert!(validate_id(&"x".repeat(129)).is_err());
    }
}
