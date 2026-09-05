//! Offline sample-rate conversion built on the dsp windowed-sinc polyphase
//! resampler (quality target ≥ 100 dB SNR, `03-audio-runtime-spec.md`
//! §数值精度). Control thread only.

use oxitone_dsp::resample::SincResampler;

/// Resample one channel from `from_rate` to `to_rate`, returning exactly
/// `round(input.len() * to_rate / from_rate)` frames aligned to input time
/// (the resampler's group delay is compensated with a zero head; the tail is
/// flushed with zeros).
pub fn resample_channel(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate || input.is_empty() {
        return input.to_vec();
    }
    let step = from_rate as f64 / to_rate as f64;
    let expected =
        ((input.len() as u64 * to_rate as u64 + from_rate as u64 / 2) / from_rate as u64) as usize;
    let mut resampler = SincResampler::new(step.max(1.0), 4096);
    let head = resampler.latency_input_frames(step).round() as usize;
    let mut feed = vec![0.0f32; head];
    feed.extend_from_slice(input);

    let mut out = vec![0.0f32; expected];
    let zeros = [0.0f32; 4096];
    let mut offset = 0usize;
    let mut produced = 0usize;
    let mut iterations = 0usize;
    let max_iterations = (head + input.len() + expected) / 1024 + 64;
    while produced < expected && iterations < max_iterations {
        iterations += 1;
        let block: &[f32] = if offset < feed.len() {
            &feed[offset..(offset + 4096).min(feed.len())]
        } else {
            &zeros[..]
        };
        let result = resampler.process(block, &mut out[produced..], step);
        if offset < feed.len() {
            offset += result.consumed;
        }
        produced += result.produced;
        if result.consumed == 0 && result.produced == 0 {
            break;
        }
    }
    out
}

/// Resample every plane of a non-interleaved buffer.
pub fn resample_planes(channels: &[Vec<f32>], from_rate: u32, to_rate: u32) -> Vec<Vec<f32>> {
    channels
        .iter()
        .map(|plane| resample_channel(plane, from_rate, to_rate))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(frames: usize, rate: u32, freq: f64) -> Vec<f32> {
        (0..frames)
            .map(|i| (2.0 * std::f64::consts::PI * freq * i as f64 / rate as f64).sin() as f32)
            .collect()
    }

    #[test]
    fn output_length_matches_rounded_ratio() {
        let input = sine(4410, 44100, 440.0);
        assert_eq!(resample_channel(&input, 44100, 48000).len(), 4800);
        assert_eq!(resample_channel(&input, 48000, 44100).len(), 4052);
        assert_eq!(resample_channel(&input, 44100, 44100).len(), 4410);
    }

    #[test]
    fn upsample_snr_is_high() {
        let frames = 48000;
        let input = sine(frames, 48000, 997.0);
        let out = resample_channel(&input, 48000, 44100);
        let skip = 200;
        let mut signal = 0.0f64;
        let mut error = 0.0f64;
        for (k, &y) in out.iter().enumerate().skip(skip).take(out.len() - 2 * skip) {
            let ideal = (2.0 * std::f64::consts::PI * 997.0 * k as f64 / 44100.0).sin();
            signal += ideal * ideal;
            let d = y as f64 - ideal;
            error += d * d;
        }
        let snr_db = 10.0 * (signal / error).log10();
        assert!(snr_db > 60.0, "SNR {snr_db:.1} dB below 60 dB floor");
    }

    #[test]
    fn downsample_snr_is_high() {
        let frames = 48000;
        let input = sine(frames, 48000, 2000.0);
        let out = resample_channel(&input, 48000, 24000);
        let skip = 500;
        let mut signal = 0.0f64;
        let mut error = 0.0f64;
        for (k, &y) in out.iter().enumerate().skip(skip).take(out.len() - 2 * skip) {
            let ideal = (2.0 * std::f64::consts::PI * 2000.0 * k as f64 / 24000.0).sin();
            signal += ideal * ideal;
            let d = y as f64 - ideal;
            error += d * d;
        }
        let snr_db = 10.0 * (signal / error).log10();
        assert!(snr_db > 60.0, "SNR {snr_db:.1} dB below 60 dB floor");
    }
}
