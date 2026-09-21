use oxitone_core::OxitoneError;
use oxitone_graph::execution::ProcessPosition;
use oxitone_graph::{
    ChannelLayout, HostContext, Plugin, PluginCapabilities, PluginDescriptor, PluginInstance,
    PluginKind, ProcessContext,
};
use std::sync::{Arc, Mutex};

pub struct Call {
    pub position: ProcessPosition,
    pub frames: usize,
    pub resets: usize,
}
pub struct Probe {
    pub calls: Arc<Mutex<Vec<Call>>>,
    pub effect: bool,
    pub fail: bool,
}
fn descriptor(effect: bool) -> PluginDescriptor {
    PluginDescriptor {
        plugin_id: "fixture.isolated".into(),
        plugin_version: "1.0.0".into(),
        kind: if effect {
            PluginKind::Effect
        } else {
            PluginKind::Instrument
        },
        input_layout: if effect {
            ChannelLayout::Stereo
        } else {
            ChannelLayout::None
        },
        output_layout: ChannelLayout::Stereo,
        parameters: vec![],
        capabilities: PluginCapabilities::default(),
        state_schema: None,
        max_polyphony: None,
    }
}
impl Plugin for Probe {
    fn descriptor(&self) -> &PluginDescriptor {
        static INSTRUMENT: std::sync::OnceLock<PluginDescriptor> = std::sync::OnceLock::new();
        static EFFECT: std::sync::OnceLock<PluginDescriptor> = std::sync::OnceLock::new();
        if self.effect {
            EFFECT.get_or_init(|| descriptor(true))
        } else {
            INSTRUMENT.get_or_init(|| descriptor(false))
        }
    }
    fn create(&self, _: &HostContext) -> Box<dyn PluginInstance> {
        Box::new(Instance {
            calls: self.calls.clone(),
            resets: 0,
            fail: self.fail,
        })
    }
}
struct Instance {
    calls: Arc<Mutex<Vec<Call>>>,
    resets: usize,
    fail: bool,
}
impl PluginInstance for Instance {
    fn requires_isolation(&self) -> bool {
        true
    }
    fn prepare(&mut self, _: f64, _: u32) {}
    fn process(&mut self, _: &mut ProcessContext<'_>) {
        panic!("isolated plugin entered realtime execution")
    }
    fn process_isolated(
        &mut self,
        context: &mut ProcessContext<'_>,
        position: &ProcessPosition,
    ) -> Result<(), OxitoneError> {
        self.calls.lock().unwrap().push(Call {
            position: *position,
            frames: context.frames,
            resets: self.resets,
        });
        for output in context.outputs.iter_mut() {
            output[..context.frames].fill(0.25);
        }
        if self.fail {
            return Err(OxitoneError::new("PluginHostTimeout", "fixture timeout"));
        }
        Ok(())
    }
    fn reset(&mut self) {
        self.resets += 1;
    }
    fn tail_frames(&self) -> u64 {
        0
    }
    fn latency_frames(&self) -> u64 {
        0
    }
}
