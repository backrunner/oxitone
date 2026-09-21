use super::*;
use oxitone_core::midi_bytes::{append_sysex, MAX_MIDI_PAYLOAD_BYTES};

#[test]
fn full_sysex_packets_validate_copy_clear_and_fail_without_heap_activity() {
    let mut info: Ready =
        serde_json::from_value(serde_json::to_value(&tests::shared().info).unwrap()).unwrap();
    info.note_output = true;
    let shared = Shared::new(info, 128, 2, true);
    let mut bytes = [0x7d; 4096];
    bytes[0] = 0xf0;
    bytes[4095] = 0xf7;
    let mut payload = Vec::with_capacity(MAX_MIDI_PAYLOAD_BYTES);
    let events: [Event; 4] = std::array::from_fn(|i| Event::SysEx {
        frame: i as u64 * 32,
        data: append_sysex(&mut payload, &bytes).unwrap(),
    });
    let mut port = RealtimePort {
        shared: shared.clone(),
        next_sequence: 0,
        output_events: [events[0]; crate::stream_wire::MAX_EVENTS],
        output_event_count: 0,
        output_payload: Vec::with_capacity(MAX_MIDI_PAYLOAD_BYTES),
    };
    let silence = [0.; 128];
    let (mut left, mut right) = ([0.; 128], [0.; 128]);
    let counts = tests::allocations::count(|| {
        let context = BlockContext {
            payload: &payload,
            reset: true,
            transport: None,
        };
        assert_eq!(
            port.submit_buses(&[[&silence, &silence]], &[events[0]; 5], context),
            Err(PortError::InvalidBlock)
        );
        assert_eq!(
            port.submit(&silence, &silence, &events),
            Err(PortError::InvalidBlock)
        );
        assert_eq!(
            port.submit_buses(&[[&silence, &silence]], &events, context),
            Ok(0)
        );
        let block = shared.pending.pop().unwrap();
        assert_eq!(block.payload, payload);
        assert!(block.reset);
        shared.recycle(block, &shared.completed);
        assert_eq!(
            port.receive(&mut left, &mut right).unwrap().unwrap().frames,
            128
        );
        for event in port.output_events() {
            let Event::SysEx { data, .. } = event else {
                panic!("lost SysEx")
            };
            assert_eq!(data.get(port.output_payload()).unwrap(), bytes);
        }
        assert_eq!(port.receive(&mut left, &mut right), Ok(None));
        assert!(port.output_payload().is_empty() && port.output_events().is_empty());
        shared.stop(Status::PluginFault);
        assert_eq!(port.receive(&mut left, &mut right), Err(PortError::Closed));
        assert!(port.output_payload().is_empty());
    });
    assert_eq!(counts, (0, 0));
}
