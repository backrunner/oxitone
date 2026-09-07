//! Control-thread compiler and transport. GPUI and IPC never run on the audio worker.
mod transport;
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
use oxitone_graph::abi::Plugin;
use oxitone_render::{
    builtin_registry,
    plugins::{load_plugin, CPlugin},
    realtime::{RealtimeConfig, RealtimeSession, SimulatedSinkConfig, TransportCmd},
    RenderGraph, RenderGraphOptions, SampleStore, TransportState,
};
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{mpsc::Sender, Arc},
};

pub struct Engine {
    pub current: Option<Arc<ViewProject>>,
    seen: Option<u64>,
    graph: Option<Box<RenderGraph>>,
    session: Option<RealtimeSession>,
    plugins: Vec<Arc<CPlugin>>,
    simulated: bool,
    events: Sender<UiEvent>,
}

impl Engine {
    pub fn new(simulated: bool, events: Sender<UiEvent>) -> Self {
        Self {
            current: None,
            seen: None,
            graph: None,
            session: None,
            plugins: vec![],
            simulated,
            events,
        }
    }

    pub fn handle(&mut self, frame: Frame) -> Value {
        let result = match frame {
            Frame::Snapshot {
                snapshot,
                asset_base_dir,
                plugins,
                allow_plugins,
                hash,
            } => self.replace(*snapshot, asset_base_dir, plugins, allow_plugins, &hash),
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
        policy: Option<AllowPlugins>,
        hash: &str,
    ) -> Result<(), OxitoneError> {
        if self.seen.is_some_and(|seen| snapshot.revision <= seen) {
            return Ok(());
        }
        self.seen = Some(snapshot.revision);
        if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(wire::invalid("invalid snapshot hash"));
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
        let mut registry = builtin_registry()?;
        let mut loaded = vec![];
        let mut libraries = crate::plugin_catalog::Libraries::new();
        for options in plugins {
            if options.manifest.plugin_id.starts_with("oxitone.")
                || registry
                    .lookup_descriptor(
                        &options.manifest.plugin_id,
                        &options.manifest.plugin_version,
                    )
                    .is_some()
            {
                return Err(wire::invalid("duplicate or reserved plugin ID/version"));
            }
            // Explicit paths and policy arrive from the locally executed authoring entry.
            let plugin =
                unsafe { load_plugin(&options, policy.unwrap_or(AllowPlugins::SignedOnly))? };
            registry.register(plugin.clone())?;
            libraries.insert(
                (
                    options.manifest.plugin_id.clone(),
                    options.manifest.plugin_version.clone(),
                ),
                crate::plugin_catalog::LibraryInfo {
                    path: options.library_path.clone(),
                    sha256: plugin.registration.sha256.clone(),
                },
            );
            loaded.push(plugin);
        }
        let mut graph = Box::new(RenderGraph::compile(
            &snapshot,
            &registry,
            &SampleStore::new(Some(PathBuf::from(base))),
            &RenderGraphOptions::default(),
        )?);
        let project = Arc::new(ViewProject {
            plugins: crate::plugin_catalog::collect(&snapshot, &registry, libraries),
            mixer_strips: Default::default(),
            snapshot,
            plan: graph.plan().into(),
            graph_latency: graph.graph_latency_frames(),
            pattern_labels: Default::default(),
            telemetry: graph.enable_preview(),
        });
        if let Some(session) = &self.session {
            session.replace_graph(graph)?;
        } else {
            if let Some(previous) = &self.graph {
                graph.seek(previous.transport().cursor);
                graph.transport_mut().state = previous.transport().state;
                graph.transport_mut().loop_region = previous.transport().loop_region;
            }
            self.graph = Some(graph);
        }
        self.plugins = loaded;
        self.current = Some(project.clone());
        let _ = self.events.send(UiEvent::Accepted(project));
        Ok(())
    }

    pub fn playback(&self) -> PlaybackStatus {
        let faults = self.plugins.iter().map(|p| p.fault_count()).sum();
        let fault_nodes = self
            .plugins
            .iter()
            .filter(|p| p.fault_count() > 0)
            .map(|p| format!("{}: {}", p.descriptor().plugin_id, p.fault_count()))
            .collect::<Vec<_>>()
            .join(", ");
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
        json!({"protocolVersion":"1.0","type":"state", "revision":self.current.as_ref().map(|p| p.snapshot.revision.to_string()),
            "seenRevision":self.seen.map(|r| r.to_string()), "cursor":status.cursor.to_string(), "audibleFrame":status.audible.to_string(),
            "playing":status.playing,"xruns":status.xruns,"pluginFaults":status.faults,
            "tracks":self.current.as_ref().map_or(0, |p| p.snapshot.tracks.len()),
            "patterns":self.current.as_ref().map_or(0, |p| p.snapshot.patterns.len())})
    }

    pub fn publish_playback(&self) {
        let _ = self.events.send(UiEvent::Playback(self.playback()));
    }
}
