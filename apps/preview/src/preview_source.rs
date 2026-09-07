//! Control-thread source fingerprint for UI-only refreshes.
use oxitone_core::{
    wire::{AllowPlugins, ProjectSnapshot, RegisterPluginOptions},
    OxitoneError,
};
use serde_json::json;
use sha2::{Digest, Sha256};

pub fn audio_key(
    snapshot: &ProjectSnapshot,
    base: &str,
    plugins: &[RegisterPluginOptions],
    policy: Option<AllowPlugins>,
) -> Result<[u8; 32], OxitoneError> {
    let registrations: Vec<_> = plugins
        .iter()
        .map(|p| json!({"path":p.library_path, "manifest":p.manifest, "hash":p.expected_hash}))
        .collect();
    let mut source =
        json!({ "snapshot": snapshot, "base": base, "plugins": registrations, "policy": policy });
    source["snapshot"]["revision"] = json!("0");
    let bytes = serde_json::to_vec(&source).map_err(|e| crate::wire::invalid(&e.to_string()))?;
    Ok(Sha256::digest(bytes).into())
}
