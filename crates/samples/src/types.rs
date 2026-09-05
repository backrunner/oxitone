//! Shared data types for the decode → prepare → cache pipeline.

use oxitone_core::wire::SampleFormat;
use oxitone_core::Beat;

/// Loop region in frames, half-open `[start_frame, end_frame)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LoopPoints {
    pub start_frame: u64,
    pub end_frame: u64,
}

/// How the importer reconciled the source channel count with the
/// `channels: 1|2` contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChannelLayoutAction {
    Kept,
    DownmixedToStereo,
}

/// Provenance recorded at import time (`06-format-and-export.md` §Sample
/// 导入策略). The realtime path never re-reads the source asset.
#[derive(Debug, Clone, PartialEq)]
pub struct SampleMetadata {
    /// Lowercase hex SHA-256 of the original asset bytes.
    pub sha256: String,
    pub format: SampleFormat,
    pub source_sample_rate: u32,
    pub source_channels: u8,
    pub source_bit_depth: Option<u8>,
    pub channel_layout_action: ChannelLayoutAction,
    /// Decoder name/version for compressed formats (e.g. `symphonia 0.5/aac`);
    /// `None` for the internal WAV/AIFF decoders.
    pub decoder: Option<String>,
}

/// Decoded PCM, non-interleaved `f32`, 1 or 2 channels at the source rate.
#[derive(Debug, Clone)]
pub struct DecodedSample {
    pub channels: Vec<Vec<f32>>,
    pub sample_rate: u32,
    /// Loop points from the source container (WAV `smpl`), if present.
    pub loop_points: Option<LoopPoints>,
    pub metadata: SampleMetadata,
}

impl DecodedSample {
    pub fn frames(&self) -> u64 {
        self.channels.first().map_or(0, |c| c.len() as u64)
    }
}

/// Immutable prepared PCM segment at the engine sample rate: edits baked,
/// SRC applied. Safe to share into a compiled render graph.
#[derive(Debug, Clone)]
pub struct PreparedSample {
    pub channels: Vec<Vec<f32>>,
    /// Engine (target) sample rate.
    pub sample_rate: u32,
    /// Loop points remapped into prepared-frame coordinates, if still valid.
    pub loop_points: Option<LoopPoints>,
    /// Declared musical length of the source material; fit/stretch base.
    pub musical_length_beats: Option<Beat>,
    pub metadata: SampleMetadata,
}

impl PreparedSample {
    pub fn frames(&self) -> u64 {
        self.channels.first().map_or(0, |c| c.len() as u64)
    }
}
