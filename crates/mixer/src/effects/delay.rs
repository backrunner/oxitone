//! `oxitone.delay` — stereo feedback delay with a one-pole lowpass in the
//! feedback path. Beat-sync convention (02-domain-spec.md §Mixer: 时间类效果
//! 参数可声明 unit 'beats'): the descriptor declares `timeBeats` (unit
//! beats) as the authoring-facing parameter — the host/compiler converts it
//! through the tempo map into `timeSeconds` parameter events (frames =
//! seconds × sampleRate is plugin-internal). Direct `timeBeats` events are
//! ignored by the instance; the DSP only honors `timeSeconds`. Delay time
//! changes glide through a smoother (tape-style). Output is 100% wet; the
//! host `mix` parameter blends dry. Reports a tail.

use std::sync::OnceLock;

use oxitone_core::wire::{ParameterMapping, ParameterSmoothing, ParameterUnit};
use oxitone_dsp::ftz::flush_denormal;
use oxitone_dsp::gain_pan::OnePoleSmoother;
use oxitone_graph::{
    HostContext, Plugin, PluginCapabilities, PluginDescriptor, PluginInstance, ProcessContext,
};

use super::{descriptor, param};

const MAX_DELAY_SECONDS: f64 = 10.0;

pub struct DelayPlugin;

static DESCRIPTOR: OnceLock<PluginDescriptor> = OnceLock::new();

impl Plugin for DelayPlugin {
    fn descriptor(&self) -> &'static PluginDescriptor {
        DESCRIPTOR.get_or_init(|| {
            descriptor(
                "oxitone.delay",
                vec![
                    param(
                        "timeBeats",
                        "Time (beats, compiler-converted)",
                        ParameterUnit::Beats,
                        0.03125,
                        8.0,
                        0.5,
                        ParameterSmoothing::Linear,
                        ParameterMapping::Log,
                    ),
                    param(
                        "timeSeconds",
                        "Time (seconds, DSP-facing)",
                        ParameterUnit::Seconds,
                        0.001,
                        MAX_DELAY_SECONDS,
                        0.25,
                        ParameterSmoothing::OnePole,
                        ParameterMapping::Log,
                    ),
                    param(
                        "feedback",
                        "Feedback",
                        ParameterUnit::Normalized,
                        0.0,
                        0.95,
                        0.35,
                        ParameterSmoothing::Linear,
                        ParameterMapping::Linear,
                    ),
                    param(
                        "feedbackFilterHz",
                        "Feedback Filter",
                        ParameterUnit::Hz,
                        100.0,
                        18_000.0,
                        8000.0,
                        ParameterSmoothing::Linear,
                        ParameterMapping::Log,
                    ),
                ],
                PluginCapabilities {
                    sidechain_input: false,
                    reports_tail: true,
                },
            )
        })
    }

    fn create(&self, host: &HostContext) -> Box<dyn PluginInstance> {
        Box::new(DelayInstance::new(host.sample_rate, host.max_block_size))
    }
}

struct DelayChannel {
    line: Vec<f32>,
    pos: usize,
    filter_state: f32,
}

impl DelayChannel {
    fn new(len: usize) -> Self {
        Self {
            line: vec![0.0; len],
            pos: 0,
            filter_state: 0.0,
        }
    }
}

struct DelayInstance {
    sample_rate: f64,
    time_seconds: f64,
    feedback: f64,
    filter_hz: f64,
    time_smooth: OnePoleSmoother,
    channels: [DelayChannel; 2],
    tail_remaining: u64,
}

impl DelayInstance {
    fn new(sample_rate: f64, max_block_size: u32) -> Self {
        let len = (MAX_DELAY_SECONDS * sample_rate) as usize + max_block_size as usize + 2;
        let mut time_smooth = OnePoleSmoother::new(sample_rate, 250.0);
        time_smooth.snap((0.25 * sample_rate) as f32);
        Self {
            sample_rate,
            time_seconds: 0.25,
            feedback: 0.35,
            filter_hz: 8000.0,
            time_smooth,
            channels: [DelayChannel::new(len), DelayChannel::new(len)],
            tail_remaining: 0,
        }
    }

    fn set_parameter(&mut self, id: &str, value: f64) {
        match id {
            // Host/compiler converts beats → seconds via the tempo map.
            "timeBeats" => {}
            "timeSeconds" => {
                self.time_seconds = value.clamp(0.001, MAX_DELAY_SECONDS);
                self.time_smooth
                    .set_target((self.time_seconds * self.sample_rate) as f32);
            }
            "feedback" => self.feedback = value.clamp(0.0, 0.95),
            "feedbackFilterHz" => self.filter_hz = value.clamp(100.0, 18_000.0),
            _ => {}
        }
    }

    fn max_tail_frames(&self) -> u64 {
        let delay = self.time_seconds * self.sample_rate;
        if self.feedback < 0.01 {
            return delay as u64;
        }
        // Repeats until -100 dB: feedback^n < 1e-5.
        let repeats = (-5.0 * 10f64.ln() / self.feedback.ln()).ceil();
        (delay * repeats) as u64
    }
}

impl PluginInstance for DelayInstance {
    fn prepare(&mut self, sample_rate: f64, max_block_size: u32) {
        *self = DelayInstance::new(sample_rate, max_block_size);
    }

    fn process(&mut self, ctx: &mut ProcessContext<'_>) {
        for event in ctx.parameter_events {
            self.set_parameter(event.parameter_id, event.value);
        }
        let frames = ctx.frames;
        let feedback = self.feedback as f32;
        let filter_coeff =
            (1.0 - (-2.0 * core::f64::consts::PI * self.filter_hz / self.sample_rate).exp()) as f32;
        let len = self.channels[0].line.len();
        let mut block_peak = 0.0f32;
        for (ch, channel) in self.channels.iter_mut().enumerate() {
            let input = &ctx.inputs[ch][..frames];
            let output = &mut ctx.outputs[ch][..frames];
            for n in 0..frames {
                block_peak = block_peak.max(input[n].abs());
                let delay = self.time_smooth.next_sample().clamp(1.0, (len - 2) as f32);
                // Linear-interpolated read `delay` frames behind the cursor.
                let read = channel.pos as f32 - delay + len as f32;
                let i0 = read as usize % len;
                let i1 = (i0 + 1) % len;
                let frac = read.fract();
                let wet = channel.line[i0] * (1.0 - frac) + channel.line[i1] * frac;
                channel.filter_state = flush_denormal(
                    channel.filter_state + (wet - channel.filter_state) * filter_coeff,
                );
                channel.line[channel.pos] =
                    flush_denormal(input[n] + channel.filter_state * feedback);
                channel.pos = (channel.pos + 1) % len;
                output[n] = wet;
            }
        }
        if block_peak > 1e-6 {
            self.tail_remaining = self.max_tail_frames();
        } else {
            self.tail_remaining = self.tail_remaining.saturating_sub(frames as u64);
        }
    }

    fn reset(&mut self) {
        for channel in &mut self.channels {
            channel.line.fill(0.0);
            channel.pos = 0;
            channel.filter_state = 0.0;
        }
        self.time_smooth
            .snap((self.time_seconds * self.sample_rate) as f32);
        self.tail_remaining = 0;
    }

    fn tail_frames(&self) -> u64 {
        self.tail_remaining
    }

    fn latency_frames(&self) -> u64 {
        0
    }
}
