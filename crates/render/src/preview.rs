//! Opt-in native telemetry. Audio writes bounded preallocated queues only.
use crate::{realtime::ring::SpscRing, RenderGraph};
use crossbeam_queue::ArrayQueue;
use oxitone_graph::{NoteEvent, NoteEventKind};
use std::sync::{
    atomic::{AtomicU32, AtomicU64, Ordering},
    Arc,
};

#[derive(Clone, Copy, Debug)]
pub struct NoteEcho {
    pub frame: u64,
    pub epoch: u64,
    pub pitch: u8,
    pub on: bool,
}

pub struct PreviewNode {
    pub id: String,
    pub audio: SpscRing,
    pub notes: ArrayQueue<NoteEcho>,
    peak: AtomicU32,
    rms: AtomicU32,
    pub dropped: AtomicU64,
}

impl PreviewNode {
    fn new(id: String) -> Self {
        Self {
            id,
            audio: SpscRing::new(4096, 2),
            notes: ArrayQueue::new(1024),
            peak: AtomicU32::new(0),
            rms: AtomicU32::new(0),
            dropped: AtomicU64::new(0),
        }
    }

    /// Display-thread read: peak since last read and the most recent segment RMS.
    pub fn meter(&self) -> (f32, f32) {
        (
            f32::from_bits(self.peak.swap(0, Ordering::Relaxed)),
            f32::from_bits(self.rms.load(Ordering::Relaxed)),
        )
    }

    pub(crate) fn capture(&self, left: &[f32], right: &[f32]) {
        let mut buffer = [0.0; 512];
        let mut peak = 0.0f32;
        let mut square = 0.0f64;
        for start in (0..left.len()).step_by(256) {
            let frames = (left.len() - start).min(256);
            for i in 0..frames {
                let finite = |value: f32| if value.is_finite() { value } else { 0. };
                let l = finite(left[start + i]);
                let r = finite(right[start + i]);
                peak = peak.max(l.abs()).max(r.abs());
                square += f64::from(l).powi(2) + f64::from(r).powi(2);
                buffer[2 * i] = l;
                buffer[2 * i + 1] = r;
            }
            let wrote = self.audio.write_frames(&buffer[..frames * 2]);
            self.dropped
                .fetch_add((frames - wrote) as u64, Ordering::Relaxed);
        }
        self.peak.fetch_max(peak.to_bits(), Ordering::Relaxed);
        let rms = (square / (left.len().max(1) * 2) as f64).sqrt() as f32;
        self.rms.store(rms.to_bits(), Ordering::Relaxed);
    }

    pub(crate) fn echo(&self, frame: u64, epoch: u64, notes: &[NoteEvent]) {
        for note in notes {
            if self
                .notes
                .push(NoteEcho {
                    frame: frame + u64::from(note.frame_offset),
                    epoch,
                    pitch: note.pitch,
                    on: note.kind == NoteEventKind::NoteOn,
                })
                .is_err()
            {
                self.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

pub struct PreviewTelemetry {
    pub channels: Vec<PreviewNode>,
    pub buses: Vec<PreviewNode>,
    pub epoch: AtomicU64,
}

impl PreviewTelemetry {
    pub(crate) fn reset(&self) {
        self.epoch.fetch_add(1, Ordering::Relaxed);
        for node in self.channels.iter().chain(self.buses.iter()) {
            while node.notes.pop().is_some() {}
            node.peak.store(0, Ordering::Relaxed);
            node.rms.store(0, Ordering::Relaxed);
        }
    }
}

impl RenderGraph {
    /// Control thread only. Attach before transferring the graph to a realtime session.
    pub fn enable_preview(&mut self) -> Arc<PreviewTelemetry> {
        let telemetry = Arc::new(PreviewTelemetry {
            channels: self
                .channels
                .iter()
                .map(|node| PreviewNode::new(node.id.clone()))
                .collect(),
            buses: self
                .mixer
                .bus_order()
                .into_iter()
                .map(PreviewNode::new)
                .collect(),
            epoch: AtomicU64::new(0),
        });
        self.preview = Some(telemetry.clone());
        telemetry
    }
}
