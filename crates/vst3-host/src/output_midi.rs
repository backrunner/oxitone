//! Bounded capture of the main VST3 event output. Never silently lose a note-off.
use crate::{
    stream_wire::{Block, MAX_EVENTS},
    wire::Event,
    Result,
};
use vst3_host::{plugin::OutputEventConsumer, Plugin, PluginEvent, PluginEventData};

pub(crate) struct Capture {
    consumer: OutputEventConsumer,
    enabled: bool,
    failures: u64,
}
impl Capture {
    pub fn new(plugin: &Plugin, enabled: bool) -> Result<Self> {
        let consumer = plugin
            .output_event_handle()
            .ok_or_else(|| crate::unsupported("VST3 output event capture unavailable"))?;
        let failures = consumer.failure_count();
        let mut capture = Self {
            consumer,
            enabled,
            failures,
        };
        capture.clear()?;
        Ok(capture)
    }
    pub fn clear(&mut self) -> Result<()> {
        while self.consumer.pop().is_some() {}
        self.check_failures()
    }
    fn check_failures(&mut self) -> Result<()> {
        let failures = self.consumer.failure_count();
        let lost = failures != self.failures;
        self.failures = failures;
        if self.enabled && lost {
            return Err(crate::Error::new(
                "RealtimeFault",
                "VST3 rejected or lost output events",
            ));
        }
        Ok(())
    }
    pub fn collect(&mut self, block: &mut Block) -> Result<()> {
        block.event_count = 0;
        block.payload.clear();
        self.check_failures()?;
        while let Some(event) = self.consumer.pop() {
            if !self.enabled {
                continue;
            }
            if block.event_count == MAX_EVENTS {
                return Err(crate::Error::new(
                    "BudgetExceeded",
                    "VST3 output exceeds 256 events",
                ));
            }
            let converted = if let PluginEventData::Data {
                data_type: 0,
                bytes,
            } = &event.data
            {
                if event.bus_index != 0
                    || event.sample_offset < 0
                    || event.sample_offset as usize >= block.frames
                {
                    return Err(crate::invalid("invalid SysEx output bus/offset"));
                }
                let data = oxitone_core::midi_bytes::append_sysex(&mut block.payload, bytes)
                    .ok_or_else(|| {
                        crate::Error::new(
                            "BudgetExceeded",
                            "invalid or oversized VST3 SysEx output",
                        )
                    })?;
                Event::SysEx {
                    frame: event.sample_offset as u64,
                    data,
                }
            } else {
                convert(&event, block.frames)?
            };
            block.events[block.event_count] = converted;
            block.event_count += 1;
        }
        Ok(())
    }
}

fn convert(event: &PluginEvent, frames: usize) -> Result<Event> {
    if event.bus_index != 0 {
        return Err(crate::unsupported(
            "only main MIDI output bus 0 is routable",
        ));
    }
    if event.sample_offset < 0 || event.sample_offset as usize >= frames {
        return Err(crate::invalid(
            "VST3 MIDI output offset outside process block",
        ));
    }
    let scaled = |v: f32| -> Result<u8> {
        if !v.is_finite() || !(0.0..=1.0).contains(&v) {
            return Err(crate::invalid("invalid VST3 MIDI output value"));
        }
        Ok((v * 127.).round() as u8)
    };
    let voice = |status, channel: i16, pitch: i16, value| -> Result<[u8; 3]> {
        if !(0..16).contains(&channel) || !(0..128).contains(&pitch) {
            return Err(crate::invalid("invalid VST3 MIDI output channel/pitch"));
        }
        Ok([status | channel as u8, pitch as u8, scaled(value)?])
    };
    let message = match event.data {
        PluginEventData::NoteOn {
            channel,
            pitch,
            velocity,
            tuning,
            ..
        } => {
            if tuning != 0. {
                return Err(crate::unsupported(
                    "MIDI output tuning requires Note Expression support",
                ));
            }
            voice(0x90, channel, pitch, velocity)?
        }
        PluginEventData::NoteOff {
            channel,
            pitch,
            velocity,
            tuning,
            ..
        } => {
            if tuning != 0. {
                return Err(crate::unsupported(
                    "MIDI output tuning requires Note Expression support",
                ));
            }
            voice(0x80, channel, pitch, velocity)?
        }
        PluginEventData::PolyPressure {
            channel,
            pitch,
            pressure,
            ..
        } => voice(0xa0, channel, pitch, pressure)?,
        PluginEventData::LegacyMidiCcOut {
            channel,
            control_number,
            value,
            value2,
        } => {
            if !(0..16).contains(&channel) || value > 127 || value2 > 127 {
                return Err(crate::invalid("invalid VST3 MIDI controller output"));
            }
            let channel = channel as u8;
            match control_number {
                0..=127 => [0xb0 | channel, control_number, value],
                128 => [0xd0 | channel, value, 0],
                129 => [0xe0 | channel, value, value2],
                130 => [0xc0 | channel, value, 0],
                _ => {
                    return Err(crate::unsupported(
                        "unsupported VST3 MIDI controller output",
                    ))
                }
            }
        }
        _ => {
            return Err(crate::unsupported(
                "unsupported VST3 output event; MIDI 1.0 channel messages required",
            ))
        }
    };
    Ok(Event::Midi {
        frame: event.sample_offset as u64,
        message,
    })
}

#[cfg(test)]
#[path = "output_midi_tests.rs"]
mod tests;
