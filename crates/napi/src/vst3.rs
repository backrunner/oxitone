//! Explicit VST3 registration; subprocess startup stays outside the engine registry mutex.
use super::*;

#[napi(js_name = "registerVst3")]
pub fn register_vst3(engine_id: String, options_json: String) -> napi::Result<String> {
    guarded(|| {
        #[cfg(target_os = "macos")]
        {
            let options: oxitone_render::vst3::RegistrationOptions =
                serde_json::from_str(&options_json)
                    .map_err(|e| OxitoneError::new("PluginConfigInvalid", e.to_string()))?;
            let policy = {
                let engines = lock_registry();
                let engine = engines
                    .get(&engine_id)
                    .ok_or_else(|| unknown_engine(&engine_id))?;
                engine
                    .options
                    .as_ref()
                    .and_then(|o| o.allow_plugins)
                    .unwrap_or(oxitone_core::wire::AllowPlugins::SignedOnly)
            };
            let plugin = oxitone_render::vst3::Vst3Plugin::load(options, policy)?;
            let key = (
                plugin.registration.plugin_id.clone(),
                plugin.registration.plugin_version.clone(),
            );
            let mut engines = lock_registry();
            let engine = engines
                .get_mut(&engine_id)
                .ok_or_else(|| unknown_engine(&engine_id))?;
            if let Some(existing) = engine.vst3_plugins.get(&key) {
                return serde_json::to_string(&existing.registration)
                    .map_err(|e| OxitoneError::new(codes::REALTIME_FAULT, e.to_string()));
            }
            if engine.plugins.contains_id(&key.0)
                && !engine.vst3_plugins.keys().any(|(id, _)| id == &key.0)
            {
                return Err(OxitoneError::new(
                    codes::PLUGIN_MANIFEST_MISMATCH,
                    "plugin ID is already reserved",
                ));
            }
            engine.plugins.register(plugin.clone())?;
            let result = serde_json::to_string(&plugin.registration)
                .map_err(|e| OxitoneError::new(codes::REALTIME_FAULT, e.to_string()))?;
            engine.vst3_plugins.insert(key, plugin);
            Ok(result)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (engine_id, options_json);
            Err(OxitoneError::new(
                "PluginCapabilityUnsupported",
                "VST3 project hosting currently requires macOS",
            ))
        }
    })
}
