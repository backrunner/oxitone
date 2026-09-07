//! `oxitone.compressor` — stereo-linked RMS/peak compressor with soft knee
//! and makeup gain. Declares `sidechain_input`: the detector reads the
//! sidechain buses when the host provides them, otherwise the main input.
//! Zero latency; gain reduction is attack/release smoothed per sample.

use std::sync::OnceLock;

use oxitone_core::wire::{ParameterMapping, ParameterSmoothing, ParameterUnit};
use oxitone_dsp::gain_pan::OnePoleSmoother;
use oxitone_graph::{
    HostContext, Plugin, PluginCapabilities, PluginDescriptor, PluginInstance, ProcessContext,
};

use super::{db_to_linear, descriptor, enum_param, param};

pub struct CompressorPlugin;

static DESCRIPTOR: OnceLock<PluginDescriptor> = OnceLock::new();

impl Plugin for CompressorPlugin {
    fn descriptor(&self) -> &'static PluginDescriptor {
        DESCRIPTOR.get_or_init(|| {
            descriptor(
                "oxitone.compressor",
                vec![
                    param(
                        "thresholdDb",
                        "Threshold",
                        ParameterUnit::Db,
                        -60.0,
                        0.0,
                        -18.0,
                        ParameterSmoothing::Linear,
                        ParameterMapping::Linear,
                    ),
                    param(
                        "ratio",
                        "Ratio",
                        ParameterUnit::Normalized,
                        1.0,
                        40.0,
                        4.0,
                        ParameterSmoothing::Linear,
                        ParameterMapping::Log,
                    ),
                    param(
                        "attackMs",
                        "Attack",
                        ParameterUnit::Normalized,
                        0.1,
                        200.0,
                        10.0,
                        ParameterSmoothing::Linear,
                        ParameterMapping::Log,
                    ),
                    param(
                        "releaseMs",
                        "Release",
                        ParameterUnit::Normalized,
                        10.0,
                        2000.0,
                        150.0,
                        ParameterSmoothing::Linear,
                        ParameterMapping::Log,
                    ),
                    param(
                        "kneeDb",
                        "Knee",
                        ParameterUnit::Db,
                        0.0,
                        24.0,
                        6.0,
                        ParameterSmoothing::Linear,
                        ParameterMapping::Linear,
                    ),
                    param(
                        "makeupDb",
                        "Makeup",
                        ParameterUnit::Db,
                        0.0,
                        24.0,
                        0.0,
                        ParameterSmoothing::OnePole,
                        ParameterMapping::Linear,
                    ),
                    enum_param("detector", "Detector (0=peak 1=rms)", 0.0, 1.0, 0.0),
                    super::controls::Control::hz(
                        "sidechainHighpassHz",
                        "Detector Highpass",
                        20.,
                        2000.,
                        20.,
                    )
                    .spec(),
                ],
                PluginCapabilities {
                    sidechain_input: true,
                    reports_tail: false,
                },
            )
        })
    }

    fn create(&self, host: &HostContext) -> Box<dyn PluginInstance> {
        Box::new(CompressorInstance::new(host.sample_rate))
    }
}

struct CompressorInstance {
    sample_rate: f64,
    threshold_db: f64,
    ratio: f64,
    attack_ms: f64,
    release_ms: f64,
    knee_db: f64,
    makeup_db: f64,
    rms_detector: bool,
    makeup_smooth: OnePoleSmoother,
    envelope: f32,
    mean_square: f32,
    gain_db: f32,
    detector_hz: OnePoleSmoother,
    detector_target: f32,
    detector_low: [f64; 2],
}

impl CompressorInstance {
    fn new(sample_rate: f64) -> Self {
        let mut makeup_smooth = OnePoleSmoother::new(sample_rate, 20.0);
        makeup_smooth.snap(1.0);
        let mut detector_hz = OnePoleSmoother::new(sample_rate, 5.);
        detector_hz.snap(20.);
        Self {
            sample_rate,
            threshold_db: -18.0,
            ratio: 4.0,
            attack_ms: 10.0,
            release_ms: 150.0,
            knee_db: 6.0,
            makeup_db: 0.0,
            rms_detector: false,
            makeup_smooth,
            envelope: 0.0,
            mean_square: 0.0,
            gain_db: 0.0,
            detector_hz,
            detector_target: 20.,
            detector_low: [0.; 2],
        }
    }

    fn set_parameter(&mut self, id: &str, value: f64) {
        match id {
            "thresholdDb" => self.threshold_db = value.clamp(-60.0, 0.0),
            "ratio" => self.ratio = value.clamp(1.0, 40.0),
            "attackMs" => self.attack_ms = value.clamp(0.1, 200.0),
            "releaseMs" => self.release_ms = value.clamp(10.0, 2000.0),
            "kneeDb" => self.knee_db = value.clamp(0.0, 24.0),
            "makeupDb" => {
                self.makeup_db = value.clamp(0.0, 24.0);
                self.makeup_smooth.set_target(db_to_linear(self.makeup_db));
            }
            "detector" => self.rms_detector = value.round() >= 1.0,
            "sidechainHighpassHz" => {
                self.detector_target = value.clamp(20., 2000.) as f32;
                self.detector_hz.set_target(self.detector_target);
            }
            _ => {}
        }
    }

    /// Static gain curve in dB (soft knee), per sample. Pure.
    fn gain_db_for(&self, level_db: f64) -> f64 {
        let over = level_db - self.threshold_db;
        let knee = self.knee_db;
        if knee > 0.0 && over.abs() <= knee / 2.0 {
            (1.0 / self.ratio - 1.0) * (over + knee / 2.0).powi(2) / (2.0 * knee)
        } else if over > 0.0 {
            over * (1.0 / self.ratio - 1.0)
        } else {
            0.0
        }
    }
}

impl PluginInstance for CompressorInstance {
    fn prepare(&mut self, sample_rate: f64, _max_block_size: u32) {
        *self = CompressorInstance::new(sample_rate);
    }

    fn process(&mut self, ctx: &mut ProcessContext<'_>) {
        for event in ctx.parameter_events {
            self.set_parameter(event.parameter_id, event.value);
        }
        super::pass_inputs(ctx);
        let frames = ctx.frames;
        // Detector source: sidechain buses when provided, else main input.
        let (det_l, det_r): (&[f32], &[f32]) = match ctx.sidechain {
            Some(sc) if sc.len() >= 2 => (&sc[0][..frames], &sc[1][..frames]),
            _ => (&ctx.inputs[0][..frames], &ctx.inputs[1][..frames]),
        };
        let attack_coeff = (-1.0 / (self.attack_ms / 1000.0 * self.sample_rate)).exp() as f32;
        let release_coeff = (-1.0 / (self.release_ms / 1000.0 * self.sample_rate)).exp() as f32;
        let rms_coeff = (-1.0 / (0.050 * self.sample_rate)).exp() as f32;
        let (left, right) = ctx.outputs.split_at_mut(1);
        let left = &mut left[0][..frames];
        let right = &mut right[0][..frames];
        for n in 0..frames {
            let hz = self.detector_hz.next_sample() as f64;
            let coefficient = 1. - (-std::f64::consts::TAU * hz / self.sample_rate).exp();
            let mut linked = 0f32;
            for (ch, input) in [det_l[n], det_r[n]].into_iter().enumerate() {
                self.detector_low[ch] += coefficient * (input as f64 - self.detector_low[ch]);
                let value = if hz <= 20.0001 {
                    input
                } else {
                    (input as f64 - self.detector_low[ch]) as f32
                };
                linked = linked.max(value.abs());
            }
            let level_db = if self.rms_detector {
                let sq = linked * linked;
                self.mean_square += (sq - self.mean_square) * (1.0 - rms_coeff);
                10.0 * (self.mean_square.max(1e-12) as f64).log10()
            } else {
                self.envelope = linked.max(self.envelope * release_coeff);
                20.0 * (self.envelope.max(1e-12) as f64).log10()
            };
            let target_db = self.gain_db_for(level_db) as f32;
            let coeff = if target_db < self.gain_db {
                attack_coeff
            } else {
                release_coeff
            };
            self.gain_db += (target_db - self.gain_db) * (1.0 - coeff);
            if (target_db - self.gain_db).abs() < 1e-4 {
                self.gain_db = target_db;
            }
            let gain = db_to_linear(self.gain_db as f64) * self.makeup_smooth.next_sample();
            left[n] *= gain;
            right[n] *= gain;
        }
    }

    fn reset(&mut self) {
        self.envelope = 0.0;
        self.mean_square = 0.0;
        self.gain_db = 0.0;
        self.makeup_smooth.snap(db_to_linear(self.makeup_db));
        self.detector_low = [0.; 2];
        self.detector_hz.snap(self.detector_target);
    }

    fn tail_frames(&self) -> u64 {
        0
    }

    fn latency_frames(&self) -> u64 {
        0
    }
}
