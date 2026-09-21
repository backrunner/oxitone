#![cfg(all(feature = "stream", target_os = "macos"))]
use oxitone_vst3_host::{
    manager_wire::ManagerOptions, schedule_wire::Schedule, stream::managed::Controller,
};
#[path = "common/stream_fixture.rs"]
mod fixture;
use fixture::{helper, options, start, wait_until};

#[test]
fn epoch_change_or_audio_drop_during_startup_reaps_the_obsolete_helper() {
    for close in [false, true] {
        let (mut control, audio) = Controller::new(ManagerOptions {
            manager_version: 1,
            sample_rate: 48000,
            block_size: 128,
            capacity: 2,
        })
        .unwrap();
        let mut audio = Some(audio);
        let path = std::env::temp_dir().join(format!(
            "oxitone-startup-gate-{}-{close}.vst3",
            std::process::id()
        ));
        let entered = path.with_extension("vst3.entered");
        let release = path.with_extension("vst3.release");
        let mut request = start("echo");
        request.source.bundle_path = path.to_str().unwrap().into();
        let executable = helper().clone();
        let preparing = std::thread::spawn(move || {
            let result = control.prepare(
                &executable,
                request,
                Schedule {
                    schedule_version: 1,
                    epoch: 1,
                    latency_blocks: 2,
                },
                options(),
            );
            (control, result)
        });
        wait_until(|| entered.exists());
        if close {
            drop(audio.take());
        } else {
            audio.as_mut().unwrap().require_epoch(2).unwrap();
        }
        std::fs::write(&release, b"continue").unwrap();
        let (mut control, result) = preparing.join().unwrap();
        assert_eq!(
            result.unwrap_err().code,
            if close {
                "RealtimeFault"
            } else {
                "SourceChanged"
            }
        );
        assert_eq!(control.live_sessions(), 0);
        let pid = std::fs::read_to_string(&entered)
            .unwrap()
            .parse::<i32>()
            .unwrap();
        assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
        if !close {
            control
                .prepare(
                    helper(),
                    start("echo"),
                    Schedule {
                        schedule_version: 1,
                        epoch: 2,
                        latency_blocks: 2,
                    },
                    options(),
                )
                .unwrap();
            assert_eq!(
                audio
                    .as_mut()
                    .unwrap()
                    .activate_next()
                    .unwrap()
                    .unwrap()
                    .epoch,
                2
            );
        }
        std::fs::remove_file(entered).unwrap();
        std::fs::remove_file(release).unwrap();
    }
}
