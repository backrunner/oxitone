//! Analytic source cycles. Prepare builds band-limited mip tables from these;
//! the UI may draw the source shape without creating a DSP instance.
pub fn cycle_value(kind: usize, phase: f64) -> f64 {
    let t = phase.rem_euclid(1.);
    let sine = |harmonic: f64| (std::f64::consts::TAU * t * harmonic).sin();
    match kind {
        1 => 2. * t - 1.,
        2 => {
            if t < 0.5 {
                1.
            } else {
                -1.
            }
        }
        3 => 4. * (t - 0.5).abs() - 1.,
        4 => (sine(1.) + 0.5 * sine(2.) + 0.25 * sine(4.)) / 1.75,
        5 => (0.5 * sine(1.) + 0.5 * sine(3.) + 0.3 * sine(5.) + 0.15 * sine(9.)) / 1.45,
        _ => sine(1.),
    }
    .clamp(-1., 1.)
}
