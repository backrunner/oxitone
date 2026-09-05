//! RIFF/WAVE writer for offline export (06-format-and-export.md §WAV 导出).
//! Default 32-bit float; 16/24-bit PCM with optional 1 LSB TPDF dither
//! (`dither: 'none'` disables; 32-bit float is never dithered). Files
//! exceeding the RIFF 32-bit size limit fail with `WavTooLarge` — Phase 1
//! does not split RF64. The dither PRNG is seeded from the project seed so
//! identical snapshots export byte-identical files.

use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::Path;

use oxitone_core::error::{codes, OxitoneError};
use oxitone_dsp::dither::TpdfDither;

/// Export bit depth (04-api-contracts.md §RenderOptions).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WavBitDepth {
    Pcm16,
    Pcm24,
    Float32,
}

impl WavBitDepth {
    pub fn bytes_per_sample(self) -> u64 {
        match self {
            WavBitDepth::Pcm16 => 2,
            WavBitDepth::Pcm24 => 3,
            WavBitDepth::Float32 => 4,
        }
    }

    fn format_tag(self) -> u16 {
        match self {
            WavBitDepth::Float32 => 3,
            _ => 1,
        }
    }

    fn bits(self) -> u16 {
        (self.bytes_per_sample() * 8) as u16
    }
}

/// RIFF header size in bytes (RIFF/WAVE/fmt/data headers).
const HEADER_BYTES: u64 = 44;
/// RIFF chunk size field limit (file size minus 8 must fit in u32).
const RIFF_LIMIT: u64 = u32::MAX as u64;

/// Total file size check: `WavTooLarge` when the render cannot fit RIFF.
pub fn check_wav_size(frames: u64, channels: u16, depth: WavBitDepth) -> Result<(), OxitoneError> {
    let data = frames
        .checked_mul(u64::from(channels))
        .and_then(|v| v.checked_mul(depth.bytes_per_sample()))
        .ok_or_else(|| too_large("data size overflows u64"))?;
    if HEADER_BYTES + data > RIFF_LIMIT {
        return Err(too_large(format!(
            "{frames} frames x {channels} ch x {} B = {} data bytes exceeds the RIFF limit",
            depth.bytes_per_sample(),
            data
        )));
    }
    Ok(())
}

fn too_large(message: impl Into<String>) -> OxitoneError {
    OxitoneError::with_path(codes::WAV_TOO_LARGE, message, "$.path")
}

fn io_err(path: &Path, error: impl std::fmt::Display) -> OxitoneError {
    OxitoneError::with_path(
        codes::INVALID_PROJECT,
        format!("wav export I/O failed: {error}"),
        path.display().to_string(),
    )
}

/// Streaming stereo WAV writer: header placeholder up front, sizes patched
/// on `finish`.
pub struct WavWriter {
    path: std::path::PathBuf,
    writer: BufWriter<File>,
    depth: WavBitDepth,
    dither: Option<TpdfDither>,
    frames: u64,
    /// Interleave/quantize scratch, reused per block.
    interleaved: Vec<u8>,
    left: Vec<f32>,
    right: Vec<f32>,
}

impl WavWriter {
    /// Create the file (truncating) and write the placeholder header.
    /// `dither_seed` drives TPDF for 16/24-bit (`None` = `dither: 'none'`).
    pub fn create(
        path: &Path,
        sample_rate: u32,
        depth: WavBitDepth,
        dither_seed: Option<u64>,
        max_block: usize,
    ) -> Result<Self, OxitoneError> {
        let file = File::create(path).map_err(|e| io_err(path, e))?;
        let mut writer = BufWriter::new(file);
        write_header(&mut writer, sample_rate, depth, 0).map_err(|e| io_err(path, e))?;
        let dither = match depth {
            WavBitDepth::Float32 => None,
            _ => dither_seed.map(TpdfDither::new),
        };
        Ok(Self {
            path: path.to_path_buf(),
            writer,
            depth,
            dither,
            frames: 0,
            interleaved: Vec::with_capacity(max_block * 2 * depth.bytes_per_sample() as usize),
            left: Vec::with_capacity(max_block),
            right: Vec::with_capacity(max_block),
        })
    }

    /// Append one stereo block. RT-tolerant (file I/O is control-side for
    /// offline export).
    pub fn write_block(&mut self, left: &[f32], right: &[f32]) -> Result<(), OxitoneError> {
        debug_assert_eq!(left.len(), right.len());
        self.left.clear();
        self.right.clear();
        self.left.extend_from_slice(left);
        self.right.extend_from_slice(right);
        if let Some(dither) = &mut self.dither {
            let bits = self.depth.bits() as u8;
            dither.process(&mut self.left, bits);
            dither.process(&mut self.right, bits);
        }
        self.interleaved.clear();
        for i in 0..self.left.len() {
            for sample in [self.left[i], self.right[i]] {
                self.encode(sample);
            }
        }
        self.writer
            .write_all(&self.interleaved)
            .map_err(|e| io_err(&self.path, e))?;
        self.frames += left.len() as u64;
        Ok(())
    }

    fn encode(&mut self, sample: f32) {
        match self.depth {
            WavBitDepth::Float32 => self.interleaved.extend_from_slice(&sample.to_le_bytes()),
            WavBitDepth::Pcm16 => {
                let v = (sample * 32768.0).round().clamp(-32768.0, 32767.0) as i16;
                self.interleaved.extend_from_slice(&v.to_le_bytes());
            }
            WavBitDepth::Pcm24 => {
                let v = (sample * 8_388_608.0)
                    .round()
                    .clamp(-8_388_608.0, 8_388_607.0) as i32;
                self.interleaved.extend_from_slice(&v.to_le_bytes()[..3]);
            }
        }
    }

    /// Frames written so far.
    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// Patch the RIFF/fmt/data sizes and flush. Enforces the size limit.
    pub fn finish(mut self) -> Result<u64, OxitoneError> {
        check_wav_size(self.frames, 2, self.depth)?;
        let data_bytes = self.frames * 2 * self.depth.bytes_per_sample();
        self.writer.flush().map_err(|e| io_err(&self.path, e))?;
        let mut file = self
            .writer
            .into_inner()
            .map_err(|e| io_err(&self.path, format!("flush failed: {e}")))?;
        file.seek(SeekFrom::Start(0))
            .map_err(|e| io_err(&self.path, e))?;
        // Patch RIFF chunk size and data chunk size.
        file.seek(SeekFrom::Start(4))
            .map_err(|e| io_err(&self.path, e))?;
        file.write_all(&((HEADER_BYTES - 8 + data_bytes) as u32).to_le_bytes())
            .map_err(|e| io_err(&self.path, e))?;
        file.seek(SeekFrom::Start(40))
            .map_err(|e| io_err(&self.path, e))?;
        file.write_all(&(data_bytes as u32).to_le_bytes())
            .map_err(|e| io_err(&self.path, e))?;
        file.flush().map_err(|e| io_err(&self.path, e))?;
        Ok(self.frames)
    }
}

fn write_header(
    writer: &mut impl Write,
    sample_rate: u32,
    depth: WavBitDepth,
    data_bytes: u32,
) -> std::io::Result<()> {
    let channels = 2u16;
    let block_align = channels * depth.bytes_per_sample() as u16;
    let byte_rate = sample_rate * u32::from(block_align);
    writer.write_all(b"RIFF")?;
    writer.write_all(&(HEADER_BYTES as u32 - 8 + data_bytes).to_le_bytes())?;
    writer.write_all(b"WAVE")?;
    writer.write_all(b"fmt ")?;
    writer.write_all(&16u32.to_le_bytes())?;
    writer.write_all(&depth.format_tag().to_le_bytes())?;
    writer.write_all(&channels.to_le_bytes())?;
    writer.write_all(&sample_rate.to_le_bytes())?;
    writer.write_all(&byte_rate.to_le_bytes())?;
    writer.write_all(&block_align.to_le_bytes())?;
    writer.write_all(&depth.bits().to_le_bytes())?;
    writer.write_all(b"data")?;
    writer.write_all(&data_bytes.to_le_bytes())?;
    Ok(())
}
