//! Versioned control messages, intentionally separate from the block processor.
use crate::engine::{invalid, Engine};
use oxitone_core::{
    wire::{ProjectSnapshot, SampleFormat},
    Beat, OxitoneError,
};
use oxitone_render::{
    midi::{export_midi, MidiExportOptions},
    wav::{WavBitDepth, WavWriter},
    ParameterEventInput,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::Cursor;
use std::path::Path;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    protocol_version: String,
    #[serde(flatten)]
    command: Command,
}

#[derive(Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum Command {
    Compile {
        snapshot: ProjectSnapshot,
    },
    ImportSample {
        format: SampleFormat,
    },
    State,
    Transport {
        command: String,
        frame: Option<String>,
        loop_region: Option<[String; 2]>,
    },
    SetParameter {
        entity_id: String,
        parameter_id: String,
        value: f64,
        at_frame: Option<String>,
    },
    RenderWav {
        frames: u32,
        start_frame: Option<String>,
        bit_depth: Option<u8>,
        dither: Option<bool>,
    },
    ExportMidi {
        #[serde(default)]
        options: MidiExportOptions,
    },
    ResolveBeatDuration {
        snapshot: ProjectSnapshot,
        start_beat: Beat,
        duration_seconds: f64,
    },
    Dispose,
}

fn frame(value: Option<String>) -> Result<Option<u64>, OxitoneError> {
    value
        .map(|v| {
            let frame = v
                .parse::<u64>()
                .map_err(|_| invalid("frame must be an unsigned decimal string"))?;
            if frame > 9_007_199_254_740_991 {
                return Err(invalid("frame must be between zero and 2^53-1"));
            }
            Ok(frame)
        })
        .transpose()
}

impl Engine {
    pub fn command(&mut self, request: Request, bytes: &[u8]) -> Result<Value, OxitoneError> {
        oxitone_core::version::check_protocol_version(&request.protocol_version)?;
        match request.command {
            Command::Compile { snapshot } => self.compile(snapshot),
            Command::ImportSample { format } => {
                let decoded = oxitone_samples::decode_bytes(bytes, format)?;
                let info = json!({ "sha256": decoded.metadata.sha256, "format": format,
                    "sampleRate": decoded.sample_rate, "channels": decoded.channels.len(),
                    "frames": decoded.frames().to_string() });
                self.assets.insert(decoded.metadata.sha256.clone(), decoded);
                Ok(info)
            }
            Command::State => self.state(),
            Command::Transport {
                command,
                frame: input,
                loop_region,
            } => {
                let position = frame(input)?;
                let region = loop_region
                    .map(|[a, b]| -> Result<_, OxitoneError> {
                        let a = frame(Some(a))?.unwrap();
                        let b = frame(Some(b))?.unwrap();
                        if b <= a {
                            return Err(invalid("loop end must be after start"));
                        }
                        Ok((a, b))
                    })
                    .transpose()?;
                let graph = self.graph_mut()?;
                match command.as_str() {
                    "play" => {
                        let at = position.unwrap_or(graph.transport().cursor);
                        if position.is_some() {
                            graph.seek(at);
                        }
                        graph.transport_mut().play_from(at, region);
                    }
                    "pause" => graph.transport_mut().pause(),
                    "stop" => {
                        graph.seek(0);
                        graph.transport_mut().stop();
                    }
                    "seek" => graph.seek(position.ok_or_else(|| invalid("seek requires frame"))?),
                    _ => return Err(invalid("unknown transport command")),
                }
                self.state()
            }
            Command::SetParameter {
                entity_id,
                parameter_id,
                value,
                at_frame,
            } => {
                self.graph_mut()?.enqueue_parameter(&ParameterEventInput {
                    entity_id,
                    parameter_id,
                    value,
                    at_frame: frame(at_frame)?,
                })?;
                Ok(Value::Null)
            }
            Command::RenderWav {
                frames,
                start_frame,
                bit_depth,
                dither,
            } => self.wav(
                frames,
                frame(start_frame)?.unwrap_or(0),
                bit_depth.unwrap_or(32),
                dither.unwrap_or(true),
            ),
            Command::ExportMidi { options } => {
                let snapshot = self
                    .snapshot
                    .as_ref()
                    .ok_or_else(|| invalid("compile a project first"))?;
                let exported = export_midi(snapshot, &options).map_err(|e| e.error)?;
                self.binary = exported.bytes;
                Ok(json!({ "bytes": self.binary.len(), "diagnostics": exported.diagnostics }))
            }
            Command::ResolveBeatDuration {
                snapshot,
                start_beat,
                duration_seconds,
            } => {
                oxitone_core::version::check_protocol_version(&snapshot.protocol_version)?;
                let beats = oxitone_graph::compile::resolve_beat_duration(
                    &snapshot,
                    start_beat,
                    duration_seconds,
                )?;
                Ok(json!(beats))
            }
            Command::Dispose => {
                *self = Engine::new()?;
                Ok(Value::Null)
            }
        }
    }

    fn wav(
        &mut self,
        frames: u32,
        start: u64,
        bits: u8,
        dither: bool,
    ) -> Result<Value, OxitoneError> {
        let depth = match bits {
            16 => WavBitDepth::Pcm16,
            24 => WavBitDepth::Pcm24,
            32 => WavBitDepth::Float32,
            _ => return Err(invalid("bitDepth must be 16, 24 or 32")),
        };
        if frames == 0 || u64::from(frames) * 2 * depth.bytes_per_sample() > 256 * 1024 * 1024 {
            return Err(invalid(
                "Wasm WAV export must contain 1 frame through 256 MiB of PCM",
            ));
        }
        start
            .checked_add(u64::from(frames))
            .ok_or_else(|| invalid("render frame overflow"))?;
        let snapshot = self
            .snapshot
            .as_ref()
            .ok_or_else(|| invalid("compile a project first"))?;
        let mut graph = self.build(snapshot, frames as f64 / snapshot.sample_rate as f64)?;
        let mut writer = WavWriter::from_writer(
            Cursor::new(Vec::new()),
            Path::new("memory.wav"),
            snapshot.sample_rate,
            depth,
            dither.then_some(snapshot.seed),
            graph.block_size(),
        )?;
        let mut left = vec![0.0; graph.block_size()];
        let mut right = vec![0.0; graph.block_size()];
        graph.seek(start);
        graph.transport_mut().begin_render(start);
        let mut remaining = frames as usize;
        while remaining > 0 {
            let count = remaining.min(left.len());
            graph.process_block(&mut left, &mut right);
            if graph.faulted() {
                return Err(invalid("render graph faulted"));
            }
            writer.write_block(&left[..count], &right[..count])?;
            remaining -= count;
        }
        self.binary = writer.into_inner()?.into_inner();
        Ok(json!({ "bytes": self.binary.len(), "frames": frames.to_string() }))
    }
}
