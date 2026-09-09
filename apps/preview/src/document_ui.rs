use crate::{backend::Command, document_wire::*, model::Diagnostic, ui::Preview};

#[derive(Default)]
pub struct DocumentUi {
    pub view: Option<DocumentView>,
    pub pending: Option<String>,
    pub error: Option<Diagnostic>,
    pub notes: crate::note_selection::NoteSelection,
    pub gesture: Option<crate::note_edit::NoteGesture>,
    pub pending_notes: Option<crate::note_edit::NoteGesture>,
    pub pending_note_source: Vec<SourceNote>,
    pub pending_note_pattern: Option<String>,
    pub presentation_revision: Option<u64>,
    pub show_code: bool,
    pub patterns_open: bool,
    pub patterns_selected: Option<String>,
    pub playlist: crate::playlist_edit::PlaylistUi,
    pub mixer: crate::mixer_edit::MixerUi,
    pub tempo: crate::tempo_edit::TempoUi,
    pub edit_shared: bool,
    pub manager: crate::plugin_manager::ManagerUi,
    pub configuration: crate::configuration_edit::ConfigurationUi,
    pub plugin: crate::plugin_edit::PluginEdit,
    pub automation: crate::automation_edit::AutomationUi,
    pub windows: crate::window_manager::WindowManager,
    serial: u64,
}
impl Preview {
    pub fn observe_document(&mut self, message: DocumentMessage) {
        self.close.observe(&message);
        match message {
            DocumentMessage::Event { view, .. } => {
                let changed = self.document.view.as_ref().is_none_or(|old| {
                    old.session_id != view.session_id || old.revision != view.revision
                });
                if self
                    .document
                    .view
                    .as_ref()
                    .is_some_and(|old| old.session_id != view.session_id)
                {
                    self.document.pending = None;
                    self.document.plugin.clear_pending();
                    self.document.pending_notes = None;
                    self.document.playlist.pending = None;
                    self.document.pending_note_source.clear();
                    self.document.presentation_revision = None;
                    self.document.mixer.pending = None;
                    self.document.manager.assignment_pending = false;
                    self.document.automation.pending = None;
                }
                if changed {
                    self.document.plugin.cancel();
                    self.document.error = None;
                    self.document.playlist.drag = None;
                    self.document.mixer.gesture = None;
                    self.document.tempo.input = None;
                    self.document.notes.clear();
                    self.document.gesture = None;
                    self.document.automation.gesture = None;
                    self.document.configuration.input = None;
                }
                if let Some(error) = &view.diagnostic {
                    self.document.error = Some(Diagnostic {
                        code: error.code.clone(),
                        message: error.message.clone(),
                        path: None,
                    });
                }
                self.status = format!(
                    "{} · Source {} · Saved {}",
                    view.status, view.revision, view.saved_revision
                );
                self.document.view = Some(view);
            }
            DocumentMessage::Response {
                request_id,
                session_id,
                accepted,
                error,
                revision,
                ..
            } => {
                if self
                    .document
                    .view
                    .as_ref()
                    .is_some_and(|view| view.session_id == session_id)
                    && self.document.pending.as_deref() == Some(&request_id)
                {
                    self.document.pending = None;
                    self.document.mixer.pending = None;
                    if self.document.manager.assignment_pending {
                        self.document.manager.assignment_pending = false;
                        if accepted {
                            self.document.manager.assignment = None;
                            self.document.manager.open = false;
                            self.document
                                .windows
                                .forget(crate::window_manager::WindowId::Plugins);
                        }
                    }
                    if !accepted || self.document.presentation_revision == Some(revision + 1) {
                        self.clear_presentation();
                    }
                    self.document.automation.pending = None;
                    if accepted {
                        self.status = format!("Source {revision} accepted");
                    }
                    if !accepted {
                        if let Some(error) = error {
                            self.document.error = Some(Diagnostic {
                                code: error.code,
                                message: error.message,
                                path: None,
                            });
                        }
                    }
                }
            }
        }
        self.settle_presentation();
    }
    pub fn presentation_active(&self) -> bool {
        self.project
            .as_ref()
            .is_some_and(|p| Some(p.snapshot.revision) == self.document.presentation_revision)
    }
    pub fn clear_presentation(&mut self) {
        self.document.plugin.clear_pending();
        self.document.playlist.pending = None;
        self.document.pending_notes = None;
        self.document.pending_note_source.clear();
        self.document.pending_note_pattern = None;
        self.document.presentation_revision = None;
    }
    pub fn settle_presentation(&mut self) {
        if self.document_ready() && !self.presentation_active() {
            if let Some(gesture) = self.document.pending_notes.take() {
                self.restore_note_selection(&gesture);
            }
            self.clear_presentation();
        }
    }
    pub fn document_ready(&self) -> bool {
        self.document.pending.is_none()
            && self.document.view.as_ref().is_some_and(|view| {
                view.status == "ready"
                    && !view.saving
                    && view.accepted_revision == view.revision as i64
                    && self
                        .project
                        .as_ref()
                        .is_some_and(|project| project.snapshot.revision == view.revision + 1)
            })
    }
    pub fn active_diagnostic(&self) -> Option<&Diagnostic> {
        self.document.error.as_ref().or(self.diagnostic.as_ref())
    }
    pub fn pattern_site(&self) -> Option<&PatternSite> {
        let project = self.project.as_ref()?;
        let clip = project
            .snapshot
            .pattern_clips
            .iter()
            .find(|clip| Some(&clip.id) == self.selected_clip.as_ref())?;
        let view = self.document.view.as_ref()?;
        let pattern_id = &self.piano_pattern()?.id;
        if self.document.edit_shared || pattern_id != &clip.pattern_id {
            view.sites
                .iter()
                .find(|site| &site.pattern_id == pattern_id && site.scope == "definition")
                .or_else(|| {
                    view.sites
                        .iter()
                        .find(|site| &site.pattern_id == pattern_id && site.references == 1)
                })
        } else {
            view.sites
                .iter()
                .find(|site| {
                    &site.pattern_id == pattern_id
                        && site.scope == "reference"
                        && site.placements == [clip.id.clone()]
                })
                .or_else(|| {
                    view.sites
                        .iter()
                        .find(|site| &site.pattern_id == pattern_id && site.references == 1)
                })
        }
    }
    pub fn edit_placement(&self) -> Option<String> {
        if !self.composite_selected()
            && !self.document.edit_shared
            && self
                .pattern_site()
                .is_some_and(|site| site.scope == "reference")
        {
            self.selected_clip.clone()
        } else {
            None
        }
    }
    pub fn document_operation_ready(&self, operation: &DocumentOperation) -> bool {
        match operation {
            DocumentOperation::RefreshPlugins
            | DocumentOperation::VerifyPlugin { .. }
            | DocumentOperation::InstallPlugin { .. }
            | DocumentOperation::UpgradePlugin { .. }
            | DocumentOperation::UninstallPlugin { .. }
            | DocumentOperation::RepairPlugin { .. }
            | DocumentOperation::Code { .. }
            | DocumentOperation::CreateFile { .. }
            | DocumentOperation::Undo
            | DocumentOperation::Redo
            | DocumentOperation::ResolveConflict { .. }
            | DocumentOperation::CancelMaterialize { .. } => {
                self.document.pending.is_none()
                    && self.document.view.as_ref().is_some_and(|view| {
                        !view.saving && view.status != "closed" && view.status != "building"
                    })
            }
            _ => self.document_ready(),
        }
    }
    pub fn document_request(&mut self, operation: DocumentOperation) {
        if !self.document_operation_ready(&operation) {
            return;
        }
        let Some(view) = &self.document.view else {
            return;
        };
        self.document.serial += 1;
        let request_id = format!("stream/gpui-{}/{}", view.session_id, self.document.serial);
        let request = DocumentRequest {
            document_protocol_version: "2.0".into(),
            session_id: view.session_id.clone(),
            request_id: request_id.clone(),
            base_revision: view.revision,
            operation,
        };
        if self.send_document(request) {
            self.document.error = None;
            self.document.pending = Some(request_id);
            self.document.presentation_revision =
                self.project.as_ref().map(|p| p.snapshot.revision);
        }
    }
    pub fn send_document(&self, request: DocumentRequest) -> bool {
        self.backend
            .commands
            .send(Command::Document(request))
            .is_ok()
    }
}
