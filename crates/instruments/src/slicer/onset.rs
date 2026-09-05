//! `onset-v1`: deterministic transient detection for the Slicer
//! (02-domain-spec.md §内置 Slicer — onset 算法版本化). Runs at compile time
//! only; never reachable from the audio callback.
//!
//! Fixed algorithm parameters (any change requires a new algorithm id):
//! - mono mix `(L + R) * 0.5` of the prepared sample,
//! - energy envelope: mean of squares over a 512-frame window at 128-frame
//!   hop (sliding `f64` accumulation),
//! - onset strength: half-wave rectified envelope difference (energy flux),
//! - threshold: `mean + (1 - sensitivity) * 4 * stddev` over the flux,
//! - peak picking: strict local maximum above threshold,
//! - minimum interval 50 ms (`ceil(0.05 * sampleRate / hop)` hops); earlier
//!   peaks within the interval of an accepted peak are skipped,
//! - slice 0 always starts at frame 0; at most 64 slices (chronological).

/// Analysis window in frames.
pub const WINDOW: usize = 512;
/// Hop in frames.
pub const HOP: usize = 128;
/// Minimum onset interval in milliseconds.
pub const MIN_INTERVAL_MS: f64 = 50.0;
/// Sensitivity range/default multiplier: threshold sigma factor at s = 0.
pub const THRESHOLD_SIGMA: f64 = 4.0;

pub const ONSET_ALGORITHM_V1: &str = "onset-v1";

/// Detect onset frame positions in `channels` (1 or 2 non-interleaved
/// channels). Deterministic: identical input always yields identical output.
pub fn detect_onsets_v1(channels: &[Vec<f32>], sample_rate: u32, sensitivity: f64) -> Vec<u64> {
    let frames = channels.first().map_or(0, Vec::len);
    if frames < WINDOW + HOP {
        return Vec::new();
    }
    let sensitivity = sensitivity.clamp(0.0, 1.0);
    let hops = (frames - WINDOW) / HOP + 1;
    // Sliding mean-of-squares envelope.
    let mut env = vec![0.0f64; hops];
    let mut sum = 0.0f64;
    for i in 0..WINDOW {
        let x = mono_at(channels, i);
        sum += x * x;
    }
    env[0] = sum / WINDOW as f64;
    for (i, slot) in env.iter_mut().enumerate().skip(1) {
        let remove_start = (i - 1) * HOP;
        let add_start = (i - 1) * HOP + WINDOW;
        let mut added = 0.0f64;
        let mut removed = 0.0f64;
        for k in 0..HOP {
            let xa = mono_at(channels, add_start + k);
            let xr = mono_at(channels, remove_start + k);
            added += xa * xa;
            removed += xr * xr;
        }
        sum += added - removed;
        *slot = (sum / WINDOW as f64).max(0.0);
    }
    // Half-wave rectified flux.
    let mut flux = vec![0.0f64; hops];
    for i in 1..hops {
        flux[i] = (env[i] - env[i - 1]).max(0.0);
    }
    let mean = flux.iter().sum::<f64>() / hops as f64;
    let variance = flux.iter().map(|f| (f - mean) * (f - mean)).sum::<f64>() / hops as f64;
    let threshold = mean + (1.0 - sensitivity) * THRESHOLD_SIGMA * variance.sqrt();
    let min_gap =
        ((MIN_INTERVAL_MS / 1000.0 * f64::from(sample_rate)) / HOP as f64).ceil() as usize;
    let mut onsets = Vec::new();
    let mut last: Option<usize> = None;
    for i in 1..hops.saturating_sub(1) {
        let is_peak = flux[i] > threshold && flux[i] >= flux[i - 1] && flux[i] > flux[i + 1];
        if !is_peak {
            continue;
        }
        if last.is_some_and(|l| i - l < min_gap.max(1)) {
            continue;
        }
        onsets.push((i * HOP) as u64);
        last = Some(i);
    }
    onsets
}

fn mono_at(channels: &[Vec<f32>], frame: usize) -> f64 {
    let l = f64::from(channels[0][frame]);
    match channels.get(1) {
        Some(r) => (l + f64::from(r[frame])) * 0.5,
        None => l,
    }
}
