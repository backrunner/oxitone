//! Main-thread configuration flushing. Never used by a graph or an audio callback.
use crate::{bus_wire::AudioBuses, native, Result};
use vst3_host::{audio::BusAudioBuffers, Plugin, RestartFlags};

/// Service lifecycle/cache notifications only on the plugin's owning control thread.
pub(crate) fn service(plugin: &mut Plugin) -> Result<RestartFlags> {
    let flags = plugin.service_host_requests().map_err(native)?;
    if flags.reload_component() {
        return Err(crate::unsupported(
            "VST3 configuration requires component replacement",
        ));
    }
    Ok(flags)
}

pub(crate) struct SilentProcessing {
    layout: AudioBuses,
    buffers: BusAudioBuffers,
    refresh_values: bool,
}
pub(crate) fn buffers(plugin: &Plugin) -> Result<BusAudioBuffers> {
    // The convenience constructor requires a positive allocation size; VST3 process itself
    // supports zero-sample parameter flushing. Keep valid channel storage but submit zero frames.
    let mut buffers = plugin.create_bus_audio_buffers(1).map_err(native)?;
    buffers.block_size = 0;
    for bus in buffers.inputs.iter_mut().chain(&mut buffers.outputs) {
        for channel in &mut bus.channels {
            channel.clear();
        }
    }
    Ok(buffers)
}
impl SilentProcessing {
    pub fn new(plugin: &mut Plugin) -> Result<Self> {
        let refresh_values = service(plugin)?.param_values_changed();
        let layout = crate::bus_wire::inspect(plugin)?;
        let buffers = buffers(plugin)?;
        Ok(Self {
            layout,
            buffers,
            refresh_values,
        })
    }
    fn refresh(&mut self, plugin: &Plugin) -> Result<bool> {
        let layout = crate::bus_wire::inspect(plugin)?;
        if layout == self.layout {
            return Ok(false);
        }
        self.buffers = buffers(plugin)?;
        self.layout = layout;
        Ok(true)
    }
    pub fn flush(&mut self, plugin: &mut Plugin) -> Result<()> {
        // No unbounded restart loop, even for a plugin that asks to restart in every process call.
        for _ in 0..8 {
            self.refresh_values |= service(plugin)?.param_values_changed();
            self.refresh(plugin)?;
            let latency = plugin.latency_samples();
            plugin
                .process_bus_audio(&mut self.buffers)
                .map_err(native)?;
            let flags = service(plugin)?;
            let changed = self.refresh(plugin)?;
            // Configuration sessions do not record automation/MIDI. Drain bounded feedback queues.
            plugin.get_parameter_changes();
            plugin.take_parameter_edits();
            plugin.take_output_events();
            let refresh_values =
                std::mem::take(&mut self.refresh_values) || flags.param_values_changed();
            if refresh_values {
                // The prior process drained queued edits. Refill at most 4096 entries, avoiding
                // the vendor queue's silent overflow when a bulk notification follows overrides.
                for parameter in crate::inspection::parameters(plugin)? {
                    if !parameter.is_read_only {
                        plugin
                            .set_parameter(parameter.id, parameter.value)
                            .map_err(native)?;
                    }
                }
            }
            if flags.is_empty()
                && !changed
                && !refresh_values
                && latency == plugin.latency_samples()
            {
                return Ok(());
            }
        }
        Err(crate::unsupported(
            "VST3 configuration did not settle after 8 restart rounds",
        ))
    }
}
