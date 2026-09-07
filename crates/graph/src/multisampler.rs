//! Control-thread multisample mapping validation shared with the native instrument.
use oxitone_core::{codes, OxitoneError};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

pub const PLUGIN_ID: &str = "oxitone.multisampler";
pub const SCHEMA_ID: crate::descriptor::StateSchemaId =
    crate::descriptor::StateSchemaId("oxitone.multisampler.regions@1");
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub version: u8,
    pub regions: Vec<Region>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Region {
    pub resource: String,
    pub root_key: u8,
    pub key_range: [u8; 2],
    pub velocity_range: [u8; 2],
    #[serde(default = "unity")]
    pub gain: f64,
}
fn unity() -> f64 {
    1.
}
pub fn parse(
    value: &Value,
    resources: Option<&BTreeMap<String, String>>,
    path: &str,
) -> Result<(State, Vec<u16>), OxitoneError> {
    let invalid = |message: String| OxitoneError::with_path(codes::INVALID_PROJECT, message, path);
    let state: State = serde_json::from_value(value.clone()).map_err(|e| invalid(e.to_string()))?;
    if state.version != 1 || state.regions.is_empty() || state.regions.len() > 256 {
        return Err(invalid("expected version 1 and 1–256 regions".into()));
    }
    let mut lookup = vec![u16::MAX; 128 * 128];
    for (index, r) in state.regions.iter().enumerate() {
        let [lo, hi] = r.key_range;
        let [soft, loud] = r.velocity_range;
        if r.root_key > 127
            || lo > hi
            || hi > 127
            || soft < 1
            || soft > loud
            || loud > 127
            || !r.gain.is_finite()
            || !(0.0..=4.0).contains(&r.gain)
            || r.resource.is_empty()
            || r.resource.len() > 128
            || !resources.is_some_and(|res| res.contains_key(&r.resource))
        {
            return Err(invalid(format!(
                "invalid multisampler region {index} or missing resource"
            )));
        }
        for key in lo..=hi {
            for velocity in soft..=loud {
                let cell = &mut lookup[key as usize * 128 + velocity as usize];
                if *cell != u16::MAX {
                    return Err(invalid(format!("overlapping multisampler region {index}")));
                }
                *cell = index as u16;
            }
        }
    }
    Ok((state, lookup))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn boundaries_and_invalid_regions() {
        let region = json!({"resource":"a","rootKey":60,"keyRange":[48,72],"velocityRange":[1,64]});
        let state = json!({"version":1,"regions":[region.clone()]});
        let resources = BTreeMap::from([("a".into(), "smp_a".into())]);
        let (_, lookup) = parse(&state, Some(&resources), "$").unwrap();
        assert_eq!(lookup[48 * 128 + 1], 0);
        assert_eq!(lookup[72 * 128 + 64], 0);
        assert_eq!(lookup[72 * 128 + 65], u16::MAX);
        assert!(parse(&state, None, "$").is_err());
        for bad in [
            json!({"version":2,"regions":[region.clone()]}),
            json!({"version":1,"regions":[]}),
            json!({"version":1,"regions":[region.clone(),region.clone()]}),
        ] {
            assert_eq!(
                parse(&bad, Some(&resources), "$").unwrap_err().code,
                codes::INVALID_PROJECT
            );
        }
        for (field, value) in [
            ("keyRange", json!([72, 48])),
            ("keyRange", json!([0, 128])),
            ("velocityRange", json!([0, 127])),
            ("velocityRange", json!([65, 64])),
            ("gain", json!(5)),
            ("rootKey", json!(128)),
            ("resource", json!("absent")),
        ] {
            let mut bad = state.clone();
            bad["regions"][0][field] = value;
            assert!(parse(&bad, Some(&resources), "$").is_err(), "{field}");
        }
    }
}
