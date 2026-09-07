//! oxitone-napi — thin versioned bridge only. No business logic: JSON in,
//! JSON out; validation lives in oxitone-core. Errors cross the boundary as
//! JSON-serialized `OxitoneError` (`{code, message, path?}`) in the JS
//! Error message; the TypeScript facade parses that structure and must never
//! branch on human-readable text. Panics are caught and mapped to a
//! structured `RealtimeFault` error so no Rust panic reaches JS.

use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};

use napi::{Error, Status};
use napi_derive::napi;
use oxitone_core::wire::{
    decode_project_snapshot, encode_project_snapshot, EngineOptions, NativeCommand, RenderPosition,
    Timecode,
};
use oxitone_core::{codes, OxitoneError, PROTOCOL_VERSION};
use oxitone_render::realtime::{RealtimeConfig, RealtimeSession, TransportCmd};
use oxitone_render::{
    builtin_registry, render_wav, resolve_parameter_event, ParamTargetIndex, ParameterEventInput,
    RenderGraph, RenderGraphOptions, SampleStore, TransportState,
};
use serde::Serialize;

mod playback;
mod plugins;
mod samples;
mod timing;
pub use plugins::{get_plugin_diagnostics, get_plugin_info, register_plugin};
pub use samples::inspect_sample;
pub use timing::resolve_beat_duration;

struct EngineState {
    plugins: oxitone_graph::PluginRegistry,
    dynamic_plugins: std::collections::BTreeMap<
        (String, String),
        std::sync::Arc<oxitone_render::plugins::CPlugin>,
    >,
    options: Option<EngineOptions>,
    last_revision: Option<u64>,
    /// Graph compiled by `compile`; transport and `setParameter` act on it.
    /// Offline (`renderWav`) recompiles from the passed snapshot but
    /// inherits the queued parameter events (see `params.rs` in
    /// `oxitone-render`). While a realtime session is running the graph is
    /// owned by the session's render worker and this is `None`.
    graph: Option<RenderGraph>,
    /// Live realtime playback session (created by the first `play`).
    session: Option<RealtimeSession>,
    /// Immutable parameter-target snapshot matching the session's graph;
    /// keeps `setParameter` validation synchronous while the graph lives
    /// on the render worker.
    param_index: Option<ParamTargetIndex>,
    parameter_events: Vec<ParameterEventInput>,
}

fn registry() -> &'static Mutex<HashMap<String, EngineState>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, EngineState>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock_registry() -> MutexGuard<'static, HashMap<String, EngineState>> {
    registry().lock().unwrap_or_else(|e| e.into_inner())
}

static NEXT_ENGINE: AtomicU64 = AtomicU64::new(1);

#[derive(Serialize)]
struct WireError<'a> {
    code: &'a str,
    message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<&'a str>,
}

fn to_napi_error(err: &OxitoneError) -> Error {
    let wire = WireError {
        code: err.code,
        message: &err.message,
        path: err.path.as_deref(),
    };
    let message = serde_json::to_string(&wire).unwrap_or_else(|_| {
        format!(
            "{{\"code\":\"{}\",\"message\":\"serialization failed\"}}",
            err.code
        )
    });
    Error::new(Status::GenericFailure, message)
}

fn internal_fault(message: &str) -> Error {
    to_napi_error(&OxitoneError::new(codes::REALTIME_FAULT, message))
}

fn guarded<F>(f: F) -> napi::Result<String>
where
    F: FnOnce() -> Result<String, OxitoneError>,
{
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(result) => result.map_err(|e| to_napi_error(&e)),
        Err(_) => Err(internal_fault("internal native panic")),
    }
}

#[napi(js_name = "createEngine")]
pub fn create_engine(options_json: Option<String>) -> napi::Result<String> {
    guarded(|| {
        let options = match options_json {
            Some(json) => Some(serde_json::from_str::<EngineOptions>(&json).map_err(|e| {
                OxitoneError::new(
                    codes::INVALID_PROJECT,
                    format!("invalid engine options: {e}"),
                )
            })?),
            None => None,
        };
        let id = format!("eng_{:016x}", NEXT_ENGINE.fetch_add(1, Ordering::Relaxed));
        let audio_backend = options
            .as_ref()
            .and_then(|options| options.audio_backend)
            .unwrap_or(oxitone_core::wire::AudioBackend::Device);
        lock_registry().insert(
            id.clone(),
            EngineState {
                plugins: builtin_registry()?,
                dynamic_plugins: Default::default(),
                options,
                last_revision: None,
                graph: None,
                session: None,
                param_index: None,
                parameter_events: Vec::new(),
            },
        );
        Ok(serde_json::json!({
            "engineId": id,
            "protocolVersion": PROTOCOL_VERSION,
            "audioBackend": audio_backend,
        })
        .to_string())
    })
}

fn unknown_engine(engine_id: &str) -> OxitoneError {
    OxitoneError::new(
        codes::INVALID_PROJECT,
        format!("unknown engine id: {engine_id}"),
    )
}

fn not_compiled(engine_id: &str) -> OxitoneError {
    OxitoneError::new(
        codes::INVALID_PROJECT,
        format!("engine {engine_id} has no compiled graph; call compile first"),
    )
}

/// `RenderGraphOptions` honoring the engine's `EngineOptions` overrides
/// (sample rate, block size, metronome).
fn graph_options(options: Option<&EngineOptions>) -> RenderGraphOptions {
    let mut graph_options = RenderGraphOptions::default();
    if let Some(options) = options {
        graph_options.compile.sample_rate = options.sample_rate;
        graph_options.compile.block_size = options.block_size;
        if let Some(metronome) = &options.metronome {
            graph_options.metronome_level = metronome
                .enabled
                .then_some(metronome.level.unwrap_or(0.5) as f32);
        }
    }
    graph_options
}

pub fn compile(engine_id: String, snapshot_json: String) -> napi::Result<String> {
    compile_with_options(engine_id, snapshot_json, None)
}

#[napi(js_name = "compile")]
pub fn compile_with_options(
    engine_id: String,
    snapshot_json: String,
    compile_options_json: Option<String>,
) -> napi::Result<String> {
    guarded(|| {
        let snapshot = decode_project_snapshot(&snapshot_json)?;
        #[derive(serde::Deserialize, Default)]
        #[serde(rename_all = "camelCase")]
        struct CompileOptions {
            asset_base_dir: Option<String>,
        }
        let compile_options = match compile_options_json {
            Some(json) => serde_json::from_str::<CompileOptions>(&json).map_err(|e| {
                OxitoneError::new(
                    codes::INVALID_PROJECT,
                    format!("invalid compile options: {e}"),
                )
            })?,
            None => CompileOptions::default(),
        };
        if compile_options
            .asset_base_dir
            .as_ref()
            .is_some_and(|path| path.is_empty() || path.contains('\0'))
        {
            return Err(OxitoneError::with_path(
                codes::INVALID_PROJECT,
                "assetBaseDir must be a nonempty local path",
                "assetBaseDir",
            ));
        }
        let (options, plugins) = {
            let engines = lock_registry();
            let engine = engines
                .get(&engine_id)
                .ok_or_else(|| unknown_engine(&engine_id))?;
            (engine.options.clone(), engine.plugins.clone())
        };
        // Compile outside the registry lock; the graph is heavy and other
        // engines must stay usable. Relative sample assets resolve against
        // the explicit project root, or the process working directory.
        let mut graph = RenderGraph::compile(
            &snapshot,
            &plugins,
            &SampleStore::new(compile_options.asset_base_dir.map(PathBuf::from)),
            &graph_options(options.as_ref()),
        )?;
        let mut engines = lock_registry();
        let engine = engines
            .get_mut(&engine_id)
            .ok_or_else(|| unknown_engine(&engine_id))?;
        let param_index = ParamTargetIndex::from_graph(&graph);
        if let Some(session) = engine.session.as_ref() {
            session.replace_graph(Box::new(graph))?;
            engine.graph = None;
        } else {
            if let Some(previous) = engine.graph.as_ref() {
                graph.seek(previous.transport().cursor);
                graph.transport_mut().state = previous.transport().state;
                graph.transport_mut().loop_region = previous.transport().loop_region;
            }
            engine.graph = Some(graph);
        }
        engine.last_revision = Some(snapshot.revision);
        engine.param_index = Some(param_index);
        engine.parameter_events.clear();
        encode_project_snapshot(&snapshot)
    })
}

#[napi(js_name = "dispose")]
pub fn dispose(engine_id: String) -> napi::Result<()> {
    match catch_unwind(AssertUnwindSafe(|| lock_registry().remove(&engine_id))) {
        Ok(_) => Ok(()),
        Err(_) => Err(internal_fault("internal native panic")),
    }
}

#[napi(js_name = "getProtocolVersion")]
pub fn get_protocol_version() -> String {
    PROTOCOL_VERSION.to_string()
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = u32::from(*chunk.get(1).unwrap_or(&0));
        let b2 = u32::from(*chunk.get(2).unwrap_or(&0));
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(BASE64[(n >> 18) as usize & 0x3F] as char);
        out.push(BASE64[(n >> 12) as usize & 0x3F] as char);
        out.push(if chunk.len() > 1 {
            BASE64[(n >> 6) as usize & 0x3F] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            BASE64[n as usize & 0x3F] as char
        } else {
            '='
        });
    }
    out
}

/// Temp file + fsync + rename, mirroring the project-file write rule in
/// `.agents/docs/06-format-and-export.md`.
fn write_file_atomic(path: &str, bytes: &[u8]) -> Result<(), OxitoneError> {
    use std::io::Write;
    let io_err = |context: &str, e: std::io::Error| {
        OxitoneError::with_path(
            codes::ASSET_UNAVAILABLE,
            format!("{context}: {e}"),
            path.to_string(),
        )
    };
    let tmp = format!("{path}.oxitone-tmp");
    let result = (|| {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&tmp, path)
    })();
    result.map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        io_err("failed to write MIDI file", e)
    })
}

fn midi_export_fault(err: oxitone_render::midi::MidiExportError) -> Error {
    if err.unassigned_track_ids.is_empty() {
        return to_napi_error(&err.error);
    }
    let wire = serde_json::json!({
        "code": err.error.code,
        "message": err.error.message,
        "path": err.error.path,
        "details": { "unassignedTrackIds": err.unassigned_track_ids },
    });
    Error::new(Status::GenericFailure, wire.to_string())
}

#[napi(js_name = "exportMidi")]
pub fn export_midi(
    engine_id: String,
    snapshot_json: String,
    options_json: Option<String>,
) -> napi::Result<String> {
    match catch_unwind(AssertUnwindSafe(|| {
        let snapshot = decode_project_snapshot(&snapshot_json).map_err(|e| to_napi_error(&e))?;
        let options = match options_json {
            Some(json) => serde_json::from_str::<oxitone_render::midi::MidiExportOptions>(&json)
                .map_err(|e| {
                    to_napi_error(&OxitoneError::new(
                        codes::INVALID_PROJECT,
                        format!("invalid MIDI export options: {e}"),
                    ))
                })?,
            None => oxitone_render::midi::MidiExportOptions::default(),
        };
        if !lock_registry().contains_key(&engine_id) {
            return Err(to_napi_error(&OxitoneError::new(
                codes::INVALID_PROJECT,
                format!("unknown engine id: {engine_id}"),
            )));
        }
        let result =
            oxitone_render::midi::export_midi(&snapshot, &options).map_err(midi_export_fault)?;
        let mut response = serde_json::json!({ "diagnostics": result.diagnostics });
        if let Some(path) = &options.path {
            write_file_atomic(path, &result.bytes).map_err(|e| to_napi_error(&e))?;
            response["path"] = serde_json::json!(path);
            response["bytes"] = serde_json::json!(result.bytes.len());
        } else {
            response["bytesBase64"] = serde_json::json!(base64_encode(&result.bytes));
        }
        Ok(response.to_string())
    })) {
        Ok(result) => result,
        Err(_) => Err(internal_fault("internal native panic")),
    }
}

fn to_range_point(position: &RenderPosition) -> oxitone_render::RenderPoint {
    match position {
        RenderPosition::Bar { bar } => oxitone_render::RenderPoint::Bar(*bar),
        RenderPosition::Beat { beat } => oxitone_render::RenderPoint::Beat(beat.to_f64()),
        RenderPosition::Timecode(Timecode::Seconds { seconds }) => {
            oxitone_render::RenderPoint::Seconds(*seconds)
        }
        RenderPosition::Timecode(Timecode::Frames { frames }) => {
            oxitone_render::RenderPoint::Frames(*frames)
        }
        RenderPosition::Marker { marker } => oxitone_render::RenderPoint::Marker(marker.clone()),
    }
}

fn parse_render_options(json: &str) -> Result<oxitone_core::wire::RenderOptions, OxitoneError> {
    serde_json::from_str(json).map_err(|e| {
        OxitoneError::new(
            codes::INVALID_PROJECT,
            format!("invalid render options: {e}"),
        )
    })
}

/// Loudness values can be `-inf` for digital silence or very short files;
/// JSON (and the protocol schema) only carry finite numbers, so non-finite
/// results are reported as the -144 dB floor.
fn finite_db(value: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        -144.0
    }
}

#[napi(js_name = "renderWav")]
pub fn render_wav_command(
    engine_id: String,
    snapshot_json: String,
    options_json: String,
) -> napi::Result<String> {
    guarded(|| {
        let snapshot = decode_project_snapshot(&snapshot_json)?;
        let options = parse_render_options(&options_json)?;
        let (parameter_events, plugins) = {
            let engines = lock_registry();
            let engine = engines
                .get(&engine_id)
                .ok_or_else(|| unknown_engine(&engine_id))?;
            (engine.parameter_events.clone(), engine.plugins.clone())
        };
        let mut render_options = oxitone_render::RenderOptions::new(PathBuf::from(&options.path));
        render_options.start = options.start.as_ref().map(to_range_point);
        render_options.end = options.end.as_ref().map(to_range_point);
        render_options.sample_rate = options.sample_rate;
        render_options.block_size = options.block_size;
        render_options.tail_seconds = options.tail_seconds;
        render_options.respect_solo = options.respect_solo.unwrap_or(false);
        render_options.seed = options.seed;
        render_options.bit_depth = match options.bit_depth {
            Some(oxitone_core::wire::BitDepth::Pcm16) => oxitone_render::BitDepth::Pcm16,
            Some(oxitone_core::wire::BitDepth::Pcm24) => oxitone_render::BitDepth::Pcm24,
            Some(oxitone_core::wire::BitDepth::Float32) | None => oxitone_render::BitDepth::Float32,
        };
        render_options.dither = match options.dither {
            Some(oxitone_core::wire::Dither::None) => oxitone_render::DitherMode::None,
            Some(oxitone_core::wire::Dither::Tpdf) | None => oxitone_render::DitherMode::Tpdf,
        };
        render_options.stems = match options.stems {
            Some(oxitone_core::wire::StemMode::MixerChannels) => {
                oxitone_render::StemMode::MixerChannels
            }
            Some(oxitone_core::wire::StemMode::Tracks) => oxitone_render::StemMode::Tracks,
            Some(oxitone_core::wire::StemMode::None) | None => oxitone_render::StemMode::None,
        };
        render_options.include_metronome = options.include_metronome.unwrap_or(false);
        render_options.parameter_events = parameter_events;

        let store = SampleStore::new(options.asset_base_dir.map(PathBuf::from));
        let report = render_wav(&snapshot, &plugins, &store, &render_options)?;
        let wire_report = oxitone_core::wire::RenderReport {
            files: report
                .files
                .iter()
                .map(|file| oxitone_core::wire::RenderFileReport {
                    path: file.path.clone(),
                    stem: file.stem.clone(),
                    duration_seconds: file.duration_seconds,
                    peak_dbfs: finite_db(file.peak_dbfs),
                    true_peak_dbfs: finite_db(file.true_peak_dbfs),
                    integrated_lufs: finite_db(file.integrated_lufs),
                })
                .collect(),
            graph_latency_frames: report.graph_latency_frames,
        };
        serde_json::to_string(&wire_report).map_err(|e| {
            OxitoneError::new(codes::REALTIME_FAULT, format!("report serialization: {e}"))
        })
    })
}

fn transport_state_json_parts(state: TransportState, cursor: u64) -> String {
    let state = match state {
        TransportState::Stopped => "stopped",
        TransportState::Playing => "playing",
        TransportState::Paused => "paused",
        TransportState::Rendering => "rendering",
    };
    serde_json::json!({
        "state": state,
        "cursor": cursor.to_string(),
    })
    .to_string()
}

fn transport_state_json(graph: &RenderGraph) -> String {
    transport_state_json_parts(graph.state(), graph.transport().cursor)
}

/// Engine options → realtime chain configuration (04-api-contracts.md
/// `EngineOptions` defaults: renderAheadBlocks 4, buffered, adapt-device,
/// follow-default).
fn realtime_config(options: Option<&EngineOptions>) -> RealtimeConfig {
    let mut config = RealtimeConfig::default();
    if let Some(options) = options {
        if let Some(blocks) = options.render_ahead_blocks {
            config.render_ahead_blocks = blocks.clamp(2, 16);
        }
        if let Some(mode) = options.latency_mode {
            config.latency_mode = mode;
        }
        if let Some(policy) = options.device_rate_policy {
            config.device_rate_policy = policy;
        }
        if let Some(policy) = options.device_change_policy {
            config.device_change_policy = policy;
        }
        config.output_device_id = options.output_device_id.clone();
    }
    config
}

#[napi(js_name = "enqueueTransport")]
pub fn enqueue_transport(engine_id: String, command_json: String) -> napi::Result<String> {
    guarded(|| {
        let command: NativeCommand = serde_json::from_str(&command_json).map_err(|e| {
            OxitoneError::new(
                codes::INVALID_PROJECT,
                format!("invalid transport command: {e}"),
            )
        })?;
        let NativeCommand::Transport {
            command,
            frame,
            beat,
            seconds,
            loop_region,
        } = command
        else {
            return Err(OxitoneError::new(
                codes::INVALID_PROJECT,
                "enqueueTransport expects a {type:'transport'} command",
            ));
        };
        if u8::from(frame.is_some()) + u8::from(beat.is_some()) + u8::from(seconds.is_some()) > 1 {
            return Err(OxitoneError::new(
                codes::INVALID_PROJECT,
                "transport command takes exactly one of frame, beat or seconds",
            ));
        }
        let mut engines = lock_registry();
        let engine = engines
            .get_mut(&engine_id)
            .ok_or_else(|| unknown_engine(&engine_id))?;
        use oxitone_core::wire::TransportCommandKind as Kind;
        // Resolve seconds before taking ownership of the graph: invalid
        // timecodes must preserve the compiled engine for subsequent calls.
        let frame = if let Some(seconds) = seconds {
            Some(if let Some(session) = engine.session.as_ref() {
                session.seconds_to_frame(seconds)?
            } else {
                engine
                    .graph
                    .as_ref()
                    .ok_or_else(|| not_compiled(&engine_id))?
                    .plan()
                    .tempo
                    .seconds_to_frame(seconds)?
            })
        } else {
            frame
        };

        // Realtime path: the session owns the graph once playback has
        // started on a device.
        if command == Kind::Play && engine.session.is_none() {
            let graph = engine
                .graph
                .take()
                .ok_or_else(|| not_compiled(&engine_id))?;
            let position = match (frame, beat) {
                (Some(frame), None) => Some(frame),
                (None, Some(beat)) => Some(graph.plan().tempo.beat_to_frame(beat)),
                (None, None) => None,
                (Some(_), Some(_)) => unreachable!("checked above"),
            };
            let config = realtime_config(engine.options.as_ref());
            return match crate::playback::start(graph, config, engine.options.as_ref()) {
                Ok(session) => {
                    let loop_region = loop_region.map(|r| (r.start_frame, r.end_frame));
                    let (state, cursor) = session.transport(TransportCmd::Play {
                        from: position,
                        loop_region,
                    })?;
                    engine.session = Some(session);
                    Ok(transport_state_json_parts(state, cursor))
                }
                // 03 §错误与恢复: 无法打开任何设备时 transport 置为
                // paused，报 DeviceUnavailable; the graph stays with the
                // engine so offline renders keep working.
                Err(failure) => {
                    let mut graph = *failure.graph;
                    graph.transport_mut().pause();
                    engine.graph = Some(graph);
                    Err(failure.error)
                }
            };
        }
        if let Some(session) = engine.session.as_ref() {
            let position = match (frame, beat) {
                (Some(frame), None) => Some(frame),
                (None, Some(beat)) => Some(session.beat_to_frame(beat)),
                (None, None) => None,
                (Some(_), Some(_)) => unreachable!("checked above"),
            };
            let cmd = match command {
                Kind::Play => TransportCmd::Play {
                    from: position,
                    loop_region: loop_region.map(|r| (r.start_frame, r.end_frame)),
                },
                Kind::Pause => TransportCmd::Pause,
                Kind::Stop => TransportCmd::Stop,
                Kind::Seek => TransportCmd::Seek {
                    frame: position.ok_or_else(|| {
                        OxitoneError::new(
                            codes::INVALID_PROJECT,
                            "seek requires a frame or beat position",
                        )
                    })?,
                },
            };
            let (state, cursor) = session.transport(cmd)?;
            return Ok(transport_state_json_parts(state, cursor));
        }

        // Offline path (pre-play): state advances without audio output.
        let graph = engine
            .graph
            .as_mut()
            .ok_or_else(|| not_compiled(&engine_id))?;
        let position = match (frame, beat) {
            (Some(frame), None) => Some(frame),
            (None, Some(beat)) => Some(graph.plan().tempo.beat_to_frame(beat)),
            (None, None) => None,
            (Some(_), Some(_)) => unreachable!("checked above"),
        };
        match command {
            Kind::Play => unreachable!("handled by the realtime path above"),
            Kind::Pause => graph.transport_mut().pause(),
            Kind::Stop => {
                graph.transport_mut().stop();
                graph.seek(0);
            }
            Kind::Seek => {
                let frame = position.ok_or_else(|| {
                    OxitoneError::new(
                        codes::INVALID_PROJECT,
                        "seek requires a frame or beat position",
                    )
                })?;
                graph.seek(frame);
            }
        }
        Ok(transport_state_json(graph))
    })
}

#[napi(js_name = "setParameter")]
pub fn set_parameter(
    engine_id: String,
    entity_id: String,
    parameter_id: String,
    value: f64,
    at_frame: Option<String>,
) -> napi::Result<()> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let at_frame = at_frame
            .map(|raw| {
                raw.parse::<u64>().map_err(|_| {
                    OxitoneError::new(
                        codes::INVALID_PROJECT,
                        format!("atFrame must be an unsigned decimal string, got {raw:?}"),
                    )
                })
            })
            .transpose()?;
        let event = ParameterEventInput {
            entity_id,
            parameter_id,
            value,
            at_frame,
        };
        let mut engines = lock_registry();
        let engine = engines
            .get_mut(&engine_id)
            .ok_or_else(|| unknown_engine(&engine_id))?;
        // Realtime path: resolve against the immutable target index and
        // forward to the render worker; it takes effect at the ring
        // horizon.
        if let Some(session) = engine.session.as_ref() {
            let index = engine
                .param_index
                .as_ref()
                .ok_or_else(|| not_compiled(&engine_id))?;
            let resolved = resolve_parameter_event(index, &event, session.cursor())?;
            session.enqueue_parameter(resolved)?;
            engine.parameter_events.push(event);
            return Ok(());
        }
        let graph = engine
            .graph
            .as_mut()
            .ok_or_else(|| not_compiled(&engine_id))?;
        graph.enqueue_parameter(&event)?;
        engine.parameter_events.push(event);
        Ok(())
    }));
    match result {
        Ok(result) => result.map_err(|e| to_napi_error(&e)),
        Err(_) => Err(internal_fault("internal native panic")),
    }
}

/// Audio output devices visible to CoreAudio (04-api-contracts.md
/// `OutputDeviceInfo`), default device first.
#[napi(js_name = "listOutputDevices")]
pub fn list_output_devices() -> napi::Result<String> {
    guarded(|| {
        let devices = oxitone_io_macos::list_output_devices()?;
        let wire: Vec<serde_json::Value> = devices
            .iter()
            .map(|device| {
                serde_json::json!({
                    "id": device.id,
                    "name": device.name,
                    "nominalSampleRates": device.nominal_sample_rates,
                    "bufferFrameSizeRange": [
                        device.buffer_frame_size_range.0,
                        device.buffer_frame_size_range.1,
                    ],
                    "isDefault": device.is_default,
                })
            })
            .collect();
        serde_json::to_string(&wire).map_err(|e| {
            OxitoneError::new(
                codes::REALTIME_FAULT,
                format!("device list serialization: {e}"),
            )
        })
    })
}

/// Output latency breakdown for a playing engine (04-api-contracts.md
/// `OutputLatency`; frames at the project sample rate, decimal strings).
/// Fails with `DeviceUnavailable` until the engine has started playback.
#[napi(js_name = "getOutputLatency")]
pub fn get_output_latency(engine_id: String) -> napi::Result<String> {
    guarded(|| {
        let engines = lock_registry();
        let engine = engines
            .get(&engine_id)
            .ok_or_else(|| unknown_engine(&engine_id))?;
        let session = engine.session.as_ref().ok_or_else(|| {
            OxitoneError::new(
                codes::DEVICE_UNAVAILABLE,
                "engine has no active output; call play first",
            )
        })?;
        let report = session.output_latency()?;
        Ok(serde_json::json!({
            "frames": report.frames.to_string(),
            "seconds": report.seconds,
            "breakdown": {
                "ring": report.ring.to_string(),
                "resampler": report.resampler.to_string(),
                "deviceBuffer": report.device_buffer.to_string(),
                "safetyOffset": report.safety_offset.to_string(),
                "deviceLatency": report.device_latency.to_string(),
            },
        })
        .to_string())
    })
}

/// Realtime diagnostics snapshot: atomic counters, `engineLoad` EMA, ring
/// occupancy, block-time percentiles and the diagnostic events raised
/// since the last call (05-performance-and-benchmarks.md §诊断).
#[napi(js_name = "getDiagnostics")]
pub fn get_diagnostics(engine_id: String) -> napi::Result<String> {
    guarded(|| {
        let engines = lock_registry();
        let engine = engines
            .get(&engine_id)
            .ok_or_else(|| unknown_engine(&engine_id))?;
        let session = engine.session.as_ref().ok_or_else(|| {
            OxitoneError::new(
                codes::DEVICE_UNAVAILABLE,
                "engine has no realtime session; call play first",
            )
        })?;
        let snapshot = session.snapshot_diagnostics();
        let state = match session.transport_state() {
            TransportState::Stopped => "stopped",
            TransportState::Playing => "playing",
            TransportState::Paused => "paused",
            TransportState::Rendering => "rendering",
        };
        let events: Vec<serde_json::Value> = snapshot
            .events
            .iter()
            .map(|event| {
                serde_json::json!({
                    "code": event.code,
                    "severity": event.severity.as_str(),
                    "frame": event.frame.to_string(),
                    "message": event.message(),
                })
            })
            .collect();
        Ok(serde_json::json!({
            "state": state,
            "cursor": session.cursor().to_string(),
            "blocks": snapshot.blocks,
            "deadlineMisses": snapshot.deadline_misses,
            "xruns": snapshot.xruns,
            "nanBlocks": snapshot.nan_blocks,
            "queueDrops": snapshot.queue_drops,
            "performanceWarnings": snapshot.performance_warnings,
            "engineLoad": snapshot.engine_load,
            "ringOccupancyFrames": snapshot.ring_occupancy_frames,
            "blockTimeNs": {
                "p50": snapshot.block_time_p50_ns,
                "p95": snapshot.block_time_p95_ns,
                "p99": snapshot.block_time_p99_ns,
                "max": snapshot.block_time_max_ns,
            },
            "events": events,
        })
        .to_string())
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use oxitone_core::beat::Beat;
    use oxitone_core::wire::{
        ChannelSpec, InstrumentRef, NoteSpec, PatternClipSpec, PatternSpec, ProjectSnapshot,
        SampleClipSpec, SampleFormat, SampleRef, TempoSegment, TimeSignatureSegment, TrackSpec,
    };
    use oxitone_core::PROTOCOL_VERSION;
    use serde_json::Value;

    use super::*;

    fn beat(n: i64, d: u32) -> Beat {
        Beat::new(n, d).unwrap()
    }

    fn base_snapshot() -> ProjectSnapshot {
        ProjectSnapshot {
            protocol_version: PROTOCOL_VERSION.to_string(),
            revision: 1,
            id: "prj_napi".into(),
            name: None,
            sample_rate: 48_000,
            block_size: 128,
            seed: 7,
            tempo_map: vec![TempoSegment {
                start_beat: Beat::ZERO,
                bpm: 120.0,
                curve: None,
            }],
            time_signature_map: vec![TimeSignatureSegment {
                start_bar: 1,
                numerator: 4,
                denominator: 4,
            }],
            markers: vec![],
            tracks: vec![],
            patterns: vec![],
            pattern_clips: vec![],
            sample_clips: vec![],
            samples: vec![],
            channels: vec![],
            mixer_channels: vec![],
            automation: vec![],
        }
    }

    /// One wavetable channel playing four quarter notes from beat 0.
    fn playable_snapshot() -> ProjectSnapshot {
        let mut snapshot = base_snapshot();
        snapshot.tracks.push(TrackSpec {
            id: "trk_a".into(),
            name: None,
            channel_ids: vec!["chn_a".into()],
            tempo: None,
            pattern_clip_ids: vec!["pcl_a".into()],
            sample_clip_ids: vec![],
            enabled: None,
            midi_channel: None,
        });
        snapshot.patterns.push(PatternSpec {
            id: "pat_a".into(),
            name: None,
            length_beats: beat(4, 1),
            notes: vec![
                NoteSpec {
                    id: None,
                    pitch: 60,
                    start: beat(0, 1),
                    duration: beat(1, 1),
                    velocity: 0.9,
                    off_velocity: None,
                    chance: None,
                    voice: None,
                    tags: None,
                },
                NoteSpec {
                    id: None,
                    pitch: 64,
                    start: beat(1, 1),
                    duration: beat(1, 1),
                    velocity: 0.9,
                    off_velocity: None,
                    chance: None,
                    voice: None,
                    tags: None,
                },
            ],
        });
        snapshot.pattern_clips.push(PatternClipSpec {
            id: "pcl_a".into(),
            pattern_id: "pat_a".into(),
            track_id: "trk_a".into(),
            start_beat: beat(0, 1),
            duration_beats: Some(beat(4, 1)),
            loop_count: None,
            last_beat: None,
            transpose: None,
            velocity_scale: None,
            probability: None,
            enabled: None,
        });
        snapshot.channels.push(ChannelSpec {
            id: "chn_a".into(),
            name: None,
            instrument: InstrumentRef {
                plugin_id: "oxitone.wavetable".into(),
                plugin_version: "1.0.0".into(),
                parameters: BTreeMap::new(),
                resources: None,
                state: None,
            },
            effect_chain: vec![],
            level: 1.0,
            pan: 0.0,
            swing: None,
            mixer_channel_id: "mix_master".into(),
            mute: None,
            solo: None,
        });
        snapshot
    }

    fn snapshot_json(snapshot: &ProjectSnapshot) -> String {
        encode_project_snapshot(snapshot).unwrap()
    }

    fn engine() -> String {
        let response = create_engine(None).unwrap();
        let parsed: Value = serde_json::from_str(&response).unwrap();
        parsed["engineId"].as_str().unwrap().to_string()
    }

    fn error_code(result: napi::Result<String>) -> String {
        let err = result.expect_err("expected an error");
        let wire: Value = serde_json::from_str(&err.reason).unwrap();
        wire["code"].as_str().unwrap().to_string()
    }

    fn out_dir(name: &str) -> PathBuf {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/tmp/napi-tests")
            .join(name);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn compile_stores_graph_and_transport_advances() {
        let id = engine();
        let snapshot = snapshot_json(&playable_snapshot());

        // Transport before compile: stable error code.
        let early = enqueue_transport(
            id.clone(),
            r#"{"type":"transport","command":"play"}"#.into(),
        );
        assert_eq!(error_code(early), "InvalidProject");

        compile(id.clone(), snapshot).unwrap();

        let playing: Value = serde_json::from_str(
            &enqueue_transport(
                id.clone(),
                r#"{"type":"transport","command":"play"}"#.into(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(playing["state"], "playing");
        assert_eq!(playing["cursor"], "0");

        // 120 BPM at 48 kHz: one beat is 24000 frames.
        let sought: Value = serde_json::from_str(
            &enqueue_transport(
                id.clone(),
                r#"{"type":"transport","command":"seek","beat":{"numerator":2,"denominator":1}}"#
                    .into(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(sought["cursor"], "48000");

        let paused: Value = serde_json::from_str(
            &enqueue_transport(
                id.clone(),
                r#"{"type":"transport","command":"pause"}"#.into(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(paused["state"], "paused");
        assert_eq!(paused["cursor"], "48000");

        let stopped: Value = serde_json::from_str(
            &enqueue_transport(
                id.clone(),
                r#"{"type":"transport","command":"stop"}"#.into(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(stopped["state"], "stopped");
        assert_eq!(stopped["cursor"], "0");

        dispose(id).unwrap();
    }

    #[test]
    fn transport_command_validation() {
        let id = engine();
        compile(id.clone(), snapshot_json(&playable_snapshot())).unwrap();

        let both = enqueue_transport(
            id.clone(),
            r#"{"type":"transport","command":"seek","frame":"0","beat":{"numerator":1,"denominator":1}}"#
                .into(),
        );
        assert_eq!(error_code(both), "InvalidProject");

        let no_position = enqueue_transport(
            id.clone(),
            r#"{"type":"transport","command":"seek"}"#.into(),
        );
        assert_eq!(error_code(no_position), "InvalidProject");

        let wrong_type = enqueue_transport(
            id.clone(),
            r#"{"type":"setParameter","entityId":"chn_a","parameterId":"level","value":1}"#.into(),
        );
        assert_eq!(error_code(wrong_type), "InvalidProject");

        dispose(id).unwrap();
    }

    #[test]
    fn set_parameter_paths() {
        let id = engine();
        let before = set_parameter(id.clone(), "chn_a".into(), "level".into(), 0.5, None);
        let err = before.expect_err("setParameter before compile must fail");
        let wire: Value = serde_json::from_str(&err.reason).unwrap();
        assert_eq!(wire["code"], "InvalidProject");

        compile(id.clone(), snapshot_json(&playable_snapshot())).unwrap();

        set_parameter(id.clone(), "chn_a".into(), "level".into(), 0.5, None).unwrap();
        set_parameter(
            id.clone(),
            "chn_a".into(),
            "level".into(),
            0.5,
            Some("48000".into()),
        )
        .unwrap();

        let unknown = set_parameter(id.clone(), "chn_ghost".into(), "level".into(), 0.5, None);
        let err = unknown.expect_err("unknown entity must fail");
        let wire: Value = serde_json::from_str(&err.reason).unwrap();
        assert_eq!(wire["code"], "AutomationTargetInvalid");

        let bad_frame = set_parameter(
            id.clone(),
            "chn_a".into(),
            "level".into(),
            0.5,
            Some("-1".into()),
        );
        let err = bad_frame.expect_err("bad atFrame must fail");
        let wire: Value = serde_json::from_str(&err.reason).unwrap();
        assert_eq!(wire["code"], "InvalidProject");

        let out_of_range = set_parameter(id.clone(), "chn_a".into(), "level".into(), 3.0, None);
        let err = out_of_range.expect_err("out-of-range value must fail");
        let wire: Value = serde_json::from_str(&err.reason).unwrap();
        assert_eq!(wire["code"], "AutomationRange");

        dispose(id).unwrap();
    }

    #[test]
    fn render_wav_produces_report_and_float32_stereo_file() {
        let id = engine();
        compile(id.clone(), snapshot_json(&playable_snapshot())).unwrap();
        let dir = out_dir("render");
        let path = dir.join("out.wav");
        let options = serde_json::json!({ "path": path.display().to_string() }).to_string();

        let report: Value = serde_json::from_str(
            &render_wav_command(
                id.clone(),
                snapshot_json(&playable_snapshot()),
                options.clone(),
            )
            .unwrap(),
        )
        .unwrap();
        let file = &report["files"][0];
        assert_eq!(file["path"], path.display().to_string());
        assert!(file["durationSeconds"].as_f64().unwrap() > 1.9);
        assert!(file["peakDbfs"].as_f64().unwrap() > -30.0);
        assert!(file["truePeakDbfs"].as_f64().is_some());
        assert!(file["integratedLufs"].as_f64().is_some());
        assert!(report["graphLatencyFrames"].as_str().is_some());

        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        let audio_format = u16::from_le_bytes([bytes[20], bytes[21]]);
        let channels = u16::from_le_bytes([bytes[22], bytes[23]]);
        let sample_rate = u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]);
        assert_eq!(audio_format, 3, "float32 WAV");
        assert_eq!(channels, 2);
        assert_eq!(sample_rate, 48_000);

        // Determinism: a second render is byte-identical.
        let path2 = dir.join("out2.wav");
        let options2 = serde_json::json!({ "path": path2.display().to_string() }).to_string();
        render_wav_command(id.clone(), snapshot_json(&playable_snapshot()), options2).unwrap();
        assert_eq!(bytes, std::fs::read(&path2).unwrap());

        dispose(id).unwrap();
    }

    #[test]
    fn render_wav_missing_asset_is_asset_unavailable() {
        let id = engine();
        let mut snapshot = base_snapshot();
        snapshot.samples.push(SampleRef {
            id: "smp_a".into(),
            asset_uri: "/nonexistent/oxitone-missing.wav".into(),
            sha256: "00".repeat(32),
            format: SampleFormat::Wav,
            sample_rate: 48_000,
            channels: 1,
            frames: 128,
            edits: None,
            musical_length_beats: None,
            provenance: None,
        });
        snapshot.sample_clips.push(SampleClipSpec {
            id: "scl_a".into(),
            sample_id: "smp_a".into(),
            track_id: "trk_a".into(),
            start_beat: Beat::ZERO,
            duration_beats: None,
            gain: None,
            pan: None,
            rate: None,
            loop_spec: None,
            tempo_sync: None,
            stretch_algorithm: None,
            enabled: None,
        });
        snapshot.tracks.push(TrackSpec {
            id: "trk_a".into(),
            name: None,
            channel_ids: vec![],
            tempo: None,
            pattern_clip_ids: vec![],
            sample_clip_ids: vec!["scl_a".into()],
            enabled: None,
            midi_channel: None,
        });

        let dir = out_dir("missing-asset");
        let options =
            serde_json::json!({ "path": dir.join("x.wav").display().to_string() }).to_string();
        let result = render_wav_command(id.clone(), snapshot_json(&snapshot), options);
        assert_eq!(error_code(result), "AssetUnavailable");
        dispose(id).unwrap();
    }

    #[test]
    fn device_commands_report_real_devices() {
        let devices: Value = serde_json::from_str(&list_output_devices().unwrap()).unwrap();
        let devices = devices.as_array().unwrap();
        assert!(!devices.is_empty(), "this machine has an output device");
        let default = devices
            .iter()
            .find(|d| d["isDefault"].as_bool() == Some(true))
            .expect("a default output device exists");
        assert!(default["id"].as_str().unwrap().len() > 1);
        assert!(default["nominalSampleRates"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r.as_u64() == Some(48_000)));
        assert_eq!(default["bufferFrameSizeRange"].as_array().unwrap().len(), 2);

        // Latency requires a playing engine.
        let err = get_output_latency("eng_missing".into()).expect_err("unknown engine");
        let wire: Value = serde_json::from_str(&err.reason).unwrap();
        assert_eq!(wire["code"], "InvalidProject");
        let id = engine();
        let err = get_output_latency(id.clone()).expect_err("no session before play");
        let wire: Value = serde_json::from_str(&err.reason).unwrap();
        assert_eq!(wire["code"], "DeviceUnavailable");
        dispose(id).unwrap();
    }

    #[test]
    fn play_starts_realtime_output_and_reports_latency() {
        let id = engine();
        compile(id.clone(), snapshot_json(&playable_snapshot())).unwrap();

        let playing: Value = serde_json::from_str(
            &enqueue_transport(
                id.clone(),
                r#"{"type":"transport","command":"play"}"#.into(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(playing["state"], "playing");

        let latency: Value =
            serde_json::from_str(&get_output_latency(id.clone()).unwrap()).unwrap();
        let frames: u64 = latency["frames"].as_str().unwrap().parse().unwrap();
        assert!(frames > 0);
        assert_eq!(
            frames,
            latency["breakdown"]["ring"]
                .as_str()
                .unwrap()
                .parse::<u64>()
                .unwrap()
                + latency["breakdown"]["resampler"]
                    .as_str()
                    .unwrap()
                    .parse::<u64>()
                    .unwrap()
                + latency["breakdown"]["deviceBuffer"]
                    .as_str()
                    .unwrap()
                    .parse::<u64>()
                    .unwrap()
                + latency["breakdown"]["safetyOffset"]
                    .as_str()
                    .unwrap()
                    .parse::<u64>()
                    .unwrap()
                + latency["breakdown"]["deviceLatency"]
                    .as_str()
                    .unwrap()
                    .parse::<u64>()
                    .unwrap()
        );

        // Let the worker render a few blocks on the real device.
        std::thread::sleep(std::time::Duration::from_millis(300));
        let diagnostics: Value =
            serde_json::from_str(&get_diagnostics(id.clone()).unwrap()).unwrap();
        assert!(diagnostics["blocks"].as_u64().unwrap() > 0);
        assert_eq!(diagnostics["xruns"].as_u64().unwrap(), 0);
        assert!(diagnostics["engineLoad"].as_f64().unwrap() >= 0.0);

        // Realtime transport: pause stops the cursor, seek moves it,
        // setParameter resolves through the session index.
        let paused: Value = serde_json::from_str(
            &enqueue_transport(
                id.clone(),
                r#"{"type":"transport","command":"pause"}"#.into(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(paused["state"], "paused");

        let sought: Value = serde_json::from_str(
            &enqueue_transport(
                id.clone(),
                r#"{"type":"transport","command":"seek","beat":{"numerator":1,"denominator":1}}"#
                    .into(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(sought["cursor"], "24000");

        set_parameter(id.clone(), "chn_a".into(), "level".into(), 0.5, None).unwrap();

        let stopped: Value = serde_json::from_str(
            &enqueue_transport(
                id.clone(),
                r#"{"type":"transport","command":"stop"}"#.into(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(stopped["state"], "stopped");
        dispose(id).unwrap();
    }

    #[test]
    fn export_midi_still_round_trips() {
        let id = engine();
        let report: Value = serde_json::from_str(
            &export_midi(id.clone(), snapshot_json(&playable_snapshot()), None).unwrap(),
        )
        .unwrap();
        assert!(report["bytesBase64"].as_str().is_some());
        assert!(report["diagnostics"]["noteTrackCount"].as_u64().unwrap() >= 1);
        dispose(id).unwrap();
    }
}
