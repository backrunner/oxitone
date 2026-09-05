//! Serde helpers for wire encodings that JSON numbers cannot hold:
//! `u64` frames/revisions travel as unsigned decimal strings
//! (06-format-and-export.md).

use serde::{Deserialize, Deserializer, Serializer};

pub mod u64_string {
    use super::*;

    pub fn serialize<S: Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
        let text = String::deserialize(deserializer)?;
        if text.is_empty() || text.starts_with('-') || (text.len() > 1 && text.starts_with('0')) {
            return Err(serde::de::Error::custom(format!(
                "frame must be an unsigned decimal string, got {text:?}"
            )));
        }
        text.parse::<u64>().map_err(serde::de::Error::custom)
    }
}

pub mod opt_u64_string {
    use super::*;

    pub fn serialize<S: Serializer>(value: &Option<u64>, serializer: S) -> Result<S::Ok, S::Error> {
        match value {
            Some(value) => serializer.serialize_str(&value.to_string()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<u64>, D::Error> {
        let text = Option::<String>::deserialize(deserializer)?;
        match text {
            None => Ok(None),
            Some(text) => {
                if text.is_empty()
                    || text.starts_with('-')
                    || (text.len() > 1 && text.starts_with('0'))
                {
                    return Err(serde::de::Error::custom(format!(
                        "frame must be an unsigned decimal string, got {text:?}"
                    )));
                }
                text.parse::<u64>()
                    .map(Some)
                    .map_err(serde::de::Error::custom)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Holder {
        #[serde(with = "super::u64_string")]
        frame: u64,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            with = "super::opt_u64_string"
        )]
        at: Option<u64>,
    }

    #[test]
    fn u64_round_trips_as_decimal_string() {
        let holder = Holder {
            frame: u64::MAX,
            at: Some(42),
        };
        let json = serde_json::to_string(&holder).unwrap();
        assert_eq!(json, r#"{"frame":"18446744073709551615","at":"42"}"#);
        assert_eq!(serde_json::from_str::<Holder>(&json).unwrap(), holder);
    }

    #[test]
    fn rejects_non_canonical_strings() {
        assert!(serde_json::from_str::<Holder>(r#"{"frame":"042"}"#).is_err());
        assert!(serde_json::from_str::<Holder>(r#"{"frame":"-1"}"#).is_err());
        assert!(serde_json::from_str::<Holder>(r#"{"frame":"18446744073709551616"}"#).is_err());
    }
}
