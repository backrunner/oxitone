use napi_derive::napi;
use oxitone_core::wire::{CacheSampleRequest, InspectSampleRequest};
use oxitone_core::{codes, OxitoneError};

#[napi(js_name = "inspectSample")]
pub fn inspect_sample(request_json: String) -> napi::Result<String> {
    super::guarded(|| {
        let request: InspectSampleRequest =
            serde_json::from_str(&request_json).map_err(|error| {
                OxitoneError::new(
                    codes::INVALID_PROJECT,
                    format!("invalid sample inspection request: {error}"),
                )
            })?;
        let info = oxitone_samples::inspect_sample(&request)?;
        serde_json::to_string(&info).map_err(|error| {
            OxitoneError::new(
                codes::REALTIME_FAULT,
                format!("sample info encoding failed: {error}"),
            )
        })
    })
}

#[napi(js_name = "cacheSample")]
pub fn cache_sample(request_json: String) -> napi::Result<String> {
    super::guarded(|| {
        let request: CacheSampleRequest = serde_json::from_str(&request_json).map_err(|error| {
            OxitoneError::new(
                codes::INVALID_PROJECT,
                format!("invalid sample cache request: {error}"),
            )
        })?;
        let info = oxitone_samples::cache_sample(&request)?;
        serde_json::to_string(&info).map_err(|error| {
            OxitoneError::new(
                codes::REALTIME_FAULT,
                format!("cached sample encoding failed: {error}"),
            )
        })
    })
}
