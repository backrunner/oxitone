//! Allocation-free bus-indexed copies. PCM is stereo planar per bus, never flattened across buses.
use super::{PortError, RealtimePort, Received, Status};
use crate::{bus_wire::MAX_BUSES, stream_wire::MAX_EVENTS, transport_wire::Transport, wire::Event};

#[derive(Debug, Clone, Copy, Default)]
pub struct BlockContext<'a> {
    pub transport: Option<Transport>,
    pub reset: bool,
    pub payload: &'a [u8],
}

impl RealtimePort {
    /// Submit every physical input slot (one silent slot for a plugin with no audio inputs).
    /// Inactive buses require silence. Rejected packets do not consume a sequence or queue slot.
    pub fn submit_buses(
        &mut self,
        inputs: &[[&[f32]; 2]],
        events: &[Event],
        context: BlockContext<'_>,
    ) -> Result<u64, PortError> {
        let shared = &self.shared;
        if shared.status() != Status::Running {
            return Err(PortError::Closed);
        }
        let frames = inputs.first().map_or(0, |bus| bus[0].len());
        if inputs.len() != shared.info.audio_buses.input_count()
            || frames == 0
            || frames > shared.max_frames
            || events.len() > MAX_EVENTS
            || context.payload.len() > oxitone_core::midi_bytes::MAX_MIDI_PAYLOAD_BYTES
            || !crate::event_wire::valid_payloads(events, context.payload)
            || context.transport.is_some_and(|t| !t.valid())
            || inputs.iter().enumerate().any(|(index, bus)| {
                let active = shared
                    .info
                    .audio_buses
                    .inputs
                    .get(index)
                    .is_some_and(|b| b.active);
                bus.iter().any(|channel| {
                    channel.len() != frames
                        || channel
                            .iter()
                            .any(|v| !v.is_finite() || (!active && *v != 0.))
                })
            })
            || events
                .iter()
                .any(|event| !shared.info.valid_event(event, frames))
        {
            return Err(PortError::InvalidBlock);
        }
        if self.next_sequence == u64::MAX {
            return Err(PortError::SequenceExhausted);
        }
        let Some(mut block) = shared.free.pop() else {
            return Err(PortError::Full);
        };
        block.sequence = self.next_sequence;
        block.reset = context.reset;
        block.restart_required = false;
        block.transport = context.transport;
        block.frames = frames;
        block.bus_count = inputs.len();
        for (channel, target) in inputs
            .iter()
            .flatten()
            .zip(block.audio.chunks_exact_mut(frames))
        {
            target.copy_from_slice(channel);
        }
        block.events[..events.len()].copy_from_slice(events);
        block.event_count = events.len();
        block.payload.clear();
        block.payload.extend_from_slice(context.payload);
        shared.recycle(block, &shared.pending);
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        Ok(sequence)
    }

    /// Retrieve every output slot. Each channel must have exactly the prepared max-block length.
    /// Empty/fault/invalid calls clear bounded caller storage, and never replay stale PCM.
    pub fn receive_buses(
        &mut self,
        outputs: &mut [[&mut [f32]; 2]],
    ) -> Result<Option<Received>, PortError> {
        self.output_event_count = 0;
        self.output_payload.clear();
        for channel in outputs.iter_mut().take(MAX_BUSES).flatten() {
            for sample in channel.iter_mut().take(self.shared.max_frames) {
                *sample = 0.;
            }
        }
        if outputs.len() != self.shared.info.audio_buses.outputs.len()
            || outputs
                .iter()
                .flatten()
                .any(|c| c.len() != self.shared.max_frames)
        {
            return Err(PortError::InvalidBlock);
        }
        if self.status() != Status::Running {
            return Err(PortError::Closed);
        }
        let Some(block) = self.shared.completed.pop() else {
            return Ok(None);
        };
        let result = Received {
            sequence: block.sequence,
            frames: block.frames,
            restart_required: block.restart_required,
        };
        for (target, source) in outputs
            .iter_mut()
            .flatten()
            .zip(block.audio.chunks_exact(block.frames))
        {
            target[..block.frames].copy_from_slice(source);
        }
        self.output_events[..block.event_count].copy_from_slice(&block.events[..block.event_count]);
        self.output_event_count = block.event_count;
        self.output_payload.extend_from_slice(&block.payload);
        self.shared.recycle(block, &self.shared.free);
        Ok(Some(result))
    }
}
