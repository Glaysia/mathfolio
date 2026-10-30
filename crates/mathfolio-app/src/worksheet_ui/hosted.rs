//! WASM host boundary. File names are metadata, never filesystem authority.
use super::*;
use serde::Serialize;

#[derive(Default)]
pub(crate) struct HostState {
    pub(crate) revision: u64,
    pub(crate) document_epoch: u64,
    next_request: u64,
    requests: Vec<HostRequest>,
    save: Option<(u64, u64, u64, String)>,
    pub(crate) context: Option<egui::Context>,
    pub(crate) fonts: egui::FontDefinitions,
    pub(crate) active: bool,
}
#[derive(Serialize)]
pub(crate) struct HostRequest {
    id: u64,
    kind: &'static str,
}
#[derive(Serialize)]
pub(crate) struct State {
    pub revision: u64,
    pub document_epoch: u64,
    pub dirty: bool,
    pub calculating: bool,
    pub pending: usize,
    pub regions: usize,
    pub selected: Option<u64>,
    pub status: String,
    pub frames: usize,
    pub undo: usize,
    pub redo: usize,
    pub worker_memory_bytes: Option<u32>,
    pub worker_count: u32,
    pub calculation_generation: u64,
    pub worker_started_generation: Option<u64>,
}
impl HostState {
    pub(crate) fn changed(&mut self) {
        self.revision = self.revision.saturating_add(1);
    }
    pub(crate) fn new_document(&mut self) -> Self {
        Self {
            revision: self.revision.saturating_add(1),
            document_epoch: self.document_epoch.saturating_add(1),
            next_request: self.next_request,
            context: self.context.clone(),
            fonts: self.fonts.clone(),
            active: self.active,
            ..Default::default()
        }
    }
    fn request(&mut self, kind: &'static str) {
        if self.requests.iter().any(|r| r.kind == kind) || self.requests.len() >= 16 {
            return;
        }
        self.next_request = self.next_request.saturating_add(1);
        self.requests.push(HostRequest {
            id: self.next_request,
            kind,
        });
    }
    pub(crate) fn repaint(&self) {
        if let Some(ctx) = &self.context {
            ctx.request_repaint();
        }
    }
}
impl WorksheetApp {
    pub(crate) fn host_state(&self) -> State {
        State {
            revision: self.host.revision,
            document_epoch: self.host.document_epoch,
            dirty: self.dirty || self.host_has_drafts(),
            calculating: self.background.running,
            pending: self.pending_regions.len(),
            regions: self.document.regions.len(),
            selected: self.selected,
            status: self.status.clone(),
            frames: self.frames,
            undo: self.undo.len(),
            redo: self.redo.len(),
            worker_memory_bytes: self.background.web_worker_memory(),
            worker_count: self.background.web_worker_count(),
            calculation_generation: self.background.submitted_generation(),
            worker_started_generation: self.background.web_worker_started(),
        }
    }
    fn host_has_drafts(&self) -> bool {
        self.inline_plots
            .values()
            .any(crate::plots::InlinePlotEdit::has_changes)
            || self
                .table_editors
                .values()
                .any(spec_table_ui::TableEditor::has_draft_changes)
            || self
                .area_editors
                .values()
                .any(area_ui::AreaEditor::has_draft_changes)
    }
    pub(crate) fn take_host_requests(&mut self) -> Vec<HostRequest> {
        std::mem::take(&mut self.host.requests)
    }
    pub(crate) fn host_context(&self) -> Result<egui::Context, String> {
        self.host
            .context
            .clone()
            .ok_or("Surface is not mounted".into())
    }
    pub(crate) fn attach_host(&mut self, ctx: egui::Context, fonts: egui::FontDefinitions) {
        self.host.context = Some(ctx);
        self.host.fonts = fonts;
        self.host.active = true;
    }
    pub(crate) fn set_host_active(&mut self, active: bool) {
        self.host.active = active;
        if active {
            self.host.repaint();
        }
    }
    pub(crate) fn host_is_active(&self) -> bool {
        self.host.active
    }
    pub(super) fn save(&mut self, save_as: bool) -> bool {
        self.host.request(if save_as { "save_as" } else { "save" });
        false // asynchronous; never dismiss the unsaved-doc modal before host ack
    }
    pub(super) fn choose_open(&mut self) {
        self.host.request("open");
    }
    pub(super) fn export_mcdx(&mut self) {
        self.host.request("export_mcdx");
    }
    pub(crate) fn host_dispose(&mut self) {
        self.background = Default::default();
        self.host = Default::default();
    }
    pub(crate) fn host_print_request(&mut self, pdf: bool) {
        self.host.request(if pdf { "pdf" } else { "print" });
    }
    pub(super) fn host_close_request(&mut self) {
        self.host.request("close");
    }
    pub(crate) fn load_bytes(
        &mut self,
        name: &str,
        bytes: &[u8],
        discard: bool,
    ) -> Result<(), String> {
        if self.host_state().dirty && !discard {
            return Err("Unsaved changes: explicit discard is required".into());
        }
        if name.is_empty() || name.len() > 1024 {
            return Err("Invalid document name".into());
        }
        let path = Path::new(name);
        let (document, package) = if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("mcdx"))
        {
            let (doc, package) = import_native_bytes(path, bytes.to_vec())?;
            (doc, Some(package))
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(mathfolio_document::is_own_extension)
        {
            (CanvasDocument::from_bytes(bytes)?, None)
        } else {
            return Err("Expected .mcdx or .mathfolio".into());
        };
        // Parse/validate everything first. Malformed bytes leave the current session intact.
        self.host = self.host.new_document();
        // External hosts can replace a document while a Rust dialog is open.
        // Never carry an old document's confirmation or property draft forward.
        self.pending = None;
        self.closing = false;
        self.page_break_request = None;
        self.page_break_markers = Default::default();
        self.plot_draft = None;
        self.solve_draft = None;
        self.print_ui = Default::default();
        self.install_document(document, package, None, path, Default::default());
        self.host.repaint();
        Ok(())
    }
    pub(crate) fn export_bytes(&mut self, format: &str) -> Result<Vec<u8>, String> {
        self.prepare_native_export()?;
        match format {
            "mathfolio" | "mathcraft" => self.document.to_bytes(),
            "mcdx" => Ok(mathfolio_document::export::prepare_mcdx(&self.document)?
                .bytes()
                .to_vec()),
            _ => Err("Unknown document format".into()),
        }
    }
    pub(crate) fn prepare_host_save(&mut self, name: &str) -> Result<(u64, u64, Vec<u8>), String> {
        let format = Path::new(name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let bytes = self.export_bytes(&format)?;
        self.host.next_request = self.host.next_request.saturating_add(1);
        let id = self.host.next_request;
        self.host.save = Some((
            id,
            self.host.revision,
            self.host.document_epoch,
            name.to_owned(),
        ));
        Ok((id, self.host.revision, bytes))
    }
    pub(crate) fn acknowledge_host_save(&mut self, id: u64, success: bool) -> Result<bool, String> {
        let Some((token, revision, epoch, name)) = self.host.save.as_ref() else {
            return Err("No pending save".into());
        };
        if *token != id {
            return Err("Stale save acknowledgement".into());
        }
        let matches = *revision == self.host.revision
            && *epoch == self.host.document_epoch
            && !self.host_has_drafts();
        let name = name.clone();
        self.host.save = None;
        if !success {
            self.status = "Host save failed or cancelled".into();
            return Ok(false);
        }
        if !matches {
            self.dirty = true;
            self.status = "Saved earlier revision; newer edits remain unsaved".into();
            return Ok(false);
        }
        self.dirty = false;
        self.path = Some(name.into());
        self.saved_date = Default::default();
        self.status = "Saved by host".into();
        if let Some(action) = self.pending.take() {
            self.apply(action);
        }
        self.host.repaint();
        Ok(true)
    }
    pub(crate) fn host_command(&mut self, command: &str) -> Result<(), String> {
        match command {
            "undo" => self.undo(),
            "redo" => self.redo(),
            "calculate_now" => self.calculate_command(CalculationCommand::Now),
            "calculate_worksheet" => self.calculate_command(CalculationCommand::Worksheet),
            "stop" => self.stop_all_calculations(),
            "new" => self.request(Pending::New),
            "save" => {
                self.save(false);
            }
            "open" => self.choose_open(),
            "close" => self.request(Pending::Close),
            _ => return Err("Unknown command".into()),
        }
        self.host.repaint();
        Ok(())
    }
    pub(crate) fn inspect_document(&self) -> Result<String, String> {
        serde_json::to_string(&self.document).map_err(|e| e.to_string())
    }
    pub(crate) fn inspect_results(&self) -> Result<wasm_bindgen::JsValue, String> {
        serde_wasm_bindgen::to_value(&self.results).map_err(|e| e.to_string())
    }
}
