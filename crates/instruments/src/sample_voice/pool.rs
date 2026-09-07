//! Heap voice pool for the heavy varispeed voices.

use super::SampleVoice;
/// Heap-allocated fixed-capacity voice pool for the heavy [`SampleVoice`]
/// (dsp's const-generic `VoicePool` would place ~450 KB voices in a stack
/// array). Allocation happens in `new` only; the steal policy mirrors
/// `oxitone_dsp::voice::VoicePool`: free slot first, otherwise quietest,
/// ties broken by oldest. RT-safe after `new`.
pub struct SampleVoicePool {
    voices: Vec<SampleVoice>,
    clock: u64,
}

impl SampleVoicePool {
    pub fn new(capacity: usize, sample_rate: f64, max_block: usize) -> Self {
        let mut voices = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            voices.push(SampleVoice::new(sample_rate, max_block));
        }
        Self { voices, clock: 0 }
    }

    pub fn reset(&mut self) {
        self.clock = 0;
        for voice in &mut self.voices {
            voice.active = false;
            voice.age = 0;
            voice.level = 0.0;
            voice.amp.reset();
        }
    }

    pub fn len(&self) -> usize {
        self.voices.len()
    }

    /// Take a free slot or steal the quietest-then-oldest active voice.
    pub fn allocate(&mut self, initial_level: f32) -> usize {
        self.clock += 1;
        let index = match self.voices.iter().position(|v| !v.active) {
            Some(i) => i,
            None => {
                let mut best = 0usize;
                for i in 1..self.voices.len() {
                    let (candidate, current) = (&self.voices[i], &self.voices[best]);
                    let quieter = candidate.level < current.level;
                    let tie_older = candidate.level == current.level && candidate.age < current.age;
                    if quieter || tie_older {
                        best = i;
                    }
                }
                best
            }
        };
        let voice = &mut self.voices[index];
        voice.active = true;
        voice.age = self.clock;
        voice.level = initial_level;
        index
    }

    pub fn release(&mut self, index: usize) {
        self.voices[index].active = false;
    }

    pub fn voice(&self, index: usize) -> &SampleVoice {
        &self.voices[index]
    }

    pub fn voice_mut(&mut self, index: usize) -> &mut SampleVoice {
        &mut self.voices[index]
    }

    pub fn active_count(&self) -> usize {
        self.voices.iter().filter(|v| v.active).count()
    }
}
