//! Static catalog projection. Selection and browsing do not instantiate a plugin.
use gpui::*;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct Usage {
    #[serde(rename = "instanceId")]
    pub(super) instance_id: Option<String>,
    pub(super) kind: String,
    pub(super) owner: String,
    pub(super) index: usize,
    pub(super) label: String,
}
impl Usage {
    pub(super) fn target(&self) -> crate::plugin_details::DetailTarget {
        use crate::plugin_details::DetailTarget;
        match self.kind.as_str() {
            "instrument" => DetailTarget::Instrument(self.owner.clone()),
            "channelInsert" => DetailTarget::ChannelInsert(self.owner.clone(), self.index),
            _ => DetailTarget::BusInsert(self.owner.clone(), self.index),
        }
    }
    pub(super) fn handle(&self) -> String {
        self.instance_id.clone().unwrap_or_else(|| {
            if self.kind == "instrument" {
                format!("{}:instrument", self.owner)
            } else {
                format!("{}:effect:{}", self.owner, self.index)
            }
        })
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntry {
    pub(super) handle: String,
    pub(super) plugin_id: String,
    pub(super) plugin_version: String,
    pub(super) display_name: String,
    pub(super) vendor: String,
    pub(super) kind: String,
    pub(super) source: String,
    pub(super) package_name: Option<String>,
    pub(super) package_version: Option<String>,
    pub(super) license: Option<String>,
    pub(super) availability: String,
    pub(super) validation: String,
    pub(super) diagnostic: Option<String>,
    pub(super) library_path: Option<String>,
    pub(super) sha256: Option<String>,
    pub(super) parameters: Vec<oxitone_core::wire::ParameterSpec>,
    pub(super) usages: Vec<Usage>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LibrarySection {
    Uses,
    Details,
}
#[derive(Default)]
pub struct ManagerUi {
    pub assignment: Option<crate::plugin_picker::Slot>,
    pub assignment_pending: bool,
    pub details_scroll: ScrollHandle,
    pub open: bool,
    pub searching: bool,
    pub(super) adding: bool,
    pub(super) package_input: String,
    pub(super) input_error: Option<String>,
    pub query: String,
    pub(super) category: usize,
    pub(super) selected: Option<String>,
    pub list_scroll: ScrollHandle,
    pub section: Option<LibrarySection>,
    pub select_all: bool,
    pub filter_bounds: std::rc::Rc<std::cell::Cell<[Bounds<Pixels>; 3]>>,
}
impl CatalogEntry {
    pub(super) fn matches(&self, category: usize, query: &str) -> bool {
        let category_match = match category {
            1 => self.kind == "instrument",
            2 => self.kind == "effect",
            _ => true,
        };
        category_match
            && format!(
                "{} {} {} {}",
                self.display_name,
                self.vendor,
                self.plugin_id,
                self.package_name.as_deref().unwrap_or("")
            )
            .to_lowercase()
            .contains(&query.to_lowercase())
    }
}

pub(super) fn parse_package_spec(input: &str) -> Option<(String, Option<String>)> {
    if input.is_empty() {
        return None;
    }
    let marker = if input.starts_with('@') {
        input[1..].find('@').map(|index| index + 1)
    } else {
        input.find('@')
    };
    let (name, version) = marker.map_or_else(
        || Some((input.to_owned(), None)),
        |index| {
            let (name, version) = input.split_at(index);
            (!name.is_empty() && version.len() > 1)
                .then(|| (name.to_owned(), Some(version[1..].to_owned())))
        },
    )?;
    let valid_component = |part: &str| {
        part.bytes()
            .next()
            .is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
            && part
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
    };
    let valid_name = if let Some(scoped) = name.strip_prefix('@') {
        scoped
            .split_once('/')
            .is_some_and(|(scope, package)| valid_component(scope) && valid_component(package))
    } else {
        valid_component(&name)
    };
    let valid_version = version.as_ref().is_none_or(|version| {
        version.len() <= 128
            && version
                .bytes()
                .next()
                .is_some_and(|b| b.is_ascii_alphanumeric())
            && version
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._+~-".contains(&b))
    });
    (valid_name && name.len() <= 256 && valid_version).then_some((name, version))
}

#[cfg(test)]
mod tests {
    use super::parse_package_spec;

    #[test]
    fn parses_scoped_and_unscoped_package_specs() {
        assert_eq!(parse_package_spec("synth"), Some(("synth".into(), None)));
        assert_eq!(
            parse_package_spec("synth@1.2.3"),
            Some(("synth".into(), Some("1.2.3".into())))
        );
        assert_eq!(
            parse_package_spec("@acme/synth@2.0.0"),
            Some(("@acme/synth".into(), Some("2.0.0".into())))
        );
        assert_eq!(
            parse_package_spec("@acme/synth"),
            Some(("@acme/synth".into(), None))
        );
        assert_eq!(parse_package_spec("@acme/synth@"), None);
        for invalid in [
            "",
            "@acme",
            "@acme/a/b",
            "two packages",
            "../file",
            "--force",
            "synth@v 2",
            "synth@@latest",
        ] {
            assert_eq!(parse_package_spec(invalid), None);
        }
    }
}
