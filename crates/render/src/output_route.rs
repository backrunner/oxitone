//! One auxiliary instrument output, fader staging and its independent channel-path PDC.
use oxitone_mixer::{ChannelInput, DelayLine};

pub(crate) struct OutputRoute {
    pub bus_index: usize,
    pub destination: String,
    pub left: Vec<f32>,
    pub right: Vec<f32>,
    pub delayed_l: Vec<f32>,
    pub delayed_r: Vec<f32>,
    delay: DelayLine,
}
impl OutputRoute {
    pub fn new(bus_index: usize, destination: String, max_frames: usize) -> Self {
        Self {
            bus_index,
            destination,
            left: vec![0.; max_frames],
            right: vec![0.; max_frames],
            delayed_l: vec![0.; max_frames],
            delayed_r: vec![0.; max_frames],
            delay: DelayLine::new(0, max_frames),
        }
    }
    pub fn align(&mut self, frames: usize, max_block: usize) {
        self.delay = DelayLine::new(frames, max_block);
        self.delay.set_delay(frames);
    }
    pub fn process_delay(&mut self, frames: usize) {
        self.delayed_l[..frames].fill(0.);
        self.delayed_r[..frames].fill(0.);
        self.delay.process_add(
            &self.left[..frames],
            &self.right[..frames],
            1.,
            &mut self.delayed_l[..frames],
            &mut self.delayed_r[..frames],
        );
    }
    pub fn reset(&mut self) {
        self.delay.reset();
    }
    pub fn input(&self, frames: usize) -> ChannelInput<'_> {
        ChannelInput {
            bus_id: &self.destination,
            left: &self.delayed_l[..frames],
            right: &self.delayed_r[..frames],
        }
    }
}
