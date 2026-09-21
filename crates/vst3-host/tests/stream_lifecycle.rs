#![cfg(all(feature = "stream", target_os = "macos"))]
use oxitone_vst3_host::{
    schedule_wire::Schedule,
    stream::{
        schedule::{Fault, Input, ScheduledPort},
        PortError, Session, Status,
    },
};
use std::time::{Duration, Instant};
#[path = "common/stream_fixture.rs"]
mod fixture;
use fixture::{helper, options, start, wait_until};
#[test]
fn repeated_stream_blocks_preserve_order_and_close_reaps_the_process() {
    let (mut session, mut port) = Session::spawn(helper(), start("echo"), options()).unwrap();
    assert_eq!(session.diagnostics().completed_blocks, 0);
    assert!(!session.diagnostics().helper_time_constraint); // fixture explicitly reports false
    let mut left = [0.; 128];
    let mut right = [0.; 128];
    for i in 0..100 {
        let input = [i as f32 / 100.; 128];
        let frames = if i % 2 == 0 { 128 } else { 17 };
        assert_eq!(port.submit(&input[..frames], &input[..frames], &[]), Ok(i));
        let mut received = None;
        wait_until(|| {
            received = port.receive(&mut left, &mut right).unwrap();
            received.is_some()
        });
        assert_eq!(received.unwrap().sequence, i);
        assert_eq!(received.unwrap().frames, frames);
        assert_eq!(&left[..frames], &input[..frames]);
        assert_eq!(left, right);
        assert!(left[frames..].iter().all(|v| *v == 0.));
    }
    let timing = session.diagnostics();
    assert_eq!(timing.completed_blocks, 100);
    assert!(timing.maximum_send_ms > 0.);
    assert!(timing.maximum_reply_wait_ms > 0.);
    session.close();
    session.close();
    assert_eq!(port.status(), Status::Closed);
    assert_eq!(port.receive(&mut left, &mut right), Err(PortError::Closed));
    assert_eq!(left, [0.; 128]);
    assert_eq!(unsafe { libc::kill(session.pid() as i32, 0) }, -1);
}
#[test]
fn initialization_identity_and_deadline_fail_without_a_live_child() {
    for (mode, code) in [
        ("startup-hang", "PluginHostTimeout"),
        ("wrong-class", "PluginConfigInvalid"),
        ("startup-error", "PluginCapabilityUnsupported"),
    ] {
        let mut settings = options();
        if mode == "startup-hang" {
            settings.startup_timeout = Duration::from_millis(50);
        }
        let error = Session::spawn(helper(), start(mode), settings)
            .err()
            .unwrap();
        assert_eq!(error.code, code);
    }
}
#[test]
fn stuck_crashed_corrupt_and_trickling_hosts_latch_faults_and_can_be_replaced() {
    for (mode, fault) in [
        ("hang", Status::TimedOut),
        ("trickle", Status::TimedOut),
        ("crash", Status::Crashed),
        ("sequence", Status::InvalidResponse),
        ("nan", Status::InvalidResponse),
        ("wrong-buses", Status::InvalidResponse),
        ("fault", Status::PluginFault),
    ] {
        let (mut session, mut port) = Session::spawn(helper(), start(mode), options()).unwrap();
        port.submit(&[0.25; 17], &[0.25; 17], &[]).unwrap();
        wait_until(|| port.status() != Status::Running);
        assert_eq!(port.status(), fault, "{mode}");
        assert_eq!(session.diagnostics().completed_blocks, 0, "{mode}");
        let mut left = [1.; 128];
        let mut right = [1.; 128];
        assert_eq!(port.receive(&mut left, &mut right), Err(PortError::Closed));
        assert_eq!(left, [0.; 128]);
        assert_eq!(right, [0.; 128]);
        session.close();
        assert_eq!(session.status(), fault);
        assert_eq!(unsafe { libc::kill(session.pid() as i32, 0) }, -1);
    }
    let (mut replacement, _) = Session::spawn(helper(), start("echo"), options()).unwrap();
    replacement.close();
}
#[test]
fn control_close_interrupts_an_inflight_hang_without_waiting_for_the_block_deadline() {
    let mut config = options();
    config.block_timeout = Duration::from_secs(10);
    let (mut session, mut port) = Session::spawn(helper(), start("hang"), config).unwrap();
    port.submit(&[0.25; 128], &[0.25; 128], &[]).unwrap();
    std::thread::sleep(Duration::from_millis(10));
    let before = Instant::now();
    session.close();
    assert!(before.elapsed() < Duration::from_secs(1));
}

#[test]
fn scheduler_deadline_stops_a_hung_helper_before_its_io_timeout() {
    let mut settings = options();
    settings.block_timeout = Duration::from_secs(10);
    let (mut session, port) = Session::spawn(helper(), start("hang"), settings).unwrap();
    let mut scheduled = ScheduledPort::prepare(
        port,
        Schedule {
            schedule_version: 1,
            epoch: 7,
            latency_blocks: 2,
        },
    )
    .unwrap();
    let before = Instant::now();
    let mut left = [1.; 128];
    let mut right = [1.; 128];
    for frame in [0, 128, 256] {
        let result = scheduled.process(
            Input {
                payload: &[],
                epoch: 7,
                frame,
                left: &[0.25; 128],
                right: &[0.25; 128],
                events: &[],
            },
            &mut left,
            &mut right,
        );
        assert_eq!(
            result,
            if frame == 256 {
                Err(Fault::DeadlineMissed)
            } else {
                Ok(())
            }
        );
        assert_eq!(left, [0.; 128]);
        assert_eq!(right, [0.; 128]);
    }
    assert_eq!(session.status(), Status::DeadlineMissed);
    session.close();
    assert!(before.elapsed() < Duration::from_secs(1));
    assert_eq!(session.status(), Status::DeadlineMissed);
    assert_eq!(unsafe { libc::kill(session.pid() as i32, 0) }, -1);
}
