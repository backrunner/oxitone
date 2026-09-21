//! Helper-owned gesture capture. Time comes only from an actually processed audio packet.
use crate::{edit_wire::*, stream_wire::Ready, Error, Result};
use std::collections::VecDeque;
use vst3_host::{ParameterEdit, ParameterEditKind, Plugin};
#[path = "edit_capture.rs"]
mod capture;
#[path = "edit_recording.rs"]
mod recording;

#[cfg(test)]
#[path = "edit_journal_tests.rs"]
mod tests;

#[derive(Default)]
pub(crate) struct Journal {
    last_id: u64,
    capture: Option<Capture>,
}
struct Capture {
    id: String,
    status: Status,
    overflow_count: u64,
    next: u64,
    events: VecDeque<Event>,
    pending: Vec<ParameterEdit>,
    end: Option<Position>,
    error: Option<Failure>,
    recording: Option<recording::Recorder>,
}
impl Journal {
    pub fn start(&mut self, plugin: &mut Plugin) -> Result<Page> {
        if self
            .capture
            .as_ref()
            .is_some_and(|c| matches!(c.status, Status::Recording | Status::Stopping))
        {
            return Err(Error::new(
                "PluginTaskConflict",
                "VST3 gesture capture is already active",
            ));
        }
        if let Some(recording) = self.capture.as_ref().and_then(|c| c.recording.as_ref()) {
            recording.restore(plugin)?;
        }
        plugin.take_parameter_edits();
        let count = plugin
            .parameter_edit_overflow_count()
            .ok_or_else(|| crate::unsupported("VST3 backend cannot detect lost editor events"))?;
        if count == u64::MAX {
            return Err(Error::new(
                "BudgetExceeded",
                "VST3 editor overflow counter exhausted",
            ));
        }
        self.last_id = self
            .last_id
            .checked_add(1)
            .ok_or_else(|| Error::new("BudgetExceeded", "VST3 gesture capture ids exhausted"))?;
        self.capture = Some(Capture {
            id: self.last_id.to_string(),
            status: Status::Recording,
            overflow_count: count,
            next: 0,
            events: VecDeque::with_capacity(CAPACITY),
            pending: Vec::with_capacity(CAPACITY),
            end: None,
            error: None,
            recording: None,
        });
        self.read(&self.last_id.to_string(), 0, false)
    }
    pub fn start_recording(
        &mut self,
        plugin: &mut Plugin,
        ready: &Ready,
        mode: RecordingMode,
        ids: Vec<u32>,
    ) -> Result<Page> {
        if !valid_selection(&ids)
            || ids.iter().any(|id| {
                !ready
                    .parameters
                    .iter()
                    .any(|p| p.id == *id && p.writable && p.automatable)
            })
        {
            return Err(crate::invalid(
                "VST3 recording requires writable automatable parameters",
            ));
        }
        self.start(plugin)?;
        match recording::Recorder::new(
            Recording {
                mode,
                parameter_ids: ids,
                sample_rate: ready.sample_rate,
            },
            plugin,
        ) {
            Ok(recording) => self.capture.as_mut().unwrap().recording = Some(recording),
            Err(error) => {
                self.capture
                    .as_mut()
                    .unwrap()
                    .fail(error.code, &error.message);
                return Err(error);
            }
        }
        self.read(&self.last_id.to_string(), 0, false)
    }
    pub fn collect(&mut self, plugin: &mut Plugin, ready: &Ready) {
        // Drain even while unarmed so unrelated audition gestures cannot fill the vendor queue.
        let edits = plugin.take_parameter_edits();
        let count = plugin.parameter_edit_overflow_count();
        if let Some(capture) = &mut self.capture {
            capture.collect(edits, count, ready);
        }
    }
    pub fn before_audio(&mut self, position: Position, frames: usize) {
        if let Some(capture) = &mut self.capture {
            capture.position(position);
            capture.samples(position, frames);
        }
    }
    pub fn filter_audio(
        &mut self,
        plugin: &mut Plugin,
        events: &mut [crate::wire::Event],
        playing: bool,
    ) -> Result<usize> {
        if let Some(capture) = &mut self.capture {
            if let Some(recording) = &mut capture.recording {
                return recording.apply(
                    plugin,
                    events,
                    playing && capture.status == Status::Recording,
                );
            }
        }
        Ok(events.len())
    }
    fn get(&mut self, id: &str) -> Result<&mut Capture> {
        self.capture
            .as_mut()
            .filter(|capture| capture.id == id)
            .ok_or_else(|| {
                Error::new(
                    "PluginTaskConflict",
                    "VST3 gesture capture is no longer current",
                )
            })
    }
    pub fn discard(&mut self, id: &str, plugin: &mut Plugin) -> Result<()> {
        if let Some(recording) = &self.get(id)?.recording {
            recording.restore(plugin)?;
        }
        self.capture = None;
        Ok(())
    }
    pub fn invalidate_for_restart(&mut self) {
        if let Some(capture) = &mut self.capture {
            if matches!(capture.status, Status::Recording | Status::Stopping) {
                capture.fail(
                    "PluginRestartRequired",
                    "VST3 layout changed during capture",
                );
            }
        }
    }
    pub fn discard_frozen(&mut self, id: &str) -> Result<()> {
        self.get(id)?;
        self.capture = None;
        Ok(())
    }
    pub fn read(&mut self, id: &str, from: u64, stop: bool) -> Result<Page> {
        let capture = self.get(id)?;
        let first = capture
            .events
            .front()
            .map_or(capture.next, |event| event.sequence);
        if from < first || from > capture.next {
            return Err(Error::new(
                "PluginTaskConflict",
                "VST3 gesture cursor is expired or ahead of the journal",
            ));
        }
        while capture
            .events
            .front()
            .is_some_and(|event| event.sequence < from)
        {
            capture.events.pop_front();
        }
        if stop && capture.status == Status::Recording {
            capture.status = Status::Stopping;
        }
        Ok(Page {
            capture_id: capture.id.clone(),
            status: capture.status,
            first_sequence: from,
            next_sequence: capture.next,
            pending_events: capture.pending.len(),
            events: if capture.error.is_some() {
                vec![]
            } else {
                capture.events.iter().take(PAGE_SIZE).cloned().collect()
            },
            end_position: capture.end,
            error: capture.error.clone(),
            recording: capture.recording.as_ref().map(|r| r.info.clone()),
        })
    }
}
