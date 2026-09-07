//! Resource-backed stereo convolution. All IR decoding/resampling occurs before construction.
use super::{
    controls::{Control as C, Controls},
    fractional::FractionalDelay,
};
use oxitone_core::{error::codes, OxitoneError};
use oxitone_dsp::convolution::PartitionedConvolver;
use oxitone_graph::{
    HostContext, Plugin, PluginCapabilities, PluginDescriptor, PluginInstance, ProcessContext,
};
use std::sync::{Arc, OnceLock};
pub const MAX_IMPULSE_FRAMES: usize = 262144;
pub struct Impulse {
    pub channels: [Vec<f32>; 2],
    pub sample_rate: f64,
}
pub trait ImpulseProvider {
    fn impulse(&self, sample_id: &str) -> Result<Impulse, OxitoneError>;
}
const TABLE: [C; 4] = [
    C::linear("predelayMs", "Predelay", 0., 200., 0.),
    C::hz("highpassHz", "Highpass", 20., 2000., 20.),
    C::hz("lowpassHz", "Lowpass", 1000., 20000., 20000.),
    C::db("outputDb", "Output", -36., 12., 0.),
];
pub struct ConvolverPlugin;
impl Plugin for ConvolverPlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        static D: OnceLock<PluginDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            super::descriptor(
                "oxitone.convolver",
                TABLE.map(C::spec).to_vec(),
                PluginCapabilities {
                    reports_tail: true,
                    sidechain_input: false,
                },
            )
        })
    }
    fn create(&self, h: &HostContext) -> Box<dyn PluginInstance> {
        Box::new(Instance::new(h.sample_rate, None))
    }
}
pub fn from_impulse(impulse: Impulse, rate: f64) -> Result<Box<dyn PluginInstance>, OxitoneError> {
    let count = impulse.channels[0].len();
    if count == 0
        || count > MAX_IMPULSE_FRAMES
        || impulse.channels[1].len() != count
        || impulse.sample_rate != rate
        || impulse
            .channels
            .iter()
            .flatten()
            .any(|v| !v.is_finite() || v.abs() > 16.)
    {
        return Err(OxitoneError::with_path(
            codes::INVALID_PROJECT,
            "impulse must be finite stereo PCM at engine rate, 1..262144 frames, peak <=16",
            "effect.resources.impulse",
        ));
    }
    Ok(Box::new(Instance::new(rate, Some(Arc::new(impulse)))))
}
fn room(rate: f64) -> Impulse {
    let length = (rate * 0.35) as usize;
    let channels = std::array::from_fn(|ch| {
        let mut seed = 0x739abc2fu32.wrapping_add(ch as u32 * 89131);
        let mut low = 0.;
        (0..length)
            .map(|i| {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let noise = seed as f64 / u32::MAX as f64 * 2. - 1.;
                low += 0.35 * (noise - low);
                let time = i as f64 / rate;
                let diffuse = low
                    * (-24. * time).exp()
                    * (time / 0.02).min(1.)
                    * 0.018
                    * (48000. / rate).sqrt();
                let early = if [0.007, 0.013, 0.023, 0.037]
                    .iter()
                    .any(|t| i == ((t + ch as f64 * 0.0007) * rate) as usize)
                {
                    0.2 * (-24. * time).exp()
                } else {
                    0.
                };
                (diffuse + early) as f32
            })
            .collect()
    });
    Impulse {
        channels,
        sample_rate: rate,
    }
}
struct Instance {
    rate: f64,
    resource: Option<Arc<Impulse>>,
    controls: Controls<4>,
    engines: [PartitionedConvolver; 2],
    predelay: [FractionalDelay; 2],
    low: [[f64; 2]; 2],
    impulse_frames: usize,
    tail: u64,
}
impl Instance {
    fn new(rate: f64, resource: Option<Arc<Impulse>>) -> Self {
        let impulse = resource.clone().unwrap_or_else(|| Arc::new(room(rate)));
        Self {
            rate,
            resource,
            controls: Controls::new(&TABLE, rate),
            engines: std::array::from_fn(|ch| PartitionedConvolver::new(&impulse.channels[ch])),
            predelay: std::array::from_fn(|_| FractionalDelay::new((rate * 0.201) as usize + 4)),
            low: [[0.; 2]; 2],
            impulse_frames: impulse.channels[0].len(),
            tail: 0,
        }
    }
}
impl PluginInstance for Instance {
    fn prepare(&mut self, rate: f64, _: u32) {
        *self = Self::new(rate, self.resource.clone());
    }
    fn try_prepare(&mut self, rate: f64, block: u32) -> Result<(), OxitoneError> {
        if self
            .resource
            .as_ref()
            .is_some_and(|r| r.sample_rate != rate)
        {
            return Err(OxitoneError::new(
                codes::INVALID_PROJECT,
                "impulse must be resampled before prepare",
            ));
        }
        self.prepare(rate, block);
        Ok(())
    }
    fn process(&mut self, ctx: &mut ProcessContext<'_>) {
        self.controls.events(ctx.parameter_events);
        for n in 0..ctx.frames {
            let v = self.controls.next();
            let delay = v[0] * 0.001 * self.rate;
            let hp = 1. - (-std::f64::consts::TAU * v[1] / self.rate).exp();
            let lp = 1. - (-std::f64::consts::TAU * v[2].min(self.rate * 0.45) / self.rate).exp();
            let mut active = false;
            for ch in 0..2 {
                let x = ctx.inputs[ch][n];
                active |= x.abs() > 1e-9;
                let wet = self.engines[ch].next(x);
                let delayed = if delay < 1. {
                    wet
                } else {
                    self.predelay[ch].read(delay)
                };
                self.predelay[ch].push(wet);
                self.low[ch][0] += hp * (delayed as f64 - self.low[ch][0]);
                let high = if v[1] <= 20.0001 {
                    delayed as f64
                } else {
                    delayed as f64 - self.low[ch][0]
                };
                self.low[ch][1] += lp * (high - self.low[ch][1]);
                let y = if v[2] >= 19999.99 {
                    high
                } else {
                    self.low[ch][1]
                };
                ctx.outputs[ch][n] = (y * 10f64.powf(v[3] / 20.)) as f32;
                for s in &mut self.low[ch] {
                    if s.abs() < 1e-20 {
                        *s = 0.;
                    }
                }
            }
            self.tail = if active {
                self.impulse_frames as u64 + 256 + (self.rate * 0.4) as u64
            } else {
                self.tail.saturating_sub(1)
            };
        }
    }
    fn reset(&mut self) {
        self.controls.reset();
        for e in &mut self.engines {
            e.reset();
        }
        for p in &mut self.predelay {
            p.reset();
        }
        self.low = [[0.; 2]; 2];
        self.tail = 0;
    }
    fn tail_frames(&self) -> u64 {
        self.tail
    }
    fn latency_frames(&self) -> u64 {
        256
    }
}
