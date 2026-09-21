//! Control-only lifecycle capacity and immutable audio format.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManagerOptions {
    pub manager_version: u32,
    pub sample_rate: u32,
    pub block_size: usize,
    pub capacity: usize,
}
impl ManagerOptions {
    pub fn validate(&self) -> crate::Result<()> {
        if self.manager_version != 1 {
            return Err(crate::Error::new(
                "ProtocolVersionUnsupported",
                "unsupported VST3 manager version",
            ));
        }
        if !(8000..=192000).contains(&self.sample_rate)
            || !(16..=4096).contains(&self.block_size)
            || !(2..=16).contains(&self.capacity)
        {
            return Err(crate::invalid("invalid VST3 manager format or capacity"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn requires_a_supported_version_bounded_capacity_and_format() {
        let valid = ManagerOptions {
            manager_version: 1,
            sample_rate: 48000,
            block_size: 128,
            capacity: 2,
        };
        valid.validate().unwrap();
        for config in [
            ManagerOptions {
                sample_rate: 0,
                ..valid
            },
            ManagerOptions {
                sample_rate: 192001,
                ..valid
            },
            ManagerOptions {
                block_size: 0,
                ..valid
            },
            ManagerOptions {
                block_size: 4097,
                ..valid
            },
            ManagerOptions {
                capacity: 1,
                ..valid
            },
            ManagerOptions {
                capacity: 17,
                ..valid
            },
        ] {
            assert_eq!(config.validate().unwrap_err().code, "PluginConfigInvalid");
        }
        assert_eq!(
            ManagerOptions {
                manager_version: 2,
                ..valid
            }
            .validate()
            .unwrap_err()
            .code,
            "ProtocolVersionUnsupported"
        );
        let mut value = serde_json::to_value(valid).unwrap();
        value["autoRestart"] = true.into();
        assert!(serde_json::from_value::<ManagerOptions>(value).is_err());
    }
}
