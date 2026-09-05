//! Unison detune/spread mixing plan. `plan_unison` runs at prepare time;
//! the returned arrays are read-only in the audio callback.

/// Hard cap on unison voices per oscillator (compile-time bound).
pub const MAX_UNISON: usize = 16;

/// Per-voice frequency ratios, pan positions, and constant-power gains.
#[derive(Debug, Clone)]
pub struct UnisonPlan {
    pub count: usize,
    /// Frequency multiplier per voice, symmetric around 1.0.
    pub ratios: [f64; MAX_UNISON],
    /// Pan per voice in -1..1 (`spread` scales the excursion).
    pub pans: [f32; MAX_UNISON],
    /// Per-voice gain; `Σ gain² == 1` keeps total power constant.
    pub gains: [f32; MAX_UNISON],
}

/// Build the mix plan for `voices` unison voices spread over
/// `±detune_cents` and `±spread` pan. `voices` is clamped to 1..=MAX_UNISON.
pub fn plan_unison(voices: usize, detune_cents: f64, spread: f32) -> UnisonPlan {
    let count = voices.clamp(1, MAX_UNISON);
    let mut plan = UnisonPlan {
        count,
        ratios: [1.0; MAX_UNISON],
        pans: [0.0; MAX_UNISON],
        gains: [0.0; MAX_UNISON],
    };
    let gain = (count as f32).sqrt().recip();
    for i in 0..count {
        let pos = if count == 1 {
            0.0
        } else {
            2.0 * i as f64 / (count - 1) as f64 - 1.0
        };
        plan.ratios[i] = 2f64.powf(pos * detune_cents / 1200.0);
        plan.pans[i] = (pos as f32 * spread).clamp(-1.0, 1.0);
        plan.gains[i] = gain;
    }
    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_voice_is_unity_center() {
        let p = plan_unison(1, 50.0, 1.0);
        assert_eq!(p.count, 1);
        assert_eq!(p.ratios[0], 1.0);
        assert_eq!(p.pans[0], 0.0);
        assert_eq!(p.gains[0], 1.0);
    }

    #[test]
    fn detune_is_symmetric_and_constant_power() {
        let p = plan_unison(5, 24.0, 0.8);
        assert_eq!(p.count, 5);
        for i in 0..5 {
            let j = 4 - i;
            assert!((p.ratios[i] * p.ratios[j] - 1.0).abs() < 1e-12);
            assert!((p.pans[i] + p.pans[j]).abs() < 1e-6);
        }
        assert_eq!(p.ratios[2], 1.0);
        let power: f32 = p.gains[..5].iter().map(|g| g * g).sum();
        assert!((power - 1.0).abs() < 1e-6);
        assert!((p.ratios[4] - 2f64.powf(24.0 / 1200.0)).abs() < 1e-12);
    }

    #[test]
    fn voice_count_is_capped() {
        assert_eq!(plan_unison(64, 10.0, 1.0).count, MAX_UNISON);
        assert_eq!(plan_unison(0, 10.0, 1.0).count, 1);
    }
}
