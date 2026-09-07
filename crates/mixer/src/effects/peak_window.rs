//! Monotonic peak queue for a fixed lookahead. Storage is allocated only at prepare.
pub(super) struct PeakWindow {
    values: Vec<f32>,
    frames: Vec<u64>,
    head: usize,
    len: usize,
    frame: u64,
    delay: u64,
}
impl PeakWindow {
    pub fn new(delay: usize) -> Self {
        Self {
            values: vec![0.; delay + 2],
            frames: vec![0; delay + 2],
            head: 0,
            len: 0,
            frame: 0,
            delay: delay as u64,
        }
    }
    pub fn next(&mut self, peak: f32) -> f32 {
        let cap = self.values.len();
        while self.len > 0 && self.frame.saturating_sub(self.frames[self.head]) > self.delay {
            self.head = (self.head + 1) % cap;
            self.len -= 1;
        }
        while self.len > 0 {
            let back = (self.head + self.len - 1) % cap;
            if self.values[back] > peak {
                break;
            }
            self.len -= 1;
        }
        let index = (self.head + self.len) % cap;
        self.values[index] = peak;
        self.frames[index] = self.frame;
        self.len += 1;
        self.frame += 1;
        self.values[self.head]
    }
    pub fn reset(&mut self) {
        self.head = 0;
        self.len = 0;
        self.frame = 0;
    }
}
