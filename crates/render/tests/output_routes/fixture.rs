use oxitone_core::OxitoneError;
use oxitone_graph::{HostContext, Plugin, PluginDescriptor, PluginInstance, ProcessContext};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[derive(Clone, Copy)]
pub enum Kind {
    Instrument,
    Delay,
}
pub struct Factory {
    pub descriptor: PluginDescriptor,
    pub kind: Kind,
    pub calls: Arc<AtomicUsize>,
    pub activation: Arc<AtomicUsize>,
}
impl Plugin for Factory {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }
    fn output_bus_count(&self) -> usize {
        match self.kind {
            Kind::Instrument => 3,
            Kind::Delay => 1,
        }
    }
    fn create(&self, host: &HostContext) -> Box<dyn PluginInstance> {
        Box::new(Instance {
            kind: self.kind,
            calls: self.calls.clone(),
            activation: self.activation.clone(),
            active: [true, false, false],
            outputs: std::array::from_fn(|_| {
                [
                    vec![0.; host.max_block_size as usize],
                    vec![0.; host.max_block_size as usize],
                ]
            }),
            frame: 0,
            delayed: [[0.; 5]; 2],
            cursor: 0,
        })
    }
}
struct Instance {
    kind: Kind,
    calls: Arc<AtomicUsize>,
    activation: Arc<AtomicUsize>,
    active: [bool; 3],
    outputs: [[Vec<f32>; 2]; 3],
    frame: usize,
    delayed: [[f32; 5]; 2],
    cursor: usize,
}
impl PluginInstance for Instance {
    fn requires_isolation(&self) -> bool {
        true
    }
    fn configure_output_buses(&mut self, buses: &[usize]) -> Result<(), OxitoneError> {
        if matches!(self.kind, Kind::Delay) {
            assert!(buses.is_empty());
            return Ok(());
        }
        self.active = [true, buses.contains(&1), buses.contains(&2)];
        self.activation.store(
            1 | usize::from(self.active[1]) << 1 | usize::from(self.active[2]) << 2,
            Ordering::Relaxed,
        );
        Ok(())
    }
    fn prepare(&mut self, _: f64, _: u32) {}
    fn process(&mut self, _: &mut ProcessContext<'_>) {
        panic!("isolated instrument entered callback");
    }
    fn process_isolated(
        &mut self,
        ctx: &mut ProcessContext<'_>,
        _: &oxitone_graph::execution::ProcessPosition,
    ) -> Result<(), OxitoneError> {
        match self.kind {
            Kind::Instrument => {
                self.calls.fetch_add(1, Ordering::Relaxed);
                for (index, bus) in self.outputs.iter_mut().enumerate() {
                    for channel in bus {
                        for (i, value) in channel[..ctx.frames].iter_mut().enumerate() {
                            *value = if self.active[index] && self.frame + i == 7 {
                                (index + 1) as f32 * 0.1
                            } else {
                                0.
                            };
                        }
                    }
                }
                for ch in 0..2 {
                    ctx.outputs[ch][..ctx.frames]
                        .copy_from_slice(&self.outputs[0][ch][..ctx.frames]);
                }
                self.frame += ctx.frames;
            }
            Kind::Delay => {
                for i in 0..ctx.frames {
                    for ch in 0..2 {
                        ctx.outputs[ch][i] = self.delayed[ch][self.cursor] * 0.5;
                        self.delayed[ch][self.cursor] = ctx.inputs[ch][i];
                    }
                    self.cursor = (self.cursor + 1) % 5;
                }
            }
        }
        Ok(())
    }
    fn output_bus(&self, index: usize) -> Option<[&[f32]; 2]> {
        self.outputs
            .get(index)
            .map(|[l, r]| [l.as_slice(), r.as_slice()])
    }
    fn reset(&mut self) {
        self.frame = 0;
        self.delayed = [[0.; 5]; 2];
        self.cursor = 0;
    }
    fn latency_frames(&self) -> u64 {
        match self.kind {
            Kind::Instrument => 7,
            Kind::Delay => 5,
        }
    }
    fn tail_frames(&self) -> u64 {
        0
    }
}
