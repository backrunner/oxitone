//! oxitone-dsp — allocation-free DSP primitives for the realtime engine (M2).
//!
//! Every `process`/`next`/`render` path in this crate is realtime-safe per
//! `.agents/docs/03-audio-runtime-spec.md`: no heap allocation, no locks, no
//! logging, no syscalls. All buffers and tables are created by `new`/`prepare`
//! on a control thread. Audio buffers are non-interleaved `f32`; filter
//! coefficients are computed in `f64`; low-cutoff biquad state and all phase
//! accumulators are `f64`.

pub mod biquad;
pub mod convolution;
pub mod dither;
pub mod envelope;
pub mod ftz;
pub mod gain_pan;
pub mod meter;
pub mod oscillator;
pub mod resample;
pub mod unison;
pub mod voice;
pub mod wsola;
