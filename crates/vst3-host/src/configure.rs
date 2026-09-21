//! Headless configuration resolution, sharing the native editor's zero-sample execution path.
use crate::{
    configuration_wire::Options, silent_processing::SilentProcessing, wire::Source, Result,
};
use serde::Deserialize;

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
    Configure,
}
pub fn execute(request: Request) -> Result<serde_json::Value> {
    let _ = request.operation;
    if request.protocol_version != 1 {
        return Err(crate::Error::new(
            "ProtocolVersionUnsupported",
            "unsupported VST3 configuration protocol",
        ));
    }
    request.source.validate()?;
    request.options.validate()?;
    let options = request.options;
    let mut loaded = crate::hosting::load(
        &request.source,
        options.sample_rate,
        options.block_size,
        120.,
        [4, 4],
        options.configuration.as_ref(),
    )?;
    let plugin = &mut loaded.plugin;
    crate::configuration::apply(
        plugin,
        options.configuration.as_ref(),
        &options.parameters,
        &[],
    )?;
    plugin.set_playing(false).map_err(crate::native)?;
    plugin.start_processing().map_err(crate::native)?;
    let result = SilentProcessing::new(plugin).and_then(|mut silent| silent.flush(plugin));
    let stopped = plugin.stop_processing().map_err(crate::native);
    result?;
    stopped?;
    if crate::bundle::bundle_hash(std::path::Path::new(&request.source.bundle_path))? != loaded.hash
    {
        return Err(crate::Error::new(
            "SourceChanged",
            "VST3 bundle changed while configuring",
        ));
    }
    crate::inspection::inspect(plugin, loaded.hash)
}
