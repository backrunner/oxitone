use super::*;

impl Engine {
    pub(super) fn transport(&mut self, cmd: NativeCommand) -> Result<(), OxitoneError> {
        let NativeCommand::Transport {
            command,
            frame,
            beat,
            seconds,
            loop_region,
        } = cmd
        else {
            return Err(wire::invalid("preview transport only"));
        };
        let current = self
            .current
            .as_ref()
            .ok_or_else(|| wire::invalid("no accepted project"))?;
        if [frame.is_some(), beat.is_some(), seconds.is_some()]
            .into_iter()
            .filter(|set| *set)
            .count()
            > 1
        {
            return Err(wire::invalid("transport position is ambiguous"));
        }
        let position = match (frame, beat, seconds) {
            (Some(frame), _, _) => Some(frame),
            (_, Some(beat), _) => {
                if beat.to_f64() < 0. {
                    return Err(wire::invalid("beat must be non-negative"));
                }
                Some(current.plan.tempo.beat_to_frame(beat))
            }
            (_, _, Some(seconds)) => Some(current.plan.tempo.seconds_to_frame(seconds)?),
            _ => None,
        };
        if loop_region.is_some_and(|region| region.end_frame <= region.start_frame) {
            return Err(wire::invalid("loop end must follow start"));
        }
        if command == TransportCommandKind::Play && self.session.is_none() {
            let graph = self
                .graph
                .take()
                .ok_or_else(|| wire::invalid("missing graph"))?;
            let started = if self.simulated {
                RealtimeSession::start_simulated(
                    graph,
                    RealtimeConfig::default(),
                    SimulatedSinkConfig {
                        sample_rate: f64::from(current.snapshot.sample_rate),
                        frames_per_slice: current.snapshot.block_size,
                        channels: 2,
                        latency_frames: 0,
                        safety_offset_frames: 0,
                    },
                    None,
                )
            } else {
                RealtimeSession::start(graph, RealtimeConfig::default())
            };
            match started {
                Ok(session) => self.session = Some(session),
                Err(failure) => {
                    self.graph = Some(failure.graph);
                    return Err(failure.error);
                }
            }
        }
        let command = match command {
            TransportCommandKind::Play => TransportCmd::Play {
                from: position,
                loop_region: loop_region.map(|region| (region.start_frame, region.end_frame)),
            },
            TransportCommandKind::Pause => TransportCmd::Pause,
            TransportCommandKind::Stop => TransportCmd::Stop,
            TransportCommandKind::Seek => TransportCmd::Seek {
                frame: position.ok_or_else(|| wire::invalid("seek requires position"))?,
            },
        };
        if let Some(session) = &self.session {
            session.transport(command)?;
        } else if let Some(graph) = &mut self.graph {
            match command {
                TransportCmd::Seek { frame } => graph.seek(frame),
                TransportCmd::Stop => {
                    graph.seek(0);
                    graph.transport_mut().state = TransportState::Stopped;
                }
                TransportCmd::Pause => graph.transport_mut().state = TransportState::Paused,
                _ => {}
            }
        }
        Ok(())
    }
}
