//! Uniform 256-frame FFT partitions. Plans, spectra and scratch are prepared off-thread.
//! Streaming accepts any block boundaries, with exactly one partition of latency.
const PART: usize = 256;
const SIZE: usize = PART * 2;
#[derive(Clone, Copy, Default)]
struct Complex {
    re: f32,
    im: f32,
}
impl Complex {
    fn mul(self, b: Self) -> Self {
        Self {
            re: self.re * b.re - self.im * b.im,
            im: self.re * b.im + self.im * b.re,
        }
    }
}
struct Fft {
    roots: [Complex; PART],
    reverse: [usize; SIZE],
}
impl Fft {
    fn new() -> Self {
        Self {
            roots: std::array::from_fn(|i| {
                let (s, c) = (-std::f64::consts::TAU * i as f64 / SIZE as f64).sin_cos();
                Complex {
                    re: c as f32,
                    im: s as f32,
                }
            }),
            reverse: std::array::from_fn(|i| i.reverse_bits() >> (usize::BITS - 9)),
        }
    }
    fn process(&self, data: &mut [Complex; SIZE], inverse: bool) {
        for i in 0..SIZE {
            if i < self.reverse[i] {
                data.swap(i, self.reverse[i]);
            }
        }
        let mut length = 2;
        while length <= SIZE {
            let half = length / 2;
            for start in (0..SIZE).step_by(length) {
                for j in 0..half {
                    let mut root = self.roots[j * SIZE / length];
                    if inverse {
                        root.im = -root.im;
                    }
                    let b = data[start + j + half].mul(root);
                    let a = data[start + j];
                    data[start + j] = Complex {
                        re: a.re + b.re,
                        im: a.im + b.im,
                    };
                    data[start + j + half] = Complex {
                        re: a.re - b.re,
                        im: a.im - b.im,
                    };
                }
            }
            length *= 2;
        }
        if inverse {
            for x in data {
                x.re /= SIZE as f32;
                x.im /= SIZE as f32;
            }
        }
    }
}
pub struct PartitionedConvolver {
    fft: Fft,
    impulse: Vec<[Complex; SIZE]>,
    history: Vec<[Complex; SIZE]>,
    scratch: [Complex; SIZE],
    input: [f32; PART],
    output: [f32; PART],
    overlap: [f32; PART],
    fill: usize,
    position: usize,
}
impl PartitionedConvolver {
    /// Callers validate the resource budget and finite impulse before constructing.
    pub fn new(impulse: &[f32]) -> Self {
        let fft = Fft::new();
        let mut spectra = Vec::with_capacity(impulse.len().div_ceil(PART).max(1));
        for chunk in impulse.chunks(PART) {
            let mut block = [Complex::default(); SIZE];
            for (s, x) in block.iter_mut().zip(chunk) {
                s.re = *x;
            }
            fft.process(&mut block, false);
            spectra.push(block);
        }
        if spectra.is_empty() {
            spectra.push([Complex::default(); SIZE]);
        }
        Self {
            fft,
            history: vec![[Complex::default(); SIZE]; spectra.len()],
            impulse: spectra,
            scratch: [Complex::default(); SIZE],
            input: [0.; PART],
            output: [0.; PART],
            overlap: [0.; PART],
            fill: 0,
            position: 0,
        }
    }
    #[inline]
    pub fn next(&mut self, input: f32) -> f32 {
        let out = self.output[self.fill];
        self.input[self.fill] = input;
        self.fill += 1;
        if self.fill == PART {
            self.partition();
            self.fill = 0;
        }
        out
    }
    fn partition(&mut self) {
        let current = &mut self.history[self.position];
        current.fill(Complex::default());
        for (bin, x) in current.iter_mut().zip(self.input) {
            bin.re = x;
        }
        self.fft.process(current, false);
        self.scratch.fill(Complex::default());
        for (p, impulse) in self.impulse.iter().enumerate() {
            let history =
                &self.history[(self.position + self.history.len() - p) % self.history.len()];
            for k in 0..SIZE {
                let value = impulse[k].mul(history[k]);
                self.scratch[k].re += value.re;
                self.scratch[k].im += value.im;
            }
        }
        self.fft.process(&mut self.scratch, true);
        for i in 0..PART {
            self.output[i] = crate::ftz::flush_denormal(self.scratch[i].re + self.overlap[i]);
            self.overlap[i] = crate::ftz::flush_denormal(self.scratch[PART + i].re);
        }
        self.position = (self.position + 1) % self.history.len();
    }
    pub fn latency_frames(&self) -> usize {
        PART
    }
    pub fn reset(&mut self) {
        for h in &mut self.history {
            h.fill(Complex::default());
        }
        self.input.fill(0.);
        self.output.fill(0.);
        self.overlap.fill(0.);
        self.fill = 0;
        self.position = 0;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn partitioned_matches_direct_convolution_across_multiple_partitions_and_reset() {
        let impulse: Vec<_> = (0..777)
            .map(|i| ((i as f64 * 0.83).sin() * (-0.01 * i as f64).exp()) as f32)
            .collect();
        let input: Vec<_> = (0..923)
            .map(|i| (i as f64 * 0.21).sin() as f32 * 0.2)
            .collect();
        let mut engine = PartitionedConvolver::new(&impulse);
        let mut expected = vec![0f64; input.len() + impulse.len() + PART];
        for (i, x) in input.iter().enumerate() {
            for (j, h) in impulse.iter().enumerate() {
                expected[i + j + PART] += *x as f64 * *h as f64;
            }
        }
        for _ in 0..2 {
            for (i, reference) in expected.iter().enumerate() {
                let y = engine.next(input.get(i).copied().unwrap_or(0.));
                assert!(
                    (y as f64 - reference).abs() < 2e-6,
                    "{i}: {y} != {reference}"
                );
            }
            engine.reset();
        }
    }
}
