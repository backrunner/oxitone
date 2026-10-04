//! Source ADSR geometry with a stable time axis for the duration of a gesture.
#[derive(Clone, Copy)]
pub struct TimeAxis {
    pub hold: f64,
    pub end: f64,
}
impl TimeAxis {
    pub fn new([attack, decay, _, release]: [f64; 4]) -> Self {
        let hold = ((attack + decay + release) * 0.3).max(0.05);
        // Leave room to drag Release to the right before the next automatic zoom.
        let end = ((attack + decay + hold + release) * 1.25).max(0.001);
        Self { hold, end }
    }
    pub fn points(self, [a, d, s, r]: [f64; 4]) -> Vec<(f32, f32)> {
        [
            (0., 0.),
            (a, 1.),
            (a + d, s),
            (a + d + self.hold, s),
            (a + d + self.hold + r, 0.),
        ]
        .map(|(x, y)| ((x / self.end) as f32, (1. - y) as f32))
        .to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_handles_follow_pointer_without_rescaling_other_segments() {
        let before = [0.1, 0.4, 0.6, 0.3];
        let axis = TimeAxis::new(before);
        let initial = axis.points(before);
        assert!((initial[4].0 - 0.8).abs() < 1e-6);
        for (parameter, node) in [(0, 1), (1, 2), (3, 4)] {
            let mut values = before;
            values[parameter] += axis.end * 0.05;
            let after = axis.points(values);
            assert!((after[node].0 - initial[node].0 - 0.05).abs() < 1e-6);
            if parameter == 3 {
                assert_eq!(
                    &after[..4],
                    &initial[..4],
                    "Release must not move earlier segments"
                );
            }
        }
        let empty = TimeAxis::new([0., 0., 0., 0.]);
        assert!(empty
            .points([0.; 4])
            .iter()
            .all(|(x, y)| x.is_finite() && y.is_finite()));
    }
}
