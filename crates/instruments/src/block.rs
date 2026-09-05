//! Sample-accurate block splitting over the ABI's two sorted event lists.
//! Both lists are sorted by `frame_offset`; the walker yields contiguous
//! render segments between event offsets. At an offset carrying both kinds,
//! parameter events apply before note events (documented built-in ordering;
//! the ABI guarantees stability, not cross-list priority).

use oxitone_graph::abi::{NoteEvent, ParameterEvent};

/// One step of a block walk.
pub enum Walk<'a, 'e> {
    /// Parameter events at the current offset (applied first).
    Parameters(&'e [ParameterEvent<'a>]),
    /// Note events at the current offset (applied after parameters).
    Notes(&'e [NoteEvent]),
    /// Render `len` frames starting at `offset`.
    Render(usize, usize),
}

/// Walk one block in offset order, invoking `f` for each step. A single
/// callback keeps the borrow story simple for `&mut self` instances.
pub fn walk_block<'a>(
    frames: usize,
    note_events: &'a [NoteEvent],
    parameter_events: &[ParameterEvent<'a>],
    mut f: impl FnMut(Walk<'_, '_>),
) {
    let (mut ni, mut pi, mut pos) = (0usize, 0usize, 0usize);
    while pos < frames {
        let ps = pi;
        while pi < parameter_events.len() && parameter_events[pi].frame_offset as usize <= pos {
            pi += 1;
        }
        if pi > ps {
            f(Walk::Parameters(&parameter_events[ps..pi]));
        }
        let ns = ni;
        while ni < note_events.len() && note_events[ni].frame_offset as usize <= pos {
            ni += 1;
        }
        if ni > ns {
            f(Walk::Notes(&note_events[ns..ni]));
        }
        let next_p = parameter_events
            .get(pi)
            .map(|e| e.frame_offset as usize)
            .unwrap_or(frames);
        let next_n = note_events
            .get(ni)
            .map(|e| e.frame_offset as usize)
            .unwrap_or(frames);
        let end = next_p.min(next_n).min(frames);
        f(Walk::Render(pos, end - pos));
        pos = end;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxitone_graph::abi::NoteEventKind;

    #[test]
    fn splits_at_event_offsets_with_params_before_notes() {
        let notes = [
            NoteEvent {
                frame_offset: 4,
                kind: NoteEventKind::NoteOn,
                pitch: 60,
                velocity: 1.0,
            },
            NoteEvent {
                frame_offset: 4,
                kind: NoteEventKind::NoteOff,
                pitch: 60,
                velocity: 1.0,
            },
        ];
        let params = [ParameterEvent {
            frame_offset: 4,
            parameter_id: "level",
            value: 0.5,
        }];
        let mut order = Vec::new();
        let mut segments = Vec::new();
        walk_block(8, &notes, &params, |step| match step {
            Walk::Parameters(_) => order.push("p"),
            Walk::Notes(n) => order.push(if n.len() == 2 { "n2" } else { "n1" }),
            Walk::Render(offset, len) => segments.push((offset, len)),
        });
        assert_eq!(order, ["p", "n2"]);
        assert_eq!(segments, [(0, 4), (4, 4)]);
    }
}
