//! Analytic, sample-free banks with eight bandlimited frames apiece.
use oxitone_dsp::oscillator::Wavetable;
pub const FRAMES: usize = 8;
pub const BANKS: usize = 3;

pub fn build(sample_rate: f64) -> Vec<Wavetable> {
    let mut tables = Vec::with_capacity(BANKS * FRAMES + 2);
    for bank in 1..=BANKS {
        for frame in 0..FRAMES {
            let t = frame as f64 / (FRAMES - 1) as f64;
            let partials: Vec<_> = (1..=128)
                .map(|h| {
                    let n = h as f64;
                    let amplitude = match bank {
                        1 => {
                            // Sine → saw → narrow pulse; retain a firm fundamental.
                            let saw = 0.64 / n;
                            let pulse =
                                1.27 * (std::f64::consts::PI * n * (0.5 - t * 0.38)).sin() / n;
                            let rich = saw * (1. - t) + pulse * t;
                            if h == 1 {
                                0.85
                            } else {
                                rich * (t * 2.).min(1.)
                            }
                        }
                        2 => {
                            // Moving harmonic comb, useful for metallic/FM bass carriers.
                            let comb = (n * (0.18 + t * 0.6) * std::f64::consts::PI).cos().powi(4);
                            if h == 1 {
                                0.65
                            } else {
                                (0.12 + comb * 0.88) / n.sqrt() * 0.22
                            }
                        }
                        _ => {
                            // Two harmonic formants sweep through vowel-like spectra.
                            let f1 = 2. + t * 7.;
                            let f2 = 12. - t * 5.;
                            if h == 1 {
                                0.5
                            } else {
                                0.55 * (-((n - f1) / 1.4).powi(2)).exp()
                                    + 0.3 * (-((n - f2) / 2.).powi(2)).exp()
                            }
                        }
                    };
                    (amplitude, 0.)
                })
                .collect();
            tables.push(Wavetable::from_partials(&partials, 2048, 11, sample_rate));
        }
    }
    // Sub pulse (25% duty) and rounded square, with no DC component.
    for rounded in [false, true] {
        let partials: Vec<_> = (1..=128)
            .map(|h| {
                let n = h as f64;
                if rounded {
                    (if h % 2 == 1 { 1. / n.powi(3) } else { 0. }, 0.)
                } else {
                    (
                        0.,
                        4. / std::f64::consts::PI * (std::f64::consts::PI * n * 0.25).sin() / n,
                    )
                }
            })
            .collect();
        tables.push(Wavetable::from_partials(&partials, 2048, 11, sample_rate));
    }
    tables
}

pub fn pair(bank: usize, position: f32) -> (usize, usize, f32) {
    let p = position.clamp(0., 1.) * (FRAMES - 1) as f32;
    let index = (p as usize).min(FRAMES - 2);
    let start = 6 + (bank - 1).min(BANKS - 1) * FRAMES;
    (start + index, start + index + 1, p - index as f32)
}

pub fn sub_table(wave: usize) -> usize {
    match wave {
        1 => 3,
        2 => 1,
        3 => 2,
        4 => 6 + BANKS * FRAMES,
        5 => 6 + BANKS * FRAMES + 1,
        _ => 0,
    }
}
