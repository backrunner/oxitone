//! One graph segment: events, instruments, inserts, mixer, and output capture.
use super::*;
use oxitone_core::OxitoneError;
use oxitone_graph::execution::Execution;

impl RenderGraph {
    pub(super) fn process_segment<E: Execution>(
        &mut self,
        out_l: &mut [f32],
        out_r: &mut [f32],
        first: bool,
    ) -> Result<(), OxitoneError> {
        let frames = out_l.len().min(self.block_size);
        if !self.transport.running() {
            for slot in out_l.iter_mut() {
                *slot = 0.0;
            }
            for slot in out_r.iter_mut() {
                *slot = 0.0;
            }
            return Ok(());
        }
        let cur = self.transport.cursor;
        let end = cur + frames as u64;
        let beat = self.plan.tempo.frame_to_beat(cur).to_f64();
        let bpm = self.plan.tempo.bpm_at_frame(cur);
        let position = if self.isolated {
            self.process_position()
        } else {
            Default::default()
        };

        for channel in &mut self.channels {
            channel.stage_initial();
            if let Some(sync) = &channel.slicer_tempo {
                channel
                    .instrument_staged
                    .set(sync.parameter_index, sync.factor(bpm));
            }
        }

        // Host parameter events due at or before this block's start
        // (control rate, ahead of automation application).
        while let Some(event) = self.param_queue.front() {
            if event.frame > cur {
                break;
            }
            let event = self.param_queue.pop_front().expect("front checked");
            crate::bindings::apply_rt_target(self, &event.target, event.value);
        }

        // Control-rate automation and parameter staging.
        apply_bindings(self, beat, first);
        for channel in &mut self.channels {
            for insert in &mut channel.inserts {
                insert.stage_tempo(bpm);
            }
        }
        for beat_param in &mut self.mixer_beat_params {
            if let Some((index, seconds)) = beat_param.state.poll(bpm) {
                self.mixer.set_insert_parameter_at(
                    beat_param.bus_index,
                    beat_param.insert,
                    index,
                    seconds,
                );
            }
        }

        // Note dispatch (swing-aware) and instrument render.
        let eval_ctx = self.eval_ctx;
        let plan = &self.plan;
        self.dispatcher.dispatch(
            plan,
            &mut self.channels,
            &self.channel_index,
            cur,
            end,
            |channel, event_beat| match plan.channels[channel].swing_binding {
                Some(binding) => binding_value_at(&plan.bindings[binding], event_beat, &eval_ctx),
                None => plan.channels[channel].swing,
            },
        );
        if let Some(preview) = &self.preview {
            let epoch = preview.epoch.load(std::sync::atomic::Ordering::Relaxed);
            for (node, channel) in preview.channels.iter().zip(&self.channels) {
                node.echo(cur, epoch, &channel.notes);
            }
        }
        self.process_channels::<E>(frames, cur, &position)?;
        let sample_rate = self.sample_rate;

        // Mixer buses (inputs in channel order: part of the deterministic
        // summation order).
        let inputs = self.channels.iter().flat_map(|channel| {
            std::iter::once(ChannelInput {
                bus_id: &channel.bus_id,
                left: &channel.delayed_l[..frames],
                right: &channel.delayed_r[..frames],
            })
            .chain(
                channel
                    .output_routes
                    .iter()
                    .map(move |route| route.input(frames)),
            )
        });
        self.mixer.process_with::<E>(
            inputs,
            &mut self.master_l[..frames],
            &mut self.master_r[..frames],
            &position,
        )?;

        // Metronome (pre-limiter), then the master protection limiter.
        if let Some(metronome) = &mut self.metronome {
            for slot in &mut self.metro_l[..frames] {
                *slot = 0.0;
            }
            for slot in &mut self.metro_r[..frames] {
                *slot = 0.0;
            }
            let (metro_l, metro_r, master_l, master_r) = (
                &mut self.metro_l,
                &mut self.metro_r,
                &mut self.master_l,
                &mut self.master_r,
            );
            metronome.render_add(
                &self.plan,
                cur,
                &mut metro_l[..frames],
                &mut metro_r[..frames],
            );
            for i in 0..frames {
                master_l[i] += metro_l[i];
                master_r[i] += metro_r[i];
            }
        }
        match &mut self.limiter {
            Some(limiter) => {
                let inputs: [&[f32]; 2] = [&self.master_l[..frames], &self.master_r[..frames]];
                let mut outputs: [&mut [f32]; 2] =
                    [&mut self.limited_l[..frames], &mut self.limited_r[..frames]];
                let mut ctx = oxitone_graph::ProcessContext {
                    frames,
                    sample_rate,
                    inputs: &inputs,
                    outputs: &mut outputs,
                    note_events: &[],
                    parameter_events: &[],
                    sidechain: None,
                };
                limiter.process(&mut ctx);
            }
            None => {
                self.limited_l[..frames].copy_from_slice(&self.master_l[..frames]);
                self.limited_r[..frames].copy_from_slice(&self.master_r[..frames]);
            }
        }

        // NaN/Inf guard: mute the block and latch the fault flag.
        let clean = self.limited_l[..frames]
            .iter()
            .chain(self.limited_r[..frames].iter())
            .all(|x| x.is_finite());
        if clean {
            out_l[..frames].copy_from_slice(&self.limited_l[..frames]);
            out_r[..frames].copy_from_slice(&self.limited_r[..frames]);
        } else {
            self.faulted = true;
            for slot in out_l.iter_mut() {
                *slot = 0.0;
            }
            for slot in out_r.iter_mut() {
                *slot = 0.0;
            }
        }

        if let Some(preview) = &self.preview {
            for (node, channel) in preview.channels.iter().zip(&self.channels) {
                node.capture(&channel.delayed_l[..frames], &channel.delayed_r[..frames]);
            }
            for (index, node) in preview.buses.iter().enumerate() {
                if node.id == "mix_master" {
                    node.capture(&out_l[..frames], &out_r[..frames]);
                } else {
                    let (left, right) = self.mixer.preview_output(index);
                    node.capture(&left[..frames], &right[..frames]);
                }
            }
        }
        self.transport.advance(frames as u64);
        self.continuous_frame = self.continuous_frame.saturating_add(frames as u64);
        Ok(())
    }
}
