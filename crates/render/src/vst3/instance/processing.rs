//! Background processing of one graph segment and every physical VST3 audio bus.
use super::*;
use oxitone_graph::execution::IsolatedMode;
use oxitone_vst3_host::{stream::Status, stream_wire::ProcessingMode, transport_wire::Transport};
use std::time::{Duration, Instant};

impl Instance {
    pub(super) fn process_external(
        &mut self,
        ctx: &mut ProcessContext<'_>,
        position: &ProcessPosition,
    ) -> Result<(), OxitoneError> {
        self.midi.output.clear();
        self.midi.output_payload.clear();
        if self
            .port
            .as_ref()
            .is_some_and(RealtimePort::restart_required)
        {
            return Err(OxitoneError::new(
                "PluginRestartRequired",
                "VST3 state is frozen; capture it before rebuilding the graph",
            ));
        }
        let mode = match position.mode {
            IsolatedMode::Realtime => ProcessingMode::Realtime,
            IsolatedMode::Offline => ProcessingMode::Offline,
        };
        if self.start.options.processing_mode != mode || (self.dirty && self.failed) {
            self.start.options.processing_mode = mode;
            self.spawn(true)?;
        }
        if self.failed {
            return Err(OxitoneError::new(
                "PluginHostCrashed",
                "VST3 instance requires reset after fault",
            ));
        }
        if ctx.frames == 0
            || ctx.frames > self.silence.len()
            || ctx.outputs.len() != 2
            || ctx.sidechain.is_some_and(|channels| {
                channels.len() != 2 || channels.iter().any(|c| c.len() < ctx.frames)
            })
        {
            return Err(invalid("unsupported VST3 graph process layout"));
        }
        if !self.options.metadata.note_input && !ctx.note_events.is_empty() {
            return Err(OxitoneError::new(
                "PluginCapabilityUnsupported",
                "VST3 MIDI generator has no note input",
            ));
        }
        events::translate(
            ctx,
            &self.midi.input,
            &self.midi.input_payload,
            self.midi.controls_parameters,
            &mut self.start.options.parameters,
            &mut self.events,
        )?;
        let port = self
            .port
            .as_mut()
            .ok_or_else(|| invalid("VST3 instance is not prepared"))?;
        let transport = Transport {
            project_frame: position.project_frame,
            continuous_frame: position.continuous_frame,
            project_beat: position.project_beat,
            bar_beat: position.bar_beat,
            tempo: position.tempo,
            time_signature: position.time_signature,
            playing: position.playing,
            cycle: position.cycle,
        };
        let (input_l, input_r) = if ctx.inputs.is_empty() {
            (&self.silence[..ctx.frames], &self.silence[..ctx.frames])
        } else if ctx.inputs.len() == 2 {
            (&ctx.inputs[0][..ctx.frames], &ctx.inputs[1][..ctx.frames])
        } else {
            return Err(invalid("unsupported VST3 graph input layout"));
        };
        let sidechain = ctx
            .sidechain
            .map_or([&self.silence[..ctx.frames]; 2], |sc| {
                [&sc[0][..ctx.frames], &sc[1][..ctx.frames]]
            });
        let mut inputs = [[&self.silence[..ctx.frames]; 2]; oxitone_vst3_host::bus_wire::MAX_BUSES];
        for (target, [left, right]) in inputs.iter_mut().zip(&self.audio_inputs).skip(1) {
            *target = [&left[..ctx.frames], &right[..ctx.frames]];
        }
        inputs[0] = [input_l, input_r];
        if ctx.sidechain.is_some() {
            inputs[1] = sidechain;
        }
        let count = port.info().audio_buses.input_count();
        if count == 1 && ctx.sidechain.is_some() {
            return Err(invalid("VST3 has no sidechain input bus"));
        }
        let sequence = port
            .submit_buses(
                &inputs[..count],
                &self.events,
                oxitone_vst3_host::stream::BlockContext {
                    payload: &self.midi.input_payload,
                    transport: Some(transport),
                    reset: self.dirty,
                },
            )
            .map_err(|e| invalid(format!("VST3 submit: {e:?}")))?;
        self.midi.input.clear();
        self.midi.input_payload.clear();
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            let count = self.audio_outputs.len();
            let mut outputs: [[&mut [f32]; 2]; oxitone_vst3_host::bus_wire::MAX_BUSES] =
                std::array::from_fn(|_| [&mut [][..], &mut [][..]]);
            for (target, [left, right]) in outputs.iter_mut().zip(&mut self.audio_outputs) {
                *target = [left, right];
            }
            match port.receive_buses(&mut outputs[..count]) {
                Ok(Some(result)) => {
                    if result.restart_required {
                        return Err(OxitoneError::new(
                            "PluginRestartRequired",
                            "VST3 processing layout changed; the block was muted",
                        ));
                    }
                    if result.sequence != sequence || result.frames != ctx.frames {
                        return Err(invalid("VST3 completion does not match graph segment"));
                    }
                    for (index, output) in ctx.outputs.iter_mut().enumerate() {
                        if let Some(main) = self.audio_outputs.first() {
                            output[..ctx.frames].copy_from_slice(&main[index][..ctx.frames]);
                        } else {
                            output[..ctx.frames].fill(0.);
                        }
                    }
                    self.processed = true;
                    self.midi
                        .receive(port.output_events(), port.output_payload())?;
                    self.dirty = false;
                    return Ok(());
                }
                Err(_) => {
                    return Err(OxitoneError::new(
                        if port.status() == Status::TimedOut {
                            "PluginHostTimeout"
                        } else {
                            "PluginHostCrashed"
                        },
                        format!("VST3 stream failed: {:?}", port.status()),
                    ))
                }
                Ok(None) if Instant::now() >= deadline => {
                    return Err(OxitoneError::new(
                        "PluginHostTimeout",
                        "VST3 graph completion deadline exceeded",
                    ))
                }
                Ok(None) => oxitone_vst3_host::stream::wait_for_completion(),
            }
        }
    }
}
