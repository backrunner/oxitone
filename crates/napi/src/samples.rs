use napi_derive::napi;
use oxitone_core::wire::InspectSampleRequest;
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
