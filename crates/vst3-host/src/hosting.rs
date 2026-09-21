use crate::{
    bundle, native,
    wire::{Configuration, Request, Source},
    Result,
};
use vst3_host::{Plugin, Vst3Host};

pub(crate) struct Loaded {
    // Plugin must be dropped before its host, on the helper main thread.
    pub plugin: Plugin,
    pub hash: String,
    _host: Vst3Host,
}

pub(crate) fn load(
    source: &Source,
    sample_rate: u32,
    block_size: usize,
    tempo: f64,
    signature: [i32; 2],
    configuration: Option<&Configuration>,
) -> Result<Loaded> {
    source.validate()?;
    let (bundle_path, hash) = bundle::verify(source)?;
    let requested = source.class_id.to_ascii_lowercase();
    if let Some(configuration) = configuration {
        if !configuration.class_id.eq_ignore_ascii_case(&requested)
            || !configuration.sha256.eq_ignore_ascii_case(&hash)
        {
            return Err(crate::Error::new(
                "PluginManifestMismatch",
                "VST3 configuration belongs to a different class or bundle",
            ));
        }
    }
    let mut host = Vst3Host::builder()
        .sample_rate(sample_rate as f64)
        .block_size(block_size)
        .tempo(tempo)
        .time_signature(signature[0], signature[1])
        .output_channels(2)
        .with_process_isolation(false)
        .build()
        .map_err(native)?;
    let plugin = match host.load_plugin_class(&bundle_path, &requested) {
        Ok(plugin) => plugin,
        // Nonstandard metadata can omit a class; actual factory identity is still mandatory.
        Err(error) if error.to_string().contains("not declared") => {
            host.load_plugin(&bundle_path).map_err(native)?
        }
        Err(error) => return Err(native(error)),
    };
    if !plugin.info().uid.eq_ignore_ascii_case(&requested) {
        return Err(crate::Error::new(
            "PluginManifestMismatch",
            "Factory returned a different VST3 class",
        ));
    }
    if bundle::bundle_hash(&bundle_path)? != hash {
        return Err(crate::Error::new(
            "SourceChanged",
            "VST3 bundle changed while loading",
        ));
    }
    Ok(Loaded {
        plugin,
        hash,
        _host: host,
    })
}

/// All VST3 calls run on the isolated helper main thread.
pub fn execute(request: Request) -> Result<serde_json::Value> {
    request.validate()?;
    let options = request.options.as_ref();
    let mut loaded = load(
        &request.source,
        options.map_or(48_000, |o| o.sample_rate),
        options.map_or(128, |o| o.block_size),
        options.map_or(120., |o| o.tempo),
        options.map_or([4, 4], |o| o.time_signature),
        options.and_then(|o| o.configuration.as_ref()),
    )?;
    match request.operation {
        crate::wire::Operation::Inspect => {
            crate::inspection::inspect(&mut loaded.plugin, loaded.hash)
        }
        crate::wire::Operation::Render => crate::render::render(
            &mut loaded.plugin,
            loaded.hash,
            request.options.expect("validated render options"),
        ),
    }
}
