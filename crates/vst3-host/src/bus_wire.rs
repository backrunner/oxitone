//! Physical VST3 bus slots, shared by inspection and the native stream handshake.
use serde::{Deserialize, Serialize};

pub const MAX_BUSES: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AudioBus {
    pub channels: usize,
    pub active: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AudioBuses {
    pub inputs: Vec<AudioBus>,
    pub outputs: Vec<AudioBus>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BusActivation {
    pub inputs: Vec<bool>,
    pub outputs: Vec<bool>,
}
impl AudioBuses {
    pub fn valid(&self) -> bool {
        self.inputs.len() <= MAX_BUSES
            && (!self.outputs.is_empty() || self.inputs.is_empty())
            && self.outputs.len() <= MAX_BUSES
            && self
                .inputs
                .iter()
                .chain(&self.outputs)
                .all(|b| (1..=2).contains(&b.channels))
    }
    pub fn input_count(&self) -> usize {
        self.inputs.len().max(1)
    }
    pub fn capacity(&self) -> usize {
        self.input_count().max(self.outputs.len())
    }
    pub fn stereo(input_channels: usize, output_channels: usize) -> Self {
        Self {
            inputs: if input_channels == 0 {
                vec![]
            } else {
                vec![AudioBus {
                    channels: input_channels,
                    active: true,
                }]
            },
            outputs: vec![AudioBus {
                channels: output_channels,
                active: true,
            }],
        }
    }
}

#[cfg(feature = "host")]
pub(crate) fn inspect(plugin: &vst3_host::Plugin) -> crate::Result<AudioBuses> {
    let layout = plugin.audio_bus_layout().map_err(crate::native)?;
    from_layout(layout)
}

#[cfg(feature = "host")]
pub(crate) fn declared(plugin: &vst3_host::Plugin) -> crate::Result<AudioBuses> {
    from_layout(plugin.declared_audio_bus_layout().map_err(crate::native)?)
}

#[cfg(feature = "host")]
fn from_layout(layout: vst3_host::audio::AudioBusLayout) -> crate::Result<AudioBuses> {
    let convert = |b: vst3_host::audio::AudioBusConfig| AudioBus {
        channels: b.channel_count,
        active: b.active,
    };
    let buses = AudioBuses {
        inputs: layout.inputs.into_iter().map(convert).collect(),
        outputs: layout.outputs.into_iter().map(convert).collect(),
    };
    if !buses.valid() {
        return Err(crate::unsupported(
            "VST3 requires mono/stereo audio buses or a MIDI-only layout without audio buses",
        ));
    }
    Ok(buses)
}

#[cfg(feature = "host")]
pub(crate) fn activate(
    plugin: &mut vst3_host::Plugin,
    activation: Option<&BusActivation>,
) -> crate::Result<AudioBuses> {
    let buses = inspect(plugin)?;
    if let Some(activation) = activation {
        if activation.inputs.len() != buses.inputs.len()
            || activation.outputs.len() != buses.outputs.len()
        {
            return Err(crate::invalid(
                "VST3 activation must specify every physical bus",
            ));
        }
        use vst3_host::{BusDirection, MediaType};
        for (direction, states, before) in [
            (BusDirection::Input, &activation.inputs, &buses.inputs),
            (BusDirection::Output, &activation.outputs, &buses.outputs),
        ] {
            for (index, (&active, old)) in states.iter().zip(before).enumerate() {
                if active != old.active {
                    plugin
                        .set_bus_active(MediaType::Audio, direction, index as i32, active)
                        .map_err(crate::native)?;
                }
            }
        }
    }
    inspect(plugin)
}
