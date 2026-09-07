//! Host-independent, control-side ownership. Failed compilation never replaces a graph.
use oxitone_core::{codes, wire::ProjectSnapshot, OxitoneError};
use oxitone_graph::PluginRegistry;
use oxitone_render::{RenderGraph, RenderGraphOptions, SampleStore};
use oxitone_samples::{prepare_cached, DecodedSample, SampleCache};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub struct Engine {
    pub registry: PluginRegistry,
    pub assets: BTreeMap<String, DecodedSample>,
    cache: SampleCache,
    pub graph: Option<RenderGraph>,
    pub snapshot: Option<ProjectSnapshot>,
    pub left: Vec<f32>,
    pub right: Vec<f32>,
    pub binary: Vec<u8>,
}

pub fn invalid(message: impl Into<String>) -> OxitoneError {
    OxitoneError::new(codes::INVALID_PROJECT, message)
}

impl Engine {
    pub fn new() -> Result<Self, OxitoneError> {
        let mut registry = oxitone_render::builtin_registry()?;
        let manifest = serde_json::from_str(include_str!("../../example-drums/manifest.json"))
            .map_err(|e| invalid(e.to_string()))?;
        // The entry is statically linked and valid for this instance's lifetime.
        let drums = unsafe {
            oxitone_render::plugins::from_entry(
                oxitone_example_drums::oxitone_plugin_entry_v1(),
                &manifest,
                "statically-linked".into(),
                None,
            )?
        };
        registry.register(drums)?;
        Ok(Self {
            registry,
            assets: BTreeMap::new(),
            cache: SampleCache::new(),
            graph: None,
            snapshot: None,
            left: Vec::new(),
            right: Vec::new(),
            binary: Vec::new(),
        })
    }

    pub fn build(
        &self,
        snapshot: &ProjectSnapshot,
        tail: f64,
    ) -> Result<RenderGraph, OxitoneError> {
        oxitone_core::version::check_protocol_version(&snapshot.protocol_version)?;
        oxitone_graph::validate::validate(snapshot, &self.registry)?;
        let mut prepared = Vec::with_capacity(snapshot.samples.len());
        for sample in &snapshot.samples {
            let decoded = self
                .assets
                .get(&sample.sha256.to_ascii_lowercase())
                .ok_or_else(|| {
                    OxitoneError::new(
                        codes::ASSET_UNAVAILABLE,
                        format!("upload sample bytes before compile: {}", sample.id),
                    )
                })?;
            if decoded.sample_rate != sample.sample_rate
                || decoded.frames() != sample.frames
                || decoded.channels.len() != sample.channels as usize
                || decoded.metadata.format != sample.format
            {
                return Err(OxitoneError::new(
                    codes::ASSET_UNAVAILABLE,
                    format!("sample metadata mismatch: {}", sample.id),
                ));
            }
            prepared.push((
                sample.id.clone(),
                prepare_cached(&self.cache, sample, decoded, snapshot.sample_rate)?,
            ));
        }
        let store = SampleStore::from_prepared(prepared, snapshot.sample_rate);
        let mut options = RenderGraphOptions::default();
        options.compile.tail_seconds = Some(tail);
        RenderGraph::compile(snapshot, &self.registry, &store, &options)
    }

    pub fn compile(&mut self, snapshot: ProjectSnapshot) -> Result<Value, OxitoneError> {
        let mut graph = self.build(&snapshot, 10.0)?;
        // Transfer transport only after all graph/sample validation succeeds.
        if let Some(old) = &self.graph {
            graph.seek(old.transport().cursor);
            graph.transport_mut().state = old.transport().state;
            graph.transport_mut().loop_region = old.transport().loop_region;
        }
        self.left = vec![0.0; graph.block_size()];
        self.right = vec![0.0; graph.block_size()];
        self.graph = Some(graph);
        self.snapshot = Some(snapshot);
        self.state()
    }

    pub fn graph_mut(&mut self) -> Result<&mut RenderGraph, OxitoneError> {
        self.graph
            .as_mut()
            .ok_or_else(|| invalid("compile a project first"))
    }

    pub fn state(&self) -> Result<Value, OxitoneError> {
        let graph = self
            .graph
            .as_ref()
            .ok_or_else(|| invalid("compile a project first"))?;
        let transport = graph.transport();
        Ok(
            json!({ "protocolVersion": "1.0", "sampleRate": graph.sample_rate(),
            "blockSize": graph.block_size(), "cursor": transport.cursor.to_string(),
            "state": format!("{:?}", transport.state).to_lowercase(),
            "latencyFrames": graph.graph_latency_frames(),
            "contentEndFrame": graph.plan().tempo.beat_to_frame(graph.plan().content_end_beat).to_string(),
            "faulted": graph.faulted() }),
        )
    }

    /// No control work, allocation, host imports or memory growth.
    pub fn process(&mut self) -> u32 {
        let Some(graph) = self.graph.as_mut() else {
            return 1;
        };
        graph.process_block(&mut self.left, &mut self.right);
        if graph.faulted() {
            2
        } else {
            0
        }
    }
}
