use crate::{transport_codec, transport_wire::Transport};

pub(crate) fn position() -> Transport {
    Transport {
        project_frame: 96000,
        continuous_frame: 256000,
        project_beat: 5.5,
        bar_beat: 3.5,
        tempo: 137.0,
        time_signature: [7, 8],
        playing: true,
        cycle: Some([3.5, 10.5]),
    }
}

#[test]
fn continuation_uses_the_explicit_musical_origin_and_freezes_a_stopped_project() {
    let position = position();
    let next = position.advanced(48000, 48000).unwrap();
    assert_eq!(next.project_frame, 144000);
    assert_eq!(next.continuous_frame, 304000);
    assert!((next.project_beat - (5.5 + 137.0 / 60.0)).abs() < 1e-12);
    assert_eq!(next.bar_beat, 7.0);
    let paused = Transport {
        playing: false,
        ..position
    }
    .advanced(48000, 48000)
    .unwrap();
    assert_eq!(paused.project_frame, position.project_frame);
    assert_eq!(paused.project_beat, position.project_beat);
    assert_eq!(paused.bar_beat, position.bar_beat);
    assert_eq!(paused.continuous_frame, 304000);
    assert!(position.advanced(usize::MAX, 48000).is_none());
    assert!(position.advanced(1, 0).is_none());
}

#[test]
fn transport_codec_preserves_context_and_rejects_reserved_or_invalid_records() {
    let mut bytes = [0u8; transport_codec::BYTES];
    for value in [
        Some(position()),
        None,
        Some(Transport {
            playing: false,
            cycle: None,
            ..position()
        }),
    ] {
        transport_codec::encode(value, &mut bytes).unwrap();
        assert_eq!(transport_codec::decode(&bytes).unwrap(), value);
    }
    for (offset, word) in [
        (0, 8u64),
        (8, u64::MAX),
        (24, f64::NAN.to_bits()),
        (32, 6.0f64.to_bits()),
        (40, 0),
        (48, 3u64 << 32 | 4),
    ] {
        transport_codec::encode(Some(position()), &mut bytes).unwrap();
        bytes[offset..offset + 8].copy_from_slice(&word.to_le_bytes());
        assert!(transport_codec::decode(&bytes).is_err());
    }
    bytes.fill(0);
    bytes[8] = 1;
    assert!(transport_codec::decode(&bytes).is_err());
    transport_codec::encode(Some(position()), &mut bytes).unwrap();
    bytes[0] &= !4;
    assert!(transport_codec::decode(&bytes).is_err());
}

#[test]
fn transport_shares_the_bounded_pcm_packet_in_both_directions() {
    let mut block = crate::stream_wire::Block::new(128);
    block.frames = 17;
    block.transport = Some(position());
    block.audio[..34].fill(0.25);
    for response in [false, true] {
        let mut bytes = Vec::new();
        crate::stream_codec::write_block(&mut bytes, &block, response).unwrap();
        let mut received = crate::stream_wire::Block::new(128);
        crate::stream_codec::read_block(&mut &bytes[..], &mut received, response).unwrap();
        assert_eq!(received.transport, block.transport);
        assert_eq!(received.audio[..34], block.audio[..34]);
        for version in [1u32, 2, 3, 4, 5, 6, 7, 8, 9, 10, 12] {
            bytes[4..8].copy_from_slice(&version.to_le_bytes());
            assert!(
                crate::stream_codec::read_block(&mut &bytes[..], &mut received, response).is_err()
            );
        }
    }
}
