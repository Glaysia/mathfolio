//! A worksheet surface: positioned structural formulas and text share one document.
//! The initial math/cursor subset remains explicitly narrower than Prime.
use crate::{
    formula,
    ribbon::{self, RibbonAction, RibbonContext, RibbonState},
    typography, worksheet_math,
};
use eframe::egui::{self, Color32, FontId, Rect, Sense, Stroke, UiBuilder, pos2, vec2};
use mathcraft_compat::{RegionEvaluation, Value};
use mathcraft_document::{
    canvas::{CanvasContent, CanvasDocument, CanvasRegion},
    native::NativePackage,
};
use mathcraft_editor::{BinaryOp, Direction, Editor, NodeKind, PortableTree};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

const BLUE: Color32 = Color32::from_rgb(0, 128, 255);
const RED: Color32 = Color32::from_rgb(175, 40, 45);

struct HistoryEntry {
    document: CanvasDocument,
    selected: Option<u64>,
    insertion: Option<(f64, f64)>,
    editing_region: Option<u64>,
    active_editor: Option<(u64, Editor)>,
    text_cursor: Option<(u64, egui::text::CCursorRange)>,
}

#[derive(Clone, Copy)]
enum RegionAction {
    Edit,
    Copy,
    Cut,
    Duplicate,
    Delete,
}

enum Pending {
    New,
    Open(PathBuf),
    Close,
}
pub struct WorksheetApp {
    document: CanvasDocument,
    page_margins: [f32; 4],
    editors: HashMap<u64, Editor>,
    text_cursors: HashMap<u64, egui::text::CCursorRange>,
    results: HashMap<u64, RegionEvaluation>,
    labelled_trees: HashMap<u64, PortableTree>,
    ribbon: RibbonState,
    calculation_error: Option<String>,
    path: Option<PathBuf>,
    status: String,
    dirty: bool,
    selected: Option<u64>,
    editing_region: Option<u64>,
    insertion: Option<(f64, f64)>,
    text_mode: bool,
    focus_new: Option<u64>,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    dragging: Option<(u64, f64, f64)>,
    pending: Option<Pending>,
    closing: bool,
    pub smoke: bool,
    pub capture: Option<PathBuf>,
    frames: usize,
    capture_sent: bool,
    zoom: f32,
    show_grid: bool,
    use_major_grid: bool,
    show_shortcuts: bool,
    reveal: Option<(f64, f64)>,
    menu_resume: Option<u64>,
    last_title: String,
}
impl Default for WorksheetApp {
    fn default() -> Self {
        Self {
            document: CanvasDocument::default(),
            page_margins: [crate::page::METRIC_MARGIN; 4],
            editors: HashMap::new(),
            text_cursors: HashMap::new(),
            results: HashMap::new(),
            labelled_trees: HashMap::new(),
            ribbon: RibbonState::default(),
            calculation_error: None,
            path: None,
            status: String::new(),
            dirty: false,
            selected: None,
            editing_region: None,
            insertion: Some((0., CanvasDocument::default().grid)),
            text_mode: false,
            focus_new: None,
            undo: vec![],
            redo: vec![],
            dragging: None,
            pending: None,
            closing: false,
            smoke: false,
            capture: None,
            frames: 0,
            capture_sent: false,
            zoom: 1.,
            show_grid: true,
            use_major_grid: false,
            show_shortcuts: false,
            reveal: None,
            menu_resume: None,
            last_title: String::new(),
        }
    }
}
impl WorksheetApp {
    fn snapshot(&self) -> HistoryEntry {
        HistoryEntry {
            text_cursor: self
                .selected
                .and_then(|id| self.text_cursors.get(&id).map(|range| (id, *range))),
            selected: self.selected,
            insertion: self.insertion,
            editing_region: self.editing_region,
            active_editor: self.selected.and_then(|id| {
                self.editors
                    .get(&id)
                    .map(|e| (id, e.clone_without_history()))
            }),
            document: CanvasDocument {
                version: self.document.version,
                title: self.document.title.clone(),
                width: self.document.width,
                height: self.document.height,
                grid: self.document.grid,
                next_id: self.document.next_id,
                regions: self.document.regions.clone(),
                native_archive: None,
            },
        }
    }
    fn remember(&mut self, before: HistoryEntry) {
        if self.undo.len() >= 50 {
            self.undo.remove(0);
        }
        self.undo.push(before);
        self.redo.clear();
        self.dirty = true;
    }
    fn restore(&mut self, mut state: HistoryEntry) {
        state.document.native_archive = self.document.native_archive.take();
        self.document = state.document;
        self.editors.clear();
        self.text_cursors.clear();
        if let Some((id, cursor)) = state.text_cursor {
            self.text_cursors.insert(id, cursor);
        }
        self.selected = state
            .selected
            .filter(|id| self.document.regions.iter().any(|r| r.id == *id));
        self.insertion = state.insertion;
        self.editing_region = state.editing_region.filter(|id| Some(*id) == self.selected);
        self.focus_new = self.editing_region;
        self.dragging = None;
        if let Some((id, editor)) = state.active_editor
            && self.document.regions.iter().any(|r| r.id == id)
        {
            self.editors.insert(id, editor);
        }
        self.dirty = true;
        self.recalculate();
    }
    fn undo(&mut self) {
        if let Some(previous) = self.undo.pop() {
            self.redo.push(self.snapshot());
            self.restore(previous);
        }
    }
    fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(self.snapshot());
            self.restore(next);
        }
    }
    fn request(&mut self, action: Pending) {
        if self.dirty {
            self.pending = Some(action);
        } else {
            self.apply(action);
        }
    }
    fn apply(&mut self, action: Pending) {
        match action {
            Pending::New => {
                let view = (self.zoom, self.show_grid);
                *self = Self::default();
                (self.zoom, self.show_grid) = view;
            }
            Pending::Open(path) => self.open(&path),
            Pending::Close => self.closing = true,
        }
    }
    pub fn open(&mut self, path: &Path) {
        let result = if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("mcdx"))
        {
            import_native(path)
        } else {
            CanvasDocument::load(path)
        };
        match result {
            Ok(document) => {
                self.page_margins = crate::page::standard_margins(&document);
                if let Some(bytes) = &document.native_archive
                    && let Ok(package) = NativePackage::from_bytes(bytes)
                    && let Ok(Some(source)) =
                        package.original_xml_part("mathcad/settings/presentation.xml")
                {
                    if let Some((_, margins)) = crate::page::source_settings(&source) {
                        self.page_margins = margins;
                    }
                    (self.show_grid, self.use_major_grid) = crate::page::view_settings(&source);
                }
                self.document = document;
                self.editors.clear();
                self.text_cursors.clear();
                self.undo.clear();
                self.redo.clear();
                self.results.clear();
                self.dirty = false;
                self.selected = None;
                self.editing_region = None;
                self.focus_new = None;
                self.dragging = None;
                self.insertion = None;
                self.path = if path
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("mathcraft"))
                {
                    Some(path.into())
                } else {
                    None
                };
                self.status = format!("Opened {}", path.display());
                self.recalculate();
            }
            Err(e) => self.status = format!("Could not open: {e}"),
        }
    }
    fn sync_editors(&mut self) -> Result<(), String> {
        for region in &mut self.document.regions {
            if let Some(editor) = self.editors.get(&region.id) {
                region.content = CanvasContent::Math(
                    editor
                        .to_portable()
                        .map_err(|e| format!("Region {}: {e:?}", region.id))?,
                );
            }
        }
        Ok(())
    }
    fn save(&mut self, save_as: bool) -> bool {
        self.focus_new = self.editing_region.or(self.focus_new);
        if let Err(e) = self.sync_editors() {
            self.status = format!("Could not save: {e}");
            return false;
        }
        let path = if save_as || self.path.is_none() {
            rfd::FileDialog::new()
                .add_filter("Mathcraft worksheet", &["mathcraft"])
                .set_directory(
                    self.path
                        .as_ref()
                        .and_then(|p| p.parent())
                        .map(Path::to_path_buf)
                        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default()),
                )
                .set_file_name("worksheet.mathcraft")
                .save_file()
        } else {
            self.path.clone()
        };
        let Some(path) = path else {
            return false;
        };
        match self.document.save(&path) {
            Ok(()) => {
                self.path = Some(path.clone());
                self.dirty = false;
                self.status = format!("Saved {}", path.display());
                true
            }
            Err(e) => {
                self.status = format!("Could not save: {e}");
                false
            }
        }
    }
    fn recalculate(&mut self) {
        self.results.clear();
        self.labelled_trees.clear();
        self.calculation_error = None;
        let mut faults: HashMap<u64, String> = self
            .editors
            .iter()
            .filter_map(|(id, editor)| {
                editor
                    .to_portable()
                    .err()
                    .map(|error| (*id, format!("Incomplete expression: {error}")))
            })
            .collect();
        let mut math = Vec::new();
        for region in &self.document.regions {
            match &region.content {
                CanvasContent::Math(tree) => {
                    if !matches!(
                        tree.kind,
                        NodeKind::Binary(BinaryOp::Define | BinaryOp::Evaluate)
                    ) {
                        continue;
                    }
                    if faults.contains_key(&region.id) {
                        math.push((region.id, region.left, region.top, fault_statement(tree)));
                        continue;
                    }
                    match worksheet_math::tree_to_mathml(tree) {
                        Ok(xml) => math.push((region.id, region.left, region.top, xml)),
                        Err(e) => {
                            faults.insert(region.id, e);
                            math.push((region.id, region.left, region.top, fault_statement(tree)));
                        }
                    }
                }
                CanvasContent::Opaque { kind, .. } if matches!(kind.as_str(), "image" | "text") => {
                }
                CanvasContent::Opaque { .. } => {
                    self.calculation_error = Some(
                        "This worksheet contains regions whose calculation is not implemented yet."
                            .into(),
                    );
                    return;
                }
                CanvasContent::Text(_) => (),
            }
        }
        let result = worksheet_math::worksheet_xml(&math).and_then(|source| {
            if let Ok(xml) = roxmltree::Document::parse(&source) {
                for region in xml.descendants().filter(|n| {
                    n.has_tag_name(("http://schemas.mathsoft.com/worksheet50", "region"))
                }) {
                    if let Some(id) = region
                        .attribute("region-id")
                        .and_then(|id| id.parse::<u64>().ok())
                        && let Some(math) = region.children().find(|n| {
                            n.has_tag_name(("http://schemas.mathsoft.com/worksheet50", "math"))
                        })
                        && let Ok(tree) =
                            worksheet_math::tree_from_mathml(&standalone(math, &source))
                    {
                        self.labelled_trees.insert(id, tree);
                    }
                }
            }
            mathcraft_compat::evaluate_worksheet(&source)
        });
        match result {
            Ok(rows) => {
                self.results = rows
                    .into_iter()
                    .filter_map(|r| r.region_id.parse::<u64>().ok().map(|id| (id, r)))
                    .collect()
            }
            Err(e) => self.calculation_error = Some(e),
        }
        for (id, detail) in faults {
            if let Some(row) = self.results.get_mut(&id) {
                if let Some(error) = &mut row.error {
                    error.detail = detail;
                }
                row.value = None;
                row.display_value = None;
            } else if let Some(region) = self.document.regions.iter().find(|r| r.id == id) {
                self.results.insert(
                    id,
                    RegionEvaluation {
                        region_id: id.to_string(),
                        result_ref: None,
                        left: region.left,
                        top: region.top,
                        value: None,
                        display_value: None,
                        error: Some(mathcraft_compat::Fault {
                            code: "incomplete".into(),
                            detail,
                            previous_region: None,
                            unsupported: true,
                        }),
                        redefinition_warning: false,
                    },
                );
            }
        }
    }
    fn insert(&mut self, position: (f64, f64), text: bool) -> u64 {
        let before = self.snapshot();
        let id = if text {
            self.document
                .add_text(position.0, position.1, String::new())
        } else {
            self.document.add_math(position.0, position.1)
        };
        self.remember(before);
        self.selected = Some(id);
        self.focus_new = Some(id);
        self.reveal = Some(position);
        self.insertion = None;
        id
    }
    fn remove_selected(&mut self) {
        if let Some(id) = self.selected {
            let before = self.snapshot();
            self.selected = None;
            self.document.regions.retain(|r| r.id != id);
            self.editors.remove(&id);
            self.text_cursors.remove(&id);
            self.editing_region = None;
            self.remember(before);
            self.recalculate();
        }
    }
    fn duplicate_selected(&mut self) {
        let content = self
            .selected
            .and_then(|id| self.document.regions.iter().find(|r| r.id == id))
            .cloned();
        if let Some(mut region) = content
            && !matches!(region.content, CanvasContent::Opaque { .. })
        {
            let before = self.snapshot();
            region.id = self.document.next_id;
            self.document.next_id += 1;
            region.left += self.document.grid * 2.;
            region.top += self.document.grid * 2.;
            region.source_region = None;
            self.selected = Some(region.id);
            self.document.regions.push(region);
            self.remember(before);
            self.recalculate();
        }
    }
    fn copy_region(&mut self, ctx: &egui::Context, cut: bool) {
        if let Some(region) = self
            .selected
            .and_then(|id| self.document.regions.iter().find(|r| r.id == id))
            && !matches!(region.content, CanvasContent::Opaque { .. })
            && let Ok(value) = serde_json::to_string(&region.content)
        {
            ctx.copy_text(format!("mathcraft-region:{value}"));
            if cut {
                self.remove_selected();
            }
        }
    }
    fn paste_region(&mut self, text: &str) -> bool {
        let encoded = if let Some(encoded) = text.strip_prefix("mathcraft-region:") {
            encoded
        } else if let Some(encoded) = text.strip_prefix("mathcraft-tree:") {
            if encoded.len() > 8 * 1024 * 1024 {
                return true;
            }
            let tree = serde_json::from_str::<PortableTree>(encoded);
            if let Ok(tree) = tree
                && let Ok(json) = serde_json::to_string(&CanvasContent::Math(tree))
            {
                return self.paste_region(&format!("mathcraft-region:{json}"));
            }
            self.status = "Clipboard formula is invalid".into();
            return true;
        } else {
            return false;
        };
        if encoded.len() > 8 * 1024 * 1024 {
            self.status = "Clipboard region is too large".into();
            return true;
        }
        let parsed = serde_json::from_str::<CanvasContent>(encoded);
        let content = match parsed {
            Ok(CanvasContent::Math(tree)) if tree.validate().is_ok() => CanvasContent::Math(tree),
            Ok(CanvasContent::Text(text)) if text.len() <= 1024 * 1024 => CanvasContent::Text(text),
            _ => {
                self.status = "Clipboard region is invalid or unsupported".into();
                return true;
            }
        };
        let position = self.insertion.unwrap_or_else(|| {
            self.selected
                .and_then(|id| self.document.regions.iter().find(|r| r.id == id))
                .map(|r| {
                    (
                        r.left + self.document.grid * 2.,
                        r.top + self.document.grid * 2.,
                    )
                })
                .unwrap_or((0., self.document.grid))
        });
        let before = self.snapshot();
        let id = self.document.next_id;
        self.document.next_id += 1;
        self.document.regions.push(CanvasRegion {
            id,
            left: position.0,
            top: position.1,
            content,
            source_region: None,
        });
        self.selected = Some(id);
        self.insertion = None;
        self.remember(before);
        self.recalculate();
        true
    }
    fn export_mcdx(&mut self) {
        if let Err(error) = self.sync_editors() {
            self.status = format!("Could not export: {error}");
            return;
        }
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Mathcad worksheet", &["mcdx"])
            .set_file_name("worksheet.mcdx")
            .save_file()
        {
            match mathcraft_document::export::export_mcdx(&self.document, &path) {
                Ok(()) => self.status = format!("Exported {}", path.display()),
                Err(error) => self.status = format!("Could not export: {error}"),
            }
        }
    }
    fn insert_template(&mut self, tree: PortableTree) {
        let before = self.snapshot();
        let id = self.selected.filter(|id| {
            self.document
                .regions
                .iter()
                .any(|r| r.id == *id && matches!(r.content, CanvasContent::Math(_)))
        });
        let id = if let Some(id) = id {
            id
        } else {
            let p = self.insertion.unwrap_or((0., self.document.grid));
            self.document.add_math(p.0, p.1)
        };
        let region = self
            .document
            .regions
            .iter_mut()
            .find(|r| r.id == id)
            .unwrap();
        if let CanvasContent::Math(current) = &mut region.content {
            let editor = self
                .editors
                .entry(id)
                .or_insert_with(|| Editor::from_portable(current).unwrap_or_default());
            let prior = editor
                .arena()
                .nodes()
                .map(|(id, _)| id.index())
                .max()
                .unwrap_or(0);
            let inserted = match tree.kind {
                NodeKind::Fraction => {
                    editor.insert_fraction();
                    Ok(())
                }
                NodeKind::Power => {
                    editor.insert_power();
                    Ok(())
                }
                NodeKind::Root => {
                    editor.insert_root();
                    Ok(())
                }
                NodeKind::Absolute => {
                    editor.insert_absolute();
                    Ok(())
                }
                _ => editor.replace_selection(&tree).map(|_| ()),
            };
            if let Err(error) = inserted {
                self.status = format!("Could not insert: {error}");
                return;
            }
            let empty = editor
                .arena()
                .nodes()
                .find(|(id, node)| id.index() > prior && *node.kind() == NodeKind::Placeholder)
                .map(|(id, _)| id);
            if let Some(empty) = empty {
                let _ = editor.place_caret(empty, Direction::Right);
            }
            if let Ok(value) = editor.to_portable() {
                *current = value;
            }
        }
        self.selected = Some(id);
        self.focus_new = Some(id);
        self.insertion = None;
        self.remember(before);
        self.recalculate();
    }
    fn choose_open(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Worksheets", &["mathcraft", "mcdx"])
            .pick_file()
        {
            self.request(Pending::Open(path));
        }
    }
    fn begin_region(&mut self, text: bool) {
        let position = self.insertion.unwrap_or_else(|| {
            self.selected
                .and_then(|id| self.document.regions.iter().find(|r| r.id == id))
                .map(|r| (r.left, r.top + self.document.grid * 5.))
                .unwrap_or((0., self.document.grid))
        });
        self.text_mode = text;
        self.insert(position, text);
        // Text insertion is a command; clicking elsewhere resumes math writing.
        self.text_mode = false;
    }
    fn chrome_ui(&mut self, ui: &mut egui::Ui) {
        let editing_before = self.editing_region;
        if self.editing_region.is_none() && self.focus_new.is_none()
            && (self.selected.is_some() || self.insertion.is_some())
            && self.menu_resume.is_none() && !ui.ctx().any_popup_open()
            && ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Key {
                key: egui::Key::Tab | egui::Key::ArrowLeft | egui::Key::ArrowRight | egui::Key::ArrowUp | egui::Key::ArrowDown,
                pressed: true, modifiers, ..
            } if !modifiers.command && !modifiers.alt)))
        {
            // Suppress automatic widget focus traversal for page-owned navigation,
            // including the first frame after switching out of formula editing.
            ui.memory_mut(|m| m.move_focus(egui::FocusDirection::None));
        }
        let name = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.document.title.clone());
        let commands = ribbon::show(
            ui,
            &mut self.ribbon,
            &RibbonContext {
                document_name: &name,
                can_undo: !self.undo.is_empty(),
                can_redo: !self.redo.is_empty(),
                has_selection: self.selected.is_some(),
                zoom: self.zoom,
                grid: self.show_grid,
            },
        );
        for command in commands {
            self.ribbon_command(ui.ctx(), command);
        }
        let hint = if self.status.is_empty() {
            self.context_hint().to_string()
        } else {
            self.status.clone()
        };
        let errors = self.results.values().filter(|r| r.error.is_some()).count();
        let calculation_error = self.calculation_error.clone();
        let dirty = self.dirty;
        egui::Panel::bottom("worksheet-status").exact_size(25.)
            .frame(egui::Frame::new().fill(Color32::from_rgb(232, 238, 243)).inner_margin(egui::Margin::symmetric(10, 3)))
            .show(ui, |ui| {
                egui::containers::Sides::new().shrink_left().truncate().show(ui,
                    |ui| {
                        if let Some(error) = &calculation_error {
                            ui.colored_label(RED, "계산을 완료하지 못했습니다").on_hover_text(error);
                        } else if errors > 0 {
                            ui.colored_label(RED, format!("오류 {errors}개")).on_hover_text("수식의 오류 표시에 마우스를 올리면 상세 내용을 볼 수 있습니다.");
                        } else { ui.small(if dirty { "변경됨" } else { "준비" }); }
                        ui.separator();
                        ui.add(egui::Label::new(&hint).truncate()).on_hover_text(&hint);
                    },
                    |ui| {
                        egui::ComboBox::from_id_salt("worksheet-zoom").width(68.)
                            .selected_text(format!("{}%", (self.zoom * 100.).round()))
                            .show_ui(ui, |ui| {
                                for zoom in [0.5, 0.75, 1., 1.25, 1.5, 2.] {
                                    ui.selectable_value(&mut self.zoom, zoom, format!("{}%", (zoom * 100.) as i32));
                                }
                                if ui.button("너비 맞춤").clicked() { self.fit_width(ui.ctx()); ui.close(); }
                            });
                        if ui.selectable_label(self.show_grid, "격자").on_hover_text("배치 격자 점 표시 / 숨기기").clicked() { self.show_grid = !self.show_grid; }
                        ui.small(format!("{}개 영역", self.document.regions.len()));
                    });
            });
        if ui.ctx().any_popup_open() {
            self.menu_resume = self.menu_resume.or(editing_before);
        } else if let Some(id) = self.menu_resume.take()
            && self.selected == Some(id)
            && self.insertion.is_none()
            && self.focus_new.is_none()
        {
            self.focus_new = Some(id);
            ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        }
    }
    fn ensure_math_region(&mut self) -> u64 {
        if let Some(id) = self.selected.filter(|id| {
            self.document
                .regions
                .iter()
                .any(|r| r.id == *id && matches!(r.content, CanvasContent::Math(_)))
        }) {
            return id;
        }
        let position = self.insertion.unwrap_or_else(|| {
            self.selected
                .and_then(|id| self.document.regions.iter().find(|r| r.id == id))
                .map(|r| (r.left, r.top + self.document.grid * 5.))
                .unwrap_or((0., self.document.grid))
        });
        let id = self.document.add_math(position.0, position.1);
        self.selected = Some(id);
        self.focus_new = Some(id);
        self.insertion = None;
        self.reveal = Some(position);
        id
    }
    fn insert_identifier(&mut self, name: &str, label: &str) {
        let before = self.snapshot();
        let id = self.ensure_math_region();
        if let Some(region) = self.document.regions.iter_mut().find(|r| r.id == id)
            && let CanvasContent::Math(tree) = &mut region.content
        {
            let editor = self
                .editors
                .entry(id)
                .or_insert_with(|| Editor::from_portable(tree).unwrap_or_default());
            editor.edit_group(|editor| {
                if editor.selection() == mathcraft_editor::Selection::None
                    && editor.arena().get(editor.caret().node).unwrap().kind()
                        != &NodeKind::Placeholder
                {
                    editor.insert_infix(BinaryOp::Multiply);
                }
                let _ = editor.replace_selection(&PortableTree {
                    kind: NodeKind::AnnotatedIdentifier {
                        name: name.into(),
                        labels: label.into(),
                    },
                    children: vec![],
                });
            });
            if let Ok(value) = editor.to_portable() {
                *tree = value;
            }
            self.remember(before);
            self.recalculate();
            self.focus_new = Some(id);
        }
    }
    fn ribbon_command(&mut self, ctx: &egui::Context, action: RibbonAction) {
        let p = || PortableTree {
            kind: NodeKind::Placeholder,
            children: vec![],
        };
        let binary = |kind| PortableTree {
            kind,
            children: vec![p(), p()],
        };
        let editing = self.editing_region.or(self.menu_resume);
        match action {
            RibbonAction::New => self.request(Pending::New),
            RibbonAction::Open => self.choose_open(),
            RibbonAction::Save => {
                self.save(false);
            }
            RibbonAction::SaveAs => {
                self.save(true);
            }
            RibbonAction::ExportMcdx => self.export_mcdx(),
            RibbonAction::Undo => self.undo(),
            RibbonAction::Redo => self.redo(),
            RibbonAction::Cut | RibbonAction::Copy => {
                if editing.is_some() {
                    self.focus_new = editing;
                    ctx.send_viewport_cmd(if action == RibbonAction::Cut {
                        egui::ViewportCommand::RequestCut
                    } else {
                        egui::ViewportCommand::RequestCopy
                    });
                } else {
                    self.copy_region(ctx, action == RibbonAction::Cut);
                }
            }
            RibbonAction::Paste => {
                self.focus_new = editing;
                ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
            }
            RibbonAction::Delete => self.remove_selected(),
            RibbonAction::InsertMath => self.begin_region(false),
            RibbonAction::InsertText => self.begin_region(true),
            RibbonAction::Fraction => self.insert_template(binary(NodeKind::Fraction)),
            RibbonAction::Power => self.insert_template(binary(NodeKind::Power)),
            RibbonAction::Root => self.insert_template(PortableTree {
                kind: NodeKind::Root,
                children: vec![
                    PortableTree {
                        kind: NodeKind::Number("2".into()),
                        children: vec![],
                    },
                    p(),
                ],
            }),
            RibbonAction::Absolute => self.insert_template(PortableTree {
                kind: NodeKind::Absolute,
                children: vec![p()],
            }),
            RibbonAction::Matrix => self.insert_template(PortableTree {
                kind: NodeKind::Matrix { rows: 2, cols: 2 },
                children: vec![p(), p(), p(), p()],
            }),
            RibbonAction::Index => self.insert_template(binary(NodeKind::Index { indices: 1 })),
            RibbonAction::Range => {
                self.insert_template(binary(NodeKind::Range { has_step: false }))
            }
            RibbonAction::Function => self.insert_template(PortableTree {
                kind: NodeKind::Function { parameter_count: 1 },
                children: vec![p(), p()],
            }),
            RibbonAction::Define | RibbonAction::Evaluate | RibbonAction::InsertSymbol(_) => {
                let text = match action {
                    RibbonAction::Define => ":",
                    RibbonAction::Evaluate => "=",
                    RibbonAction::InsertSymbol(s) => s,
                    _ => unreachable!(),
                };
                let before = self.snapshot();
                let id = self.ensure_math_region();
                {
                    if let Some(region) = self.document.regions.iter_mut().find(|r| r.id == id)
                        && let CanvasContent::Math(tree) = &mut region.content
                    {
                        let editor = self
                            .editors
                            .entry(id)
                            .or_insert_with(|| Editor::from_portable(tree).unwrap_or_default());
                        let outcome = formula::input_text_with_actions(editor, text);
                        if let Ok(value) = editor.to_portable() {
                            *tree = value;
                        }
                        if outcome.document_changed {
                            self.remember(before);
                        }
                        if outcome.document_changed || outcome.evaluation_requested {
                            self.recalculate();
                        }
                        self.focus_new = Some(id);
                    }
                }
            }
            RibbonAction::InsertIdentifier { name, label } => self.insert_identifier(name, label),
            RibbonAction::InsertFunction(name) => {
                let count = if name == "atan2" { 2 } else { 1 };
                let mut children = vec![PortableTree {
                    kind: NodeKind::Identifier(name.into()),
                    children: vec![],
                }];
                children.extend((0..count).map(|_| p()));
                self.insert_template(PortableTree {
                    kind: NodeKind::Call {
                        argument_count: count,
                    },
                    children,
                });
            }
            RibbonAction::Recalculate => {
                self.recalculate();
                self.focus_new = editing;
            }
            RibbonAction::Zoom(zoom) => self.zoom = zoom.clamp(0.5, 2.),
            RibbonAction::ToggleGrid => self.show_grid = !self.show_grid,
        }
    }
    fn context_hint(&self) -> &'static str {
        if self.editing_region.is_some() {
            if self
                .selected
                .and_then(|id| self.document.regions.iter().find(|r| r.id == id))
                .is_some_and(|r| matches!(r.content, CanvasContent::Text(_)))
            {
                "텍스트 편집 · Esc 선택으로 돌아가기"
            } else {
                "수식 편집 · Space 범위 선택 · Enter 다음 영역"
            }
        } else if self.selected.is_some() {
            "방향키 이동 · Enter 편집 · Delete 삭제 · 우클릭 메뉴"
        } else if self.insertion.is_some() {
            if self.text_mode {
                "텍스트 입력 · Ctrl+Shift+M 수식"
            } else {
                "수식 입력 · : 정의 · = 계산 · Ctrl+T 텍스트"
            }
        } else {
            "클릭 후 입력 · Ctrl+T 텍스트 · F1 단축키"
        }
    }
    fn fit_width(&mut self, ctx: &egui::Context) {
        self.zoom =
            ((ctx.content_rect().width() - 64.) / self.document.width as f32).clamp(0.5, 2.);
    }
    fn global_editing_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F1)) {
            self.show_shortcuts = !self.show_shortcuts;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F9)) {
            self.recalculate();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::T)) {
            self.begin_region(true);
        }
        if ctx.input_mut(|i| {
            i.consume_key(
                egui::Modifiers {
                    shift: true,
                    ..egui::Modifiers::COMMAND
                },
                egui::Key::M,
            )
        }) {
            self.begin_region(false);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Num0)) {
            self.zoom = 1.;
        }
        if ctx.input_mut(|i| {
            i.consume_key(egui::Modifiers::COMMAND, egui::Key::Plus)
                || i.consume_key(egui::Modifiers::COMMAND, egui::Key::Equals)
        }) {
            self.zoom = (self.zoom * 1.1).min(2.);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Minus)) {
            self.zoom = (self.zoom / 1.1).max(0.5);
        }
    }
    fn region_keyboard(&mut self, ui: &mut egui::Ui, previous_editing: Option<u64>) {
        if self.pending.is_some() || ui.ctx().any_popup_open() {
            return;
        }
        let escape = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        let toggle = ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter));
        if escape || (toggle && self.editing_region.is_some()) {
            // egui can release focus for Escape before this widget is visited.
            // Preserve the region selection for the first Escape in either case.
            if self
                .editing_region
                .take()
                .or(previous_editing.filter(|id| Some(*id) == self.selected))
                .is_none()
            {
                self.insertion = self
                    .selected
                    .and_then(|id| self.document.regions.iter().find(|r| r.id == id))
                    .map(|r| (r.left, r.top));
                self.selected = None;
            }
            self.focus_new = None;
            ui.memory_mut(|m| {
                if let Some(id) = m.focused() {
                    m.surrender_focus(id);
                }
            });
            return;
        }
        if self.editing_region.is_some() {
            return;
        }
        if (toggle || ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)))
            && self.selected.is_some()
        {
            self.focus_new = self.selected;
            ui.ctx().request_repaint();
            return;
        }
        let direction = ui.input_mut(|i| {
            for (key, x, y) in [
                (egui::Key::ArrowLeft, -1., 0.),
                (egui::Key::ArrowRight, 1., 0.),
                (egui::Key::ArrowUp, 0., -1.),
                (egui::Key::ArrowDown, 0., 1.),
            ] {
                if i.consume_key(
                    egui::Modifiers {
                        shift: true,
                        ..egui::Modifiers::NONE
                    },
                    key,
                ) {
                    return Some((x * 5., y * 5.));
                }
                if i.consume_key(egui::Modifiers::NONE, key) {
                    return Some((x, y));
                }
            }
            None
        });
        if let Some((x, y)) = direction {
            self.nudge(x, y);
        }
        let tab = ui.input_mut(|i| {
            if i.consume_key(
                egui::Modifiers {
                    shift: true,
                    ..egui::Modifiers::NONE
                },
                egui::Key::Tab,
            ) {
                Some(false)
            } else if i.consume_key(egui::Modifiers::NONE, egui::Key::Tab) {
                Some(true)
            } else {
                None
            }
        });
        if let Some(forward) = tab {
            let mut regions: Vec<_> = self.document.regions.iter().collect();
            regions.sort_by(|a, b| {
                a.top
                    .total_cmp(&b.top)
                    .then(a.left.total_cmp(&b.left))
                    .then(a.id.cmp(&b.id))
            });
            if !regions.is_empty() {
                let here = regions.iter().position(|r| Some(r.id) == self.selected);
                let next = match (here, forward) {
                    (Some(i), true) => (i + 1) % regions.len(),
                    (Some(i), false) => (i + regions.len() - 1) % regions.len(),
                    (None, true) => 0,
                    (None, false) => regions.len() - 1,
                };
                self.selected = Some(regions[next].id);
                self.insertion = None;
                self.reveal = Some((regions[next].left, regions[next].top));
            }
        }
    }
    fn nudge(&mut self, x: f64, y: f64) {
        let step = self.document.grid;
        if let Some(region) = self
            .selected
            .and_then(|id| self.document.regions.iter().find(|r| r.id == id))
        {
            let id = region.id;
            let left = (region.left + x * step).clamp(0., (self.document.width - step).max(0.));
            let top = (region.top + y * step).max(0.);
            if region.left == left && region.top == top {
                return;
            }
            let before = self.snapshot();
            if self.document.move_region(id, left, top).is_ok() {
                self.remember(before);
                self.recalculate();
                self.reveal = Some((left, top));
            }
        } else if let Some((left, top)) = self.insertion {
            self.insertion = Some((
                (left + x * step).clamp(0., (self.document.width - step).max(0.)),
                (top + y * step).max(0.),
            ));
            self.reveal = self.insertion;
        }
    }
    fn shortcuts_ui(&mut self, ctx: &egui::Context) {
        if self.show_shortcuts {
            egui::Window::new("키보드로 작성하기")
                .open(&mut self.show_shortcuts)
                .resizable(false)
                .show(ctx, |ui| {
                    egui::Grid::new("shortcut-list")
                        .spacing(vec2(24., 10.))
                        .show(ui, |ui| {
                            for (key, action) in [
                                (": / =", "정의 / 계산"),
                                ("/ · ^ · √", "분수 · 거듭제곱 · 제곱근"),
                                ("Space", "부분식 선택 범위 확장"),
                                ("Tab / Shift+Tab", "다음 / 이전 입력 칸"),
                                ("↑ / ↓", "수식의 위 / 아래 입력 칸"),
                                ("Shift+← / →", "문자 범위 선택"),
                                ("Enter / Esc", "다음 수식 / 영역 선택"),
                                ("Ctrl+T", "텍스트 영역 삽입"),
                                ("Ctrl+Z / Ctrl+Y", "실행 취소 / 다시 실행"),
                                ("Ctrl++ / Ctrl+− / Ctrl+0", "종이 확대 / 축소 / 100%"),
                                ("F9", "워크시트 다시 계산"),
                            ] {
                                ui.label(egui::RichText::new(key).strong());
                                ui.label(action);
                                ui.end_row();
                            }
                        });
                });
        }
    }
    fn page_geometry(&self) -> crate::page::PageGeometry {
        crate::page::PageGeometry::new(&self.document, self.page_margins)
    }
    fn paper_size(&self) -> egui::Vec2 {
        let bottom = self
            .document
            .regions
            .iter()
            .map(|r| r.top + 180.)
            .chain(self.insertion.map(|(_, y)| y + 80.))
            .fold(0., f64::max);
        self.page_geometry().size(bottom)
    }
    fn workspace_ui(&mut self, ui: &mut egui::Ui) {
        if ui.rect_contains_pointer(ui.max_rect()) {
            self.zoom = (self.zoom * ui.input(|i| i.zoom_delta())).clamp(0.5, 2.);
        }
        let available = ui.available_width();
        egui::ScrollArea::both()
            .id_salt("free-worksheet")
            .show(ui, |ui| {
                let size = self.paper_size();
                let scaled = size * self.zoom;
                let width = available.max(scaled.x + 48.);
                let (bounds, _) =
                    ui.allocate_exact_size(vec2(width, scaled.y + 48.), Sense::hover());
                let origin = bounds.min + vec2(((width - scaled.x) / 2.).max(24.), 24.);
                let paper_rect = Rect::from_min_size(origin, scaled);
                ui.painter().rect_filled(
                    paper_rect.translate(vec2(0., 3.)).expand(2.),
                    2.,
                    Color32::from_black_alpha(12),
                );
                let layer =
                    egui::LayerId::new(ui.layer_id().order, egui::Id::new("worksheet-paper"));
                let transform = egui::emath::TSTransform::from_translation(origin.to_vec2())
                    * egui::emath::TSTransform::from_scaling(self.zoom);
                ui.ctx().set_sublayer(ui.layer_id(), layer);
                ui.ctx().set_transform_layer(layer, transform);
                let mut page = ui.new_child(
                    UiBuilder::new()
                        .id_salt("paper-content")
                        .layer_id(layer)
                        .max_rect(Rect::from_min_size(egui::Pos2::ZERO, size))
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                );
                page.set_clip_rect(transform.inverse() * ui.clip_rect());
                self.page_ui(&mut page);
                if let Some((x, y)) = self.reveal.take() {
                    let target = Rect::from_min_size(
                        origin + self.page_geometry().to_paper(x, y).to_vec2() * self.zoom,
                        vec2(120., 80.) * self.zoom,
                    );
                    ui.scroll_to_rect(target, None);
                }
            });
    }
    fn page_ui(&mut self, ui: &mut egui::Ui) {
        let before = self.snapshot();
        let previous_editing = self.editing_region;
        let size = self.paper_size();
        let geometry = self.page_geometry();
        let pages = ((size.y + geometry.grid) / geometry.stride()).round() as usize;
        let (paper, background) = ui.allocate_exact_size(size, Sense::click());
        let grid = geometry.grid.max(6.);
        let visible = paper.intersect(ui.clip_rect());
        for page in 0..pages {
            let sheet = Rect::from_min_size(
                paper.min + vec2(0., page as f32 * geometry.stride()),
                vec2(geometry.width, geometry.height),
            );
            if !sheet.intersects(visible) {
                continue;
            }
            ui.painter()
                .rect_filled(sheet, 0., Color32::from_rgb(246, 250, 246));
            ui.painter().rect_stroke(
                sheet,
                0.,
                Stroke::new(0.5, Color32::from_gray(160)),
                egui::StrokeKind::Inside,
            );
            if self.show_grid {
                let body_origin = paper.min + geometry.body_origin(page).to_vec2();
                let body = Rect::from_min_size(
                    body_origin,
                    vec2(geometry.body_width(), geometry.body_height()),
                );
                let clip = body.intersect(visible);
                let grid_stroke = |distance: f32| {
                    let major = distance / geometry.major_spacing();
                    if self.use_major_grid && (major - major.round()).abs() < 0.001 {
                        Stroke::new(1.2, Color32::from_rgb(181, 218, 249))
                    } else {
                        Stroke::new(0.6, Color32::from_rgb(189, 222, 248))
                    }
                };
                let mut x =
                    body.left() + ((clip.left() - body.left()) / grid).ceil().max(0.) * grid;
                while x <= clip.right() {
                    ui.painter().line_segment(
                        [pos2(x, clip.top()), pos2(x, clip.bottom())],
                        grid_stroke(x - body.left()),
                    );
                    x += grid;
                }
                let mut y = body.top() + ((clip.top() - body.top()) / grid).ceil().max(0.) * grid;
                while y <= clip.bottom() {
                    ui.painter().line_segment(
                        [pos2(clip.left(), y), pos2(clip.right(), y)],
                        grid_stroke(y - body.top()),
                    );
                    y += grid;
                }
            }
        }
        let mut changed = false;
        let mut evaluation_requested = false;
        let mut geometry_changed = false;
        let mut movement_started = false;
        let mut hit_region = false;
        let mut context_selection = None;
        let mut context_action = None;
        self.editing_region = None;
        for region in &mut self.document.regions {
            let origin = paper.min + geometry.to_paper(region.left, region.top).to_vec2();
            let max = Rect::from_min_size(
                origin,
                vec2((paper.right() - origin.x - 15.).max(100.), 220.),
            );
            let id = region.id;
            let mut child = ui.new_child(
                UiBuilder::new()
                    .id_salt(("region", id))
                    .max_rect(max)
                    .layout(egui::Layout::left_to_right(egui::Align::Min)),
            );
            child.set_clip_rect(paper.intersect(ui.clip_rect()));
            let row_result = self.results.get(&id);
            let display = row_result
                .and_then(|r| r.display_value.as_ref().or(r.value.as_ref()))
                .map(display_result);
            let response = match &mut region.content {
                CanvasContent::Math(tree) => {
                    let editor = self
                        .editors
                        .entry(id)
                        .or_insert_with(|| Editor::from_portable(tree).unwrap_or_default());
                    let outcome = formula::formula_ui_with_labels(
                        &mut child,
                        editor,
                        display.as_ref().map(DisplayResult::borrowed),
                        self.focus_new == Some(id),
                        self.labelled_trees.get(&id),
                    );
                    evaluation_requested |= outcome.evaluation_requested;
                    let response = outcome.response;
                    if response.changed() {
                        if let Ok(value) = editor.to_portable() {
                            *tree = value;
                        }
                        changed = true;
                    }
                    response
                }
                CanvasContent::Text(text) => {
                    let text_id = egui::Id::new(("worksheet-text", id));
                    if self.focus_new == Some(id) {
                        if let Some(range) = self.text_cursors.get(&id) {
                            let mut state =
                                egui::text_edit::TextEditState::load(child.ctx(), text_id)
                                    .unwrap_or_default();
                            state.cursor.set_char_range(Some(*range));
                            state.store(child.ctx(), text_id);
                        }
                        child.memory_mut(|m| m.request_focus(text_id));
                    }
                    let output = egui::TextEdit::multiline(text)
                        .id(text_id)
                        .frame(egui::Frame::NONE)
                        .desired_rows(1)
                        .desired_width(340.)
                        .hint_text("텍스트를 입력하세요")
                        .font(typography::body_font())
                        .char_limit(65536)
                        .show(&mut child);
                    if let Some(range) = output.cursor_range {
                        self.text_cursors.insert(id, range);
                    }
                    let response = output.response.response;
                    changed |= response.changed();
                    response
                }
                CanvasContent::Opaque { .. } => child.add(
                    egui::Label::new(
                        egui::RichText::new("가져온 영역 · 읽기 전용")
                            .color(Color32::from_gray(130)),
                    )
                    .sense(Sense::click()),
                ),
            };
            if self.focus_new == Some(id) {
                response.request_focus();
                self.focus_new = None;
            }
            if response.clicked() || response.has_focus() {
                self.selected = Some(id);
                self.insertion = None;
            }
            if response.has_focus() {
                self.editing_region = Some(id);
            }
            hit_region |= response.hovered();
            if response.secondary_clicked() {
                context_selection = Some(id);
            }
            response.context_menu(|ui| {
                for (label, action) in [
                    ("편집", RegionAction::Edit),
                    ("영역 복사", RegionAction::Copy),
                    ("영역 잘라내기", RegionAction::Cut),
                    ("복제", RegionAction::Duplicate),
                    ("삭제", RegionAction::Delete),
                ] {
                    if ui
                        .add_enabled(
                            !matches!(region.content, CanvasContent::Opaque { .. }),
                            egui::Button::new(label),
                        )
                        .clicked()
                    {
                        context_action = Some((id, action));
                        ui.close();
                    }
                }
            });
            if self.selected == Some(id) || response.hovered() {
                let border = response.rect.expand(1.);
                let stroke = Stroke::new(0.5, Color32::from_rgb(78, 126, 155));
                if response.has_focus() {
                    let points = [
                        border.left_top(),
                        border.right_top(),
                        border.right_bottom(),
                        border.left_bottom(),
                        border.left_top(),
                    ];
                    child
                        .painter()
                        .extend(egui::Shape::dashed_line(&points, stroke, 4., 2.));
                } else {
                    child
                        .painter()
                        .rect_stroke(border, 1., stroke, egui::StrokeKind::Outside);
                }
                let handle = Rect::from_min_size(
                    origin - vec2(14., 0.),
                    vec2(12., response.rect.height().max(20.)),
                );
                let drag = ui
                    .interact(handle, egui::Id::new(("move", id)), Sense::click_and_drag())
                    .on_hover_cursor(egui::CursorIcon::Grab)
                    .on_hover_text("드래그하여 이동 · 클릭 후 방향키로 미세 이동");
                hit_region |= drag.hovered();
                if drag.clicked() {
                    self.selected = Some(id);
                    response.surrender_focus();
                    self.editing_region = None;
                }
                if drag.drag_started() {
                    self.selected = Some(id);
                    response.surrender_focus();
                    self.editing_region = None;
                    self.dragging = Some((id, region.left, region.top));
                    movement_started = true;
                }
                if drag.dragged()
                    && let Some((moving, start_x, start_y)) = self.dragging
                    && moving == id
                    && let Some(delta) = drag.total_drag_delta()
                {
                    let target = geometry.to_paper(start_x, start_y) + delta;
                    (region.left, region.top) = geometry.to_body_clamped(target);
                    geometry_changed = true;
                }
                if drag.drag_stopped() {
                    let snap = |v: f64| {
                        let r = v % self.document.grid;
                        if r > self.document.grid / 2. {
                            v + self.document.grid - r
                        } else {
                            v - r
                        }
                    };
                    region.left = snap(region.left);
                    region.top = snap(region.top);
                    self.dragging = None;
                    geometry_changed = true;
                }
            }
            if let Some(error) = row_result.and_then(|r| r.error.as_ref()) {
                let indicator =
                    Rect::from_min_size(response.rect.right_top() + vec2(3., 4.), vec2(18., 18.));
                child
                    .painter()
                    .circle_filled(indicator.center(), 7., RED.gamma_multiply(0.12));
                child.painter().text(
                    indicator.center(),
                    egui::Align2::CENTER_CENTER,
                    "!",
                    FontId::proportional(13.),
                    RED,
                );
                child
                    .interact(
                        response.rect.union(indicator),
                        egui::Id::new(("error", id)),
                        Sense::hover(),
                    )
                    .on_hover_text(&error.detail);
            }
        }
        if changed || movement_started {
            self.remember(before);
        }
        if changed || geometry_changed {
            self.status.clear();
            self.dirty = true;
        }
        if changed || geometry_changed || evaluation_requested {
            self.recalculate();
            ui.ctx().request_repaint();
        }
        if let Some(id) = context_selection {
            self.selected = Some(id);
            self.editing_region = None;
            self.insertion = None;
            ui.memory_mut(|m| {
                if let Some(focus) = m.focused() {
                    m.surrender_focus(focus);
                }
            });
        }
        if let Some((id, action)) = context_action {
            self.selected = Some(id);
            match action {
                RegionAction::Edit => self.focus_new = Some(id),
                RegionAction::Copy => self.copy_region(ui.ctx(), false),
                RegionAction::Cut => self.copy_region(ui.ctx(), true),
                RegionAction::Duplicate => self.duplicate_selected(),
                RegionAction::Delete => self.remove_selected(),
            }
        }
        if let Some((x, y)) = self.insertion {
            let p = paper.min + geometry.to_paper(x, y).to_vec2();
            ui.painter().line_segment(
                [p - vec2(5.5, 0.), p + vec2(5.5, 0.)],
                Stroke::new(1., BLUE),
            );
            ui.painter().line_segment(
                [p - vec2(0., 5.5), p + vec2(0., 5.5)],
                Stroke::new(1., BLUE),
            );
        }
        if background.clicked()
            && !hit_region
            && let Some(pointer) = background.interact_pointer_pos()
        {
            let Some((body_x, body_y)) = geometry.to_body((pointer - paper.min).to_pos2()) else {
                return;
            };
            let snap = |v: f64| {
                let r = v % self.document.grid;
                if r > self.document.grid / 2. {
                    v + self.document.grid - r
                } else {
                    v - r
                }
            };
            self.insertion = Some((snap(body_x), snap(body_y)));
            self.selected = None;
            self.editing_region = None;
            ui.memory_mut(|m| {
                if let Some(id) = m.focused() {
                    m.surrender_focus(id);
                }
            });
            background.request_focus();
        }
        self.region_keyboard(ui, previous_editing);
        if self.editing_region.is_none()
            && (self.selected.is_some() || self.insertion.is_some())
            && !ui.ctx().any_popup_open()
        {
            if !background.has_focus() {
                background.request_focus();
            }
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    background.id,
                    egui::EventFilter {
                        tab: true,
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        ..Default::default()
                    },
                )
            });
        }
        // Enter commits the current formula and advances to a new insertion point.
        if let Some(id) = self.selected
            && self.editing_region == Some(id)
            && let Some(region) = self.document.regions.iter().find(|r| r.id == id)
            && matches!(region.content, CanvasContent::Math(_))
            && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter))
        {
            let display = self
                .results
                .get(&id)
                .and_then(|r| r.display_value.as_ref().or(r.value.as_ref()))
                .map(display_result);
            let height = self
                .editors
                .get(&id)
                .map(|e| {
                    formula::measured_size(ui, e, display.as_ref().map(DisplayResult::borrowed)).y
                        as f64
                })
                .unwrap_or(self.document.grid * 3.);
            let advance =
                ((height + self.document.grid) / self.document.grid).ceil() * self.document.grid;
            self.insertion = Some((
                region.left,
                region.top + advance.max(self.document.grid * 3.),
            ));
            self.reveal = self.insertion;
            self.selected = None;
            self.editing_region = None;
            ui.memory_mut(|m| {
                if let Some(focused) = m.focused() {
                    m.surrender_focus(focused)
                }
            });
            background.request_focus();
        }
        if let Some(position) = self.insertion {
            let ime_id = ui.id().with("insertion-preedit");
            let mut preedit = ui
                .data(|data| data.get_temp::<String>(ime_id))
                .unwrap_or_default();
            for event in ui.input(|i| i.events.clone()) {
                match event {
                    egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => preedit = text,
                    egui::Event::Ime(egui::ImeEvent::Commit(_)) => preedit.clear(),
                    _ => (),
                }
            }
            let cursor =
                paper.min + geometry.to_paper(position.0, position.1).to_vec2() + vec2(2., 2.);
            if !preedit.is_empty() {
                ui.painter().text(
                    cursor,
                    egui::Align2::LEFT_TOP,
                    &preedit,
                    typography::MathStyle::Variable.font(typography::MATH_SIZE),
                    Color32::from_gray(50),
                );
            }
            ui.data_mut(|data| data.insert_temp(ime_id, preedit));
            if background.has_focus() {
                let to_global = ui
                    .ctx()
                    .layer_transform_to_global(ui.layer_id())
                    .unwrap_or_default();
                ui.output_mut(|o| {
                    o.ime = Some(egui::output::IMEOutput {
                        purpose: egui::IMEPurpose::Normal,
                        rect: to_global * Rect::from_min_size(cursor, vec2(120., 30.)),
                        cursor_rect: to_global * Rect::from_min_size(cursor, vec2(1., 30.)),
                        should_interrupt_composition: false,
                    })
                });
            }
            let text: String = ui.input(|i| {
                i.events
                    .iter()
                    .filter_map(|e| match e {
                        egui::Event::Text(s) | egui::Event::Paste(s) => Some(s.as_str()),
                        egui::Event::Ime(egui::ImeEvent::Commit(s)) => Some(s.as_str()),
                        _ => None,
                    })
                    .collect()
            });
            if !text.is_empty() {
                ui.data_mut(|data| data.remove::<String>(ime_id));
                let is_text = self.text_mode;
                let id = self.insert(position, is_text);
                if is_text {
                    self.document
                        .regions
                        .iter_mut()
                        .find(|r| r.id == id)
                        .unwrap()
                        .content = CanvasContent::Text(text);
                    ui.ctx().request_repaint();
                    return;
                }
                let mut editor = Editor::new();
                formula::input_text(&mut editor, &text);
                if let Ok(tree) = editor.to_portable()
                    && let Some(region) = self.document.regions.iter_mut().find(|r| r.id == id)
                {
                    region.content = CanvasContent::Math(tree);
                }
                self.editors.insert(id, editor);
                self.recalculate();
                ui.ctx().request_repaint();
            }
        }
    }
}

fn fault_statement(tree: &PortableTree) -> String {
    const ML: &str = "http://schemas.mathsoft.com/math50";
    if matches!(tree.kind, NodeKind::Binary(BinaryOp::Define))
        && let Some(mut lhs) = tree.children.first()
    {
        while matches!(lhs.kind, NodeKind::Function { .. } | NodeKind::Index { .. }) {
            let Some(name) = lhs.children.first() else {
                break;
            };
            lhs = name;
        }
        if matches!(
            lhs.kind,
            NodeKind::Identifier(_)
                | NodeKind::AnnotatedIdentifier { .. }
                | NodeKind::NativeIdentifier { .. }
        ) && let Ok(name) = worksheet_math::tree_to_mathml(lhs)
        {
            return format!("<ml:define xmlns:ml=\"{ML}\">{name}<ml:unsupported/></ml:define>");
        }
    }
    format!(
        "<ml:eval xmlns:ml=\"{ML}\"><ml:unsupported/><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>"
    )
}

enum DisplayResult {
    Text(String),
    Matrix {
        rows: usize,
        cols: usize,
        cells: Vec<String>,
        brackets: bool,
    },
}
impl DisplayResult {
    fn borrowed(&self) -> formula::FormulaResult<'_> {
        match self {
            Self::Text(text) => formula::FormulaResult::Text(text),
            Self::Matrix {
                rows,
                cols,
                cells,
                brackets,
            } => formula::FormulaResult::Matrix {
                rows: *rows,
                cols: *cols,
                cells,
                brackets: *brackets,
            },
        }
    }
}
fn display_result(value: &Value) -> DisplayResult {
    match value {
        Value::Symbolic { mathml } => DisplayResult::Text(mathml.clone()),
        Value::Scalar { quantity } => DisplayResult::Text(quantity.to_string()),
        Value::Matrix {
            rows,
            cols,
            elements,
        } => {
            let shown_rows = (*rows).min(12);
            let shown_cols = (*cols).min(12);
            let cells = (0..shown_rows)
                .flat_map(|row| {
                    (0..shown_cols).map(move |col| {
                        if (*rows > 12 && row == 11) || (*cols > 12 && col == 11) {
                            "…".to_owned()
                        } else {
                            elements[row * cols + col].to_string()
                        }
                    })
                })
                .collect();
            DisplayResult::Matrix {
                rows: shown_rows,
                cols: shown_cols,
                cells,
                brackets: true,
            }
        }
        Value::Range {
            start, step, count, ..
        } => {
            if *count == 0 {
                return DisplayResult::Text("∅".into());
            }
            let cells = (0..(*count).min(12))
                .map(|i| {
                    if *count > 12 && i == 11 {
                        "…".into()
                    } else {
                        mathcraft_core::Quantity {
                            value: start.value + step.value * i as f64,
                            ..*start
                        }
                        .to_string()
                    }
                })
                .collect();
            DisplayResult::Matrix {
                rows: (*count).min(12),
                cols: 1,
                cells,
                brackets: false,
            }
        }
        Value::RangeValues { elements } => {
            if elements.is_empty() {
                return DisplayResult::Text("∅".into());
            }
            let cells = elements
                .iter()
                .take(12)
                .enumerate()
                .map(|(i, q)| {
                    if elements.len() > 12 && i == 11 {
                        "…".into()
                    } else {
                        q.to_string()
                    }
                })
                .collect();
            DisplayResult::Matrix {
                rows: elements.len().min(12),
                cols: 1,
                cells,
                brackets: false,
            }
        }
    }
}
#[cfg(test)]
fn value_text(value: &Value) -> String {
    match value {
        Value::Symbolic { mathml } => mathml.clone(),
        Value::Scalar { quantity } => quantity.to_string(),
        Value::Matrix {
            rows,
            cols,
            elements,
        } => format!(
            "[{} × {}] {}",
            rows,
            cols,
            elements
                .iter()
                .map(|q| mathcraft_core::format_complex(q.value))
                .collect::<Vec<_>>()
                .join("; ")
        ),
        Value::Range {
            start, step, end, ..
        } => format!(
            "{}, {} .. {}",
            start,
            mathcraft_core::Quantity {
                value: start.value + step.value,
                ..*start
            },
            end
        ),
        Value::RangeValues { elements } => elements
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; "),
    }
}

fn standalone(node: roxmltree::Node<'_, '_>, source: &str) -> String {
    let raw = &source[node.range()];
    let end = raw.find('>').unwrap_or(raw.len());
    let at = if raw[..end].ends_with('/') {
        end - 1
    } else {
        end
    };
    let mut declarations = String::new();
    for namespace in node.namespaces() {
        let name = namespace
            .name()
            .map_or("xmlns".to_owned(), |p| format!("xmlns:{p}"));
        if !raw[..end].contains(&format!("{name}=")) {
            declarations.push_str(&format!(" {name}=\"{}\"", namespace.uri()));
        }
    }
    format!("{}{}{}", &raw[..at], declarations, &raw[at..])
}
fn import_native(path: &Path) -> Result<CanvasDocument, String> {
    let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > 16 * 1024 * 1024 {
        return Err("This worksheet exceeds the current 16 MiB import limit".into());
    }
    let source_bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    if source_bytes.len() > 16 * 1024 * 1024 {
        return Err("This worksheet exceeds the current 16 MiB import limit".into());
    }
    let package = NativePackage::from_bytes(&source_bytes)?;
    let source = package.worksheet_xml()?;
    let xml = roxmltree::Document::parse(&source).map_err(|e| e.to_string())?;
    let mut document = CanvasDocument {
        title: path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        native_archive: Some(source_bytes),
        ..Default::default()
    };
    if let Some(presentation) = package
        .original_xml_part("mathcad/settings/presentation.xml")
        .ok()
        .flatten()
        && let Ok(grid) = mathcraft_compat::PlacementGrid::from_presentation(
            presentation.trim_start_matches('\u{feff}'),
        )
    {
        document.grid = grid.spacing();
        if let Some((size, _)) = crate::page::source_settings(&presentation) {
            document.width = f64::from(size.x);
            document.height = f64::from(size.y);
        }
    }
    let notes: HashMap<_, _> = mathcraft_document::import_mcdx(path)
        .ok()
        .into_iter()
        .flat_map(|d| d.rows)
        .filter(|r| r.note)
        .filter_map(|r| r.imported.map(|i| (i.region_id, r.input)))
        .collect();
    let regions = xml
        .root_element()
        .children()
        .find(|n| n.has_tag_name(("http://schemas.mathsoft.com/worksheet50", "regions")))
        .ok_or("Missing worksheet regions")?;
    for region in regions.children().filter(roxmltree::Node::is_element) {
        let source_id = region.attribute("region-id").unwrap_or("");
        let left: f64 = region
            .attribute("left")
            .unwrap_or("0")
            .parse()
            .map_err(|_| "Invalid region left")?;
        let top: f64 = region
            .attribute("top")
            .unwrap_or("0")
            .parse()
            .map_err(|_| "Invalid region top")?;
        let child = region.children().find(roxmltree::Node::is_element);
        let content = if let Some(child) = child {
            if child.has_tag_name(("http://schemas.mathsoft.com/worksheet50", "math")) {
                let tree = worksheet_math::tree_from_mathml(&standalone(child, &source));
                match tree {
                    Ok(tree) => CanvasContent::Math(tree),
                    Err(_) => CanvasContent::Opaque {
                        xml: source[region.range()].into(),
                        kind: "math".into(),
                    },
                }
            } else if let Some(note) = notes.get(source_id) {
                CanvasContent::Text(note.clone())
            } else {
                CanvasContent::Opaque {
                    xml: source[region.range()].into(),
                    kind: child.tag_name().name().into(),
                }
            }
        } else {
            CanvasContent::Opaque {
                xml: source[region.range()].into(),
                kind: "empty region".into(),
            }
        };
        let id = document.next_id;
        document.next_id += 1;
        document.regions.push(CanvasRegion {
            id,
            left,
            top,
            content,
            source_region: Some(source_id.into()),
        });
    }
    document.validate()?;
    Ok(document)
}

impl eframe::App for WorksheetApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let title = format!(
            "{}{} — Mathcraft",
            self.path
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| self.document.title.clone()),
            if self.dirty { " *" } else { "" }
        );
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
        if ctx.input(|i| i.viewport().close_requested()) && self.dirty && !self.closing {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::Close);
        }
        if self.closing {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if self.pending.is_none() {
            if ctx.input_mut(|i| {
                i.consume_key(
                    egui::Modifiers {
                        shift: true,
                        ..egui::Modifiers::COMMAND
                    },
                    egui::Key::S,
                )
            }) {
                self.save(true);
            } else if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
                self.save(false);
            }
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::N)) {
                self.request(Pending::New);
            }
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::O))
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("Worksheets", &["mathcraft", "mcdx"])
                    .pick_file()
            {
                self.request(Pending::Open(path));
            }
            self.global_editing_shortcuts(&ctx);
            // Document history owns Ctrl+Z/Y so movement, text and math are one undo stream.
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z)) {
                self.undo();
            }
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Y))
                || ctx.input_mut(|i| {
                    i.consume_key(
                        egui::Modifiers {
                            shift: true,
                            ..egui::Modifiers::COMMAND
                        },
                        egui::Key::Z,
                    )
                })
            {
                self.redo();
            }
            if self.editing_region.is_none() {
                let copy = ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::C))
                    || ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Copy)));
                let cut = ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::X))
                    || ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Cut)));
                if copy || cut {
                    self.copy_region(&ctx, cut);
                }
                let pasted = ctx.input(|i| {
                    i.events.iter().find_map(|e| {
                        if let egui::Event::Paste(text) = e {
                            (text.starts_with("mathcraft-region:")
                                || text.starts_with("mathcraft-tree:"))
                            .then(|| text.clone())
                        } else {
                            None
                        }
                    })
                });
                if let Some(text) = pasted
                    && self.paste_region(&text)
                {
                    ctx.input_mut(|i| i.events.retain(|e| !matches!(e, egui::Event::Paste(_))));
                }
                if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Delete)) {
                    self.remove_selected();
                }
            }
        }
        if self.pending.is_some() {
            ui.disable();
        }
        self.chrome_ui(ui);
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::from_rgb(192, 192, 192)))
            .show(ui, |ui| self.workspace_ui(ui));
        self.shortcuts_ui(&ctx);
        if self.pending.is_some() {
            let modal = egui::Modal::new(egui::Id::new("unsaved-worksheet")).show(&ctx, |ui| {
                ui.heading("변경 사항을 저장할까요?");
                ui.label("저장하지 않은 변경 사항이 있습니다.");
                ui.horizontal(|ui| {
                    if ui.button("저장").clicked()
                        && self.save(false)
                        && let Some(action) = self.pending.take()
                    {
                        self.apply(action);
                    }
                    if ui.button("저장하지 않음").clicked()
                        && let Some(action) = self.pending.take()
                    {
                        self.apply(action);
                    }
                    if ui.button("취소").clicked() {
                        self.pending = None;
                    }
                });
            });
            if modal.should_close() {
                self.pending = None;
            }
        }
        self.frames += 1;
        if let Some(path) = &self.capture {
            let shot = ctx.input(|i| {
                i.events.iter().find_map(|e| {
                    if let egui::Event::Screenshot { image, .. } = e {
                        Some(image.clone())
                    } else {
                        None
                    }
                })
            });
            if let Some(shot) = shot {
                let bytes: Vec<u8> = shot.pixels.iter().flat_map(|p| p.to_array()).collect();
                if let Some(image) =
                    image::RgbaImage::from_raw(shot.size[0] as u32, shot.size[1] as u32, bytes)
                {
                    let _ = image.save(path);
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            if self.frames >= 3 && !self.capture_sent {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                self.capture_sent = true;
            }
            ctx.request_repaint();
        }
        if self.smoke {
            if self.frames >= 3 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                ctx.request_repaint();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(app: &mut WorksheetApp, ctx: &egui::Context, events: Vec<egui::Event>) {
        let frame = ctx.run_ui(
            egui::RawInput {
                events,
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1200., 900.))),
                ..Default::default()
            },
            |ui| app.page_ui(ui),
        );
        frame.drop_without_applying_deltas();
    }
    fn enter() -> egui::Event {
        egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }
    fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }
    #[test]
    fn undo_restores_the_editing_caret_and_typing_continues_in_the_same_region() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp {
            insertion: Some((38.4, 38.4)),
            ..Default::default()
        };
        frame(&mut app, &context, vec![egui::Event::Text("a:=12".into())]);
        frame(&mut app, &context, vec![]);
        let id = app.selected.unwrap();
        let editor = app.editors.get_mut(&id).unwrap();
        let number = editor.arena().get(editor.root()).unwrap().children()[1];
        editor.place_text_caret(number, 1).unwrap();
        frame(&mut app, &context, vec![egui::Event::Text("3".into())]);
        app.undo();
        assert_eq!(app.selected, Some(id));
        assert_eq!(app.editors[&id].caret().value_range.unwrap().focus, 1);
        frame(&mut app, &context, vec![egui::Event::Text("9".into())]);
        assert_eq!(app.selected, Some(id));
        assert_eq!(
            app.editors[&id].arena().get(number).unwrap().kind(),
            &NodeKind::Number("192".into())
        );
        assert!(app.redo.is_empty());
    }
    #[test]
    fn inserting_a_fraction_from_the_menu_preserves_the_selected_operand() {
        let mut app = WorksheetApp::default();
        let id = app.insert((38.4, 38.4), false);
        let mut editor = Editor::new();
        formula::input_text(&mut editor, "1+23");
        let root = editor.root();
        let operand = editor.arena().get(root).unwrap().children()[1];
        editor.select_subtree(operand).unwrap();
        let before = editor.to_portable().unwrap();
        app.document.regions[0].content = CanvasContent::Math(before.clone());
        app.editors.insert(id, editor);
        let placeholder = || PortableTree {
            kind: NodeKind::Placeholder,
            children: vec![],
        };
        app.insert_template(PortableTree {
            kind: NodeKind::Fraction,
            children: vec![placeholder(), placeholder()],
        });
        let tree = app.editors[&id].to_portable().unwrap();
        assert_eq!(tree.children[1].kind, NodeKind::Fraction);
        assert_eq!(
            tree.children[1].children[0].kind,
            NodeKind::Number("23".into())
        );
        assert_eq!(app.editors[&id].root(), root);
        app.undo();
        assert_eq!(app.editors[&id].to_portable().unwrap(), before);
        assert_eq!(
            app.editors[&id].selection(),
            mathcraft_editor::Selection::Subtree(operand)
        );
    }

    #[test]
    fn document_undo_restores_text_selection_before_further_typing() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp::default();
        app.begin_region(true);
        let id = app.selected.unwrap();
        frame(&mut app, &context, vec![]);
        frame(&mut app, &context, vec![egui::Event::Text("abcd".into())]);
        let text_id = egui::Id::new(("worksheet-text", id));
        let range = egui::text::CCursorRange::one(egui::text::CCursor::new(2));
        let mut state = egui::text_edit::TextEditState::load(&context, text_id).unwrap();
        state.cursor.set_char_range(Some(range));
        state.store(&context, text_id);
        app.text_cursors.insert(id, range);
        frame(&mut app, &context, vec![egui::Event::Text("X".into())]);
        assert!(matches!(&app.document.regions[0].content, CanvasContent::Text(s) if s == "abXcd"));
        app.undo();
        frame(&mut app, &context, vec![egui::Event::Text("Y".into())]);
        assert_eq!(app.editing_region, Some(id));
        assert!(matches!(&app.document.regions[0].content, CanvasContent::Text(s) if s == "abYcd"));
    }

    #[test]
    fn escape_selects_region_arrows_move_it_and_enter_resumes_editing() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp {
            insertion: Some((38.4, 38.4)),
            ..Default::default()
        };
        frame(&mut app, &context, vec![egui::Event::Text("23".into())]);
        frame(&mut app, &context, vec![]);
        let id = app.selected.unwrap();
        let original_x = app.document.regions[0].left;
        frame(
            &mut app,
            &context,
            vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
        );
        assert_eq!(app.selected, Some(id));
        assert!(app.editing_region.is_none());
        frame(
            &mut app,
            &context,
            vec![key(egui::Key::ArrowRight, egui::Modifiers::NONE)],
        );
        assert!((app.document.regions[0].left - original_x - app.document.grid).abs() < 1e-10);
        app.undo();
        assert_eq!(app.selected, Some(id));
        assert_eq!(app.document.regions[0].left, original_x);
        frame(&mut app, &context, vec![enter()]);
        assert_eq!(app.focus_new, Some(id));
        assert_eq!(app.document.regions.len(), 1);
        frame(&mut app, &context, vec![egui::Event::Text("4".into())]);
        assert_eq!(app.editing_region, Some(id));
        assert_eq!(
            app.editors[&id].to_portable().unwrap().kind,
            NodeKind::Number("234".into())
        );
    }
    #[test]
    fn cancelled_blank_page_composition_does_not_create_an_empty_region() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp {
            insertion: Some((38.4, 38.4)),
            ..Default::default()
        };
        frame(
            &mut app,
            &context,
            vec![egui::Event::Ime(egui::ImeEvent::Preedit {
                text: "한".into(),
                active_range_chars: None,
            })],
        );
        assert!(app.document.regions.is_empty());
        assert!(app.undo.is_empty());
        frame(
            &mut app,
            &context,
            vec![egui::Event::Ime(egui::ImeEvent::Commit(String::new()))],
        );
        assert!(app.document.regions.is_empty());
        frame(
            &mut app,
            &context,
            vec![egui::Event::Ime(egui::ImeEvent::Commit("한글".into()))],
        );
        assert_eq!(app.document.regions.len(), 1);
        let id = app.selected.unwrap();
        assert_eq!(
            app.editors[&id].to_portable().unwrap().kind,
            NodeKind::Identifier("한글".into())
        );
    }
    #[test]
    fn text_mode_accepts_first_committed_text_without_a_math_parser() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp {
            insertion: Some((38.4, 38.4)),
            text_mode: true,
            ..Default::default()
        };
        frame(
            &mut app,
            &context,
            vec![egui::Event::Ime(egui::ImeEvent::Commit(
                "계산 메모: 2+3=5".into(),
            ))],
        );
        assert!(
            matches!(&app.document.regions[0].content, CanvasContent::Text(text) if text == "계산 메모: 2+3=5")
        );
        assert!(app.editors.is_empty());
    }
    fn workspace_frame(
        app: &mut WorksheetApp,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::emath::TSTransform {
        let mut layer = None;
        let output = ctx.run_ui(
            egui::RawInput {
                events,
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1100., 900.))),
                ..Default::default()
            },
            |ui| {
                app.chrome_ui(ui);
                egui::CentralPanel::default().show(ui, |ui| {
                    layer = Some(egui::LayerId::new(
                        ui.layer_id().order,
                        egui::Id::new("worksheet-paper"),
                    ));
                    app.workspace_ui(ui);
                });
            },
        );
        fn find_text(shape: &egui::epaint::Shape) -> Option<Rect> {
            match shape {
                egui::epaint::Shape::Text(text) if text.galley.text() == "파일" => {
                    Some(Rect::from_min_size(text.pos, text.galley.size()))
                }
                egui::epaint::Shape::Vec(shapes) => shapes.iter().find_map(find_text),
                _ => None,
            }
        }
        if let Some(rect) = output.shapes.iter().find_map(|s| find_text(&s.shape)) {
            ctx.data_mut(|data| data.insert_temp(egui::Id::new("test-file-menu"), rect));
        }
        output.drop_without_applying_deltas();
        ctx.layer_transform_to_global(layer.unwrap()).unwrap()
    }
    #[test]
    fn closing_a_ribbon_menu_resumes_typing_without_a_second_escape_action() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp::default();
        let id = app.insert((38.4, 38.4), false);
        let mut editor = Editor::new();
        formula::input_text(&mut editor, "12");
        app.document.regions[0].content = CanvasContent::Math(editor.to_portable().unwrap());
        app.editors.insert(id, editor);
        workspace_frame(&mut app, &context, vec![]);
        workspace_frame(&mut app, &context, vec![]);
        let point = context
            .data(|data| data.get_temp::<Rect>(egui::Id::new("test-file-menu")))
            .unwrap()
            .center();
        for pressed in [true, false] {
            workspace_frame(
                &mut app,
                &context,
                vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        assert_eq!(app.menu_resume, Some(id));
        workspace_frame(
            &mut app,
            &context,
            vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
        );
        workspace_frame(&mut app, &context, vec![egui::Event::Text("3".into())]);
        assert_eq!(app.editing_region, Some(id));
        assert_eq!(
            app.editors[&id].to_portable().unwrap().kind,
            NodeKind::Number("123".into())
        );
    }

    #[test]
    fn region_keyboard_navigation_keeps_focus_on_the_page_with_toolbar_present() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp::default();
        for (top, text) in [(38.4, "1"), (115.2, "2")] {
            let id = app.insert((38.4, top), false);
            let mut editor = Editor::new();
            formula::input_text(&mut editor, text);
            app.document
                .regions
                .iter_mut()
                .find(|r| r.id == id)
                .unwrap()
                .content = CanvasContent::Math(editor.to_portable().unwrap());
            app.editors.insert(id, editor);
        }
        let first = app.document.regions[0].id;
        workspace_frame(&mut app, &context, vec![]);
        workspace_frame(
            &mut app,
            &context,
            vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
        );
        workspace_frame(
            &mut app,
            &context,
            vec![key(egui::Key::Tab, egui::Modifiers::NONE)],
        );
        assert_eq!(app.selected, Some(first));
        assert!(app.editing_region.is_none());
        workspace_frame(&mut app, &context, vec![enter()]);
        workspace_frame(&mut app, &context, vec![egui::Event::Text("4".into())]);
        assert_eq!(app.editing_region, Some(first));
        assert_eq!(
            app.editors[&first].to_portable().unwrap().kind,
            NodeKind::Number("14".into())
        );
    }

    #[test]
    fn page_zoom_keeps_chrome_scale_and_pointer_document_coordinates() {
        for zoom in [0.5, 1., 2.] {
            let context = egui::Context::default();
            crate::configure(&context);
            let mut app = WorksheetApp {
                zoom,
                ..Default::default()
            };
            let transform = workspace_frame(&mut app, &context, vec![]);
            assert_eq!(context.zoom_factor(), 1.);
            assert_eq!(transform.scaling, zoom);
            let expected = (app.document.grid * 6., app.document.grid * 7.);
            let point = transform * app.page_geometry().to_paper(expected.0, expected.1);
            workspace_frame(
                &mut app,
                &context,
                vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
            workspace_frame(
                &mut app,
                &context,
                vec![egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            let (x, y) = app
                .insertion
                .unwrap_or_else(|| panic!("insertion missing at zoom {zoom}"));
            assert!(
                (x - expected.0).abs() < 0.001 && (y - expected.1).abs() < 0.001,
                "zoom={zoom} position={x},{y}"
            );
            assert!(app.document.regions.is_empty());
        }
    }
    #[test]
    fn typing_on_page_defining_evaluating_and_reopening_is_one_workflow() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp::default();
        assert!(app.document.regions.is_empty());
        app.insertion = Some((38.4, 38.4));
        frame(&mut app, &context, vec![egui::Event::Text("a:=2".into())]);
        frame(&mut app, &context, vec![enter()]);
        frame(&mut app, &context, vec![egui::Event::Text("a+3=".into())]);
        assert_eq!(app.document.regions.len(), 2);
        let id = app.document.regions[1].id;
        assert_eq!(
            app.results[&id].value.as_ref().unwrap().numeric_json()["real"],
            5.
        );
        assert!(app.document.regions[1].top > app.document.regions[0].top);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "mathcraft-workflow-{}-{stamp}.mathcraft",
            std::process::id()
        ));
        app.sync_editors().unwrap();
        app.document.save(&path).unwrap();
        let doc = CanvasDocument::load(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        let mut reopened = WorksheetApp {
            document: doc,
            ..Default::default()
        };
        reopened.recalculate();
        assert_eq!(
            reopened.results[&id].value.as_ref().unwrap().numeric_json()["real"],
            5.
        );
    }
    #[test]
    fn moving_definition_changes_calculation_order_and_undo_restores_it() {
        let mut app = WorksheetApp::default();
        for (y, text) in [(38.4, "a:=2"), (115.2, "a+3=")] {
            let id = app.insert((38.4, y), false);
            let mut editor = Editor::new();
            formula::input_text(&mut editor, text);
            app.document
                .regions
                .iter_mut()
                .find(|r| r.id == id)
                .unwrap()
                .content = CanvasContent::Math(editor.to_portable().unwrap());
        }
        app.recalculate();
        let eval = app.document.regions[1].id;
        let definition = app.document.regions[0].id;
        assert!(app.results[&eval].error.is_none());
        let before = app.snapshot();
        app.document.move_region(definition, 38.4, 230.4).unwrap();
        app.remember(before);
        app.recalculate();
        assert_eq!(
            app.results[&eval].error.as_ref().unwrap().code,
            "bad%_variable"
        );
        app.undo();
        assert_eq!(
            app.results[&eval].value.as_ref().unwrap().numeric_json()["real"],
            5.
        );
    }
    #[test]
    fn typed_unit_and_output_override_keep_physical_and_display_values() {
        let mut app = WorksheetApp::default();
        for (y, text) in [(38.4, "d:=2m"), (115.2, "d=cm")] {
            let id = app.insert((38.4, y), false);
            let mut editor = Editor::new();
            formula::input_text(&mut editor, text);
            app.document
                .regions
                .iter_mut()
                .find(|r| r.id == id)
                .unwrap()
                .content = CanvasContent::Math(editor.to_portable().unwrap());
        }
        app.recalculate();
        let id = app.document.regions[1].id;
        let row = &app.results[&id];
        assert_eq!(row.value.as_ref().unwrap().numeric_json()["real"], 2.);
        assert_eq!(
            row.display_value.as_ref().unwrap().numeric_json()["real"],
            200.
        );
        assert_eq!(value_text(row.value.as_ref().unwrap()), "2 m");
    }
    #[test]
    fn clicking_empty_paper_and_typing_keeps_first_character() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp::default();
        let point = pos2(100., 100.);
        frame(&mut app, &context, vec![]);
        frame(
            &mut app,
            &context,
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        frame(
            &mut app,
            &context,
            vec![egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(app.insertion.is_some());
        frame(&mut app, &context, vec![egui::Event::Text("7=".into())]);
        assert_eq!(app.document.regions.len(), 1);
        let id = app.document.regions[0].id;
        assert_eq!(
            app.results[&id].value.as_ref().unwrap().numeric_json()["real"],
            7.
        );
    }
    #[test]
    fn each_new_formula_character_can_arrive_in_consecutive_frames() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp {
            insertion: Some((38.4, 38.4)),
            ..Default::default()
        };
        for text in ["a", ":", "2"] {
            frame(&mut app, &context, vec![egui::Event::Text(text.into())]);
        }
        frame(&mut app, &context, vec![enter()]);
        for text in ["a", "+", "3", "="] {
            frame(&mut app, &context, vec![egui::Event::Text(text.into())]);
        }
        let id = app.document.regions[1].id;
        assert_eq!(
            app.results[&id].value.as_ref().unwrap().numeric_json()["real"],
            5.
        );
    }
    #[test]
    fn new_formula_accepts_all_text_and_paste_events_in_order() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp {
            insertion: Some((38.4, 38.4)),
            ..Default::default()
        };
        frame(
            &mut app,
            &context,
            vec![
                egui::Event::Text("2".into()),
                egui::Event::Text("+3".into()),
                egui::Event::Paste("=".into()),
            ],
        );
        let id = app.document.regions[0].id;
        assert_eq!(
            app.results[&id].value.as_ref().unwrap().numeric_json()["real"],
            5.
        );
    }
    #[test]
    fn incomplete_redefinition_poison_is_local_and_does_not_show_stale_answer() {
        let mut app = WorksheetApp::default();
        let mut ids = Vec::new();
        for (i, text) in ["a:=2", "a:=3", "a=", "2+3="].iter().enumerate() {
            let id = app.insert((38.4, 38.4 + i as f64 * 96.), false);
            let mut editor = Editor::new();
            formula::input_text(&mut editor, text);
            let tree = editor.to_portable().unwrap();
            app.document
                .regions
                .iter_mut()
                .find(|r| r.id == id)
                .unwrap()
                .content = CanvasContent::Math(tree);
            app.editors.insert(id, editor);
            ids.push(id);
        }
        app.recalculate();
        assert_eq!(
            app.results[&ids[2]].value.as_ref().unwrap().numeric_json()["real"],
            3.
        );
        let editor = app.editors.get_mut(&ids[1]).unwrap();
        let rhs = editor.arena().get(editor.root()).unwrap().children()[1];
        editor.set_atom(rhs, NodeKind::Number(".".into())).unwrap();
        app.recalculate();
        assert!(app.results[&ids[1]].value.is_none());
        assert!(app.results[&ids[1]].error.is_some());
        assert!(app.results[&ids[2]].value.is_none());
        assert!(app.results[&ids[2]].error.is_some());
        assert_eq!(
            app.results[&ids[3]].value.as_ref().unwrap().numeric_json()["real"],
            5.
        );
        assert!(app.sync_editors().is_err());
    }
    #[test]
    fn region_and_formula_clipboard_keep_structure_and_use_new_ids() {
        let mut app = WorksheetApp::default();
        let mut editor = Editor::new();
        formula::input_text(&mut editor, "2+3=");
        let tree = editor.to_portable().unwrap();
        let text = format!("mathcraft-tree:{}", serde_json::to_string(&tree).unwrap());
        assert!(app.paste_region(&text));
        let first = app.document.regions[0].id;
        app.duplicate_selected();
        let second = app.document.regions[1].id;
        assert_ne!(first, second);
        app.recalculate();
        assert_eq!(
            app.results[&first].value.as_ref().unwrap().numeric_json()["real"],
            5.
        );
        assert_eq!(
            app.results[&second].value.as_ref().unwrap().numeric_json()["real"],
            5.
        );
        app.remove_selected();
        assert_eq!(app.document.regions.len(), 1);
        app.undo();
        assert_eq!(app.document.regions.len(), 2);
    }
    #[test]
    fn ribbon_unit_picker_keeps_unit_namespace_after_same_name_variable_definition() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp::default();
        frame(&mut app, &context, vec![egui::Event::Text("m:=5".into())]);
        frame(&mut app, &context, vec![enter()]);
        frame(&mut app, &context, vec![egui::Event::Text("2".into())]);
        let id = app.selected.unwrap();
        app.ribbon_command(
            &context,
            RibbonAction::InsertIdentifier {
                name: "m",
                label: "UNIT",
            },
        );
        app.ribbon_command(&context, RibbonAction::Evaluate);
        let quantity = app.results[&id].value.as_ref().unwrap();
        assert_eq!(quantity.numeric_json()["real"], 2.);
        assert_eq!(value_text(quantity), "2 m");
        assert!(app.editors[&id].arena().nodes().any(|(_, n)| matches!(n.kind(), NodeKind::AnnotatedIdentifier {name, labels} if name == "m" && labels == "UNIT")));
    }

    #[test]
    fn ribbon_identifier_on_blank_page_is_one_document_command() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut app = WorksheetApp::default();
        app.ribbon_command(
            &context,
            RibbonAction::InsertIdentifier {
                name: "c",
                label: "CONSTANT",
            },
        );
        assert_eq!(app.document.regions.len(), 1);
        assert_eq!(app.undo.len(), 1);
        app.ribbon_command(&context, RibbonAction::Evaluate);
        let id = app.selected.unwrap();
        assert_eq!(
            app.results[&id].value.as_ref().unwrap().numeric_json()["real"],
            299792458.
        );
        app.undo();
        app.undo();
        assert!(app.document.regions.is_empty());
    }
}
