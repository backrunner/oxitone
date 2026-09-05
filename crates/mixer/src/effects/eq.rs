//! `oxitone.eq` — 4-band EQ: lowshelf / peak / peak / highshelf on
//! `f64`-state biquads (03-audio-runtime-spec.md §数值精度). Band gains are
//! one-pole smoothed; coefficients are redesigned at block rate.

use std::sync::OnceLock;

use oxitone_core::wire::{ParameterMapping, ParameterSmoothing, ParameterUnit};
use oxitone_dsp::biquad::{design, BiquadF64, BiquadKind};
use oxitone_dsp::gain_pan::OnePoleSmoother;
use oxitone_graph::{
    HostContext, Plugin, PluginCapabilities, PluginDescriptor, PluginInstance, ProcessContext,
};

use super::{descriptor, param};

const KINDS: [BiquadKind; 4] = [
    BiquadKind::LowShelf,
    BiquadKind::Peak,
    BiquadKind::Peak,
    BiquadKind::HighShelf,
];
const DEFAULT_FREQS: [f64; 4] = [120.0, 500.0, 2500.0, 8000.0];

pub struct EqPlugin;

static DESCRIPTOR: OnceLock<PluginDescriptor> = OnceLock::new();

impl Plugin for EqPlugin {
    fn descriptor(&self) -> &'static PluginDescriptor {
        DESCRIPTOR.get_or_init(|| {
            let mut parameters = Vec::new();
            for band in 1..=4 {
                let prefix = format!("band{band}");
                parameters.push(param(
                    &format!("{prefix}.freqHz"),
                    &format!("Band {band} Frequency"),
                    ParameterUnit::Hz,
                    20.0,
                    20_000.0,
                    DEFAULT_FREQS[band - 1],
                    ParameterSmoothing::Linear,
                    ParameterMapping::Log,
                ));
                if band == 2 || band == 3 {
                    parameters.push(param(
                        &format!("{prefix}.q"),
                        &format!("Band {band} Q"),
                        ParameterUnit::Normalized,
                        0.1,
                        18.0,
                        1.0,
                        ParameterSmoothing::Linear,
                        ParameterMapping::Log,
                    ));
                }
                parameters.push(param(
                    &format!("{prefix}.gainDb"),
                    &format!("Band {band} Gain"),
                    ParameterUnit::Db,
                    -24.0,
                    24.0,
                    0.0,
                    ParameterSmoothing::OnePole,
                    ParameterMapping::Linear,
                ));
            }
            descriptor("oxitone.eq", parameters, PluginCapabilities::default())
        })
    }

    fn create(&self, host: &HostContext) -> Box<dyn PluginInstance> {
        Box::new(EqInstance::new(host.sample_rate, host.max_block_size))
    }
}

struct Band {
    freq_hz: f64,
    q: f64,
    gain_db: f64,
    gain_smooth: OnePoleSmoother,
    left: BiquadF64,
    right: BiquadF64,
}

impl Band {
    fn redesign(&mut self, kind: BiquadKind, sample_rate: f64) {
        let gain_db = self.gain_smooth.value() as f64;
        let coeffs = design(kind, sample_rate, self.freq_hz, self.q, gain_db);
        self.left.set_coeffs(coeffs);
        self.right.set_coeffs(coeffs);
    }
}

struct EqInstance {
    sample_rate: f64,
    bands: Vec<Band>,
}

impl EqInstance {
    fn new(sample_rate: f64, _max_block_size: u32) -> Self {
        let unity = design(BiquadKind::Peak, sample_rate, 1000.0, 1.0, 0.0);
        let bands = DEFAULT_FREQS
            .iter()
            .map(|&freq| {
                let mut gain_smooth = OnePoleSmoother::new(sample_rate, 20.0);
                gain_smooth.snap(0.0);
                Band {
                    freq_hz: freq,
                    q: 1.0,
                    gain_db: 0.0,
                    gain_smooth,
                    left: BiquadF64::new(unity),
                    right: BiquadF64::new(unity),
                }
            })
            .collect();
        Self { sample_rate, bands }
    }

    /// Allocation-free `bandN.field` parsing (audio-thread safe).
    fn set_parameter(&mut self, id: &str, value: f64) {
        let Some(rest) = id.strip_prefix("band") else {
            return;
        };
        let Some((index, field)) = rest.split_once('.') else {
            return;
        };
        let Ok(index) = index.parse::<usize>() else {
            return;
        };
        if !(1..=4).contains(&index) {
            return;
        }
        let band = &mut self.bands[index - 1];
        match field {
            "freqHz" => band.freq_hz = value.clamp(20.0, 20_000.0),
            "q" => band.q = value.clamp(0.1, 18.0),
            "gainDb" => {
                band.gain_db = value.clamp(-24.0, 24.0);
                band.gain_smooth.set_target(band.gain_db as f32);
            }
            _ => {}
        }
    }
}

impl PluginInstance for EqInstance {
    fn prepare(&mut self, sample_rate: f64, max_block_size: u32) {
        *self = EqInstance::new(sample_rate, max_block_size);
    }

    fn process(&mut self, ctx: &mut ProcessContext<'_>) {
        for event in ctx.parameter_events {
            self.set_parameter(event.parameter_id, event.value);
        }
        super::pass_inputs(ctx);
        let frames = ctx.frames;
        let (left, right) = ctx.outputs.split_at_mut(1);
        let left = &mut left[0][..frames];
        let right = &mut right[0][..frames];
        for (band, kind) in self.bands.iter_mut().zip(KINDS) {
            // Advance the gain smoother across the block at control rate.
            for _ in 0..frames {
                band.gain_smooth.next_sample();
            }
            band.redesign(kind, self.sample_rate);
            oxitone_dsp::biquad::process_stereo_f64(&mut band.left, &mut band.right, left, right);
        }
    }

    fn reset(&mut self) {
        for band in &mut self.bands {
            band.left.reset();
            band.right.reset();
            band.gain_smooth.snap(band.gain_db as f32);
        }
    }

    fn tail_frames(&self) -> u64 {
        0
    }

    fn latency_frames(&self) -> u64 {
        0
    }
}
