use oxitone_core::{
    wire::{EffectRef, InsertRouting, MixerChannelSpec},
    OxitoneError,
};
use oxitone_graph::{
    HostContext, Plugin, PluginDescriptor, PluginInstance, PluginRegistry, ProcessContext,
};
use oxitone_mixer::{builtin_effect_plugins, ChannelInput, DelayLine, MixerEngine};
use std::sync::Arc;

pub const BLOCK: usize = 32;
struct Factory {
    descriptor: PluginDescriptor,
    latency: usize,
}
impl Plugin for Factory {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }
    fn input_bus_count(&self) -> usize {
        3
    }
    fn output_bus_count(&self) -> usize {
        3
    }
    fn create(&self, _: &HostContext) -> Box<dyn PluginInstance> {
        Box::new(Instance {
            inputs: [[0.; BLOCK]; 2],
            output: [[0.; BLOCK]; 2],
            active: [false; 3],
            delay: DelayLine::new(self.latency, BLOCK),
            latency: self.latency,
        })
    }
}
struct Instance {
    inputs: [[f32; BLOCK]; 2],
    output: [[f32; BLOCK]; 2],
    active: [bool; 3],
    delay: DelayLine,
    latency: usize,
}
impl PluginInstance for Instance {
    fn requires_isolation(&self) -> bool {
        true
    }
    fn configure_input_buses(&mut self, buses: &[usize]) -> Result<(), OxitoneError> {
        for &bus in buses {
            self.active[bus] = true;
        }
        Ok(())
    }
    fn configure_output_buses(&mut self, _: &[usize]) -> Result<(), OxitoneError> {
        Ok(())
    }
    fn input_bus_mut(&mut self, index: usize) -> Option<[&mut [f32]; 2]> {
        assert!(self.active[index]);
        // Only left is used by this mono fixture; host still stages both stereo channels.
        let [left, right] = &mut self.inputs;
        assert_eq!(index, 2);
        Some([left, right])
    }
    fn output_bus(&self, _: usize) -> Option<[&[f32]; 2]> {
        Some([&self.output[0], &self.output[1]])
    }
    fn prepare(&mut self, _: f64, _: u32) {
        self.delay.set_delay(self.latency);
    }
    fn process(&mut self, ctx: &mut ProcessContext<'_>) {
        let mut mixed = [[0.; BLOCK]; 2];
        for (ch, channel) in mixed.iter_mut().enumerate() {
            for (n, sample) in channel.iter_mut().enumerate().take(ctx.frames) {
                *sample = ctx.inputs[ch][n]
                    + ctx.sidechain.map_or(0., |sidechain| sidechain[ch][n])
                    + if self.active[2] {
                        self.inputs[ch][n]
                    } else {
                        0.
                    };
            }
        }
        ctx.outputs[0][..ctx.frames].fill(0.);
        ctx.outputs[1][..ctx.frames].fill(0.);
        let (left, right) = ctx.outputs.split_at_mut(1);
        self.delay.process_add(
            &mixed[0][..ctx.frames],
            &mixed[1][..ctx.frames],
            1.,
            &mut left[0][..ctx.frames],
            &mut right[0][..ctx.frames],
        );
        for (ch, output) in self.output.iter_mut().enumerate() {
            output[..ctx.frames].copy_from_slice(&ctx.outputs[ch][..ctx.frames]);
        }
    }
    fn reset(&mut self) {
        self.delay.reset();
        self.inputs = [[0.; BLOCK]; 2];
    }
    fn latency_frames(&self) -> u64 {
        self.latency as u64
    }
    fn tail_frames(&self) -> u64 {
        0
    }
}
pub fn registry() -> PluginRegistry {
    let mut registry = PluginRegistry::new();
    for (id, latency) in [("fixture.multi", 0), ("fixture.delay", 7)] {
        let mut descriptor = builtin_effect_plugins()[0].descriptor().clone();
        descriptor.plugin_id = id.into();
        descriptor.parameters.clear();
        descriptor.capabilities.sidechain_input = id == "fixture.multi";
        descriptor.state_schema = None;
        registry
            .register(Arc::new(Factory {
                descriptor,
                latency,
            }))
            .unwrap();
    }
    registry
}
pub fn effect(id: &str, instance: &str) -> EffectRef {
    EffectRef {
        plugin_id: id.into(),
        plugin_version: "1.0.0".into(),
        instance_id: Some(instance.into()),
        parameters: Default::default(),
        resources: None,
        state: None,
        bypass: None,
        mix: None,
    }
}
pub fn bus(id: &str, inserts: Vec<EffectRef>) -> MixerChannelSpec {
    MixerChannelSpec {
        id: id.into(),
        name: None,
        level: 1.,
        balance: 0.,
        master_send_ratio: Some(0.),
        inserts,
        insert_routes: None,
        sends: vec![],
        mute: None,
        solo: None,
    }
}
pub fn routed() -> Vec<MixerChannelSpec> {
    let source = bus("source", vec![effect("fixture.delay", "source_delay")]);
    let mut owner = bus(
        "owner",
        vec![
            effect("fixture.delay", "prefix"),
            effect("fixture.multi", "routed"),
            effect("fixture.delay", "suffix"),
        ],
    );
    owner.insert_routes = Some(
        [(
            "routed".into(),
            InsertRouting {
                inputs: Some([("2".into(), "source".into())].into()),
                outputs: Some([("1".into(), "mix_master".into())].into()),
            },
        )]
        .into(),
    );
    owner.master_send_ratio = Some(1.);
    vec![owner, source]
}
pub fn render(engine: &mut MixerEngine, id: &str) -> Vec<f32> {
    let mut result = vec![];
    for block in 0..3 {
        let mut input = [0.; BLOCK];
        if block == 0 {
            input[0] = 1.;
        }
        let mut left = [0.; BLOCK];
        let mut right = [0.; BLOCK];
        engine
            .process_with::<oxitone_graph::execution::Offline>(
                [ChannelInput {
                    bus_id: id,
                    left: &input,
                    right: &input,
                }]
                .into_iter(),
                &mut left,
                &mut right,
                &Default::default(),
            )
            .unwrap();
        result.extend(left);
    }
    result
}
