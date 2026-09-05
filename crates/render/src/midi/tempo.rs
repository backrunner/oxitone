//! Conductor track events: tempo (with resampling of continuous segments),
//! time signatures, and markers.
//!
//! SMF only carries discrete tempo events. `step` segments are written
//! exactly at their segment start; `linear`/`exponential` segments are
//! resampled every `resolution` ticks into a staircase approximation (a
//! MIDI format boundary, not an implementation defect — receivers replay
//! the staircase). The BPM curves mirror the closed forms in
//! `oxitone_transport::tempo` so the exported tempo matches audio playback.
//! Consecutive events with an identical microseconds-per-quarter value are
//! emitted only once.

use oxitone_core::beat::Beat;
use oxitone_core::error::OxitoneError;
use oxitone_core::wire::{ProjectSnapshot, TempoCurve};
use oxitone_transport::{TempoMap, TimeSignatureMap};

use super::beat_to_ticks;
use super::smf::{write_varlen, TrackEvent};

/// Microseconds per quarter note, round-half-up, clamped to the SMF 24-bit range.
fn micros_per_quarter(bpm: f64) -> u32 {
    ((60_000_000.0 / bpm) + 0.5)
        .floor()
        .clamp(1.0, 0x00FF_FFFF as f64) as u32
}

const RANK_TIME_SIGNATURE: u8 = 0;
const RANK_TEMPO: u8 = 1;
const RANK_MARKER: u8 = 2;

struct TempoSink {
    events: Vec<TrackEvent>,
    sequence: u64,
    last_micros: Option<u32>,
    count: u32,
}

impl TempoSink {
    fn push(&mut self, tick: u64, bpm: f64) {
        let micros = micros_per_quarter(bpm);
        if self.last_micros == Some(micros) {
            return;
        }
        self.last_micros = Some(micros);
        self.events.push(TrackEvent {
            tick,
            rank: RANK_TEMPO,
            pitch: 0,
            sequence: self.sequence,
            data: vec![
                0xFF,
                0x51,
                0x03,
                (micros >> 16) as u8,
                (micros >> 8) as u8,
                micros as u8,
            ],
        });
        self.sequence += 1;
        self.count += 1;
    }
}

/// Build the sorted conductor track events. Returns the events and the
/// number of tempo meta events (for diagnostics).
pub(crate) fn conductor_events(
    snapshot: &ProjectSnapshot,
    ppq: u32,
    resolution: u32,
) -> Result<(Vec<TrackEvent>, u32), OxitoneError> {
    // Compiling both maps validates them with their dedicated error codes
    // (`TempoRange`, `TempoMapOrder`, `InvalidProject` paths).
    TempoMap::compile(&snapshot.tempo_map, snapshot.sample_rate.max(1))?;
    let signatures = TimeSignatureMap::compile(&snapshot.time_signature_map)?;

    let mut sink = TempoSink {
        events: Vec::new(),
        sequence: 0,
        last_micros: None,
        count: 0,
    };

    for seg in &snapshot.time_signature_map {
        let beat = signatures.bar_beat_to_beat(seg.start_bar, Beat::ZERO)?;
        sink.events.push(TrackEvent {
            tick: beat_to_ticks(beat, ppq),
            rank: RANK_TIME_SIGNATURE,
            pitch: 0,
            sequence: sink.sequence,
            data: vec![
                0xFF,
                0x58,
                0x04,
                seg.numerator.min(255) as u8,
                seg.denominator.trailing_zeros() as u8,
                24,
                8,
            ],
        });
        sink.sequence += 1;
    }

    let table = oxitone_graph::compile::effective_tempo_table(
        snapshot,
        snapshot.sample_rate.max(1),
        snapshot.seed,
        0.0,
    )?;
    let segments = &table;
    for (i, seg) in segments.iter().enumerate() {
        let start_tick = beat_to_ticks(seg.start_beat, ppq);
        let Some(next) = segments.get(i + 1) else {
            sink.push(start_tick, seg.bpm);
            break;
        };
        let end_tick = beat_to_ticks(next.start_beat, ppq);
        match seg.curve.unwrap_or(TempoCurve::Step) {
            TempoCurve::Step => sink.push(start_tick, seg.bpm),
            curve => {
                let start_beat = seg.start_beat.to_f64();
                let length = next.start_beat.to_f64() - start_beat;
                let (b0, b1) = (seg.bpm, next.bpm);
                let mut tick = start_tick;
                while tick < end_tick {
                    let dx = tick as f64 / f64::from(ppq) - start_beat;
                    let bpm = match curve {
                        TempoCurve::Linear => b0 + (b1 - b0) * dx / length,
                        TempoCurve::Exponential => b0 * (dx / length * (b1 / b0).ln()).exp(),
                        TempoCurve::Step => unreachable!(),
                    };
                    sink.push(tick, bpm);
                    tick += u64::from(resolution);
                }
            }
        }
    }

    let mut markers: Vec<_> = snapshot.markers.iter().collect();
    markers.sort_by(|a, b| a.id.cmp(&b.id));
    for marker in markers {
        let name = marker.name.clone().unwrap_or_else(|| marker.id.clone());
        let mut data = vec![0xFF, 0x06];
        write_varlen(name.len() as u64, &mut data);
        data.extend_from_slice(name.as_bytes());
        sink.events.push(TrackEvent {
            tick: beat_to_ticks(marker.start_beat, ppq),
            rank: RANK_MARKER,
            pitch: 0,
            sequence: sink.sequence,
            data,
        });
        sink.sequence += 1;
    }

    sink.events.sort_by_key(|e| (e.tick, e.rank, e.sequence));
    Ok((sink.events, sink.count))
}
