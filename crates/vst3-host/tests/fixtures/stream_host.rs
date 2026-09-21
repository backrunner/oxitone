//! Standalone adversarial helper fixture, compiled by rustc; no audio device or plugin dependency.
use std::{
    io::{Read, Write},
    os::{fd::FromRawFd, unix::net::UnixStream},
    time::Duration,
};
fn main() {
    let mut socket = unsafe { UnixStream::from_raw_fd(3) };
    let mut size = [0u8; 4];
    socket.read_exact(&mut size).unwrap();
    let mut bytes = vec![0; u32::from_le_bytes(size) as usize];
    socket.read_exact(&mut bytes).unwrap();
    let start = String::from_utf8(bytes).unwrap();
    if start.contains("startup-error") {
        let reply = br#"{"streamProtocolVersion":11,"error":{"code":"PluginCapabilityUnsupported","message":"fixture rejected activation"}}"#;
        socket.write_all(&(reply.len() as u32).to_le_bytes()).unwrap();
        socket.write_all(reply).unwrap();
        return;
    }
    if start.contains("startup-hang") {
        std::thread::sleep(Duration::from_secs(60));
        return;
    }
    if start.contains("startup-gate") {
        let path = start
            .split("\"bundlePath\":\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap();
        std::fs::write(format!("{path}.entered"), std::process::id().to_string()).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while !std::path::Path::new(&format!("{path}.release")).exists() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    let ready = format!(
        r#"{{"streamProtocolVersion":11,"sampleRate":48000,"blockSize":128,"classId":"{}","sha256":"{}","inputChannels":2,"outputChannels":2,"category":"Fx","audioBuses":{{"inputs":[{{"channels":2,"active":true}}],"outputs":[{{"channels":2,"active":true}}]}},"noteInput":true,"noteOutput":false,"latencyFrames":0,"tailFrames":0,"helperTimeConstraint":false,"parameters":[{{"id":9,"writable":true,"automatable":true}}]}}"#,
        if start.contains("wrong-class") {
            "2".repeat(32)
        } else {
            "1".repeat(32)
        },
        "a".repeat(64)
    );
    socket
        .write_all(&(ready.len() as u32).to_le_bytes())
        .unwrap();
    socket.write_all(ready.as_bytes()).unwrap();
    println!("Plugin stdout is deliberately not a protocol channel.");
    let mut sequence = 0;
    let mut editor_open = false;
    loop {
        let mut header = [0u8; 40];
        if socket.read_exact(&mut header[..4]).is_err() {
            return;
        }
        if &header[..4] == b"OXVC" {
            socket.read_exact(&mut size).unwrap();
            let mut request = vec![0; u32::from_le_bytes(size) as usize];
            socket.read_exact(&mut request).unwrap();
            let request = String::from_utf8(request).unwrap();
            if start.contains("control-hang") { std::thread::sleep(Duration::from_secs(60)); return; }
            if request.contains("openEditor") { editor_open = true; }
            if request.contains("closeEditor") { editor_open = false; }
            let reply = if start.contains("control-reject") {
                String::from(r#"{"controlProtocolVersion":1,"ok":false,"error":{"code":"PluginCapabilityUnsupported","message":"no editor"}}"#)
            } else if start.contains("control-fault") {
                String::from(r#"{"controlProtocolVersion":1,"ok":false,"error":{"code":"RealtimeFault","message":"layout changed"}}"#)
            } else {
                let next = if start.contains("control-sequence") { sequence + 1 } else { sequence };
                format!(r#"{{"controlProtocolVersion":{},"ok":true,"state":{{"editorOpen":{editor_open},"nextSequence":{next}}}}}"#, if start.contains("control-invalid") { 2 } else { 1 })
            };
            socket.write_all(b"OXVC").unwrap();
            socket.write_all(&(reply.len() as u32).to_le_bytes()).unwrap();
            socket.write_all(reply.as_bytes()).unwrap();
            continue;
        }
        socket.read_exact(&mut header[4..]).unwrap();
        sequence += 1;
        let frames = u32::from_le_bytes(header[16..20].try_into().unwrap()) as usize;
        let events = u32::from_le_bytes(header[20..24].try_into().unwrap()) as usize;
        if frames > 4096 || events > 256 {
            return;
        }
        let mut transport = [0u8; 72];
        socket.read_exact(&mut transport).unwrap();
        let mut audio = vec![0u8; frames * 8];
        socket.read_exact(&mut audio).unwrap();
        let mut event_data = vec![0u8; events * 24];
        socket.read_exact(&mut event_data).unwrap();
        if start.contains("hang") {
            std::thread::sleep(Duration::from_secs(60));
            return;
        }
        if start.contains("crash") {
            std::process::exit(23);
        }
        if start.contains("fault") {
            header.fill(0);
            header[..4].copy_from_slice(b"OXVF");
            header[4..8].copy_from_slice(&11u32.to_le_bytes());
            socket.write_all(&header).unwrap();
            return;
        }
        header[20..24].copy_from_slice(&0u32.to_le_bytes());
        let reset = u32::from_le_bytes(header[24..28].try_into().unwrap()) & 4;
        header[24..28].copy_from_slice(&(2 | reset).to_le_bytes());
        if start.contains("sequence") {
            header[8] = header[8].wrapping_add(1);
        }
        if start.contains("nan") {
            audio[..4].copy_from_slice(&f32::NAN.to_le_bytes());
        }
        if start.contains("wrong-buses") {
            header[32..36].copy_from_slice(&2u32.to_le_bytes());
            audio.extend_from_within(..);
        }
        socket.write_all(&header).unwrap();
        socket.write_all(&transport).unwrap();
        if start.contains("trickle") {
            for byte in audio {
                socket.write_all(&[byte]).unwrap();
                std::thread::sleep(Duration::from_millis(20));
            }
        } else {
            socket.write_all(&audio).unwrap();
        }
    }
}
