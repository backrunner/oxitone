//! Sample-frame transport mapped to the current project tempo and written meter.
use super::RenderGraph;
use oxitone_graph::execution::ProcessPosition;

impl RenderGraph {
    pub(super) fn process_position(&self) -> ProcessPosition {
        let frame = self.transport.cursor;
        let beat = self.plan.tempo.frame_to_beat(frame);
        let (_, beat_in_bar) = self.plan.time_signatures.beat_to_bar_beat(beat);
        let signature = self.plan.time_signatures.signature_at(beat);
        ProcessPosition {
            mode: oxitone_graph::execution::IsolatedMode::Realtime,
            project_frame: frame,
            continuous_frame: self.continuous_frame,
            project_beat: beat.to_f64(),
            bar_beat: beat
                .checked_sub(beat_in_bar)
                .unwrap_or(oxitone_core::Beat::ZERO)
                .to_f64(),
            tempo: self.plan.tempo.bpm_at_frame(frame),
            time_signature: [signature[0] as i32, signature[1] as i32],
            playing: self.transport.running(),
            cycle: self.transport.loop_region.map(|(start, end)| {
                [
                    self.plan.tempo.frame_to_beat(start).to_f64(),
                    self.plan.tempo.frame_to_beat(end).to_f64(),
                ]
            }),
        }
    }
}
