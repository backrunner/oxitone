//! ADSR envelope with sample-accurate segment boundaries. v1 uses linear
//! ramps for every segment, which keeps boundaries exact and the tail free
//! of denormals; the level accumulator is `f64`. RT-safe after `new`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdsrStage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

pub struct Adsr {
    sample_rate: f64,
    attack_s: f64,
    decay_s: f64,
    sustain: f32,
    release_s: f64,
    stage: AdsrStage,
    level: f64,
    step: f64,
    remaining: u32,
    curves: [f64; 3],
    origin: f64,
    target: f64,
    total: u32,
    factor: f64,
}

impl Adsr {
    pub fn new(sample_rate: f64) -> Self {
        Self {
            sample_rate,
            attack_s: 0.01,
            decay_s: 0.1,
            sustain: 0.8,
            release_s: 0.2,
            stage: AdsrStage::Idle,
            level: 0.0,
            step: 0.0,
            remaining: 0,
            curves: [0.; 3],
            origin: 0.,
            target: 0.,
            total: 0,
            factor: 1.,
        }
    }

    /// Parameter update at a block boundary (control rate). Segment steps
    /// are (re)computed at the next segment start, not mid-segment.
    pub fn set_params(&mut self, attack_s: f64, decay_s: f64, sustain: f32, release_s: f64) {
        self.attack_s = attack_s.max(0.0);
        self.decay_s = decay_s.max(0.0);
        self.sustain = sustain.clamp(0.0, 1.0);
        self.release_s = release_s.max(0.0);
    }

    fn segment_samples(&self, seconds: f64) -> u32 {
        (seconds * self.sample_rate).round() as u32
    }

    /// Curvature is latched at the next segment start; zero is the original linear ramp.
    pub fn set_curves(&mut self, attack: f64, decay: f64, release: f64) {
        self.curves = [
            attack.clamp(-1., 1.),
            decay.clamp(-1., 1.),
            release.clamp(-1., 1.),
        ];
    }

    fn start_segment(&mut self, samples: u32, target: f64, curve: usize) {
        self.origin = self.level;
        self.target = target;
        self.total = samples;
        self.factor = 2f64.powf(self.curves[curve] * 4.);
    }

    #[inline]
    fn advance_segment(&mut self) {
        self.remaining -= 1;
        if self.factor == 1. {
            self.level += self.step;
        } else {
            let t = 1. - self.remaining as f64 / self.total as f64;
            let shaped = t / (t + (1. - t) * self.factor);
            self.level = self.origin + (self.target - self.origin) * shaped;
        }
    }

    /// Start the attack from the current level (click-free retrigger).
    pub fn note_on(&mut self) {
        let samples = self.segment_samples(self.attack_s);
        if samples == 0 {
            self.level = 1.0;
            self.enter_decay();
            return;
        }
        self.step = (1.0 - self.level) / f64::from(samples);
        self.start_segment(samples, 1., 0);
        self.remaining = samples;
        self.stage = AdsrStage::Attack;
    }

    /// Enter release from any stage; the ramp starts at the current level.
    pub fn note_off(&mut self) {
        if self.stage == AdsrStage::Idle {
            return;
        }
        let samples = self.segment_samples(self.release_s);
        if samples == 0 {
            self.level = 0.0;
            self.stage = AdsrStage::Idle;
            return;
        }
        self.step = -self.level / f64::from(samples);
        self.start_segment(samples, 0., 2);
        self.remaining = samples;
        self.stage = AdsrStage::Release;
    }

    fn enter_decay(&mut self) {
        let samples = self.segment_samples(self.decay_s);
        if samples == 0 || self.level <= self.sustain as f64 {
            self.level = self.sustain as f64;
            self.stage = AdsrStage::Sustain;
            return;
        }
        self.step = (self.sustain as f64 - self.level) / f64::from(samples);
        self.start_segment(samples, self.sustain as f64, 1);
        self.remaining = samples;
        self.stage = AdsrStage::Decay;
    }

    /// Advance one sample. RT-safe.
    #[inline]
    pub fn next_sample(&mut self) -> f32 {
        match self.stage {
            AdsrStage::Idle => 0.0,
            AdsrStage::Sustain => self.sustain,
            AdsrStage::Attack => {
                self.advance_segment();
                if self.remaining == 0 {
                    self.level = 1.0;
                    self.enter_decay();
                }
                self.level as f32
            }
            AdsrStage::Decay => {
                self.advance_segment();
                if self.remaining == 0 {
                    self.level = self.sustain as f64;
                    self.stage = AdsrStage::Sustain;
                }
                self.level as f32
            }
            AdsrStage::Release => {
                self.advance_segment();
                if self.remaining == 0 {
                    self.level = 0.0;
                    self.stage = AdsrStage::Idle;
                }
                self.level as f32
            }
        }
    }

    /// Fill `out` with consecutive envelope values. RT-safe.
    pub fn process(&mut self, out: &mut [f32]) {
        for x in out.iter_mut() {
            *x = self.next_sample();
        }
    }

    pub fn stage(&self) -> AdsrStage {
        self.stage
    }

    pub fn is_active(&self) -> bool {
        self.stage != AdsrStage::Idle
    }

    pub fn level(&self) -> f32 {
        self.level as f32
    }

    /// Flush playback state for seek/loop while retaining envelope settings.
    pub fn reset(&mut self) {
        self.stage = AdsrStage::Idle;
        self.level = 0.0;
        self.step = 0.0;
        self.remaining = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env() -> Adsr {
        // 1 kHz sample rate: 1 ms == 1 sample for readable assertions.
        let mut e = Adsr::new(1000.0);
        e.set_params(0.010, 0.010, 0.5, 0.010);
        e
    }

    #[test]
    fn attack_reaches_peak_sample_accurately() {
        let mut e = env();
        e.note_on();
        let out: Vec<f32> = (0..10).map(|_| e.next_sample()).collect();
        assert!((out[0] - 0.1).abs() < 1e-6);
        for (i, &v) in out.iter().enumerate().skip(1) {
            assert!((v - out[i - 1] - 0.1).abs() < 1e-5);
        }
        assert_eq!(e.level(), 1.0);
        assert_eq!(e.stage(), AdsrStage::Decay);
    }

    #[test]
    fn decay_lands_on_sustain_and_holds() {
        let mut e = env();
        e.note_on();
        for _ in 0..20 {
            e.next_sample();
        }
        assert_eq!(e.stage(), AdsrStage::Sustain);
        assert_eq!(e.level(), 0.5);
        assert_eq!(e.next_sample(), 0.5);
    }

    #[test]
    fn release_from_sustain_reaches_idle() {
        let mut e = env();
        e.note_on();
        for _ in 0..20 {
            e.next_sample();
        }
        e.note_off();
        assert_eq!(e.stage(), AdsrStage::Release);
        for _ in 0..10 {
            e.next_sample();
        }
        assert_eq!(e.stage(), AdsrStage::Idle);
        assert_eq!(e.next_sample(), 0.0);
    }

    #[test]
    fn release_during_attack_starts_from_current_level() {
        let mut e = env();
        e.note_on();
        for _ in 0..5 {
            e.next_sample();
        }
        let level = e.level();
        assert!((level - 0.5).abs() < 1e-6);
        e.note_off();
        assert!((e.next_sample() - (level - 0.05)).abs() < 1e-5);
    }

    #[test]
    fn zero_times_jump_segments() {
        let mut e = Adsr::new(1000.0);
        e.set_params(0.0, 0.0, 0.25, 0.0);
        e.note_on();
        assert_eq!(e.stage(), AdsrStage::Sustain);
        assert_eq!(e.level(), 0.25);
        e.note_off();
        assert_eq!(e.stage(), AdsrStage::Idle);
    }

    #[test]
    fn curved_segments_keep_duration_endpoints_and_retrigger_continuity() {
        for curve in [-1., -0.5, 0.5, 1.] {
            let mut e = env();
            e.set_curves(curve, curve, curve);
            e.note_on();
            let first: Vec<_> = (0..10).map(|_| e.next_sample()).collect();
            assert!(first.windows(2).all(|v| v[1] >= v[0]));
            assert_eq!(e.stage(), AdsrStage::Decay);
            assert_eq!(e.level(), 1.);
            for _ in 0..10 {
                e.next_sample();
            }
            assert_eq!(e.level(), 0.5);
            assert_eq!(e.stage(), AdsrStage::Sustain);
            e.note_off();
            let start = e.level();
            e.next_sample();
            assert!(e.level() < start);
            let at = e.level();
            e.note_on();
            assert_eq!(at, e.level());
            e.next_sample();
            assert!(e.level() > at);
            e.note_off();
            for _ in 0..10 {
                e.next_sample();
            }
            assert_eq!(e.stage(), AdsrStage::Idle);
            assert_eq!(e.level(), 0.);
        }
    }
}
