//! Oscillators: band-limited-ish basic waveforms (PolyBLEP saw/square) and
//! mip-mapped wavetable reading. Phase accumulators are `f64`; mip level
//! selection happens in `prepare` so `process`/`next` only read preallocated
//! tables (02-domain-spec.md §WavetableSynth). RT-safe after construction.

use core::f64::consts::TAU;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Waveform {
    Sine,
    Saw,
    Square,
    Triangle,
}

/// PolyBLEP residual for a discontinuity at phase 0; `dt` is phase increment.
fn poly_blep(t: f64, dt: f64) -> f64 {
    if t < dt {
        let u = t / dt;
        2.0 * u - u * u - 1.0
    } else if t > 1.0 - dt {
        let u = (t - 1.0) / dt;
        u * u + 2.0 * u + 1.0
    } else {
        0.0
    }
}

fn sample_waveform(wf: Waveform, phase: f64, dt: f64) -> f64 {
    match wf {
        Waveform::Sine => (phase * TAU).sin(),
        Waveform::Saw => 2.0 * phase - 1.0 - poly_blep(phase, dt),
        Waveform::Square => {
            let naive = if phase < 0.5 { 1.0 } else { -1.0 };
            naive + poly_blep(phase, dt) - poly_blep((phase + 0.5).fract(), dt)
        }
        Waveform::Triangle => 4.0 * (phase - 0.5).abs() - 1.0,
    }
}

/// Single oscillator with an `f64` phase accumulator in cycles. RT-safe.
pub struct Oscillator {
    phase: f64,
}

impl Oscillator {
    pub fn new() -> Self {
        Self { phase: 0.0 }
    }

    pub fn set_phase(&mut self, phase: f64) {
        self.phase = phase.rem_euclid(1.0);
    }

    pub fn phase(&self) -> f64 {
        self.phase
    }

    #[inline]
    fn advance(&mut self, freq_hz: f64, sample_rate: f64) -> f64 {
        let dt = freq_hz / sample_rate;
        let phase = self.phase;
        self.phase = (self.phase + dt).rem_euclid(1.0);
        phase
    }

    /// One sample at constant frequency. RT-safe.
    #[inline]
    pub fn next(&mut self, wf: Waveform, freq_hz: f64, sample_rate: f64) -> f32 {
        let dt = (freq_hz / sample_rate).min(0.5);
        let phase = self.advance(freq_hz, sample_rate);
        sample_waveform(wf, phase, dt) as f32
    }

    /// Fill `out` at constant frequency. RT-safe.
    pub fn render(&mut self, wf: Waveform, freq_hz: f64, sample_rate: f64, out: &mut [f32]) {
        for x in out.iter_mut() {
            *x = self.next(wf, freq_hz, sample_rate);
        }
    }
}

impl Default for Oscillator {
    fn default() -> Self {
        Self::new()
    }
}

/// Band-limited wavetable with precomputed mip levels. Level k keeps
/// harmonics `1..=N >> (k + 1)`; each level stores `N + 1` samples with the
/// wrap point duplicated so reads never branch on the table edge.
pub struct Wavetable {
    levels: Vec<Vec<f32>>,
    table_len: usize,
    sample_rate: f64,
}

impl Wavetable {
    /// Build mip levels from one cycle of `base`. Prepare-time only: the
    /// naive DFT is O(levels · N²) and allocates all level tables.
    pub fn new(base: &[f32], level_count: usize, sample_rate: f64) -> Self {
        let n = base.len();
        assert!(n >= 4, "wavetable needs at least 4 samples");
        let levels_count = level_count.max(1);
        let mut re = vec![0.0f64; n / 2 + 1];
        let mut im = vec![0.0f64; n / 2 + 1];
        for (h, (re_h, im_h)) in re.iter_mut().zip(im.iter_mut()).enumerate() {
            for (i, &x) in base.iter().enumerate() {
                let ang = -TAU * h as f64 * i as f64 / n as f64;
                *re_h += x as f64 * ang.cos();
                *im_h += x as f64 * ang.sin();
            }
        }
        let mut levels = Vec::with_capacity(levels_count);
        for k in 0..levels_count {
            let hmax = (n >> (k + 1)).min(n / 2);
            let mut table = Vec::with_capacity(n + 1);
            for i in 0..n {
                let mut v = re[0] / n as f64;
                for h in 1..=hmax {
                    let ang = TAU * h as f64 * i as f64 / n as f64;
                    v += 2.0 / n as f64 * (re[h] * ang.cos() - im[h] * ang.sin());
                }
                table.push(v as f32);
            }
            table.push(table[0]);
            levels.push(table);
        }
        Self {
            levels,
            table_len: n,
            sample_rate,
        }
    }

    /// Smallest mip level whose top harmonic stays below Nyquist at
    /// `freq_hz`. Called from `prepare`, never per sample.
    pub fn select_level(&self, freq_hz: f64) -> usize {
        let nyquist = self.sample_rate / 2.0;
        for k in 0..self.levels.len() {
            let harmonics = (self.table_len >> (k + 1)).max(1) as f64;
            if harmonics * freq_hz < nyquist {
                return k;
            }
        }
        self.levels.len() - 1
    }

    pub fn level_table(&self, level: usize) -> &[f32] {
        &self.levels[level.min(self.levels.len() - 1)]
    }

    pub fn level_count(&self) -> usize {
        self.levels.len()
    }
}

/// Wavetable playback state. `prepare` picks the mip level; `next` only
/// reads the preselected table. RT-safe.
pub struct WavetableReader {
    phase: f64,
    level: usize,
}

impl WavetableReader {
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            level: 0,
        }
    }

    /// Control-rate mip selection (per note-on / per block). Not per sample.
    pub fn prepare(&mut self, table: &Wavetable, freq_hz: f64) {
        self.level = table.select_level(freq_hz);
    }

    pub fn set_phase(&mut self, phase: f64) {
        self.phase = phase.rem_euclid(1.0);
    }

    /// Linear-interpolated read at the prepared level. RT-safe.
    #[inline]
    pub fn next(&mut self, table: &Wavetable, freq_hz: f64) -> f32 {
        let t = table.level_table(self.level);
        let n = table.table_len;
        let pos = self.phase * n as f64;
        let i = pos as usize;
        let frac = (pos - i as f64) as f32;
        let out = t[i] + (t[i + 1] - t[i]) * frac;
        self.phase = (self.phase + freq_hz / table.sample_rate).rem_euclid(1.0);
        out
    }

    /// Same-phase interpolation between two prepared cycles; neither table is modified.
    /// Both cycles must use the same table length, mip layout and sample rate.
    /// The zero-position fast path preserves the original reader's exact samples.
    #[inline]
    pub fn next_blend(
        &mut self,
        table: &Wavetable,
        target: &Wavetable,
        position: f32,
        freq_hz: f64,
    ) -> f32 {
        if position == 0. {
            return self.next(table, freq_hz);
        }
        let position = position.clamp(0., 1.);
        let t = target.level_table(self.level);
        let pos = self.phase * target.table_len as f64;
        let i = pos as usize;
        let frac = (pos - i as f64) as f32;
        let other = t[i] + (t[i + 1] - t[i]) * frac;
        let base = self.next(table, freq_hz);
        base + (other - base) * position
    }

    /// Fill `out` at constant frequency. RT-safe.
    pub fn render(&mut self, table: &Wavetable, freq_hz: f64, out: &mut [f32]) {
        for x in out.iter_mut() {
            *x = self.next(table, freq_hz);
        }
    }
}

impl Default for WavetableReader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "oscillator_tests.rs"]
mod tests;
