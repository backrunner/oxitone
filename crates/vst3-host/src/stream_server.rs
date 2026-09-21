//! Persistent native plugin, confined to the helper main thread and its private duplex socket.
use crate::{
    native, stream_codec as codec,
    stream_wire::{Block, Parameter, Ready, Start, STREAM_VERSION},
    Result,
};
use std::{io::Read, os::unix::net::UnixStream};
use vst3_host::ProcessMode;

pub fn run(socket: &mut UnixStream) {
    let mut started = false;
    if let Err(error) = serve(socket, &mut started) {
        if !started {
            let _ = codec::write_json(
                socket,
                &serde_json::json!({"streamProtocolVersion": STREAM_VERSION, "error": error}),
            );
        }
    }
}
fn serve(socket: &mut UnixStream, started: &mut bool) -> Result<()> {
    if !cfg!(target_os = "macos") {
        return Err(crate::unsupported("macOS VST3 streams only"));
    }
    let value = codec::read_json(socket).map_err(crate::invalid)?;
    let start: Start = serde_json::from_value(value).map_err(crate::invalid)?;
    start.validate()?;
    let options = &start.options;
    let mut loaded = crate::hosting::load(
        &start.source,
        options.sample_rate,
        options.block_size,
        options.transport.map_or(options.tempo, |t| t.tempo),
        options
            .transport
            .map_or(options.time_signature, |t| t.time_signature),
        options.configuration.as_ref(),
    )?;
    let plugin = &mut loaded.plugin;
    crate::configuration::apply(
        plugin,
        options.configuration.as_ref(),
        &options.parameters,
        &[],
    )?;
    plugin
        .set_process_mode(match options.processing_mode {
            crate::stream_wire::ProcessingMode::Realtime => ProcessMode::Realtime,
            crate::stream_wire::ProcessingMode::Offline => ProcessMode::Offline,
        })
        .map_err(native)?;
    let buses = crate::bus_wire::activate(plugin, options.bus_activation.as_ref())?;
    if options.midi_output && plugin.info().has_midi_output {
        plugin
            .set_bus_active(
                vst3_host::MediaType::Event,
                vst3_host::BusDirection::Output,
                0,
                true,
            )
            .map_err(native)?;
    }
    let input = buses.inputs.first().map_or(0, |bus| bus.channels);
    let output = buses.outputs.first().map_or(0, |bus| bus.channels);
    let mut ready = Ready {
        stream_protocol_version: STREAM_VERSION,
        sample_rate: options.sample_rate,
        block_size: options.block_size,
        class_id: plugin.info().uid.to_ascii_lowercase(),
        sha256: loaded.hash,
        input_channels: input,
        output_channels: output,
        audio_buses: buses,
        note_input: plugin.info().has_midi_input,
        note_output: plugin.info().has_midi_output,
        category: plugin.info().category.clone(),
        latency_frames: plugin.latency_samples(),
        tail_frames: plugin.tail_samples(),
        helper_time_constraint: false,
        parameters: crate::inspection::parameters(plugin)?
            .iter()
            .map(|p| Parameter {
                id: p.id,
                writable: !p.is_read_only,
                automatable: p.can_automate,
            })
            .collect(),
    };
    ready.validate(&start)?;
    let mut buffers = plugin
        .create_bus_audio_buffers(options.block_size)
        .map_err(native)?;
    let mut block = Block::with_buses(options.block_size, ready.audio_buses.capacity());
    let mut transport = options
        .transport
        .unwrap_or(crate::transport_wire::Transport {
            project_frame: 0,
            continuous_frame: 0,
            project_beat: 0.0,
            bar_beat: 0.0,
            tempo: options.tempo,
            time_signature: options.time_signature,
            playing: true,
            cycle: None,
        });
    let mut transport_frames = 0usize;
    // State restoration may request a metadata refresh; capabilities were queried after it.
    plugin.take_restart_flags();
    plugin.set_playing(transport.playing).map_err(native)?;
    plugin.start_processing().map_err(native)?;
    let flags = plugin.take_restart_flags();
    if flags.io_changed()
        || flags.reload_component()
        || flags.param_id_mapping_changed()
        || crate::bus_wire::inspect(plugin)? != ready.audio_buses
    {
        return Err(crate::unsupported(
            "VST3 activation requires a new layout/controller",
        ));
    }
    ready.latency_frames = plugin.latency_samples();
    ready.tail_frames = plugin.tail_samples();
    ready.validate(&start)?;
    let mut controls = crate::control_server::Controls::new(plugin, &ready)?;
    let mut midi = crate::output_midi::Capture::new(plugin, options.midi_output)?;
    ready.helper_time_constraint =
        crate::stream_thread::configure(options.sample_rate, options.block_size);
    codec::write_json(socket, &ready).map_err(crate::invalid)?;
    *started = true;
    let mut sequence = 0u64;
    let result = (|| -> Result<()> {
        loop {
            controls.wait(socket, plugin, &ready)?;
            let mut magic = [0; 4];
            socket.read_exact(&mut magic).map_err(crate::invalid)?;
            if &magic == crate::control_wire::MAGIC {
                controls.handle(socket, plugin, &ready, sequence)?;
                continue;
            }
            codec::read_block(&mut magic.as_slice().chain(&mut *socket), &mut block, false)
                .map_err(crate::invalid)?;
            if block.sequence != sequence
                || sequence == u64::MAX
                || block.events[..block.event_count]
                    .iter()
                    .any(|e| !ready.valid_event(e, block.frames))
                || block.bus_count != ready.audio_buses.input_count()
            {
                return Err(crate::invalid(
                    "invalid sequence, event or input in VST3 stream",
                ));
            }
            sequence += 1;
            let processing_start = std::time::Instant::now();
            let processed = (|| -> Result<()> {
                if controls.restart_required() {
                    return Ok(());
                }
                if block.reset {
                    plugin.reset_processing().map_err(native)?;
                    midi.clear()?;
                }
                let frames = block.frames;
                crate::bus_buffers::input(&mut buffers, &block)?;
                if let Some(position) = block.transport {
                    transport = position;
                    transport_frames = 0;
                }
                let position = transport
                    .advanced(transport_frames, options.sample_rate)
                    .ok_or_else(|| crate::invalid("VST3 transport overflow"))?;
                position.apply(plugin)?;
                controls.before_audio(
                    plugin,
                    &ready,
                    crate::edit_wire::Position {
                        audio_sequence: block.sequence,
                        transport: position,
                        reset: block.reset,
                    },
                    frames,
                );
                let events = &mut block.events[..block.event_count];
                events.sort_by_key(|event| (event.frame(), event.priority()));
                controls.apply_events(plugin, events, &block.payload, frames, position.playing)?;
                block.event_count = 0;
                block.payload.clear();
                if !controls.can_process(plugin, &ready)? {
                    return Ok(());
                }
                plugin.process_bus_audio(&mut buffers).map_err(native)?;
                midi.collect(&mut block)?;
                transport_frames = transport_frames
                    .checked_add(frames)
                    .ok_or_else(|| crate::invalid("VST3 transport overflow"))?;
                controls.after_audio(plugin, &ready)?;
                if !controls.restart_required() {
                    crate::bus_buffers::output(&buffers, &mut block)?;
                }
                Ok(())
            })();
            if let Err(error) = processed {
                let _ = codec::write_fault(socket);
                return Err(error);
            }
            if controls.restart_required() {
                block.event_count = 0;
                block.payload.clear();
                block.restart_required = true;
                block.bus_count = ready.audio_buses.outputs.len();
                block.audio[..block.frames * block.bus_count * 2].fill(0.);
            }
            block.processing_micros = processing_start
                .elapsed()
                .as_nanos()
                .div_ceil(1000)
                .min(u32::MAX as u128) as u32;
            codec::write_block(socket, &block, true).map_err(crate::invalid)?;
        }
    })();
    let closed = controls.close(plugin);
    result.and(closed)
}
