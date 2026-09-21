use crate::{Error, Result};
use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use std::path::Path;
#[cfg(test)]
#[path = "wave_tests.rs"]
mod tests;

pub struct Output {
    writer: WavWriter<std::io::BufWriter<std::fs::File>>,
    peak: f32,
    frames: u64,
}

/// Bounded offline input; no system audio device is used.
pub struct Input {
    channels: usize,
    frames: usize,
    samples: Vec<f32>,
}
impl Input {
    pub fn open(path: &Path, sample_rate: u32) -> Result<Self> {
        let metadata =
            std::fs::metadata(path).map_err(|error| Error::new("AssetUnavailable", error))?;
        if metadata.len() > 1024 * 1024 * 1024 {
            return Err(Error::new("BudgetExceeded", "input WAV exceeds 1 GiB"));
        }
        if !metadata.is_file() {
            return Err(Error::new(
                "AssetUnavailable",
                "Input WAV must be a regular file",
            ));
        }
        let mut reader =
            WavReader::open(path).map_err(|error| Error::new("AssetUnavailable", error))?;
        let spec = reader.spec();
        let channels = usize::from(spec.channels);
        // Bound decoded allocation independently of compressed/on-disk sample width.
        if reader.len() > 64 * 1024 * 1024 {
            return Err(Error::new(
                "BudgetExceeded",
                "Decoded input WAV exceeds 256 MiB",
            ));
        }
        if !(1..=2).contains(&channels) || spec.sample_rate != sample_rate {
            return Err(Error::new(
                "PluginConfigInvalid",
                "input WAV must be mono/stereo at the render sample rate",
            ));
        }
        if spec.bits_per_sample == 0 || spec.bits_per_sample > 32 {
            return Err(Error::new(
                "PluginConfigInvalid",
                "unsupported input WAV bit depth",
            ));
        }
        let mut samples = Vec::new();
        match spec.sample_format {
            SampleFormat::Float => {
                if spec.bits_per_sample != 32 {
                    return Err(Error::new(
                        "PluginConfigInvalid",
                        "only 32-bit float WAV is supported",
                    ));
                }
                for value in reader.samples::<f32>() {
                    let value = value.map_err(|error| Error::new("AssetUnavailable", error))?;
                    if !value.is_finite() {
                        return Err(Error::new(
                            "AssetUnavailable",
                            "input WAV contains a non-finite sample",
                        ));
                    }
                    samples.push(value);
                }
            }
            SampleFormat::Int => {
                let scale = 2_f32.powi(i32::from(spec.bits_per_sample) - 1);
                for value in reader.samples::<i32>() {
                    let value = value.map_err(|error| Error::new("AssetUnavailable", error))?;
                    samples.push(value as f32 / scale);
                }
            }
        }
        if samples.len() % channels != 0 {
            return Err(Error::new(
                "AssetUnavailable",
                "input WAV has a partial frame",
            ));
        }
        let frames = samples.len() / channels;
        Ok(Self {
            channels,
            frames,
            samples,
        })
    }
    pub fn fill(&self, destinations: &mut [Vec<f32>], start: u64, frames: usize) {
        let mono_output = destinations.len() == 1;
        for (channel, destination) in destinations.iter_mut().enumerate() {
            for (offset, sample) in destination.iter_mut().take(frames).enumerate() {
                let frame = start as usize + offset;
                *sample = if frame < self.frames {
                    let left = self.samples[frame * self.channels];
                    if self.channels == 1 {
                        left
                    } else if mono_output {
                        (left + self.samples[frame * self.channels + 1]) * 0.5
                    } else {
                        self.samples[frame * self.channels + channel]
                    }
                } else {
                    0.0
                };
            }
        }
    }
}
impl Output {
    pub fn create(path: &Path, sample_rate: u32) -> Result<Self> {
        let spec = WavSpec {
            channels: 2,
            sample_rate,
            bits_per_sample: 32,
            sample_format: SampleFormat::Float,
        };
        Ok(Self {
            writer: WavWriter::new(
                std::io::BufWriter::new(
                    std::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(path)
                        .map_err(crate::bundle::asset)?,
                ),
                spec,
            )
            .map_err(|error| Error::new("AssetUnavailable", error))?,
            peak: 0.0,
            frames: 0,
        })
    }
    pub fn write(&mut self, left: &[f32], right: &[f32], frames: usize) -> Result<()> {
        for index in 0..frames {
            let l = left.get(index).copied().unwrap_or(0.0);
            let r = right.get(index).copied().unwrap_or(0.0);
            if !l.is_finite() || !r.is_finite() {
                return Err(Error::new("RealtimeFault", "VST3 output is not finite"));
            }
            self.peak = self.peak.max(l.abs()).max(r.abs());
            self.writer
                .write_sample(l)
                .map_err(|error| Error::new("AssetUnavailable", error))?;
            self.writer
                .write_sample(r)
                .map_err(|error| Error::new("AssetUnavailable", error))?;
        }
        self.frames += frames as u64;
        Ok(())
    }
    pub fn finish(self) -> Result<f32> {
        self.writer
            .finalize()
            .map_err(|error| Error::new("AssetUnavailable", error))?;
        Ok(self.peak)
    }
}
