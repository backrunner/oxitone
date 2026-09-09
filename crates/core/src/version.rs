//! Protocol version gate. Mirrors `packages/protocol/src/version.ts`.

use crate::error::{codes, OxitoneError};

/// Current protocol version (`major.minor`).
pub const PROTOCOL_VERSION: &str = "1.2";
pub const PROTOCOL_MAJOR: u64 = 1;
pub const PROTOCOL_MINOR: u64 = 2;

/// Validate a `protocolVersion` string from a wire message. Unknown major
/// versions and minors newer than this build are rejected.
pub fn check_protocol_version(version: &str) -> Result<(), OxitoneError> {
    let (major, minor) = version
        .split_once('.')
        .and_then(
            |(major, minor)| match (major.parse::<u64>(), minor.parse::<u64>()) {
                (Ok(major), Ok(minor)) => Some((major, minor)),
                _ => None,
            },
        )
        .ok_or_else(|| {
            OxitoneError::new(
                codes::PROTOCOL_VERSION_UNSUPPORTED,
                format!("malformed protocolVersion: {version:?}"),
            )
        })?;
    if major != PROTOCOL_MAJOR || minor > PROTOCOL_MINOR {
        return Err(OxitoneError::new(
            codes::PROTOCOL_VERSION_UNSUPPORTED,
            format!("unsupported protocolVersion {version}; this build speaks {PROTOCOL_VERSION}"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_current_version() {
        assert!(check_protocol_version("1.0").is_ok());
        assert!(check_protocol_version("1.1").is_ok());
    }

    #[test]
    fn rejects_unknown_major_and_newer_minor() {
        for version in ["2.0", "0.9", "1.3", "banana", "1", "1.0.0"] {
            let err = check_protocol_version(version).unwrap_err();
            assert_eq!(err.code, codes::PROTOCOL_VERSION_UNSUPPORTED, "{version}");
        }
    }
}
