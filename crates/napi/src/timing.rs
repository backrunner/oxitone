use napi_derive::napi;
use oxitone_core::{codes, wire::decode_project_snapshot, Beat, OxitoneError, PROTOCOL_VERSION};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Query {
    start_beat: Beat,
    duration_seconds: f64,
}

#[napi(js_name = "resolveBeatDuration")]
pub fn resolve_beat_duration(snapshot_json: String, query_json: String) -> napi::Result<String> {
    super::guarded(|| {
        let snapshot = decode_project_snapshot(&snapshot_json)?;
        let query: Query = serde_json::from_str(&query_json).map_err(|e| {
            OxitoneError::new(codes::INVALID_PROJECT, format!("invalid timing query: {e}"))
        })?;
        let duration = oxitone_graph::compile::resolve_beat_duration(
            &snapshot,
            query.start_beat,
            query.duration_seconds,
        )?;
        Ok(
            serde_json::json!({ "protocolVersion": PROTOCOL_VERSION, "durationBeats": duration })
                .to_string(),
        )
    })
}
