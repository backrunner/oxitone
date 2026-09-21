//! Versioned control-side preparation; not a plugin transport/seek protocol.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Schedule {
    pub schedule_version: u32,
    pub epoch: u64,
    pub latency_blocks: usize,
}
impl Schedule {
    pub fn validate(&self) -> crate::Result<()> {
        if self.schedule_version != 1 {
            return Err(crate::Error::new(
                "ProtocolVersionUnsupported",
                "unsupported VST3 schedule version",
            ));
        }
        if self.epoch > 9_007_199_254_740_991 || !(2..=16).contains(&self.latency_blocks) {
            return Err(crate::invalid("invalid VST3 schedule epoch or latency"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_version_bounds_and_unknown_fields() {
        for (version, epoch, latency, code) in [
            (1, 0, 2, None),
            (1, 9_007_199_254_740_991, 16, None),
            (2, 0, 2, Some("ProtocolVersionUnsupported")),
            (1, 9_007_199_254_740_992, 2, Some("PluginConfigInvalid")),
            (1, 0, 1, Some("PluginConfigInvalid")),
            (1, 0, 17, Some("PluginConfigInvalid")),
        ] {
            let schedule = Schedule {
                schedule_version: version,
                epoch,
                latency_blocks: latency,
            };
            assert_eq!(schedule.validate().err().map(|e| e.code), code);
        }
        assert!(serde_json::from_str::<Schedule>(
            r#"{"scheduleVersion":1,"epoch":0,"latencyBlocks":2,"seekFrame":0}"#
        )
        .is_err());
    }
}
