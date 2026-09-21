use oxitone_vst3_host::{stream::SessionOptions, stream_wire::Start};
use std::{
    path::PathBuf,
    process::Command,
    sync::OnceLock,
    time::{Duration, Instant},
};

pub fn helper() -> &'static PathBuf {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        let directory =
            std::env::temp_dir().join(format!("oxitone-stream-fixture-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("stream-host");
        let status = Command::new("rustc")
            .args(["--edition=2021", "-O"])
            .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/stream_host.rs"))
            .arg("-o")
            .arg(&path)
            .status()
            .unwrap();
        assert!(status.success());
        path
    })
}
pub fn start(mode: &str) -> Start {
    serde_json::from_value(serde_json::json!({"streamProtocolVersion":11,
        "source":{"bundlePath":format!("/tmp/{mode}.vst3"),"classId":"1".repeat(32),"expectedHash":"a".repeat(64),"allowPlugins":"any"},
        "options":{"sampleRate":48000,"blockSize":128,"parameters":{},"tempo":120,"timeSignature":[4,4]}})).unwrap()
}
pub fn options() -> SessionOptions {
    SessionOptions {
        // Fresh locally compiled Mach-O executables may incur first-launch validation under test load.
        startup_timeout: Duration::from_secs(3),
        block_timeout: Duration::from_millis(100),
        queue_depth: 2,
    }
}
pub fn wait_until(mut predicate: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(3);
    while !predicate() {
        assert!(
            Instant::now() < end,
            "stream failed to make bounded progress"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}
