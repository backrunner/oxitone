#![cfg(all(feature = "stream", target_os = "macos"))]
use oxitone_vst3_host::{
    control_wire::Command,
    stream::{Session, Status},
};
use std::time::{Duration, Instant};
#[path = "common/stream_fixture.rs"]
mod fixture;
use fixture::{helper, options, start, wait_until};

#[test]
fn control_and_pcm_share_one_session_without_consuming_each_others_frames() {
    let (mut session, mut port) = Session::spawn(helper(), start("echo"), options()).unwrap();
    let control = session.controller();
    let timeout = Duration::from_secs(1);
    assert!(
        control
            .request(Command::OpenEditor {}, timeout)
            .unwrap()
            .editor_open
    );
    let mut left = [0.; 128];
    let mut right = left;
    for i in 0..100 {
        assert_eq!(
            control
                .request(
                    Command::SetParameter {
                        parameter_id: 9,
                        value: 0.25
                    },
                    timeout
                )
                .unwrap()
                .next_sequence,
            i
        );
        port.submit(&[0.25; 128], &[0.5; 128], &[]).unwrap();
        wait_until(|| port.receive(&mut left, &mut right).unwrap().is_some());
        assert_eq!(left, [0.25; 128]);
        assert_eq!(right, [0.5; 128]);
        assert_eq!(
            control
                .request(Command::Poll {}, timeout)
                .unwrap()
                .next_sequence,
            i + 1
        );
    }
    assert!(
        !control
            .request(Command::CloseEditor {}, timeout)
            .unwrap()
            .editor_open
    );
    session.close();
    assert_eq!(
        control.request(Command::Poll {}, timeout).unwrap_err().code,
        "PluginHostCrashed"
    );
    let (replacement, _port) = Session::spawn(helper(), start("echo"), options()).unwrap();
    assert!(control.request(Command::Poll {}, timeout).is_err());
    assert_eq!(
        replacement
            .controller()
            .request(Command::Poll {}, timeout)
            .unwrap()
            .next_sequence,
        0
    );
}

#[test]
fn rejects_bad_parameters_and_unsupported_editors_without_retiring_valid_audio() {
    let (session, _port) = Session::spawn(helper(), start("control-reject"), options()).unwrap();
    let control = session.controller();
    for (id, value) in [(99, 0.5), (9, -1.), (9, f64::NAN)] {
        assert_eq!(
            control
                .request(
                    Command::SetParameter {
                        parameter_id: id,
                        value
                    },
                    Duration::from_secs(1)
                )
                .unwrap_err()
                .code,
            "PluginConfigInvalid"
        );
    }
    assert_eq!(
        control
            .request(Command::OpenEditor {}, Duration::from_secs(1))
            .unwrap_err()
            .code,
        "PluginCapabilityUnsupported"
    );
    assert_eq!(session.status(), Status::Running);
}

#[test]
fn timeout_and_invalid_control_frames_retire_and_reap_helpers() {
    for (mode, code, status) in [
        ("control-hang", "PluginHostTimeout", Status::TimedOut),
        (
            "control-invalid",
            "PluginHostCrashed",
            Status::InvalidResponse,
        ),
        (
            "control-sequence",
            "PluginHostCrashed",
            Status::InvalidResponse,
        ),
        ("control-fault", "RealtimeFault", Status::PluginFault),
    ] {
        let (mut session, _port) = Session::spawn(helper(), start(mode), options()).unwrap();
        assert_eq!(
            session
                .controller()
                .request(Command::Poll {}, Duration::from_millis(50))
                .unwrap_err()
                .code,
            code
        );
        wait_until(|| session.status() == status);
        session.close();
        assert_eq!(unsafe { libc::kill(session.pid() as i32, 0) }, -1);
    }
}

#[test]
fn close_interrupts_a_pending_control_call_without_waiting_for_its_timeout() {
    let (mut session, _port) = Session::spawn(helper(), start("control-hang"), options()).unwrap();
    let control = session.controller();
    let waiter =
        std::thread::spawn(move || control.request(Command::Poll {}, Duration::from_secs(10)));
    std::thread::sleep(Duration::from_millis(30));
    let before = Instant::now();
    session.close();
    assert!(waiter.join().unwrap().is_err());
    assert!(before.elapsed() < Duration::from_secs(1));
}
