use super::*;
use oxitone_core::midi_bytes::append_sysex;

#[test]
fn sysex_bytes_offsets_and_zero_audio_round_trip_then_clear() {
    let mut block = Block::new(128);
    block.frames = 128;
    block.bus_count = 0;
    let mut bytes = [0x7d; 4096];
    bytes[0] = 0xf0;
    bytes[4095] = 0xf7;
    for (index, frame) in [0, 13, 63, 127].into_iter().enumerate() {
        let data = append_sysex(&mut block.payload, &bytes).unwrap();
        block.events[index] = Event::SysEx { frame, data };
    }
    block.event_count = 4;
    let mut wire = Vec::new();
    write_block(&mut wire, &block, true).unwrap();
    assert_eq!(
        wire.len(),
        40 + TRANSPORT_BYTES + 4 * 24 + MAX_MIDI_PAYLOAD_BYTES
    );
    let mut decoded = Block::new(128);
    read_block(&mut wire.as_slice(), &mut decoded, true).unwrap();
    for (index, event) in decoded.events[..4].iter().enumerate() {
        let Event::SysEx { frame, data } = event else {
            panic!("lost SysEx")
        };
        assert_eq!(*frame, [0, 13, 63, 127][index]);
        assert_eq!(data.get(&decoded.payload).unwrap(), bytes);
    }
    for (at, value) in [
        (36, 16385u32),
        (112 + 8, u32::MAX),
        (112 + 12, 4097),
        (112 + 16, 1),
        (4, 10),
    ] {
        let mut bad = wire.clone();
        bad[at..at + 4].copy_from_slice(&value.to_le_bytes());
        assert!(read_block(&mut bad.as_slice(), &mut decoded, true).is_err());
    }
    let payload_start = 112 + 4 * 24;
    for (offset, value) in [(0, 0), (1, 0xf8), (4095, 0)] {
        let mut bad = wire.clone();
        bad[payload_start + offset] = value;
        assert!(read_block(&mut bad.as_slice(), &mut decoded, true).is_err());
    }
    block.payload.clear();
    block.event_count = 0;
    wire.clear();
    write_block(&mut wire, &block, true).unwrap();
    read_block(&mut wire.as_slice(), &mut decoded, true).unwrap();
    assert!(decoded.payload.is_empty());
    assert_eq!(decoded.event_count, 0);
}
