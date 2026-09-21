use crate::{
    bundle, native,
    wave::{Input, Output},
    wire::RenderOptions,
    Result,
};
use std::path::Path;
use vst3_host::{audio::AudioBuffers, Plugin, ProcessMode};

pub fn render(
    plugin: &mut Plugin,
    plugin_hash: String,
    mut options: RenderOptions,
) -> Result<serde_json::Value> {
    let (mut events, payload) = crate::event_wire::flatten(std::mem::take(&mut options.events));
    crate::configuration::apply(
        plugin,
        options.configuration.as_ref(),
        &options.parameters,
        &events,
    )?;
    events.sort_by_key(|event| (event.frame(), event.priority()));
    plugin
        .set_process_mode(ProcessMode::Offline)
        .map_err(native)?;
    let (input_channels, output_channels) = crate::inspection::channels(plugin)?;
    if input_channels == 0 && options.input_path.is_some() {
        return Err(crate::unsupported("VST3 instrument has no audio input"));
    }
    let mut buffers = AudioBuffers::new(
        input_channels,
        output_channels,
        options.block_size,
        options.sample_rate as f64,
    );
    let input = options
        .input_path
        .as_deref()
        .map(|path| Input::open(Path::new(path), options.sample_rate))
        .transpose()?;
    let mut output = Output::create(Path::new(&options.path), options.sample_rate)?;
    plugin.start_processing().map_err(native)?;
    let mut cursor = 0u64;
    while cursor < options.frames + options.tail_frames {
        let frames =
            ((options.frames + options.tail_frames - cursor) as usize).min(options.block_size);
        crate::process_buffers::set_frames(&mut buffers, frames)?;
        for input in &mut buffers.inputs {
            input[..frames].fill(0.0);
        }
        if let Some(input) = &input {
            let content = options.frames.saturating_sub(cursor).min(frames as u64) as usize;
            input.fill(&mut buffers.inputs, cursor, content);
        }
        for out in &mut buffers.outputs {
            out[..frames].fill(0.0);
        }
        if cursor < options.frames {
            super::events::apply(plugin, &events, &payload, cursor, frames)?;
        }
        plugin.process_audio(&mut buffers).map_err(native)?;
        let left = buffers
            .outputs
            .first()
            .map_or(&[][..], |output| &output[..frames]);
        let right = buffers
            .outputs
            .get(1)
            .map_or(left, |output| &output[..frames]);
        output.write(left, right, frames)?;
        cursor += frames as u64;
    }
    plugin.stop_processing().map_err(native)?;
    let peak = output.finish()?;
    let file_hash = bundle::file_hash(Path::new(&options.path))?;
    let report = serde_json::json!({ "protocolVersion": 1, "path": options.path, "frames": options.frames + options.tail_frames, "sampleRate": options.sample_rate, "sha256": file_hash, "pluginSha256": plugin_hash, "latencyFrames": plugin.latency_samples(), "tailFrames": plugin.tail_samples(), "peak": peak });
    Ok(report)
}
