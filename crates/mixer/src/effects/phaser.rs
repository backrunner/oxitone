//! `oxitone.phaser` — LFO-modulated first-order allpass cascade (4 or 8
//! stages) with a feedback path, running at 2x oversampling
//! (03-audio-runtime-spec.md §数值精度: 含反馈的 Phaser 必须 oversample).
//! Output is a fixed 50/50 dry+wet blend (the notches come from the blend);
//! the host `mix` parameter scales the whole effect. LFO phase is `f64`.

use std::sync::OnceLock;

use oxitone_core::wire::{ParameterMapping, ParameterSmoothing, ParameterUnit};
use oxitone_dsp::ftz::flush_denormal;
use oxitone_graph::{
    HostContext, Plugin, PluginCapabilities, PluginDescriptor, PluginInstance, ProcessContext,
};

use super::oversample::Oversampler2x;
use super::{descriptor, enum_param, param};

const MAX_STAGES: usize = 8;

pub struct PhaserPlugin;

static DESCRIPTOR: OnceLock<PluginDescriptor> = OnceLock::new();

impl Plugin for PhaserPlugin {
    fn descriptor(&self) -> &'static PluginDescriptor {
        DESCRIPTOR.get_or_init(|| {
            descriptor(
                "oxitone.phaser",
                vec![
                    param(
                        "rateHz",
                        "Rate",
                        ParameterUnit::Hz,
                        0.05,
                        10.0,
                        0.5,
                        ParameterSmoothing::Linear,
                        ParameterMapping::Log,
                    ),
                    param(
                        "depth",
                        "Depth",
                        ParameterUnit::Normalized,
                        0.0,
                        1.0,
                        0.7,
                        ParameterSmoothing::Linear,
                        ParameterMapping::Linear,
                    ),
                    param(
                        "centerHz",
                        "Center Frequency",
                        ParameterUnit::Hz,
                        100.0,
                        8000.0,
                        800.0,
                        ParameterSmoothing::Linear,
                        ParameterMapping::Log,
                    ),
                    param(
                        "feedback",
                        "Feedback",
                        ParameterUnit::Normalized,
                        0.0,
                        0.9,
                        0.3,
                        ParameterSmoothing::Linear,
                        ParameterMapping::Linear,
                    ),
                    enum_param("stages", "Stages (0=4 1=8)", 0.0, 1.0, 0.0),
                ],
                PluginCapabilities::default(),
            )
        })
    }

    fn create(&self, host: &HostContext) -> Box<dyn PluginInstance> {
        Box::new(PhaserInstance::new(host.sample_rate, host.max_block_size))
    }
}

/// First-order allpass: y = a*x + z; z = x - a*y.
#[derive(Clone, Copy)]
struct Allpass {
    z: f32,
}

impl Allpass {
    #[inline]
    fn next(&mut self, x: f32, a: f32) -> f32 {
        let y = a * x + self.z;
        self.z = flush_denormal(x - a * y);
        y
    }
}

struct ChannelState {
    stages: [Allpass; MAX_STAGES],
    feedback: f32,
    os: Oversampler2x,
}

impl ChannelState {
    fn new(max_block: usize) -> Self {
        Self {
            stages: [Allpass { z: 0.0 }; MAX_STAGES],
            feedback: 0.0,
            os: Oversampler2x::new(max_block),
        }
    }
}

struct PhaserInstance {
    sample_rate: f64,
    rate_hz: f64,
    depth: f64,
    center_hz: f64,
    feedback: f64,
    stage_count: usize,
    lfo_phase: f64,
    left: ChannelState,
    right: ChannelState,
}

impl PhaserInstance {
    fn new(sample_rate: f64, max_block_size: u32) -> Self {
        Self {
            sample_rate,
            rate_hz: 0.5,
            depth: 0.7,
            center_hz: 800.0,
            feedback: 0.3,
            stage_count: 4,
            lfo_phase: 0.0,
            left: ChannelState::new(max_block_size as usize),
            right: ChannelState::new(max_block_size as usize),
        }
    }

    fn set_parameter(&mut self, id: &str, value: f64) {
        match id {
            "rateHz" => self.rate_hz = value.clamp(0.05, 10.0),
            "depth" => self.depth = value.clamp(0.0, 1.0),
            "centerHz" => self.center_hz = value.clamp(100.0, 8000.0),
            "feedback" => self.feedback = value.clamp(0.0, 0.9),
            "stages" => self.stage_count = if value.round() >= 1.0 { 8 } else { 4 },
            _ => {}
        }
    }

    /// Left advances the shared LFO; the right channel reads the same start
    /// phase offset by a quarter cycle for stereo width.
    #[allow(clippy::too_many_arguments)]
    fn process_channel(
        channel: &mut ChannelState,
        buf: &mut [f32],
        sample_rate: f64,
        rate_hz: f64,
        depth: f64,
        center_hz: f64,
        feedback: f64,
        stage_count: usize,
        lfo_start_phase: f64,
    ) {
        let frames = buf.len();
        let fs2 = 2.0 * sample_rate;
        let phase_inc = rate_hz / fs2;
        let band = channel.os.upsample(buf);
        let mut phase = lfo_start_phase % 1.0;
        for x in band.iter_mut() {
            let lfo = (phase * core::f64::consts::TAU).sin();
            phase = (phase + phase_inc) % 1.0;
            // ±2 octaves around the center frequency.
            let freq = center_hz * 2f64.powf(depth * 2.0 * lfo);
            let t = (core::f64::consts::PI * freq / fs2).tan();
            let a = ((t - 1.0) / (t + 1.0)) as f32;
            let dry = *x;
            let mut y = dry + channel.feedback * feedback as f32;
            for stage in channel.stages.iter_mut().take(stage_count) {
                y = stage.next(y, a);
            }
            channel.feedback = flush_denormal(y);
            *x = 0.5 * (dry + y);
        }
        let down = channel.os.downsample(frames);
        buf.copy_from_slice(down);
    }
}

impl PluginInstance for PhaserInstance {
    fn prepare(&mut self, sample_rate: f64, max_block_size: u32) {
        *self = PhaserInstance::new(sample_rate, max_block_size);
    }

    fn process(&mut self, ctx: &mut ProcessContext<'_>) {
        for event in ctx.parameter_events {
            self.set_parameter(event.parameter_id, event.value);
        }
        super::pass_inputs(ctx);
        let frames = ctx.frames;
        let (sample_rate, rate_hz, depth, center_hz, feedback, stage_count, lfo_phase) = (
            self.sample_rate,
            self.rate_hz,
            self.depth,
            self.center_hz,
            self.feedback,
            self.stage_count,
            self.lfo_phase,
        );
        let (left, right) = ctx.outputs.split_at_mut(1);
        Self::process_channel(
            &mut self.left,
            &mut left[0][..frames],
            sample_rate,
            rate_hz,
            depth,
            center_hz,
            feedback,
            stage_count,
            lfo_phase,
        );
        Self::process_channel(
            &mut self.right,
            &mut right[0][..frames],
            sample_rate,
            rate_hz,
            depth,
            center_hz,
            feedback,
            stage_count,
            lfo_phase + 0.25,
        );
        self.lfo_phase = (lfo_phase + rate_hz / (2.0 * sample_rate) * 2.0 * frames as f64) % 1.0;
    }

    fn reset(&mut self) {
        for channel in [&mut self.left, &mut self.right] {
            for stage in &mut channel.stages {
                stage.z = 0.0;
            }
            channel.feedback = 0.0;
            channel.os.reset();
        }
        self.lfo_phase = 0.0;
    }

    fn tail_frames(&self) -> u64 {
        0
    }

    fn latency_frames(&self) -> u64 {
        self.left.os.latency_frames()
    }
}
