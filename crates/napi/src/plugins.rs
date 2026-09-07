//! Explicit per-engine plugin registration. Native loading stays outside
//! the engine-registry mutex so other engines can continue processing.
use super::*;
use oxitone_core::wire::{AllowPlugins, RegisterPluginOptions};

#[napi(js_name = "getPluginInfo")]
pub fn get_plugin_info(
    engine_id: String,
    plugin_id: String,
    plugin_version: String,
) -> napi::Result<String> {
    guarded(|| {
        let engines = lock_registry();
        let engine = engines
            .get(&engine_id)
            .ok_or_else(|| unknown_engine(&engine_id))?;
        let descriptor = engine
            .plugins
            .lookup_descriptor(&plugin_id, &plugin_version)
            .ok_or_else(|| {
                OxitoneError::new(codes::PLUGIN_MANIFEST_MISMATCH, "unknown plugin ID/version")
            })?;
        Ok(serde_json::json!({
            "protocolVersion": PROTOCOL_VERSION, "pluginId": descriptor.plugin_id,
            "pluginVersion": descriptor.plugin_version, "abiMajor": 1,
            "kind": match descriptor.kind { oxitone_graph::PluginKind::Instrument => "instrument", oxitone_graph::PluginKind::Effect => "effect" },
            "parameters": descriptor.parameters, "stateSchema": descriptor.state_schema.map(|id| id.0),
        }).to_string())
    })
}

#[napi(js_name = "registerPlugin")]
pub fn register_plugin(engine_id: String, options_json: String) -> napi::Result<String> {
    guarded(|| {
        let options: RegisterPluginOptions = serde_json::from_str(&options_json)
            .map_err(|e| OxitoneError::new(codes::PLUGIN_MANIFEST_MISMATCH, e.to_string()))?;
        let policy = {
            let engines = lock_registry();
            let engine = engines
                .get(&engine_id)
                .ok_or_else(|| unknown_engine(&engine_id))?;
            engine
                .options
                .as_ref()
                .and_then(|o| o.allow_plugins)
                .unwrap_or(if cfg!(debug_assertions) {
                    AllowPlugins::Any
                } else {
                    AllowPlugins::SignedOnly
                })
        };
        // Native plugins are explicitly trusted by the caller of this API.
        let loaded = unsafe { oxitone_render::plugins::load_plugin(&options, policy)? };
        let key = (
            loaded.registration.plugin_id.clone(),
            loaded.registration.plugin_version.clone(),
        );
        let mut engines = lock_registry();
        let engine = engines
            .get_mut(&engine_id)
            .ok_or_else(|| unknown_engine(&engine_id))?;
        if let Some(existing) = engine.dynamic_plugins.get(&key) {
            if existing.registration.sha256 != loaded.registration.sha256 {
                return Err(OxitoneError::new(
                    codes::PLUGIN_MANIFEST_MISMATCH,
                    "plugin ID/version already registered with a different binary",
                ));
            }
            return serde_json::to_string(&existing.registration)
                .map_err(|e| OxitoneError::new(codes::REALTIME_FAULT, e.to_string()));
        }
        let has_dynamic_version = engine.dynamic_plugins.keys().any(|(id, _)| id == &key.0);
        if engine.plugins.contains_id(&key.0) && !has_dynamic_version {
            return Err(OxitoneError::new(
                codes::PLUGIN_MANIFEST_MISMATCH,
                "built-in plugin IDs are reserved",
            ));
        }
        engine.plugins.register(loaded.clone())?;
        let result = serde_json::to_string(&loaded.registration)
            .map_err(|e| OxitoneError::new(codes::REALTIME_FAULT, e.to_string()))?;
        engine.dynamic_plugins.insert(key, loaded);
        Ok(result)
    })
}

#[napi(js_name = "getPluginDiagnostics")]
pub fn get_plugin_diagnostics(engine_id: String) -> napi::Result<String> {
    guarded(|| {
        let engines = lock_registry();
        let engine = engines
            .get(&engine_id)
            .ok_or_else(|| unknown_engine(&engine_id))?;
        Ok(serde_json::Value::Array(engine.dynamic_plugins.values().map(|p| serde_json::json!({
            "pluginId": p.registration.plugin_id, "pluginVersion": p.registration.plugin_version, "faults": p.fault_count()
        })).collect()).to_string())
    })
}
