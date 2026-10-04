//! Control-thread compiler and transport. GPUI and IPC never run on the audio worker.
mod plugins;
mod transport;
mod vst3_control;
use crate::{
    model::{Diagnostic, PlaybackStatus, UiEvent, ViewProject},
    wire::{self, Frame},
};
use oxitone_core::{
    wire::{
        AllowPlugins, NativeCommand, ProjectSnapshot, RegisterPluginOptions, TransportCommandKind,
    },
    OxitoneError,
};
use oxitone_render::{
    realtime::{RealtimeConfig, RealtimeSession, SimulatedSinkConfig, TransportCmd},
    RenderGraph, RenderGraphOptions, SampleStore, TransportState,
};
use plugins::LoadedPlugins;
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{mpsc::Sender, Arc},
};
pub use vst3_control::ControlCompleted;

pub struct Engine {
    pub current: Option<Arc<ViewProject>>,
    seen: Option<u64>,
    audio_key: Option<[u8; 32]>,
    graph: Option<Box<RenderGraph>>,
    controls: Option<oxitone_render::plugin_controls::ControlRegistry>,
    session: Option<RealtimeSession>,
    plugins: LoadedPlugins,
    simulated: bool,
    metronome: bool,
    events: Sender<UiEvent>,
}

impl Engine {
    pub fn new(simulated: bool, events: Sender<UiEvent>) -> Self {
        Self {
            current: None,
            seen: None,
            audio_key: None,
            graph: None,
            controls: None,
            session: None,
            plugins: LoadedPlugins::default(),
            simulated,
            metronome: false,
            events,
        }
    }

    pub fn handle(&mut self, frame: Frame) -> Value {
        let result = match frame {
            Frame::Vst3Instances { snapshot_revision } => {
                return self.vst3_instances(snapshot_revision)
            }
            Frame::Vst3Control { .. } => Err(wire::invalid(
                "VST3 control requires the background backend",
            )),
            Frame::Document { message } => message.validate().map(|()| {
                let _ = self.events.send(UiEvent::Document(message));
            }),
            Frame::Snapshot {
                snapshot,
                asset_base_dir,
                plugins,
                vst3_plugins,
                plugin_uis,
                allow_plugins,
                hash,
            } => self.replace(
                *snapshot,
                asset_base_dir,
                plugins,
                vst3_plugins,
                allow_plugins,
                &hash,
                plugin_uis,
            ),
            Frame::Transport { command } => {
                wire::transport(command).and_then(|cmd| self.transport(cmd))
            }
            Frame::Diagnostic {
                code,
                message,
                path,
            } => {
                let _ = self.events.send(UiEvent::Diagnostic(Diagnostic {
                    code,
                    message,
                    path,
                }));
                Ok(())
            }
            Frame::Status { state } => {
                let _ = self.events.send(UiEvent::Status(state));
                Ok(())
            }
            Frame::Query => Ok(()),
            Frame::Shutdown => {
                let _ = self.events.send(UiEvent::Shutdown);
                Ok(())
            }
        };
        match result {
            Ok(()) => self.state(),
            Err(error) => self.error(error),
        }
    }

    pub fn error(&self, error: OxitoneError) -> Value {
        let _ = self.events.send(UiEvent::Diagnostic(Diagnostic {
            code: error.code.into(),
            message: error.message.clone(),
            path: error.path.clone(),
        }));
        json!({"protocolVersion":"1.0","type":"rejected","code":error.code,"message":error.message,"path":error.path})
    }

    fn replace(
        &mut self,
        snapshot: ProjectSnapshot,
        base: String,
        plugins: Vec<RegisterPluginOptions>,
        vst3_plugins: Vec<Value>,
        policy: Option<AllowPlugins>,
        hash: &str,
        plugin_uis: Value,
    ) -> Result<(), OxitoneError> {
        if self.seen.is_some_and(|seen| snapshot.revision <= seen) {
            return Ok(());
        }
        self.seen = Some(snapshot.revision);
        if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(wire::invalid("invalid snapshot hash"));
        }
        // Compute this locally, excluding only UI metadata and the presentation revision.
        // Never trust the runner hash as authorization to skip native graph validation.
        let audio_key =
            crate::preview_source::audio_key(&snapshot, &base, &plugins, &vst3_plugins, policy)?;
        if self.audio_key == Some(audio_key) {
            if let Some(previous) = &self.current {
                let project = Arc::new(ViewProject {
                    panels: crate::plugin_layout_registry::resolve(
                        &plugin_uis,
                        &previous.plugins,
                        Some(&previous.panels),
                    ),
                    snapshot,
                    plan: previous.plan.clone(),
                    telemetry: previous.telemetry.clone(),
                    graph_latency: previous.graph_latency,
                    plugins: previous.plugins.clone(),
                    pattern_labels: previous.pattern_labels.clone(),
                    automation_previews: previous.automation_previews.clone(),
                    mixer_strips: Default::default(),
                });
                self.current = Some(project.clone());
                let _ = self.events.send(UiEvent::Accepted(project));
                return Ok(());
            }
        }
        if self.current.as_ref().is_some_and(|current| {
            self.session.is_some()
                && (snapshot.sample_rate != current.snapshot.sample_rate
                    || snapshot.block_size != current.snapshot.block_size)
        }) {
            return Err(wire::invalid(
                "restart preview to change sample rate or block size during a session",
            ));
        }
        let (registry, loaded, libraries) = LoadedPlugins::load(
            plugins,
            vst3_plugins,
            policy.unwrap_or(AllowPlugins::SignedOnly),
        )?;
        let mut graph = Box::new(RenderGraph::compile(
            &snapshot,
            &registry,
            &SampleStore::new(Some(PathBuf::from(base))),
            &RenderGraphOptions {
                respect_solo: true,
                metronome_level: Some(0.5),
                ..Default::default()
            },
        )?);
        graph.set_metronome_enabled(self.metronome);
        let mut catalog = crate::plugin_catalog::collect(&snapshot, &registry, libraries);
        crate::plugin_catalog::collect_instances(&mut catalog, &graph.plugin_controls());
        let project = Arc::new(ViewProject {
            panels: crate::plugin_layout_registry::resolve(
                &plugin_uis,
                &catalog,
                self.current.as_ref().map(|p| &p.panels),
            ),
            plugins: catalog,
            mixer_strips: Default::default(),
            snapshot,
            plan: graph.plan().into(),
            graph_latency: graph.graph_latency_frames(),
            pattern_labels: Default::default(),
            automation_previews: Default::default(),
            telemetry: graph.enable_preview(),
        });
        let controls = graph.plugin_controls();
        if let Some(session) = &self.session {
            session.replace_graph(graph)?;
        } else {
            if let Some(previous) = &self.graph {
                graph.seek(previous.transport().cursor);
                graph.transport_mut().state = previous.transport().state;
                graph.transport_mut().loop_region = previous.transport().loop_region;
            }
            graph.activate_plugin_controls();
            self.graph = Some(graph);
        }
        self.controls = Some(controls);
        self.plugins = loaded;
        self.audio_key = Some(audio_key);
        self.current = Some(project.clone());
        let _ = self.events.send(UiEvent::Accepted(project));
        Ok(())
    }

    pub fn playback(&self) -> PlaybackStatus {
        let (faults, fault_nodes) = self.plugins.faults();
        match &self.session {
            Some(session) => {
                let stats = session.snapshot_diagnostics();
                if let Some(event) = stats.events.last() {
                    let _ = self.events.send(UiEvent::Diagnostic(Diagnostic {
                        code: event.code.into(),
                        message: format!("{} (frame {})", event.message(), event.frame),
                        path: None,
                    }));
                }
                let cursor = session.cursor();
                let latency = session.output_latency().map_or(0, |l| l.frames)
                    + self.current.as_ref().map_or(0, |p| p.graph_latency);
                let playing = session.transport_state() == TransportState::Playing;
                PlaybackStatus {
                    cursor,
                    audible: if playing {
                        cursor.saturating_sub(latency)
                    } else {
                        cursor
                    },
                    playing,
                    load: stats.engine_load,
                    xruns: stats.xruns,
                    latency,
                    faults,
                    fault_nodes,
                }
            }
            None => {
                let cursor = self
                    .graph
                    .as_ref()
                    .map_or(0, |graph| graph.transport().cursor);
                PlaybackStatus {
                    cursor,
                    audible: cursor,
                    faults,
                    fault_nodes,
                    ..Default::default()
                }
            }
        }
    }

    pub fn state(&self) -> Value {
        let status = self.playback();
        json!({"protocolVersion":"1.0","documentProtocolVersion":"2.0","type":"state", "revision":self.current.as_ref().map(|p| p.snapshot.revision.to_string()),
            "seenRevision":self.seen.map(|r| r.to_string()), "cursor":status.cursor.to_string(), "audibleFrame":status.audible.to_string(),
            "playing":status.playing,"xruns":status.xruns,"pluginFaults":status.faults,
            "tracks":self.current.as_ref().map_or(0, |p| p.snapshot.tracks.len()),
            "patterns":self.current.as_ref().map_or(0, |p| p.snapshot.patterns.len())})
    }

    pub fn publish_playback(&self) {
        let _ = self.events.send(UiEvent::Playback(self.playback()));
    }
}
