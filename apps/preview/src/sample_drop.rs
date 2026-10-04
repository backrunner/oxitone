//! Drag payloads contain source addresses; only the document service imports and writes audio.
use crate::{
    document_wire::DocumentOperation, plugin_details::DetailTarget, ui::Preview,
    window_manager::WindowId,
};
use gpui::{prelude::*, *};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SampleSource {
    File { path: String },
    Project { index: usize },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SampleDestination {
    Arrangement {
        #[serde(skip_serializing_if = "Option::is_none")]
        track: Option<usize>,
        start_beat: f64,
    },
    Plugin {
        owner: String,
        index: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        slot: Option<usize>,
        resource: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SampleDrop {
    pub source: SampleSource,
    pub destination: SampleDestination,
}
pub struct SampleDrag {
    pub source: SampleSource,
    pub name: String,
    pub anchor: Point<Pixels>,
    pub position: Point<Pixels>,
    pub destination: Option<SampleDestination>,
    pub moved: bool,
}
impl SampleDrag {
    pub fn new(source: SampleSource, name: String, at: Point<Pixels>) -> Self {
        Self {
            source,
            name,
            anchor: at,
            position: at,
            destination: None,
            moved: false,
        }
    }
}
impl Preview {
    pub fn move_sample(&mut self, event: &MouseMoveEvent, cx: &App) {
        if self.document.browser.drag.is_none() {
            return;
        }
        let destination = self.sample_destination(event, cx);
        let drag = self.document.browser.drag.as_mut().unwrap();
        drag.moved |= (event.position - drag.anchor).magnitude() > f64::from(px(3.));
        drag.position = event.position;
        drag.destination = destination;
    }
    fn sample_destination(&self, event: &MouseMoveEvent, cx: &App) -> Option<SampleDestination> {
        let order = self.document.view.as_ref()?.arrangement_order.as_ref()?;
        if let Some(hit) = self.document.windows.hit(event.position) {
            for entity in self.plugin_windows.values() {
                if hit != WindowId::Plugin(entity.entity_id().as_u64()) {
                    continue;
                }
                let plugin = entity.read(cx);
                plugin.details.as_ref()?;
                let target = crate::plugin_identity::follow(
                    self.project.as_ref()?,
                    &plugin.target,
                    &plugin.identity,
                )?;
                let resource = plugin
                    .sample_slots
                    .borrow()
                    .iter()
                    .find(|(_, b)| b.contains(&event.position))
                    .map(|(key, _)| key.clone())?;
                let bus = matches!(target, DetailTarget::BusInsert(..));
                let owners = if bus {
                    &order.mixer_channels
                } else {
                    &order.channels
                };
                return Some(SampleDestination::Plugin {
                    owner: if bus { "bus" } else { "channel" }.into(),
                    index: owners.iter().position(|id| id == target.owner())?,
                    slot: target.slot(),
                    resource,
                });
            }
            return None;
        }
        if !self
            .document
            .playlist
            .viewport
            .get()
            .contains(&event.position)
        {
            return None;
        }
        for (id, bounds) in self.document.playlist.rows.borrow().iter() {
            if bounds.contains(&event.position) {
                let beat =
                    f64::from(f32::from(event.position.x - bounds.origin.x)) / f64::from(self.zoom);
                let step = if event.modifiers.alt { 1. / 960. } else { 0.25 };
                return Some(SampleDestination::Arrangement {
                    track: Some(order.tracks.iter().position(|v| v == id)?),
                    start_beat: (beat / step).round().max(0.) * step,
                });
            }
        }
        let bounds = self.workspace.arrangement.bounds();
        let beat = f64::from(f32::from(
            event.position.x - bounds.origin.x - self.workspace.arrangement.offset().x,
        )) / f64::from(self.zoom);
        Some(SampleDestination::Arrangement {
            track: None,
            start_beat: (beat * 4.).round().max(0.) / 4.,
        })
    }
    pub fn finish_sample(&mut self) {
        let Some(drag) = self.document.browser.drag.take().filter(|d| d.moved) else {
            return;
        };
        if let Some(destination) = drag.destination {
            self.document_request(DocumentOperation::SampleDrop {
                drop: SampleDrop {
                    source: drag.source,
                    destination,
                },
            });
        } else {
            self.status = "Drop onto an Arrangement lane or a plugin sample slot".into();
        }
    }
}
pub fn ghost(this: &Preview) -> Div {
    let mut root = div();
    if let Some(drag) = this.document.browser.drag.as_ref().filter(|d| d.moved) {
        root = root
            .absolute()
            .left(drag.position.x + px(14.))
            .top(drag.position.y + px(16.))
            .px_3()
            .py_2()
            .rounded_md()
            .bg(rgb(this.theme.panel))
            .border_1()
            .border_color(rgb(if drag.destination.is_some() {
                this.theme.accent
            } else {
                this.theme.muted
            }))
            .text_size(px(11.))
            .child(format!(
                "{} · {}",
                drag.name,
                if drag.destination.is_some() {
                    "Drop sample"
                } else {
                    "Choose a sample slot or lane"
                }
            ));
    }
    root
}
