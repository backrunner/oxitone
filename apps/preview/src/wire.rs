use oxitone_core::{
    codes,
    wire::{AllowPlugins, NativeCommand, ProjectSnapshot, RegisterPluginOptions},
    OxitoneError,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::{self, Read, Write};

pub const MAX_FRAME: usize = 64 * 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Frame {
    Snapshot {
        snapshot: Box<ProjectSnapshot>,
        asset_base_dir: String,
        #[serde(default)]
        plugins: Vec<RegisterPluginOptions>,
        #[serde(default)]
        plugin_uis: Value,
        allow_plugins: Option<AllowPlugins>,
        hash: String,
    },
    Diagnostic {
        code: String,
        message: String,
        path: Option<String>,
    },
    Status {
        state: String,
    },
    Transport {
        command: Value,
    },
    Query,
    Shutdown,
}

pub fn decode(value: Value) -> Result<Frame, OxitoneError> {
    let version = value
        .get("protocolVersion")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("missing preview protocolVersion"))?;
    oxitone_core::version::check_protocol_version(version)?;
    serde_json::from_value(value).map_err(|e| invalid(&e.to_string()))
}

pub fn transport(mut value: Value) -> Result<NativeCommand, OxitoneError> {
    let object = value
        .as_object_mut()
        .ok_or_else(|| invalid("invalid transport command"))?;
    object.insert("type".into(), json!("transport"));
    serde_json::from_value(value).map_err(|e| invalid(&e.to_string()))
}

pub fn invalid(message: &str) -> OxitoneError {
    OxitoneError::new(codes::INVALID_PROJECT, message)
}

pub fn read_frame(stream: &mut impl Read) -> io::Result<Option<Value>> {
    let mut header = [0u8; 4];
    match stream.read(&mut header[..1])? {
        0 => return Ok(None),
        _ => stream.read_exact(&mut header[1..])?,
    }
    let length = u32::from_be_bytes(header) as usize;
    if length == 0 || length > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid preview frame length",
        ));
    }
    let mut body = vec![0; length];
    stream.read_exact(&mut body)?;
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

pub fn write_frame(stream: &mut impl Write, value: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(value)?;
    if body.len() > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "preview frame too large",
        ));
    }
    stream.write_all(&(body.len() as u32).to_be_bytes())?;
    stream.write_all(&body)?;
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fragmented_frames_versions_and_limits() {
        let value = json!({"protocolVersion":"1.0","type":"query"});
        let mut bytes = vec![];
        write_frame(&mut bytes, &value).unwrap();
        assert!(matches!(
            decode(read_frame(&mut &bytes[..]).unwrap().unwrap()).unwrap(),
            Frame::Query
        ));
        assert!(decode(json!({"protocolVersion":"2.0","type":"query"})).is_err());
        assert!(decode(json!({"protocolVersion":"1.0","type":"setParameter"})).is_err());
        assert!(read_frame(&mut &bytes[..bytes.len() - 1]).is_err());
        assert!(read_frame(&mut &(MAX_FRAME as u32 + 1).to_be_bytes()[..]).is_err());
    }
}
