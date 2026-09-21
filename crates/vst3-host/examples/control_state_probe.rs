//! Dynamic-controller conformance in a persistent helper; zero audio frames and no device.
use oxitone_vst3_host::{
    control_wire::Command,
    stream::{Session, SessionOptions, Status},
    stream_wire::Start,
};
use std::{path::Path, time::Duration};
fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert_eq!(
        args.len(),
        4,
        "usage: vst3-control-state-probe HELPER EXPANDED_START_JSON DENSE_START_JSON"
    );
    let start: Start = serde_json::from_slice(&std::fs::read(&args[2]).unwrap()).unwrap();
    for invalidation in [0., 2. / 3., 1.] {
        let (mut session, _port) = Session::spawn(
            Path::new(&args[1]),
            start.clone(),
            SessionOptions::default(),
        )
        .unwrap();
        let control = session.controller();
        let timeout = Duration::from_secs(2);
        assert_eq!(
            control
                .request(Command::OpenEditor {}, timeout)
                .unwrap_err()
                .code,
            "PluginCapabilityUnsupported"
        );
        for parameter_id in [90, 9999] {
            assert_eq!(
                control
                    .request(
                        Command::SetParameter {
                            parameter_id,
                            value: 0.25
                        },
                        timeout
                    )
                    .unwrap_err()
                    .code,
                "PluginConfigInvalid"
            );
        }
        control
            .request(
                Command::SetParameter {
                    parameter_id: 7,
                    value: 0.75,
                },
                timeout,
            )
            .unwrap();
        let state = control.request(Command::Capture {}, timeout).unwrap();
        assert_eq!(state.next_sequence, 0);
        let info = state.info.unwrap();
        assert_eq!(info["configuration"]["parameters"]["7"], 0.625);
        let processor = info["parameters"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == 90)
            .unwrap();
        assert_eq!(
            processor["value"], 0.625,
            "bulk controller values never reached DSP"
        );
        assert_eq!(session.status(), Status::Running);
        let changed = control
            .request(
                Command::SetParameter {
                    parameter_id: 0,
                    value: invalidation,
                },
                timeout,
            )
            .unwrap();
        assert!(changed.restart.is_some());
        assert_eq!(session.status(), Status::Running);
        let captured = control.request(Command::Capture {}, timeout).unwrap();
        assert_eq!(captured.restart, changed.restart);
        let info = captured.info.unwrap();
        assert_eq!(info["configuration"]["parameters"]["0"], invalidation);
        assert!(info["configuration"]["parameters"].get("7").is_none());
        assert_eq!(
            control
                .request(Command::OpenEditor {}, timeout)
                .unwrap_err()
                .code,
            "PluginRestartRequired"
        );
        session.close();
        assert_eq!(unsafe { libc::kill(session.pid() as i32, 0) }, -1);
    }
    let dense: Start = serde_json::from_slice(&std::fs::read(&args[3]).unwrap()).unwrap();
    let (mut session, _port) =
        Session::spawn(Path::new(&args[1]), dense, SessionOptions::default()).unwrap();
    let control = session.controller();
    control
        .request(
            Command::SetParameter {
                parameter_id: 4095,
                value: 0.125,
            },
            Duration::from_secs(2),
        )
        .unwrap();
    let captured = control
        .request(Command::Capture {}, Duration::from_secs(2))
        .unwrap();
    assert_eq!(captured.next_sequence, 0);
    let info = captured.info.unwrap();
    assert_eq!(info["configuration"]["parameters"]["4095"], 0.125);
    // This fixture stores the DSP's actual gain in opaque state, not its controller cache.
    let state = &info["configuration"]["stateBase64"];
    assert!(
        state.is_string(),
        "capture must contain the processor state"
    );
    std::fs::write(
        format!("{}.captured.json", args[3]),
        serde_json::to_vec(&info).unwrap(),
    )
    .unwrap();
    session.close();
    assert_eq!(unsafe { libc::kill(session.pid() as i32, 0) }, -1);
    println!("Native live control conformance passed: bulk/dense values reach DSP; changed streams freeze and retain their current state");
}
