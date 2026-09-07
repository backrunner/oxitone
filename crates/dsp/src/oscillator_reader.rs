use super::Wavetable;
use core::f64::consts::TAU;

/// Wavetable playback state. `prepare` picks the mip level; `next` only
/// reads the preselected table. RT-safe.
pub struct WavetableReader {
    phase: f64,
    pub(super) level: usize,
}

impl WavetableReader {
    pub fn phase(&self) -> f64 {
        self.phase
    }

    /// Read a bounded phase-distorted cycle at a conservatively selected mip.
    /// The caller selects a mip using the warp's maximum slope and FM bandwidth.
    #[inline]
    pub fn next_warped(
        &mut self,
        table: &Wavetable,
        target: &Wavetable,
        position: f32,
        freq_hz: f64,
        mode: usize,
        amount: f32,
        phase_mod: f64,
    ) -> f32 {
        if (mode == 0 || amount == 0.) && phase_mod == 0. {
            return self.next_blend(table, target, position, freq_hz);
        }
        let phase = self.phase;
        let u = (phase + phase_mod).rem_euclid(1.);
        let a = amount.clamp(0., 1.) as f64;
        let warped = match mode {
            1 => u + a * (TAU * u).sin() * 0.15,
            2 => {
                let pivot = 0.5 - 0.45 * a;
                if u < pivot {
                    u * 0.5 / pivot
                } else {
                    0.5 + (u - pivot) * 0.5 / (1. - pivot)
                }
            }
            // Integer harmonic sync with continuous crossfade: no moving wrap discontinuity.
            3 => (u * (1. + (a * 7.).floor())).rem_euclid(1.),
            _ => u,
        };
        self.phase = warped.rem_euclid(1.);
        let first = self.next_blend(table, target, position, freq_hz);
        let out = if mode == 3 {
            self.phase = (u * (2. + (a * 7.).floor())).rem_euclid(1.);
            let next = self.next_blend(table, target, position, freq_hz);
            first + (next - first) * (a * 7.).fract() as f32
        } else {
            first
        };
        self.phase = (phase + freq_hz / table.sample_rate).rem_euclid(1.);
        out
    }
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
