//! oxitone-samples — sample asset decoding, non-destructive edit baking,
//! sample-rate conversion, and prepared-sample caching. Everything in this
//! crate runs on the control thread during import/prepare
//! (`.agents/docs/02-domain-spec.md` §Sample, `06-format-and-export.md`
//! §Sample 导入策略); nothing here is reachable from the audio callback.
//!
//! Pipeline: [`decode_asset`] verifies the content hash and decodes bytes to
//! non-interleaved `f32` (WAV/AIFF natively, FLAC/MP3/MP4/M4A via symphonia),
//! [`prepare`] bakes the [`SampleEditSpec`](oxitone_core::wire::SampleEditSpec)
//! (trim, level, normalize, fades) and converts to the engine sample rate,
//! and [`SampleCache`] memoizes prepared segments by
//! (content hash, edit spec hash, target rate).

pub mod cache;
mod decode;
mod edit;
mod inspect;
pub mod resample;
mod types;
pub use inspect::inspect_sample;

#[cfg(test)]
mod fixtures;

pub use cache::{prepare_cached, CacheKey, SampleCache};
pub use decode::{decode_asset, decode_bytes, load_asset, sha256_hex};
pub use edit::prepare;
pub use types::{ChannelLayoutAction, DecodedSample, LoopPoints, PreparedSample, SampleMetadata};

/// Stable error code for assets whose container, codec, bit depth, or channel
/// configuration the importer cannot represent (`channels: 1|2` contract in
/// `04-api-contracts.md`). Reported with the asset URI as `path`.
pub use oxitone_core::error::codes::SAMPLE_FORMAT_UNSUPPORTED;
