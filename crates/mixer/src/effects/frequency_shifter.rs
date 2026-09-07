//! Windowed FIR Hilbert quadrature with signed single-sideband modulation.
use super::controls::{Control as C, Controls};
use oxitone_graph::{
    HostContext, Plugin, PluginCapabilities, PluginDescriptor, PluginInstance, ProcessContext,
};
use std::sync::OnceLock;
const TAPS: usize = 257;
const DELAY: usize = 128;
const TABLE: [C; 3] = [
    C::linear("shiftHz", "Shift (Hz)", -5000., 5000., 100.),
    C::linear("stereoHz", "Stereo Offset", -100., 100., 0.),
    C::db("outputDb", "Output", -24., 12., 0.),
];
pub struct FrequencyShifterPlugin;
impl Plugin for FrequencyShifterPlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        static D: OnceLock<PluginDescriptor> = OnceLock::new();
        D.get_or_init(|| {
            super::descriptor(
                "oxitone.frequency-shifter",
                TABLE.map(C::spec).to_vec(),
                PluginCapabilities::default(),
            )
        })
    }
    fn create(&self, h: &HostContext) -> Box<dyn PluginInstance> {
        Box::new(Instance::new(h.sample_rate))
    }
}
struct Instance {
    rate: f64,
    controls: Controls<3>,
    taps: [f64; TAPS],
    history: [[f32; TAPS]; 2],
    position: usize,
    phase: [f64; 2],
}
impl Instance {
    fn new(rate: f64) -> Self {
        let taps = std::array::from_fn(|i| {
            let n = i as i32 - DELAY as i32;
            if n % 2 == 0 {
                return 0.;
            }
            let angle = std::f64::consts::TAU * i as f64 / (TAPS - 1) as f64;
            2. / (std::f64::consts::PI * n as f64)
                * (0.42 - 0.5 * angle.cos() + 0.08 * (2. * angle).cos())
        });
        Self {
            rate,
            controls: Controls::new(&TABLE, rate),
            taps,
            history: [[0.; TAPS]; 2],
            position: 0,
            phase: [0.; 2],
        }
    }
}
impl PluginInstance for Instance {
    fn prepare(&mut self, rate: f64, _: u32) {
        *self = Self::new(rate);
    }
    fn process(&mut self, ctx: &mut ProcessContext<'_>) {
        self.controls.events(ctx.parameter_events);
        for n in 0..ctx.frames {
            let v = self.controls.next();
            for ch in 0..2 {
                self.history[ch][self.position] = ctx.inputs[ch][n];
                let real = self.history[ch][(self.position + TAPS - DELAY) % TAPS] as f64;
                let mut imag = 0.;
                for k in (1..TAPS).step_by(2) {
                    imag +=
                        self.taps[k] * self.history[ch][(self.position + TAPS - k) % TAPS] as f64;
                }
                let (sin, cos) = (self.phase[ch] * std::f64::consts::TAU).sin_cos();
                ctx.outputs[ch][n] = ((real * cos - imag * sin) * 10f64.powf(v[2] / 20.)) as f32;
                self.phase[ch] = (self.phase[ch]
                    + (v[0] + (ch as f64 * 2. - 1.) * v[1] * 0.5) / self.rate)
                    .rem_euclid(1.);
            }
            self.position = (self.position + 1) % TAPS;
        }
    }
    fn reset(&mut self) {
        self.controls.reset();
        self.history = [[0.; TAPS]; 2];
        self.phase = [0.; 2];
        self.position = 0;
    }
    fn tail_frames(&self) -> u64 {
        0
    }
    fn latency_frames(&self) -> u64 {
        DELAY as u64
    }
}
