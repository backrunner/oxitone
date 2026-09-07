//! Shared preallocated fractional delay; writes and reads never allocate.
pub(super) struct FractionalDelay {
    data: Vec<f32>,
    position: usize,
}
impl FractionalDelay {
    pub fn new(frames: usize) -> Self {
        Self {
            data: vec![0.; frames.max(4)],
            position: 0,
        }
    }
    /// Delay >= 1; call before push. Linear interpolation is bounded and stable in feedback.
    #[inline]
    pub fn read(&self, delay: f64) -> f32 {
        let delay = delay.clamp(1., (self.data.len() - 2) as f64);
        let whole = delay as usize;
        let frac = (delay - whole as f64) as f32;
        let a = (self.position + self.data.len() - whole) % self.data.len();
        let b = (a + self.data.len() - 1) % self.data.len();
        self.data[a] + (self.data[b] - self.data[a]) * frac
    }
    #[inline]
    pub fn push(&mut self, value: f32) {
        self.data[self.position] = oxitone_dsp::ftz::flush_denormal(value);
        self.position = (self.position + 1) % self.data.len();
    }
    pub fn reset(&mut self) {
        self.data.fill(0.);
        self.position = 0;
    }
}
