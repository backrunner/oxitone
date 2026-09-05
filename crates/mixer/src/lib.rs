//! oxitone-mixer — mixer bus processing core and the Phase 1 built-in effect
//! plugins (02-domain-spec.md §Mixer 与 routing, 03-audio-runtime-spec.md
//! §DSP 处理顺序/PDC, 08-plugin-abi.md). Effects implement the
//! `oxitone-graph` Plugin ABI v1 traits; the mixer engine consumes
//! `build_mixer_routing` topological order for deterministic summation,
//! pre/post-fader sends, sidechain detector routing and plugin delay
//! compensation.

pub mod bus;
pub mod effects;
pub mod meter;
pub mod pdc;

pub use bus::{ChannelInput, MixerEngine};
pub use effects::builtin_effect_plugins;
pub use meter::{BusMeter, TruePeakMeter};
pub use pdc::{plan_pdc, DelayLine, PdcPlan};
