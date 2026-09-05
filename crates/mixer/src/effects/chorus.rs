//! `oxitone.chorus` — LFO-modulated short delay blended 50/50 with dry (the
//! blend is the effect; host `mix` scales it further). LFO phase is `f64`,
//! the right channel is offset a quarter cycle. Linear-interpolated reads
//! from a preallocated delay line. Zero reported latency.

use std::sync::OnceLock;

use oxitone_core::wire::{ParameterMapping, ParameterSmoothing, ParameterUnit};
use oxitone_graph::{
    HostContext, Plugin, PluginCapabilities, PluginDescriptor, PluginInstance, ProcessContext,
};

use super::{descriptor, param};

const MAX_DELAY_SECONDS: f64 = 0.05;

pub struct ChorusPlugin;

static DESCRIPTOR: OnceLock<PluginDescriptor> = OnceLock::new();

impl Plugin for ChorusPlugin {
    fn descriptor(&self) -> &'static PluginDescriptor {
        DESCRIPTOR.get_or_init(|| {
            descriptor(
                "oxitone.chorus",
                vec![
                    param(
                        "rateHz",
                        "Rate",
                        ParameterUnit::Hz,
                        0.05,
                        8.0,
                        0.8,
                        ParameterSmoothing::Linear,
                        ParameterMapping::Log,
                    ),
                    param(
                        "depth",
                        "Depth",
                        ParameterUnit::Normalized,
                        0.0,
                        1.0,
                        0.5,
                        ParameterSmoothing::Linear,
                        ParameterMapping::Linear,
                    ),
                    param(
                        "delayMs",
                        "Base Delay",
                        ParameterUnit::Normalized,
                        5.0,
                        30.0,
                        15.0,
                        ParameterSmoothing::Linear,
                        ParameterMapping::Linear,
                    ),
                ],
                PluginCapabilities::default(),
            )
        })
    }

    fn create(&self, host: &HostContext) -> Box<dyn PluginInstance> {
        Box::new(ChorusInstance::new(host.sample_rate, host.max_block_size))
    }
}

struct ChorusInstance {
    sample_rate: f64,
    rate_hz: f64,
    depth: f64,
    delay_ms: f64,
    lfo_phase: f64,
    lines: [Vec<f32>; 2],
    pos: usize,
}

impl ChorusInstance {
    fn new(sample_rate: f64, max_block_size: u32) -> Self {
        let len = (MAX_DELAY_SECONDS * sample_rate) as usize + max_block_size as usize + 2;
        Self {
            sample_rate,
            rate_hz: 0.8,
            depth: 0.5,
            delay_ms: 15.0,
            lfo_phase: 0.0,
            lines: [vec![0.0; len], vec![0.0; len]],
            pos: 0,
        }
    }

    fn set_parameter(&mut self, id: &str, value: f64) {
        match id {
            "rateHz" => self.rate_hz = value.clamp(0.05, 8.0),
            "depth" => self.depth = value.clamp(0.0, 1.0),
            "delayMs" => self.delay_ms = value.clamp(5.0, 30.0),
            _ => {}
        }
    }
}

impl PluginInstance for ChorusInstance {
    fn prepare(&mut self, sample_rate: f64, max_block_size: u32) {
        *self = ChorusInstance::new(sample_rate, max_block_size);
    }

    fn process(&mut self, ctx: &mut ProcessContext<'_>) {
        for event in ctx.parameter_events {
            self.set_parameter(event.parameter_id, event.value);
        }
        super::pass_inputs(ctx);
        let frames = ctx.frames;
        let len = self.lines[0].len();
        let base_delay = self.delay_ms / 1000.0 * self.sample_rate;
        let mod_depth = self.depth * 0.008 * self.sample_rate; // ±8 ms sweep
        let phase_inc = self.rate_hz / self.sample_rate;
        let phase = self.lfo_phase;
        let (left, right) = ctx.outputs.split_at_mut(1);
        for (ch, output) in [&mut left[0], &mut right[0]].into_iter().enumerate() {
            let line = &mut self.lines[ch];
            let mut pos = self.pos;
            let mut ch_phase = (phase + 0.25 * ch as f64) % 1.0;
            for x in output.iter_mut().take(frames) {
                let lfo = (ch_phase * core::f64::consts::TAU).sin();
                ch_phase = (ch_phase + phase_inc) % 1.0;
                let delay = (base_delay + mod_depth * (1.0 + lfo)).clamp(1.0, (len - 2) as f64);
                let read = pos as f64 - delay + len as f64;
                let i0 = read as usize % len;
                let i1 = (i0 + 1) % len;
                let frac = read.fract() as f32;
                let wet = line[i0] * (1.0 - frac) + line[i1] * frac;
                line[pos] = *x;
                pos = (pos + 1) % len;
                *x = 0.5 * (*x + wet);
            }
            if ch == 1 {
                self.pos = pos;
            }
        }
        self.lfo_phase = (phase + phase_inc * frames as f64) % 1.0;
    }

    fn reset(&mut self) {
        for line in &mut self.lines {
            line.fill(0.0);
        }
        self.pos = 0;
        self.lfo_phase = 0.0;
    }

    fn tail_frames(&self) -> u64 {
        0
    }

    fn latency_frames(&self) -> u64 {
        0
    }
}
