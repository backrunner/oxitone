//! Resolve overlaps off-thread into ordered non-overlapping time spans.
use super::{lane_beat, CompiledLane};
use oxitone_core::wire::{AutomationLaneSpec, AutomationPlayback, ProjectSnapshot};
use oxitone_transport::EvalContext;

#[derive(Debug, Clone)]
pub struct LanePlacement {
    pub start: f64,
    pub end: f64,
    pub origin: f64,
}

pub(super) fn compile(
    snapshot: &ProjectSnapshot,
    lane: &AutomationLaneSpec,
    solo_active: bool,
) -> Option<Vec<LanePlacement>> {
    if lane.playback != Some(AutomationPlayback::Playlist) {
        return None;
    }
    let clips: Vec<_> = snapshot
        .automation_clips
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .filter(|c| {
            c.lane_id == lane.id
                && c.enabled != Some(false)
                && snapshot
                    .tracks
                    .iter()
                    .any(|t| t.id == c.track_id && t.audible(solo_active))
        })
        .collect();
    let mut boundaries: Vec<_> = clips
        .iter()
        .flat_map(|c| {
            let start = c.start_beat.to_f64();
            [start, start + c.duration_beats.unwrap().to_f64()]
        })
        .collect();
    boundaries.sort_by(f64::total_cmp);
    boundaries.dedup();
    boundaries
        .windows(2)
        .filter_map(|w| {
            let winner = clips
                .iter()
                .filter(|c| {
                    c.start_beat.to_f64() <= w[0]
                        && c.start_beat.to_f64() + c.duration_beats.unwrap().to_f64() > w[0]
                })
                .max_by(|a, b| {
                    a.start_beat
                        .to_f64()
                        .total_cmp(&b.start_beat.to_f64())
                        .then(a.id.cmp(&b.id))
                })?;
            Some(LanePlacement {
                start: w[0],
                end: w[1],
                origin: winner.start_beat.to_f64(),
            })
        })
        .collect::<Vec<_>>()
        .into()
}

fn position(lane: &CompiledLane, beat: f64) -> Option<(usize, f64)> {
    let Some(spans) = &lane.placements else {
        return Some((0, lane_beat(lane, beat)));
    };
    let index = spans.partition_point(|s| s.start <= beat).checked_sub(1)?;
    let span = &spans[index];
    (beat < span.end).then(|| (index, lane_beat(lane, beat - span.origin)))
}

fn context(lane: &CompiledLane, ctx: &EvalContext) -> EvalContext {
    // Clip sources restart at their local zero. Seeking does not re-anchor their phase.
    if lane.placements.is_some() {
        EvalContext {
            origin_beat: 0.,
            ..*ctx
        }
    } else {
        *ctx
    }
}
pub(super) fn value(lane: &CompiledLane, beat: f64, ctx: &EvalContext) -> Option<f64> {
    let (_, local) = position(lane, beat)?;
    Some(lane.automation.value_at(local, &context(lane, ctx)))
}
pub fn lane_has_edge(lane: &CompiledLane, start: f64, end: f64, ctx: &EvalContext) -> bool {
    match (position(lane, start), position(lane, end)) {
        (Some((i, a)), Some((j, b))) => {
            i != j || b < a || lane.automation.has_edge(a, b, &context(lane, ctx))
        }
        (None, None) => false,
        _ => true,
    }
}
