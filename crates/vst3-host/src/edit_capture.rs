//! Pending vendor validation and positioning of the bounded capture journal.
use super::*;
impl Capture {
    pub(super) fn fail(&mut self, code: &str, message: &str) {
        self.status = Status::Failed;
        self.pending.clear();
        self.error = Some(Failure {
            code: code.into(),
            message: message.into(),
        });
    }
    pub(super) fn collect(&mut self, edits: Vec<ParameterEdit>, count: Option<u64>, ready: &Ready) {
        if self.status != Status::Recording {
            return;
        }
        if count != Some(self.overflow_count) {
            self.fail("BudgetExceeded", "VST3 vendor editor feedback lost events");
            return;
        }
        if self.pending.len() + edits.len() > CAPACITY {
            self.fail("BudgetExceeded", "VST3 pending gesture budget exceeded");
            return;
        }
        for edit in edits {
            let parameter = ready
                .parameters
                .binary_search_by_key(&edit.id, |p| p.id)
                .ok()
                .map(|index| &ready.parameters[index]);
            let valid_value = match edit.kind {
                ParameterEditKind::ValueChange => edit
                    .value
                    .is_some_and(|value| value.is_finite() && (0.0..=1.0).contains(&value)),
                _ => edit.value.is_none(),
            };
            if !parameter.is_some_and(|p| p.writable && p.automatable) || !valid_value {
                self.fail(
                    "PluginConfigInvalid",
                    "VST3 editor reported an invalid or non-automatable parameter gesture",
                );
                return;
            }
            if let Some(recording) = &mut self.recording {
                if let Err(error) = recording.collect(&edit) {
                    self.fail(error.code, &error.message);
                    return;
                }
            }
            self.pending.push(edit);
        }
    }
    pub(super) fn position(&mut self, position: Position) {
        if !matches!(self.status, Status::Recording | Status::Stopping)
            || !position.transport.playing
        {
            return;
        }
        if self.events.len() + self.pending.len() > CAPACITY
            || self.next.saturating_add(self.pending.len() as u64) > MAX_SEQUENCE
        {
            self.fail(
                "BudgetExceeded",
                "VST3 unacknowledged gesture budget exceeded",
            );
            return;
        }
        for edit in self.pending.drain(..) {
            self.events.push_back(Event {
                sequence: self.next,
                parameter_id: edit.id,
                value: edit.value,
                frames: None,
                position,
                kind: match edit.kind {
                    ParameterEditKind::BeginGesture => Kind::Begin,
                    ParameterEditKind::ValueChange => Kind::Value,
                    ParameterEditKind::EndGesture => Kind::End,
                },
            });
            self.next += 1;
        }
        if self.status == Status::Stopping {
            self.status = Status::Stopped;
            self.end = Some(position);
        }
    }
    pub(super) fn samples(&mut self, position: Position, frames: usize) {
        if self.status != Status::Recording || !position.transport.playing {
            return;
        }
        let Some(recording) = &self.recording else {
            return;
        };
        let count = recording.values().count();
        if self.events.len() + count > CAPACITY
            || self.next.saturating_add(count as u64) > MAX_SEQUENCE
        {
            self.fail("BudgetExceeded", "VST3 recording sample budget exceeded");
            return;
        }
        for (parameter_id, value) in recording.values() {
            self.events.push_back(Event {
                sequence: self.next,
                parameter_id,
                value: Some(value),
                frames: Some(frames),
                position,
                kind: Kind::Sample,
            });
            self.next += 1;
        }
    }
}
