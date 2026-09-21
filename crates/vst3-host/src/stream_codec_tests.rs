use super::*;
use std::io::Cursor;

#[test]
fn midi_only_responses_have_no_pcm_but_retain_frames_events_and_reset() {
    let mut block = Block::new(128);
    block.frames = 17;
    block.bus_count = 0;
    block.reset = true;
    block.audio.fill(f32::NAN); // Not part of the zero-bus response.
    block.event_count = 1;
    block.events[0] = Event::Midi {
        frame: 16,
        message: [0x9f, 60, 100],
    };
    let mut bytes = Vec::new();
    assert!(write_block(&mut bytes, &block, false).is_err());
    write_block(&mut bytes, &block, true).unwrap();
    assert_eq!(bytes.len(), 40 + TRANSPORT_BYTES + 24);
    let mut received = Block::new(128);
    read_block(&mut bytes.as_slice(), &mut received, true).unwrap();
    assert_eq!(
        (received.frames, received.bus_count, received.event_count),
        (17, 0, 1)
    );
    assert!(received.reset);
    assert!(matches!(
        received.events[0],
        Event::Midi {
            frame: 16,
            message: [0x9f, 60, 100]
        }
    ));
    bytes[4..8].copy_from_slice(&10u32.to_le_bytes());
    assert!(read_block(&mut bytes.as_slice(), &mut received, true).is_err());
}

#[test]
fn output_midi_rejects_wrong_direction_parameters_reserved_bytes_and_invalid_messages() {
    let mut block = Block::new(128);
    block.frames = 128;
    block.event_count = 1;
    block.events[0] = Event::Parameter {
        frame: 0,
        parameter_id: 1,
        value: 0.5,
    };
    assert!(write_block(&mut Vec::new(), &block, true).is_err());
    block.events[0] = Event::Midi {
        frame: 127,
        message: [0x9f, 60, 127],
    };
    let mut bytes = Vec::new();
    write_block(&mut bytes, &block, true).unwrap();
    let offset = 40 + TRANSPORT_BYTES + 128 * 8;
    for (field, value) in [
        (0, 1u32),
        (4, 128),
        (8, 1),
        (12, 0x7f3cf0),
        (12, 0x7f3cc0),
        (12, 0x7f8090),
        (16, 1),
        (20, 1),
    ] {
        let mut bad = bytes.clone();
        bad[offset + field..offset + field + 4].copy_from_slice(&value.to_le_bytes());
        assert!(read_block(&mut bad.as_slice(), &mut block, true).is_err());
    }
    block.restart_required = true;
    assert!(write_block(&mut Vec::new(), &block, true).is_err());
}

#[test]
fn restart_flag_requires_a_silent_response_and_cannot_appear_in_requests() {
    let mut block = Block::new(128);
    block.frames = 128;
    block.restart_required = true;
    assert!(write_block(&mut Vec::new(), &block, false).is_err());
    let mut bytes = Vec::new();
    write_block(&mut bytes, &block, true).unwrap();
    let mut decoded = Block::new(128);
    read_block(&mut bytes.as_slice(), &mut decoded, true).unwrap();
    assert!(decoded.restart_required);
    bytes[40 + TRANSPORT_BYTES..44 + TRANSPORT_BYTES].copy_from_slice(&0.25f32.to_le_bytes());
    assert!(read_block(&mut bytes.as_slice(), &mut decoded, true).is_err());
    block.audio[0] = 0.25;
    assert!(write_block(&mut Vec::new(), &block, true).is_err());
    block.audio.fill(0.);
    block.restart_required = false;
    bytes.clear();
    write_block(&mut bytes, &block, false).unwrap();
    bytes[24] |= 8;
    assert!(read_block(&mut bytes.as_slice(), &mut decoded, false).is_err());
}

#[derive(Default)]
struct Counted {
    bytes: Cursor<Vec<u8>>,
    writes: usize,
    reads: usize,
}
impl Write for Counted {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        self.bytes.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl Read for Counted {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.reads += 1;
        self.bytes.read(bytes)
    }
}

#[test]
fn maximum_event_burst_is_one_write_and_two_reads_and_preserves_timing() {
    let mut block = Block::new(4096);
    block.frames = 4096;
    block.processing_micros = u32::MAX;
    block.reset = true;
    block.audio.fill(0.25);
    block.event_count = MAX_EVENTS;
    for (i, event) in block.events.iter_mut().enumerate() {
        *event = Event::Parameter {
            frame: (i * 16) as u64,
            parameter_id: i as u32,
            value: 0.5,
        };
    }
    for response in [false, true] {
        if response {
            for (i, event) in block.events.iter_mut().enumerate() {
                *event = Event::Midi {
                    frame: (i * 16) as u64,
                    message: [0x9f, 60, 100],
                };
            }
        }
        let mut wire = Counted::default();
        write_block(&mut wire, &block, response).unwrap();
        assert_eq!(wire.writes, 1);
        wire.bytes.set_position(0);
        let mut received = Block::new(4096);
        read_block(&mut wire, &mut received, response).unwrap();
        assert_eq!(wire.reads, 2);
        assert_eq!(received.audio, block.audio);
        assert!(received.reset);
        assert_eq!(
            received.processing_micros,
            if response { u32::MAX } else { 0 }
        );
        assert_eq!(received.event_count, MAX_EVENTS);
        if response {
            assert!(matches!(
                received.events[255],
                Event::Midi {
                    frame: 4080,
                    message: [0x9f, 60, 100]
                }
            ));
        }
        if !response {
            assert!(matches!(
                received.events[255],
                Event::Parameter {
                    frame: 4080,
                    parameter_id: 255,
                    value: 0.5
                }
            ));
        }
    }
}

#[test]
fn oversize_blocks_and_truncated_event_payloads_are_rejected_without_panics() {
    let mut block = Block::new(4097);
    block.frames = 4097;
    assert!(write_block(&mut Vec::new(), &block, false).is_err());
    block.frames = 4096;
    block.event_count = 257;
    assert!(write_block(&mut Vec::new(), &block, false).is_err());
    block.event_count = 1;
    let mut bytes = Vec::new();
    write_block(&mut bytes, &block, false).unwrap();
    assert!(read_block(&mut &bytes[..bytes.len() - 1], &mut block, false).is_err());
    bytes[16..20].copy_from_slice(&4097u32.to_le_bytes());
    assert!(read_block(&mut &bytes[..], &mut block, false).is_err());
}
#[test]
fn full_bus_frames_round_trip_and_reject_oversize_or_reserved_bus_headers() {
    let mut block = Block::with_buses(4096, 16);
    block.frames = 4096;
    block.bus_count = 16;
    for (i, channel) in block.audio.chunks_exact_mut(4096).enumerate() {
        channel.fill(i as f32);
    }
    let mut bytes = Vec::new();
    write_block(&mut bytes, &block, false).unwrap();
    let mut decoded = Block::with_buses(4096, 16);
    read_block(&mut &bytes[..], &mut decoded, false).unwrap();
    assert_eq!(decoded.bus_count, 16);
    assert_eq!(decoded.audio, block.audio);
    for (offset, bad) in [(32, 0u32), (32, 17), (32, u32::MAX), (36, 1), (16, 4097)] {
        let mut corrupted = bytes.clone();
        corrupted[offset..offset + 4].copy_from_slice(&bad.to_le_bytes());
        assert!(read_block(&mut &corrupted[..], &mut decoded, false).is_err());
    }
    assert!(read_block(&mut &bytes[..], &mut Block::new(4096), false).is_err());
    assert!(read_block(&mut &bytes[..], &mut Block::with_buses(128, 16), false).is_err());
}
