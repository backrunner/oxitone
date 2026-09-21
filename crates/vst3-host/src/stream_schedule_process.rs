//! Segment assembly, event rebasing, bounded completion drain and deadline enforcement.
use super::{Fault, Input, ScheduledPort};
use crate::{
    stream::{PortError, Status},
    stream_wire::MAX_EVENTS,
    wire::Event,
};

impl ScheduledPort {
    pub(super) fn process_inner(
        &mut self,
        input: Input<'_>,
        left: &mut [f32],
        right: &mut [f32],
    ) -> Result<(), Fault> {
        let size = self.port.info().block_size;
        let frames = input.left.len();
        if self.port.status() != Status::Running {
            return Err(Fault::Stream(self.port.status()));
        }
        if input.epoch != self.config.epoch || input.frame != self.frame {
            return Err(Fault::Discontinuity);
        }
        if frames == 0
            || frames > size
            || input.right.len() != frames
            || left.len() != frames
            || right.len() != frames
            || input.events.len() > MAX_EVENTS
            || input.payload.len() > oxitone_core::midi_bytes::MAX_MIDI_PAYLOAD_BYTES
            || !crate::event_wire::valid_payloads(input.events, input.payload)
            || input.left.iter().chain(input.right).any(|v| !v.is_finite())
            || (self.port.info().input_channels == 0
                && input.left.iter().chain(input.right).any(|v| *v != 0.))
            || input
                .events
                .iter()
                .any(|e| !self.port.info().valid_event(e, frames))
        {
            return Err(Fault::InvalidBlock);
        }
        self.frame
            .checked_add(frames as u64)
            .ok_or(Fault::FrameOverflow)?;
        // Reject accumulated event overflow before accepting any audio from this call.
        let first_events = input
            .events
            .iter()
            .filter(|e| e.frame() < (size - self.input.frames) as u64)
            .count();
        if self.input.event_count + first_events > MAX_EVENTS {
            return Err(Fault::EventOverflow);
        }
        let first_bytes: usize = input
            .events
            .iter()
            .filter_map(|event| match event {
                Event::SysEx { frame, data } if *frame < (size - self.input.frames) as u64 => {
                    Some(data.length as usize)
                }
                _ => None,
            })
            .sum();
        if self.input.payload.len() + first_bytes > oxitone_core::midi_bytes::MAX_MIDI_PAYLOAD_BYTES
        {
            return Err(Fault::EventOverflow);
        }
        self.collect()?;
        let mut offset = 0;
        while offset < frames {
            let start = self.input.frames;
            let count = (size - start).min(frames - offset);
            self.copy_output(
                &mut left[offset..offset + count],
                &mut right[offset..offset + count],
            )?;
            self.input.audio[start..start + count]
                .copy_from_slice(&input.left[offset..offset + count]);
            self.input.audio[size + start..size + start + count]
                .copy_from_slice(&input.right[offset..offset + count]);
            for event in input.events {
                let frame = event.frame() as usize;
                if (offset..offset + count).contains(&frame) {
                    let mut event = *event;
                    if let Event::SysEx { data, .. } = &mut event {
                        *data = oxitone_core::midi_bytes::append_sysex(
                            &mut self.input.payload,
                            data.get(input.payload).ok_or(Fault::InvalidBlock)?,
                        )
                        .ok_or(Fault::EventOverflow)?;
                    }
                    let (Event::Parameter { frame, .. }
                    | Event::Midi { frame, .. }
                    | Event::SysEx { frame, .. }
                    | Event::NoteOn { frame, .. }
                    | Event::NoteOff { frame, .. }) = &mut event;
                    *frame = (*frame - offset as u64) + start as u64;
                    self.input.events[self.input.event_count] = event;
                    self.input.event_count += 1;
                }
            }
            self.input.frames += count;
            self.frame += count as u64;
            offset += count;
            if self.input.frames == size {
                let (l, r) = self.input.audio.split_at(size);
                let events = &self.input.events[..self.input.event_count];
                let submission = self.port.submit_buses(
                    &[[l, r]],
                    events,
                    crate::stream::BlockContext {
                        transport: self.input.transport.take(),
                        reset: false,
                        payload: &self.input.payload,
                    },
                );
                let sequence = submission.map_err(|error| match error {
                    PortError::Full => Fault::QueueFull,
                    PortError::Closed => Fault::Stream(self.port.status()),
                    _ => Fault::InvalidBlock,
                })?;
                if sequence != self.frame / size as u64 - 1 {
                    return Err(Fault::InvalidResponse);
                }
                self.input.frames = 0;
                self.input.event_count = 0;
                self.input.payload.clear();
            }
        }
        Ok(())
    }
    fn collect(&mut self) -> Result<(), Fault> {
        // Bound work even if the IO worker keeps publishing while we drain.
        for _ in 0..self.port.shared.completed.capacity() {
            let received = self
                .port
                .receive(&mut self.receive_left, &mut self.receive_right)
                .map_err(|_| Fault::Stream(self.port.status()))?;
            let Some(received) = received else { break };
            if received.restart_required {
                return Err(Fault::RestartRequired);
            }
            if received.sequence != self.next_completion
                || received.sequence >= self.port.next_sequence
                || received.frames != self.port.info().block_size
            {
                return Err(Fault::InvalidResponse);
            }
            let slot_count = self.output.len();
            let slot = &mut self.output[(received.sequence % slot_count as u64) as usize];
            if slot.sequence.is_some() {
                return Err(Fault::InvalidResponse);
            }
            let (l, r) = slot.audio.split_at_mut(received.frames);
            l.copy_from_slice(&self.receive_left);
            r.copy_from_slice(&self.receive_right);
            slot.sequence = Some(received.sequence);
            self.next_completion += 1;
        }
        Ok(())
    }
    fn copy_output(&mut self, left: &mut [f32], right: &mut [f32]) -> Result<(), Fault> {
        let Some(frame) = self.frame.checked_sub(self.delay_frames as u64) else {
            return Ok(());
        };
        let size = self.port.info().block_size;
        let sequence = frame / size as u64;
        let offset = (frame % size as u64) as usize;
        let slot_count = self.output.len();
        let slot = &mut self.output[(sequence % slot_count as u64) as usize];
        if slot.sequence != Some(sequence) {
            return Err(Fault::DeadlineMissed);
        }
        left.copy_from_slice(&slot.audio[offset..offset + left.len()]);
        right.copy_from_slice(&slot.audio[size + offset..size + offset + right.len()]);
        if offset + left.len() == size {
            slot.sequence = None;
        }
        Ok(())
    }
}
