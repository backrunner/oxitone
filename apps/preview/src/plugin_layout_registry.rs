//! UI failure is local: retain a compatible accepted panel or use the native fallback.
use crate::{plugin_catalog::Catalog, plugin_layout::Layout, plugin_layout_validation::validate};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
};

type Key = (String, String);
#[derive(Clone, Default)]
pub struct Panels {
    pub layouts: BTreeMap<Key, Arc<Layout>>,
    pub errors: BTreeMap<Key, String>,
    pub global_error: Option<String>,
}
impl Panels {
    pub fn error(&self, key: &Key) -> Option<&str> {
        self.errors
            .get(key)
            .map(String::as_str)
            .or(self.global_error.as_deref())
    }
}
pub fn resolve(raw: &Value, catalog: &Catalog, previous: Option<&Panels>) -> Panels {
    let mut result = Panels::default();
    if raw.is_null() {
        return result;
    }
    let entries = raw.as_array().filter(|a| a.len() <= 64);
    if entries.is_none() || raw.to_string().len() > 2 * 1024 * 1024 {
        result.global_error = Some("PluginUiInvalid: expected at most 64 panels / 2 MiB".into());
        retain_all(&mut result, previous, catalog);
        return result;
    }
    let mut seen = HashSet::new();
    for entry in entries.unwrap() {
        let key = entry
            .get("pluginId")
            .and_then(Value::as_str)
            .zip(entry.get("pluginVersion").and_then(Value::as_str))
            .map(|(a, b)| (a.to_owned(), b.to_owned()));
        let Some(key) = key else {
            result.global_error = Some("PluginUiInvalid: missing plugin identity".into());
            continue;
        };
        let parsed: Result<Arc<Layout>, String> = (|| {
            if !seen.insert(key.clone()) {
                return Err("Duplicate panel registration".into());
            }
            if entry.to_string().len() > 256 * 1024 {
                return Err("Panel exceeds 256 KiB".into());
            }
            let layout: Layout =
                serde_json::from_value(entry.clone()).map_err(|e| e.to_string())?;
            let info = catalog
                .get(&key)
                .ok_or("Plugin version is not present in the project")?;
            validate(&layout, &info.descriptor)?;
            Ok(Arc::new(layout))
        })();
        match parsed {
            Ok(layout) => {
                result.layouts.insert(key, layout);
            }
            Err(error) => {
                result.layouts.remove(&key);
                result.errors.insert(
                    key.clone(),
                    format!(
                        "PluginUiInvalid: {}",
                        error.chars().take(180).collect::<String>()
                    ),
                );
                retain(&mut result, previous, catalog, &key);
            }
        }
    }
    if result.global_error.is_some() {
        retain_all(&mut result, previous, catalog);
    }
    result
}
fn retain(result: &mut Panels, previous: Option<&Panels>, catalog: &Catalog, key: &Key) {
    if let Some(layout) = previous.and_then(|p| p.layouts.get(key)) {
        if catalog
            .get(key)
            .is_some_and(|info| validate(layout, &info.descriptor).is_ok())
        {
            result
                .layouts
                .entry(key.clone())
                .or_insert_with(|| layout.clone());
        }
    }
}
fn retain_all(result: &mut Panels, previous: Option<&Panels>, catalog: &Catalog) {
    for key in catalog.keys() {
        retain(result, previous, catalog, key);
    }
}
