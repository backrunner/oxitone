//! Display-thread analysis only: ring consumption, Hann FFT and stereo phase points.
use oxitone_render::preview::{NoteEcho, PreviewNode};
use std::collections::{HashMap, VecDeque};

#[derive(Default)]
pub struct NodeAnalysis {
    pub peak: f32,
    pub rms: f32,
    pub wave: Vec<[f32; 2]>,
    pub true_peak: f32,
    pub dropped: u64,
    true_peak_meter: Option<oxitone_mixer::meter::TruePeakMeter>,
    pub active: [u64; 2],
    epoch: u64,
    pending: VecDeque<NoteEcho>,
}

impl NodeAnalysis {
    pub fn update(&mut self, node: &PreviewNode, audible: u64, epoch: u64, playing: bool) {
        if epoch != self.epoch {
            self.epoch = epoch;
            self.pending.clear();
            self.active = [0; 2];
            self.wave.clear();
            self.true_peak_meter = None;
            self.true_peak = 0.;
            // The UI is the sole audio-ring consumer, including flush after seek.
            let mut stale = [0.; 8192];
            node.audio.read_frames(&mut stale);
        }
        let dropped = node.dropped.load(std::sync::atomic::Ordering::Relaxed);
        if dropped != self.dropped {
            self.active = [0; 2];
            self.pending.clear();
            self.dropped = dropped;
        }
        let (peak, rms) = node.meter();
        self.peak = peak.max(self.peak * 0.86);
        self.rms = if playing { rms } else { self.rms * 0.8 };
        let mut buffer = [0.0; 8192];
        let count = node.audio.read_frames(&mut buffer);
        if count > 0 {
            if node.id == "mix_master" {
                let meter = self
                    .true_peak_meter
                    .get_or_insert_with(|| oxitone_mixer::meter::TruePeakMeter::new(4096));
                let mut left = [0.; 4096];
                let mut right = [0.; 4096];
                for i in 0..count {
                    left[i] = buffer[2 * i];
                    right[i] = buffer[2 * i + 1];
                }
                meter.add_block(&left[..count], &right[..count]);
                self.true_peak = meter.true_peak();
            }
            self.wave.extend(
                buffer[..count * 2]
                    .chunks_exact(2)
                    .map(|frame| [frame[0], frame[1]]),
            );
            if self.wave.len() > 1024 {
                self.wave.drain(..self.wave.len() - 1024);
            }
        }
        while let Some(note) = node.notes.pop() {
            if note.epoch == epoch && self.pending.len() < 8192 {
                self.pending.push_back(note);
            }
        }
        while self
            .pending
            .front()
            .is_some_and(|note| note.frame <= audible)
        {
            let note = self.pending.pop_front().unwrap();
            let word = &mut self.active[usize::from(note.pitch / 64)];
            let bit = 1u64 << (note.pitch % 64);
            if note.on {
                *word |= bit;
            } else {
                *word &= !bit;
            }
        }
        if !playing {
            self.active = [0; 2];
        }
    }
    pub fn sounding(&self, pitch: u8) -> bool {
        self.active[usize::from(pitch / 64)] & (1u64 << (pitch % 64)) != 0
    }
}

pub type AnalysisMap = HashMap<String, NodeAnalysis>;

pub fn spectrum(samples: &[[f32; 2]]) -> Vec<f32> {
    const N: usize = 512;
    let mut real = [0.0f64; N];
    let mut imag = [0.0f64; N];
    for (i, sample) in samples.iter().rev().take(N).enumerate() {
        let window = 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / (N - 1) as f64).cos();
        real[i] = f64::from((sample[0] + sample[1]) * 0.5) * window;
    }
    for i in 0..N {
        let reverse = i.reverse_bits() >> (usize::BITS - 9);
        if i < reverse {
            real.swap(i, reverse);
        }
    }
    let mut size = 2;
    while size <= N {
        for start in (0..N).step_by(size) {
            for offset in 0..size / 2 {
                let angle = -std::f64::consts::TAU * offset as f64 / size as f64;
                let (sin, cos) = angle.sin_cos();
                let left = start + offset;
                let right = left + size / 2;
                let r = real[right] * cos - imag[right] * sin;
                let i = real[right] * sin + imag[right] * cos;
                real[right] = real[left] - r;
                imag[right] = imag[left] - i;
                real[left] += r;
                imag[left] += i;
            }
        }
        size *= 2;
    }
    (1..N / 2)
        .map(|i| ((real[i].hypot(imag[i]) * 4.0 / N as f64).max(1e-9).log10() * 20.0) as f32)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hann_fft_finds_a_bin_centered_tone() {
        let audio: Vec<_> = (0..512)
            .map(|i| {
                let v = (std::f64::consts::TAU * 16.0 * i as f64 / 512.0).sin() as f32;
                [v, v]
            })
            .collect();
        let bins = spectrum(&audio);
        assert_eq!(
            bins.iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .unwrap()
                .0
                + 1,
            16
        );
        assert!(bins[15].abs() < 0.1);
    }
}
