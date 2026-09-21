use oxitone_vst3_host::{
    wire::{Request, MAX_REQUEST},
    Error,
};
use std::io::{Read, Write};

fn main() {
    #[cfg(all(feature = "stream", unix))]
    if std::env::args().nth(1).as_deref() == Some("--stream") {
        use std::os::fd::FromRawFd;
        // Rust control-side spawn supplies a dedicated duplex socket at fd 3.
        let mut socket = unsafe { std::os::unix::net::UnixStream::from_raw_fd(3) };
        oxitone_vst3_host::stream_server::run(&mut socket);
        return;
    }
    #[cfg(unix)]
    {
        use std::os::fd::FromRawFd;
        // SDK spawn supplies a dedicated result pipe at fd 3; plugin stdout is discarded.
        let mut output = unsafe { std::fs::File::from_raw_fd(3) };
        let result = run();
        let reply = match result {
            Ok(value) => value,
            Err(error) => serde_json::json!({ "protocolVersion": 1, "error": error }),
        };
        let _ = serde_json::to_writer(&mut output, &reply);
        let _ = output.flush();
    }
    #[cfg(not(unix))]
    std::process::exit(2);
}
fn run() -> oxitone_vst3_host::Result<serde_json::Value> {
    if !cfg!(target_os = "macos") {
        return Err(oxitone_vst3_host::unsupported("macOS VST3 bundles only"));
    }
    let mut input = Vec::new();
    std::io::stdin()
        .take(MAX_REQUEST as u64 + 1)
        .read_to_end(&mut input)
        .map_err(|e| Error::new("InvalidProject", e))?;
    if input.len() > MAX_REQUEST {
        return Err(Error::new("BudgetExceeded", "request exceeds 8 MiB"));
    }
    let value: serde_json::Value =
        serde_json::from_slice(&input).map_err(oxitone_vst3_host::invalid)?;
    #[cfg(target_os = "macos")]
    if value.get("operation").and_then(|v| v.as_str()) == Some("edit") {
        let request = serde_json::from_value(value).map_err(oxitone_vst3_host::invalid)?;
        return oxitone_vst3_host::editor::execute(request);
    }
    if value.get("operation").and_then(|v| v.as_str()) == Some("listClasses") {
        let request = serde_json::from_value(value).map_err(oxitone_vst3_host::invalid)?;
        return oxitone_vst3_host::discovery::execute(request);
    }
    if value.get("operation").and_then(|v| v.as_str()) == Some("configure") {
        let request = serde_json::from_value(value).map_err(oxitone_vst3_host::invalid)?;
        return oxitone_vst3_host::configure::execute(request);
    }
    let request: Request = serde_json::from_value(value).map_err(oxitone_vst3_host::invalid)?;
    request.validate()?;
    oxitone_vst3_host::hosting::execute(request)
}
