//! Background vendor calls, bound to both the accepted source snapshot and native graph.
use super::Engine;
use oxitone_core::OxitoneError;
use oxitone_render::plugin_controls::{request::Request, ControlRegistry};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

#[cfg(test)]
#[path = "vst3_control_tests.rs"]
mod tests;

pub struct ControlJob {
    snapshot_revision: String,
    request: Request,
    registry: ControlRegistry,
    deadline: Instant,
}
pub struct ControlCompleted {
    snapshot_revision: String,
    graph_generation: String,
    result: Result<Value, OxitoneError>,
}
impl ControlJob {
    pub fn run(mut self) -> ControlCompleted {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.request.timeout_ms = remaining(self.deadline)?;
            self.request.execute(&self.registry)
        }))
        .unwrap_or_else(|_| {
            Err(OxitoneError::new(
                "RealtimeFault",
                "native control task panicked",
            ))
        });
        ControlCompleted {
            snapshot_revision: self.snapshot_revision,
            graph_generation: self.request.graph_generation,
            result,
        }
    }
}
impl Engine {
    fn controls_for(&self, revision: &str) -> Result<ControlRegistry, OxitoneError> {
        if self
            .current
            .as_ref()
            .map(|p| p.snapshot.revision.to_string())
            .as_deref()
            != Some(revision)
        {
            return Err(OxitoneError::new(
                "SourceChanged",
                "Preview snapshot revision is no longer current",
            ));
        }
        self.controls.clone().ok_or_else(|| {
            OxitoneError::new(
                "PluginHostUnavailable",
                "Preview has no accepted plugin graph",
            )
        })
    }
    pub fn vst3_instances(&self, revision: String) -> Value {
        match self.controls_for(&revision) {
            Ok(registry) => {
                json!({"protocolVersion":"1.0","type":"vst3Instances","snapshotRevision":revision,"inventory":registry.inventory("vst3")})
            }
            Err(error) => self.error(error),
        }
    }
    pub fn prepare_vst3_control(
        &self,
        revision: String,
        value: Value,
        received: Instant,
    ) -> Result<ControlJob, OxitoneError> {
        let request = Request::decode(&value.to_string())?;
        let deadline = received + Duration::from_millis(request.timeout_ms);
        remaining(deadline)?;
        let registry = self.controls_for(&revision)?;
        registry.check(&request.graph_generation)?;
        Ok(ControlJob {
            snapshot_revision: revision,
            request,
            registry,
            deadline,
        })
    }
    pub fn complete_vst3_control(&self, completed: ControlCompleted) -> Value {
        let result = self
            .controls_for(&completed.snapshot_revision)
            .and_then(|registry| registry.check(&completed.graph_generation))
            .and(completed.result);
        match result {
            Ok(result) => {
                json!({"protocolVersion":"1.0","type":"vst3Control","snapshotRevision":completed.snapshot_revision,"result":result})
            }
            Err(error) => self.error(error),
        }
    }
}

fn remaining(deadline: Instant) -> Result<u64, OxitoneError> {
    let remaining = deadline
        .saturating_duration_since(Instant::now())
        .as_millis() as u64;
    if remaining == 0 {
        Err(OxitoneError::new(
            "PluginHostTimeout",
            "Preview VST3 request expired before execution",
        ))
    } else {
        Ok(remaining)
    }
}
