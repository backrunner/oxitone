//! Closing a document requires its own Save acknowledgement and the matching clean view.
use crate::document_wire::{DocumentMessage, DocumentView};

#[derive(Default)]
pub struct CloseState {
    pub open: bool,
    pub error: Option<String>,
    save: Option<Save>,
}
struct Save {
    request: String,
    session: String,
    revision: u64,
    acknowledged: bool,
}
impl CloseState {
    pub fn saving(&self) -> bool {
        self.save.is_some()
    }
    pub fn cancel(&mut self) {
        *self = Self::default();
    }
    pub fn begin_save(&mut self, request: String, view: &DocumentView) {
        self.error = None;
        self.save = Some(Save {
            request,
            session: view.session_id.clone(),
            revision: view.revision,
            acknowledged: false,
        });
    }
    pub fn observe(&mut self, message: &DocumentMessage) {
        let Some(save) = &mut self.save else { return };
        match message {
            DocumentMessage::Response {
                request_id,
                session_id,
                accepted,
                error,
                ..
            } if request_id == &save.request && session_id == &save.session => {
                if *accepted {
                    save.acknowledged = true;
                } else {
                    self.error = Some(error.as_ref().map_or_else(
                        || "Save failed. Your draft is still open.".into(),
                        |error| error.message.clone(),
                    ));
                    self.save = None;
                }
            }
            DocumentMessage::Event { view, .. }
                if view.session_id != save.session || view.revision != save.revision =>
            {
                self.error = Some(
                    "The project changed while saving. Review the latest changes before closing."
                        .into(),
                );
                self.save = None;
            }
            _ => {}
        }
    }
    pub fn complete(&self, view: Option<&DocumentView>, pending: bool) -> bool {
        self.save.as_ref().zip(view).is_some_and(|(save, view)| {
            save.acknowledged
                && !pending
                && !view.modified
                && !view.saving
                && view.status == "ready"
                && view.session_id == save.session
                && view.revision == save.revision
                && view.saved_revision >= 0
                && view.saved_revision as u64 == save.revision
        })
    }
}
