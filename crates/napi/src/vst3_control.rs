//! Async instance controls. Never hold the engine registry while calling a vendor helper.
use super::{guarded, lock_registry, not_compiled, unknown_engine};
use napi::{bindgen_prelude::AsyncTask, Env, Task};
use napi_derive::napi;
use oxitone_core::OxitoneError;
use oxitone_render::plugin_controls::{request::Request, ControlRegistry};

fn controls(engine_id: &str) -> Result<ControlRegistry, OxitoneError> {
    let engines = lock_registry();
    let engine = engines
        .get(engine_id)
        .ok_or_else(|| unknown_engine(engine_id))?;
    engine
        .controls
        .clone()
        .ok_or_else(|| not_compiled(engine_id))
}

#[napi(js_name = "getVst3Instances")]
pub fn get_vst3_instances(engine_id: String) -> napi::Result<String> {
    guarded(|| {
        serde_json::to_string(&controls(&engine_id)?.inventory("vst3"))
            .map_err(|e| OxitoneError::new("RealtimeFault", e.to_string()))
    })
}

pub struct ControlTask {
    engine_id: String,
    request_json: String,
}
impl Task for ControlTask {
    type Output = String;
    type JsValue = String;

    fn compute(&mut self) -> napi::Result<String> {
        guarded(|| {
            let request = Request::decode(&self.request_json)?;
            let registry = controls(&self.engine_id)?;
            let result = request.execute(&registry)?;
            // A graph may have been replaced/disposed while vendor code was running.
            controls(&self.engine_id)?.check(&request.graph_generation)?;
            Ok(result.to_string())
        })
    }

    fn resolve(&mut self, _env: Env, output: String) -> napi::Result<String> {
        guarded(|| {
            let request = Request::decode(&self.request_json)?;
            controls(&self.engine_id)?.check(&request.graph_generation)?;
            Ok(output)
        })
    }
}

#[napi(js_name = "controlVst3Instance", ts_return_type = "Promise<string>")]
pub fn control_vst3_instance(engine_id: String, request_json: String) -> AsyncTask<ControlTask> {
    AsyncTask::new(ControlTask {
        engine_id,
        request_json,
    })
}
