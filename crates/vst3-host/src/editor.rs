//! One isolated, silent configuration editor. All plugin and AppKit calls stay on main.
use crate::configuration_wire::Options;
use crate::silent_processing::SilentProcessing;
use crate::{wire::Source, Result};
use serde::Deserialize;

pub(crate) mod window;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    protocol_version: u32,
    operation: Operation,
    source: Source,
    options: Options,
}
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Operation {
    Edit,
}

pub fn execute(request: Request) -> Result<serde_json::Value> {
    let _ = request.operation;
    if request.protocol_version != 1 {
        return Err(crate::Error::new(
            "ProtocolVersionUnsupported",
            "unsupported VST3 editor protocol",
        ));
    }
    request.source.validate()?;
    let options = request.options;
    options.validate()?;
    let mtm = window::initialize()?;
    let mut loaded = crate::hosting::load(
        &request.source,
        options.sample_rate,
        options.block_size,
        120.,
        [4, 4],
        options.configuration.as_ref(),
    )?;
    crate::configuration::apply(
        &mut loaded.plugin,
        options.configuration.as_ref(),
        &options.parameters,
        &[],
    )?;
    loaded.plugin.set_playing(false).map_err(crate::native)?;
    loaded.plugin.start_processing().map_err(crate::native)?;
    let result = (|| {
        let mut silent = SilentProcessing::new(&mut loaded.plugin)?;
        silent.flush(&mut loaded.plugin)?;
        window::show(&mut loaded.plugin, &mut silent, mtm)
    })();
    let stopped = loaded.plugin.stop_processing().map_err(crate::native);
    let accepted = result?;
    stopped?;
    if !accepted {
        return Ok(serde_json::json!({"protocolVersion":1,"accepted":false}));
    }
    if crate::bundle::bundle_hash(std::path::Path::new(&request.source.bundle_path))? != loaded.hash
    {
        return Err(crate::Error::new(
            "SourceChanged",
            "VST3 bundle changed while editing",
        ));
    }
    let info = crate::inspection::inspect(&mut loaded.plugin, loaded.hash)?;
    Ok(serde_json::json!({"protocolVersion":1,"accepted":true,"info":info}))
}
