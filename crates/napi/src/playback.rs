//! Host selection stays outside the render path. Simulated mode never opens CoreAudio.
use oxitone_core::wire::{AudioBackend, EngineOptions};
use oxitone_render::{
    realtime::{RealtimeConfig, RealtimeSession, SessionStartError, SimulatedSinkConfig},
    RenderGraph,
};

pub fn start(
    graph: RenderGraph,
    config: RealtimeConfig,
    options: Option<&EngineOptions>,
) -> Result<RealtimeSession, SessionStartError> {
    if options.and_then(|options| options.audio_backend) == Some(AudioBackend::Simulated) {
        let sink = SimulatedSinkConfig {
            sample_rate: graph.sample_rate(),
            frames_per_slice: graph.block_size() as u32,
            channels: 2,
            latency_frames: 0,
            safety_offset_frames: 0,
        };
        RealtimeSession::start_simulated(Box::new(graph), config, sink, None)
    } else {
        RealtimeSession::start(Box::new(graph), config)
    }
}
