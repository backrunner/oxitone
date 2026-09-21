//! Real VST3 bus/activation conformance and IPC timings; never opens an audio device.
use oxitone_vst3_host::{
    bus_wire::BusActivation,
    stream::{BlockContext, PortError, Session, SessionOptions},
    stream_wire::Start,
};
use std::{
    path::Path,
    time::{Duration, Instant},
};

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(
        args.len(),
        4,
        "usage: vst3-bus-probe HELPER START_JSON REPORT_JSON"
    );
    let original: Start = serde_json::from_slice(&std::fs::read(&args[2]).unwrap()).unwrap();
    let mut results = Vec::new();
    for mode in ["sidechain", "inactive-middle", "all-active", "asymmetric"] {
        let mut start = original.clone();
        let sidechain = mode == "sidechain";
        start.source.class_id = format!(
            "6E33225254224A00AA69301AF318797{}",
            if sidechain { "E" } else { "F" }
        );
        start.options.bus_activation = match mode {
            "sidechain" => Some(BusActivation {
                inputs: vec![true; 2],
                outputs: vec![true],
            }),
            "all-active" => Some(BusActivation {
                inputs: vec![true; 3],
                outputs: vec![true; 3],
            }),
            "asymmetric" => Some(BusActivation {
                inputs: vec![true, false, true],
                outputs: vec![false, true, false],
            }),
            _ => None,
        };
        let (mut session, mut port) =
            Session::spawn(Path::new(&args[1]), start, SessionOptions::default()).unwrap();
        let input_count = port.info().audio_buses.input_count();
        let output_count = port.info().audio_buses.outputs.len();
        let middle_active = port.info().audio_buses.inputs[1].active;
        let output_active: Vec<_> = port
            .info()
            .audio_buses
            .outputs
            .iter()
            .map(|b| b.active)
            .collect();
        let signals = [
            0.25,
            0.5,
            if middle_active { 0.125 } else { 0. },
            if middle_active { 0.375 } else { 0. },
            -0.5,
            0.75,
        ];
        let input = signals.map(|value| [value; 128]);
        let mut storage = [[0.; 128]; 6];
        let [a, b, c, d, e, f] = &mut storage;
        let mut output = [
            [&mut a[..], &mut b[..]],
            [&mut c[..], &mut d[..]],
            [&mut e[..], &mut f[..]],
        ];
        let mut timings = Vec::with_capacity(400);
        let mut processed = 0;
        for index in 0..400 {
            let frames = [1, 17, 128, 7, 111, 128][index % 6];
            let inputs = [
                [&input[0][..frames], &input[1][..frames]],
                [&input[2][..frames], &input[3][..frames]],
                [&input[4][..frames], &input[5][..frames]],
            ];
            assert_eq!(
                port.submit(&input[0][..frames], &input[1][..frames], &[]),
                Err(PortError::InvalidBlock)
            );
            let before = Instant::now();
            port.submit_buses(
                &inputs[..input_count],
                &[],
                BlockContext {
                    payload: &[],
                    reset: index % 11 == 0,
                    transport: None,
                },
            )
            .unwrap();
            let deadline = before + Duration::from_secs(2);
            loop {
                if let Some(block) = port.receive_buses(&mut output[..output_count]).unwrap() {
                    assert_eq!(block.sequence, index as u64);
                    assert_eq!(block.frames, frames);
                    break;
                }
                assert!(Instant::now() < deadline);
                oxitone_vst3_host::stream::wait_for_completion();
            }
            timings.push(before.elapsed().as_secs_f64() * 1000.);
            let expected = if sidechain {
                [0.0625, 0.125, 0., 0., 0., 0.]
            } else {
                [
                    0.1875,
                    0.59375,
                    if middle_active { 0.125 } else { 0. },
                    if middle_active { 0.125 } else { 0. },
                    0.125,
                    -0.1875,
                ]
            };
            for (bus_index, bus) in output[..output_count].iter().enumerate() {
                for (channel, samples) in bus.iter().enumerate() {
                    let value = if output_active[bus_index] {
                        expected[bus_index * 2 + channel]
                    } else {
                        0.
                    };
                    assert!(
                        samples[..frames].iter().all(|v| (*v - value).abs() < 1e-6),
                        "{mode}: wrong PCM on bus {bus_index} channel {channel}"
                    );
                    assert!(samples[frames..].iter().all(|v| *v == 0.));
                }
            }
            processed += frames;
        }
        let diagnostics = session.diagnostics();
        session.close();
        timings.sort_by(f64::total_cmp);
        results.push(serde_json::json!({"mode":mode,"blocks":400,"frames":processed,"inputBuses":input_count,"outputBuses":output_count,"roundtripP95Ms":timings[380],"roundtripP99Ms":timings[396],"roundtripSamplesMs":timings,"diagnostics":diagnostics}));
    }
    let report = serde_json::json!({"scenario":"vst3-multibus", "streamProtocolVersion":11,"sampleRate":48000,"maxBlockSize":128,"queueDepth":4,"warmupBlocks":0,"device":null,"callbackP95Ms":null,"callbackP99Ms":null,"xruns":null,"results":results});
    std::fs::write(&args[3], serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    println!("Verified 1600 real VST3 bus blocks, sidechain, inactive slots, mono conversion, short blocks and reset");
}
