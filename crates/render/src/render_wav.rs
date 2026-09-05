//! Offline WAV export (06-format-and-export.md §WAV 导出,
//! 04-api-contracts.md §RenderOptions/RenderReport). Drives the same
//! `RenderGraph`/block renderer used for playback — only the output sink
//! and clock differ (03-audio-runtime-spec.md §Offline parity). Stems tap
//! the per-bus master-route contributions of one shared render pass;
//! loudness is computed on the same blocks that are written.

use std::path::{Path, PathBuf};

use oxitone_core::error::{codes, OxitoneError};
use oxitone_core::pcg32::{hash64, Hash64Part};
use oxitone_core::wire::ProjectSnapshot;
use oxitone_graph::compile::CompileOptions;
use oxitone_graph::PluginRegistry;

use crate::assets::SampleStore;
use crate::build::RenderGraphOptions;
use crate::graph::RenderGraph;
use crate::loudness::FileMeter;
use crate::transport::{resolve_range, RangePoint};
use crate::wav::{check_wav_size, WavBitDepth, WavWriter};

/// Render range endpoint (`bar`/`beat`/timecode/`marker`, 四选一).
pub type RenderPoint = RangePoint;

/// Export bit depth (`float32` default).
pub type BitDepth = WavBitDepth;

/// Dither mode for 16/24-bit export (default TPDF).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DitherMode {
    Tpdf,
    None,
}

/// Stem grouping (default `None`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StemMode {
    None,
    MixerChannels,
    Tracks,
}

/// `renderWav` options (04-api-contracts.md §RenderOptions).
#[derive(Debug, Clone)]
pub struct RenderOptions {
    /// Output file (`stems: none`) or output directory (stems).
    pub path: PathBuf,
    pub start: Option<RenderPoint>,
    pub end: Option<RenderPoint>,
    pub sample_rate: Option<u32>,
    pub block_size: Option<u32>,
    pub tail_seconds: Option<f64>,
    pub respect_solo: bool,
    pub seed: Option<u64>,
    pub bit_depth: BitDepth,
    pub dither: DitherMode,
    pub stems: StemMode,
    pub include_metronome: bool,
    /// Click level when `include_metronome` is set (default 0.5).
    pub metronome_level: Option<f32>,
    /// Master limiter/clip protection (default on).
    pub master_limiter: bool,
    /// Host `setParameter` events applied to the compiled graph before the
    /// render (04-api-contracts.md §Wire messages: events take effect when
    /// the offline render processes their frame).
    pub parameter_events: Vec<crate::params::ParameterEventInput>,
}

impl RenderOptions {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            start: None,
            end: None,
            sample_rate: None,
            block_size: None,
            tail_seconds: None,
            respect_solo: false,
            seed: None,
            bit_depth: BitDepth::Float32,
            dither: DitherMode::Tpdf,
            stems: StemMode::None,
            include_metronome: false,
            metronome_level: None,
            master_limiter: true,
            parameter_events: Vec::new(),
        }
    }
}

/// Per-file loudness report entry.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderFileReport {
    pub path: String,
    /// Stem entity ID; `None` for the master file.
    pub stem: Option<String>,
    pub duration_seconds: f64,
    pub peak_dbfs: f64,
    pub true_peak_dbfs: f64,
    pub integrated_lufs: f64,
}

/// `renderWav` report (04-api-contracts.md §RenderReport).
#[derive(Debug, Clone, PartialEq)]
pub struct RenderReport {
    pub files: Vec<RenderFileReport>,
    pub graph_latency_frames: u64,
}

fn io_err(path: &Path, message: impl Into<String>) -> OxitoneError {
    OxitoneError::with_path(
        codes::INVALID_PROJECT,
        message.into(),
        path.display().to_string(),
    )
}

/// Filesystem-safe stem file name.
fn sanitize(stem: &str) -> String {
    stem.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// One output file being rendered.
struct Output {
    stem: Option<String>,
    /// Buses tapped for this file; empty = master (final limited output).
    buses: Vec<String>,
    path: PathBuf,
    writer: WavWriter,
    meter: FileMeter,
    sum_l: Vec<f32>,
    sum_r: Vec<f32>,
}

/// Compile the graph and export WAV file(s).
pub fn render_wav(
    snapshot: &ProjectSnapshot,
    registry: &PluginRegistry,
    samples: &SampleStore,
    options: &RenderOptions,
) -> Result<RenderReport, OxitoneError> {
    let sample_rate = options.sample_rate.unwrap_or(snapshot.sample_rate);
    let block_size = options.block_size.unwrap_or(snapshot.block_size);
    let seed = options.seed.unwrap_or(snapshot.seed);
    let tail_seconds = options.tail_seconds.unwrap_or(0.0).max(0.0);

    let graph_options = RenderGraphOptions {
        compile: CompileOptions {
            sample_rate: options.sample_rate,
            block_size: options.block_size,
            seed: options.seed,
            tail_seconds: Some(tail_seconds),
        },
        master_limiter: options.master_limiter,
        respect_solo: options.respect_solo,
        metronome_level: options
            .include_metronome
            .then_some(options.metronome_level.unwrap_or(0.5)),
    };
    let mut graph = RenderGraph::compile(snapshot, registry, samples, &graph_options)?;
    for event in &options.parameter_events {
        graph.enqueue_parameter(event)?;
    }
    let (start, end) = resolve_range(
        options.start.as_ref(),
        options.end.as_ref(),
        graph.plan(),
        &snapshot.markers,
    )?;
    let tail = (tail_seconds * f64::from(sample_rate) + 0.5).floor() as u64;
    let total = end - start + tail;
    check_wav_size(total, 2, options.bit_depth)?;

    let block = block_size as usize;
    let dither_seed = |index: u64| match options.dither {
        DitherMode::None => None,
        DitherMode::Tpdf => Some(hash64(&[
            Hash64Part::Int(seed),
            Hash64Part::Str("oxitone.render.dither".to_string()),
            Hash64Part::Int(index),
        ])),
    };

    // Output list: master first, then stems in deterministic order.
    if options.stems == StemMode::Tracks {
        std::fs::create_dir_all(&options.path).map_err(|e| io_err(&options.path, e.to_string()))?;
        let mut pass = options.clone();
        pass.stems = StemMode::None;
        pass.start = Some(RangePoint::Frames(start));
        pass.end = Some(RangePoint::Frames(end));
        pass.path = options.path.join("master.wav");
        let mut report = render_wav(snapshot, registry, samples, &pass)?;
        let mut tracks: Vec<_> = snapshot
            .tracks
            .iter()
            .filter(|t| t.enabled != Some(false))
            .collect();
        tracks.sort_by(|a, b| a.id.cmp(&b.id));
        for track in tracks {
            let mut isolated = snapshot.clone();
            isolated.tempo_map = oxitone_graph::compile::effective_tempo_table(
                snapshot,
                sample_rate,
                seed,
                tail_seconds,
            )?;
            isolated.automation.retain(|lane| {
                lane.target.entity_id != snapshot.id || lane.target.parameter_id != "tempo"
            });
            for other in &mut isolated.tracks {
                other.enabled = Some(other.id == track.id);
            }
            pass.path = options.path.join(format!("{}.wav", sanitize(&track.id)));
            let mut stem = render_wav(&isolated, registry, samples, &pass)?;
            for file in &mut stem.files {
                file.stem = Some(track.id.clone());
            }
            report.files.extend(stem.files);
        }
        return Ok(report);
    }
    let mut specs: Vec<(Option<String>, Vec<String>, PathBuf)> = Vec::new();
    match options.stems {
        StemMode::None => specs.push((None, Vec::new(), options.path.clone())),
        mode => {
            std::fs::create_dir_all(&options.path).map_err(|e| {
                io_err(
                    &options.path,
                    format!("cannot create output directory: {e}"),
                )
            })?;
            specs.push((None, Vec::new(), options.path.join("master.wav")));
            match mode {
                StemMode::MixerChannels => {
                    for bus in graph.mixer().bus_order() {
                        if bus == oxitone_graph::MASTER_MIXER_CHANNEL_ID {
                            continue;
                        }
                        specs.push((
                            Some(bus.clone()),
                            vec![bus.clone()],
                            options.path.join(format!("{}.wav", sanitize(&bus))),
                        ));
                    }
                }
                StemMode::Tracks => {
                    let mut tracks: Vec<&_> = snapshot
                        .tracks
                        .iter()
                        .filter(|t| t.enabled != Some(false))
                        .collect();
                    tracks.sort_by(|a, b| a.id.cmp(&b.id));
                    for track in tracks {
                        specs.push((
                            Some(track.id.clone()),
                            graph.buses_of_track(&track.id),
                            options.path.join(format!("{}.wav", sanitize(&track.id))),
                        ));
                    }
                }
                StemMode::None => unreachable!(),
            }
        }
    }

    let mut outputs = Vec::with_capacity(specs.len());
    for (index, (stem, buses, path)) in specs.into_iter().enumerate() {
        outputs.push(Output {
            stem,
            buses,
            writer: WavWriter::create(
                &path,
                sample_rate,
                options.bit_depth,
                dither_seed(index as u64),
                block,
            )?,
            meter: FileMeter::new(sample_rate, block),
            path,
            sum_l: vec![0.0; block],
            sum_r: vec![0.0; block],
        });
    }
    if options.stems != StemMode::None {
        graph.mixer_mut().enable_stem_taps(true);
    }

    graph.seek(start);
    graph.transport.begin_render(start);

    let mut out_l = vec![0.0f32; block];
    let mut out_r = vec![0.0f32; block];
    let mut written = 0_u64;
    while written < total {
        let n = ((total - written) as usize).min(block);
        graph.process_block(&mut out_l, &mut out_r);
        let (metro_l, metro_r) = graph.metronome_output();
        for output in &mut outputs {
            if output.buses.is_empty() {
                output.meter.add_block(&out_l[..n], &out_r[..n]);
                output.writer.write_block(&out_l[..n], &out_r[..n])?;
            } else {
                for slot in &mut output.sum_l[..n] {
                    *slot = 0.0;
                }
                for slot in &mut output.sum_r[..n] {
                    *slot = 0.0;
                }
                for bus in &output.buses {
                    if let Some((tap_l, tap_r)) = graph.mixer().stem_output(bus) {
                        for i in 0..n {
                            output.sum_l[i] += tap_l[i];
                            output.sum_r[i] += tap_r[i];
                        }
                    }
                }
                if options.include_metronome {
                    for i in 0..n {
                        output.sum_l[i] += metro_l[i];
                        output.sum_r[i] += metro_r[i];
                    }
                }
                output
                    .meter
                    .add_block(&output.sum_l[..n], &output.sum_r[..n]);
                output
                    .writer
                    .write_block(&output.sum_l[..n], &output.sum_r[..n])?;
            }
        }
        written += n as u64;
    }

    let mut files = Vec::with_capacity(outputs.len());
    for output in outputs {
        let frames = output.writer.frames();
        let (peak_dbfs, true_peak_dbfs, integrated_lufs) = output.meter.finish();
        output.writer.finish()?;
        files.push(RenderFileReport {
            path: output.path.display().to_string(),
            stem: output.stem,
            duration_seconds: frames as f64 / f64::from(sample_rate),
            peak_dbfs,
            true_peak_dbfs,
            integrated_lufs,
        });
    }
    Ok(RenderReport {
        files,
        graph_latency_frames: graph.graph_latency_frames(),
    })
}
