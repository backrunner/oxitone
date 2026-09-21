//! Settings shared by silent configuration inspection and the native configuration window.
use crate::wire::Configuration;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Options {
    pub sample_rate: u32,
    pub block_size: usize,
    pub configuration: Option<Configuration>,
    pub parameters: BTreeMap<String, f64>,
}
impl Options {
    pub fn validate(&self) -> crate::Result<()> {
        crate::wire::processing_settings(
            self.sample_rate,
            self.block_size,
            120.,
            [4, 4],
            self.configuration.as_ref(),
            &self.parameters,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn configuration_bounds_and_versions_are_checked_before_loading() {
        let mut value = serde_json::json!({"sampleRate":48000,"blockSize":128,"parameters":{}});
        serde_json::from_value::<Options>(value.clone())
            .unwrap()
            .validate()
            .unwrap();
        for field in ["sampleRate", "blockSize"] {
            let mut invalid = value.clone();
            invalid[field] = 0.into();
            assert!(serde_json::from_value::<Options>(invalid)
                .unwrap()
                .validate()
                .is_err());
        }
        value["configuration"] = serde_json::json!({"formatVersion":2,"classId":"1".repeat(32),"sha256":"a".repeat(64),"stateBase64":"","parameters":{}});
        assert_eq!(
            serde_json::from_value::<Options>(value)
                .unwrap()
                .validate()
                .unwrap_err()
                .code,
            "ProtocolVersionUnsupported"
        );
    }
}
