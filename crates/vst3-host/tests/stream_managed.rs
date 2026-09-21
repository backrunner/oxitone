#![cfg(all(feature = "stream", target_os = "macos"))]
use oxitone_vst3_host::{
    manager_wire::ManagerOptions,
    schedule_wire::Schedule,
    stream::{
        managed::{ActivateError, Controller, ProcessError},
        schedule::Input,
        Status,
    },
};
use std::{
    path::Path,
    time::{Duration, Instant},
};
#[path = "common/stream_fixture.rs"]
mod fixture;
use fixture::{helper, options, start, wait_until};

fn manager() -> (Controller, oxitone_vst3_host::stream::managed::AudioSlot) {
    Controller::new(ManagerOptions {
        manager_version: 1,
        sample_rate: 48000,
        block_size: 128,
        capacity: 2,
    })
    .unwrap()
}
fn schedule(epoch: u64) -> Schedule {
    Schedule {
        schedule_version: 1,
        epoch,
        latency_blocks: 2,
    }
}
fn dead(pid: u32) {
    assert_eq!(unsafe { libc::kill(pid as i32, 0) }, -1);
}

#[test]
fn full_capacity_rejects_before_spawn_and_retirement_frees_capacity_only_on_control() {
    let (mut control, mut audio) = manager();
    control
        .prepare(helper(), start("echo"), schedule(0), options())
        .unwrap();
    control
        .prepare(helper(), start("echo"), schedule(1), options())
        .unwrap();
    let first_pid = control.pid(0).unwrap();
    assert_eq!(control.info(0).unwrap().latency_frames, 256);
    assert_eq!(control.plugin_info(0).unwrap().input_channels, 2);
    let second_pid = control.pid(1).unwrap();
    // Missing executable would fail differently if the capacity check happened after spawn.
    assert_eq!(
        control
            .prepare(
                Path::new("/missing/helper"),
                start("echo"),
                schedule(2),
                options()
            )
            .unwrap_err()
            .code,
        "BudgetExceeded"
    );
    assert_eq!(control.live_sessions(), 2);
    assert_eq!(audio.activate_next().unwrap().unwrap().epoch, 0);
    assert_eq!(audio.activate_next().unwrap().unwrap().epoch, 1);
    assert_eq!(control.live_sessions(), 2);
    assert_eq!(control.status(0), Some(Status::Invalidated));
    assert_eq!(control.reclaim(), 1);
    dead(first_pid);
    assert_eq!(control.live_sessions(), 1);
    control
        .prepare(helper(), start("echo"), schedule(2), options())
        .unwrap();
    let queued_pid = control.pid(2).unwrap();
    control.shutdown();
    control.shutdown();
    dead(second_pid);
    dead(queued_pid);
    assert_eq!(control.live_sessions(), 0);
    assert_eq!(audio.activate_next(), Err(ActivateError::Closed));
}

#[test]
fn failed_prepare_does_not_replace_active_and_successful_epochs_cannot_be_reused() {
    let (mut control, mut audio) = manager();
    control
        .prepare(helper(), start("echo"), schedule(8), options())
        .unwrap();
    audio.activate_next().unwrap();
    for (mode, code) in [
        ("wrong-class", "PluginConfigInvalid"),
        ("startup-hang", "PluginHostTimeout"),
    ] {
        let mut config = options();
        if mode == "startup-hang" {
            config.startup_timeout = Duration::from_millis(50);
        }
        assert_eq!(
            control
                .prepare(helper(), start(mode), schedule(9), config)
                .unwrap_err()
                .code,
            code
        );
        assert_eq!(control.live_sessions(), 1);
        assert_eq!(control.status(8), Some(Status::Running));
        assert_eq!(audio.activate_next().unwrap(), None);
        assert_eq!(audio.active().unwrap().epoch, 8);
    }
    for epoch in [0, 8] {
        assert_eq!(
            control
                .prepare(helper(), start("echo"), schedule(epoch), options())
                .unwrap_err()
                .code,
            "PluginConfigInvalid"
        );
    }
    let mut format = start("echo");
    format.options.block_size = 256;
    assert_eq!(
        control
            .prepare(helper(), format, schedule(9), options())
            .unwrap_err()
            .code,
        "PluginConfigInvalid"
    );
    // A failed attempt did not reserve epoch 9; a corrected publication can use it.
    control
        .prepare(helper(), start("echo"), schedule(9), options())
        .unwrap();
    assert_eq!(audio.activate_next().unwrap().unwrap().epoch, 9);
    assert_eq!(control.reclaim(), 1);
}

#[test]
fn helper_crash_before_activation_keeps_old_session_and_reaps_failed_candidate() {
    let (mut control, mut audio) = manager();
    control
        .prepare(helper(), start("echo"), schedule(0), options())
        .unwrap();
    audio.activate_next().unwrap();
    control
        .prepare(helper(), start("echo"), schedule(1), options())
        .unwrap();
    let pid = control.pid(1).unwrap();
    // Exit only after handshake completion: a startup exit is a prepare failure, not the
    // queued-candidate failure under test. Explicit termination avoids a wall-clock race.
    assert_eq!(unsafe { libc::kill(pid as i32, libc::SIGKILL) }, 0);
    wait_until(|| control.status(1) == Some(Status::Crashed));
    assert_eq!(
        audio.activate_next(),
        Err(ActivateError::PreparedFault {
            epoch: 1,
            status: Status::Crashed
        })
    );
    assert_eq!(audio.active().unwrap().epoch, 0);
    assert_eq!(control.status(0), Some(Status::Running));
    assert_eq!(control.reclaim(), 1);
    dead(pid);
}

#[test]
fn shutdown_interrupts_active_io_and_closes_pending_helpers_without_audio_waiting() {
    let (mut control, mut audio) = manager();
    let mut settings = options();
    settings.block_timeout = Duration::from_secs(10);
    control
        .prepare(helper(), start("hang"), schedule(0), settings)
        .unwrap();
    audio.activate_next().unwrap();
    control
        .prepare(helper(), start("echo"), schedule(1), options())
        .unwrap();
    let pids = [control.pid(0).unwrap(), control.pid(1).unwrap()];
    let mut left = [1.; 128];
    let mut right = [1.; 128];
    audio
        .process(
            Input {
                payload: &[],
                epoch: 0,
                frame: 0,
                left: &[0.25; 128],
                right: &[0.25; 128],
                events: &[],
            },
            &mut left,
            &mut right,
        )
        .unwrap();
    std::thread::sleep(Duration::from_millis(10));
    let now = Instant::now();
    // Control owns shutdown; render continues to own AudioSlot throughout.
    std::thread::scope(|scope| {
        let close = scope.spawn(move || {
            control.shutdown();
            control
        });
        while audio.activate_next() != Err(ActivateError::Closed) {
            assert!(now.elapsed() < Duration::from_secs(1));
            std::thread::yield_now();
        }
        left.fill(9.);
        assert_eq!(
            audio.process(
                Input {
                    payload: &[],
                    epoch: 0,
                    frame: 128,
                    left: &[0.; 128],
                    right: &[0.; 128],
                    events: &[]
                },
                &mut left,
                &mut right
            ),
            Err(ProcessError::Closed)
        );
        assert_eq!(left, [0.; 128]);
        assert_eq!(right, [0.; 128]);
        assert_eq!(close.join().unwrap().live_sessions(), 0);
    });
    assert!(now.elapsed() < Duration::from_secs(1));
    for pid in pids {
        dead(pid);
    }
}

#[test]
fn dropping_audio_on_control_thread_makes_reclaim_close_all_unconsumed_sessions() {
    let (mut control, audio) = manager();
    control
        .prepare(helper(), start("echo"), schedule(0), options())
        .unwrap();
    let pid = control.pid(0).unwrap();
    drop(audio);
    assert_eq!(control.reclaim(), 1);
    dead(pid);
    assert_eq!(
        control
            .prepare(helper(), start("echo"), schedule(1), options())
            .unwrap_err()
            .code,
        "RealtimeFault"
    );
}
