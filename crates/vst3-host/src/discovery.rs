//! Factory enumeration runs only in a disposable helper, after the normal bundle policy/hash checks.
use crate::{
    bundle, invalid, native,
    wire::{Policy, Source},
    Error, Result,
};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    protocol_version: u32,
    operation: Operation,
    source: BundleSource,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum Operation {
    ListClasses,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BundleSource {
    bundle_path: String,
    expected_hash: Option<String>,
    allow_plugins: Policy,
}

pub fn execute(request: Request) -> Result<serde_json::Value> {
    if request.protocol_version != 1 {
        return Err(Error::new(
            "ProtocolVersionUnsupported",
            "unsupported VST3 discovery protocol",
        ));
    }
    let _ = request.operation;
    // The verifier operates on bundles; no class is selected or instantiated by this placeholder.
    let source = Source {
        bundle_path: request.source.bundle_path,
        expected_hash: request.source.expected_hash,
        allow_plugins: request.source.allow_plugins,
        class_id: "0".repeat(32),
    };
    source.validate()?;
    let (path, hash) = bundle::verify(&source)?;
    // Upstream detailed discovery may initialize its first component to inspect buses. It remains
    // on the helper main thread and under the SDK's whole-process timeout, never in Node/Preview.
    let metadata = vst3_host::discovery::get_detailed_plugin_info(&path).map_err(native)?;
    if bundle::bundle_hash(&path)? != hash {
        return Err(Error::new(
            "SourceChanged",
            "VST3 bundle changed during class discovery",
        ));
    }
    if metadata.classes.len() > 1024 {
        return Err(Error::new(
            "BudgetExceeded",
            "VST3 factory exceeds 1024 classes",
        ));
    }
    let mut classes = Vec::new();
    for class in metadata
        .classes
        .into_iter()
        .filter(|c| c.category == "Audio Module Class")
    {
        if class.class_id.len() != 32 || !class.class_id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(invalid("VST3 factory returned an invalid class ID"));
        }
        let class_id = class.class_id.to_ascii_lowercase();
        if classes
            .iter()
            .any(|c: &serde_json::Value| c["classId"] == class_id)
        {
            return Err(invalid("VST3 factory returned duplicate audio classes"));
        }
        classes.push(serde_json::json!({"classId": class_id, "name": class.name, "category": class.category, "version": class.version}));
    }
    Ok(
        serde_json::json!({"protocolVersion": 1, "bundlePath": source.bundle_path,
        "sha256": hash, "vendor": metadata.factory.vendor, "classes": classes}),
    )
}
