use oxitone_core::OxitoneError;
use oxitone_graph::{
    control::NativeControl, ChannelLayout, HostContext, Plugin, PluginCapabilities,
    PluginDescriptor, PluginInstance, PluginKind, ProcessContext,
};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Barrier,
    },
    time::Duration,
};

pub struct Probe {
    descriptor: PluginDescriptor,
    seeded: bool,
    barrier: Option<Arc<Barrier>>,
}
impl Probe {
    pub fn new(effect: bool, seeded: bool, barrier: Option<Arc<Barrier>>) -> Self {
        Self {
            descriptor: PluginDescriptor {
                plugin_id: if effect {
                    "fixture.effect"
                } else {
                    "fixture.instrument"
                }
                .into(),
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
                parameters: vec![serde_json::from_value(serde_json::json!({
                    "id":"gain", "label":"Gain", "unit":"normalized", "min":0., "max":1.,
                    "default":1., "smoothing":"none", "rate":"control"
                }))
                .unwrap()],
                capabilities: PluginCapabilities::default(),
                state_schema: None,
                max_polyphony: None,
            },
            seeded,
            barrier,
        }
    }
}
impl Plugin for Probe {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }
    fn create(&self, _: &HostContext) -> Box<dyn PluginInstance> {
        Box::new(Instance {
            control: Arc::new(Control {
                value: AtomicU64::new(1f64.to_bits()),
                barrier: self.barrier.clone(),
            }),
            seeded: self.seeded,
        })
    }
    fn try_create_configured(
        &self,
        host: &HostContext,
        params: &BTreeMap<String, f64>,
        _: Option<&serde_json::Value>,
    ) -> Result<Box<dyn PluginInstance>, OxitoneError> {
        let instance = self.create(host);
        if self.seeded {
            instance
                .native_control()
                .unwrap()
                .request(&params["gain"].to_string(), Duration::from_secs(1))?;
        }
        Ok(instance)
    }
}
struct Control {
    value: AtomicU64,
    barrier: Option<Arc<Barrier>>,
}
impl NativeControl for Control {
    fn protocol(&self) -> &'static str {
        "vst3"
    }
    fn request(&self, json: &str, _: Duration) -> Result<String, OxitoneError> {
        if json == "wait" {
            let barrier = self.barrier.as_ref().unwrap();
            barrier.wait();
            barrier.wait();
        } else if let Ok(value) = json.parse::<f64>() {
            self.value.store(value.to_bits(), Ordering::Relaxed);
        }
        Ok(f64::from_bits(self.value.load(Ordering::Relaxed)).to_string())
    }
}
struct Instance {
    control: Arc<Control>,
    seeded: bool,
}
impl PluginInstance for Instance {
    fn initial_parameters_applied(&self) -> bool {
        self.seeded
    }
    fn native_control(&self) -> Option<Arc<dyn NativeControl>> {
        Some(self.control.clone())
    }
    fn prepare(&mut self, _: f64, _: u32) {}
    fn process(&mut self, ctx: &mut ProcessContext<'_>) {
        for event in ctx.parameter_events {
            self.control
                .value
                .store(event.value.to_bits(), Ordering::Relaxed);
        }
        let value = f64::from_bits(self.control.value.load(Ordering::Relaxed)) as f32;
        for output in ctx.outputs.iter_mut() {
            output[..ctx.frames].fill(value);
        }
    }
    fn reset(&mut self) {}
    fn tail_frames(&self) -> u64 {
        0
    }
    fn latency_frames(&self) -> u64 {
        0
    }
}
