//! A worksheet surface: positioned structural formulas and text share one document.
//! The initial math/cursor subset remains explicitly narrower than Prime.
use crate::{formula, worksheet_math};
use eframe::egui::{self, Color32, FontId, Rect, Sense, Stroke, UiBuilder, pos2, vec2};
use mathcraft_compat::{RegionEvaluation, Value};
use mathcraft_document::{
    canvas::{CanvasContent, CanvasDocument, CanvasRegion},
    native::NativePackage,
};
use mathcraft_editor::{BinaryOp, Editor, NodeKind};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

const BLUE: Color32 = Color32::from_rgb(25, 88, 170);
const RED: Color32 = Color32::from_rgb(175, 40, 45);

enum Pending {
    New,
    Open(PathBuf),
    Close,
}
pub struct WorksheetApp {
    document: CanvasDocument,
    editors: HashMap<u64, Editor>,
    results: HashMap<u64, RegionEvaluation>,
    calculation_error: Option<String>,
    path: Option<PathBuf>,
    status: String,
    dirty: bool,
    selected: Option<u64>,
    insertion: Option<(f64, f64)>,
    text_mode: bool,
    focus_new: Option<u64>,
    undo: Vec<CanvasDocument>,
    redo: Vec<CanvasDocument>,
    dragging: Option<(u64, f64, f64)>,
    pending: Option<Pending>,
    closing: bool,
    pub smoke: bool,
    pub capture: Option<PathBuf>,
    frames: usize,
    capture_sent: bool,
}
impl Default for WorksheetApp {
    fn default() -> Self {
        Self {
            document: CanvasDocument::default(),
            editors: HashMap::new(),
            results: HashMap::new(),
            calculation_error: None,
            path: None,
            status: "Click the page and type. : define · = evaluate · / fraction · ^ power".into(),
            dirty: false,
            selected: None,
            insertion: None,
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
        }
    }
}
impl WorksheetApp {
    fn snapshot(&self) -> CanvasDocument {
        CanvasDocument {
            version: self.document.version,
            title: self.document.title.clone(),
            width: self.document.width,
            height: self.document.height,
            grid: self.document.grid,
            next_id: self.document.next_id,
            regions: self.document.regions.clone(),
            native_archive: None,
        }
    }
    fn remember(&mut self, before: CanvasDocument) {
        if self.undo.len() >= 50 {
            self.undo.remove(0);
        }
        self.undo.push(before);
        self.redo.clear();
        self.dirty = true;
    }
    fn restore(&mut self, mut doc: CanvasDocument) {
        doc.native_archive = self.document.native_archive.take();
        self.document = doc;
        self.editors.clear();
        self.selected = None;
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
                *self = Self::default();
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
                self.document = document;
                self.editors.clear();
                self.undo.clear();
                self.redo.clear();
                self.results.clear();
                self.dirty = false;
                self.selected = None;
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
        if let Err(e) = self.sync_editors() {
            self.status = format!("Could not save: {e}");
            return false;
        }
        let path = if save_as || self.path.is_none() {
            rfd::FileDialog::new()
                .add_filter("Mathcraft worksheet", &["mathcraft"])
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
        self.calculation_error = None;
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
                    match worksheet_math::tree_to_mathml(tree) {
                        Ok(xml) => math.push((region.id, region.left, region.top, xml)),
                        Err(e) => {
                            self.calculation_error = Some(format!("Region {}: {e}", region.id));
                            return;
                        }
                    }
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
        let result = worksheet_math::worksheet_xml(&math)
            .and_then(|xml| mathcraft_compat::evaluate_worksheet(&xml));
        match result {
            Ok(rows) => {
                self.results = rows
                    .into_iter()
                    .filter_map(|r| r.region_id.parse::<u64>().ok().map(|id| (id, r)))
                    .collect()
            }
            Err(e) => self.calculation_error = Some(e),
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
        self.insertion = None;
        id
    }
    fn page_ui(&mut self, ui: &mut egui::Ui) {
        let before = self.snapshot();
        let bottom = self
            .document
            .regions
            .iter()
            .map(|r| r.top as f32 + 180.)
            .fold(self.document.height as f32, f32::max);
        let pages = (bottom / self.document.height as f32).ceil().max(1.);
        let (paper, background) = ui.allocate_exact_size(
            vec2(
                self.document.width as f32,
                self.document.height as f32 * pages,
            ),
            Sense::click(),
        );
        ui.painter().rect_filled(paper, 0., Color32::WHITE);
        let grid = (self.document.grid as f32).max(6.);
        let visible = paper.intersect(ui.clip_rect());
        let line = Stroke::new(0.5, Color32::from_rgb(230, 235, 240));
        let mut x = paper.left() + ((visible.left() - paper.left()) / grid).ceil() * grid;
        while x < visible.right() {
            ui.painter()
                .line_segment([pos2(x, paper.top()), pos2(x, paper.bottom())], line);
            x += grid;
        }
        let mut y = paper.top() + ((visible.top() - paper.top()) / grid).ceil() * grid;
        while y < visible.bottom() {
            ui.painter()
                .line_segment([pos2(paper.left(), y), pos2(paper.right(), y)], line);
            y += grid;
        }
        let first_page = ((visible.top() - paper.top()) / self.document.height as f32)
            .floor()
            .max(1.) as usize;
        let last_page = ((visible.bottom() - paper.top()) / self.document.height as f32)
            .ceil()
            .min(pages) as usize;
        for n in first_page..last_page {
            let y = paper.top() + n as f32 * self.document.height as f32;
            ui.painter().line_segment(
                [pos2(paper.left(), y), pos2(paper.right(), y)],
                Stroke::new(2., Color32::from_gray(180)),
            );
        }
        if self.document.regions.is_empty() {
            ui.painter().text(
                paper.min + vec2(55., 50.),
                egui::Align2::LEFT_TOP,
                "Click anywhere to begin",
                FontId::proportional(17.),
                Color32::from_gray(145),
            );
        }
        let mut changed = false;
        let mut geometry_changed = false;
        let mut movement_started = false;
        let mut hit_region = false;
        for region in &mut self.document.regions {
            let origin = paper.min + vec2(region.left as f32, region.top as f32);
            let max = Rect::from_min_size(
                origin,
                vec2((paper.right() - origin.x - 15.).max(100.), 220.),
            );
            let id = region.id;
            let mut child = ui.new_child(
                UiBuilder::new()
                    .id_salt(("region", id))
                    .max_rect(max)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            child.set_clip_rect(paper.intersect(ui.clip_rect()));
            let row_result = self.results.get(&id);
            let display = row_result
                .and_then(|r| r.display_value.as_ref().or(r.value.as_ref()))
                .map(value_text);
            let response = match &mut region.content {
                CanvasContent::Math(tree) => {
                    let editor = self
                        .editors
                        .entry(id)
                        .or_insert_with(|| Editor::from_portable(tree).unwrap_or_default());
                    let response = if self.focus_new == Some(id) {
                        formula::formula_ui_with_focus(&mut child, editor, display.as_deref(), true)
                    } else {
                        formula::formula_ui_with_result(&mut child, editor, display.as_deref())
                    };
                    if response.changed() {
                        match editor.to_portable() {
                            Ok(value) => *tree = value,
                            Err(e) => self.status = format!("Incomplete formula: {e:?}"),
                        }
                        changed = true;
                    }
                    response
                }
                CanvasContent::Text(text) => {
                    let text_id = egui::Id::new(("worksheet-text", id));
                    if self.focus_new == Some(id) {
                        child.memory_mut(|m| m.request_focus(text_id));
                    }
                    let response = child.add(
                        egui::TextEdit::multiline(text)
                            .id(text_id)
                            .frame(egui::Frame::NONE)
                            .desired_rows(1)
                            .desired_width(340.)
                            .char_limit(65536),
                    );
                    changed |= response.changed();
                    response
                }
                CanvasContent::Opaque { kind, .. } => child.add(
                    egui::Label::new(format!("[{kind} — not yet editable]")).sense(Sense::click()),
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
            hit_region |= response.hovered();
            if self.selected == Some(id) || response.hovered() {
                let handle = Rect::from_min_size(
                    origin - vec2(14., 0.),
                    vec2(12., response.rect.height().max(20.)),
                );
                child
                    .painter()
                    .rect_filled(handle, 1., BLUE.gamma_multiply(0.25));
                let drag =
                    ui.interact(handle, egui::Id::new(("move", id)), Sense::click_and_drag());
                hit_region |= drag.hovered();
                if drag.clicked() {
                    self.selected = Some(id);
                    response.surrender_focus();
                }
                if drag.drag_started() {
                    self.dragging = Some((id, region.left, region.top));
                    movement_started = true;
                }
                if drag.dragged()
                    && let Some((moving, start_x, start_y)) = self.dragging
                    && moving == id
                    && let Some(delta) = drag.total_drag_delta()
                {
                    region.left = (start_x + delta.x as f64).max(0.);
                    region.top = (start_y + delta.y as f64).max(0.);
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
                child.painter().text(
                    response.rect.left_bottom() + vec2(12., 3.),
                    egui::Align2::LEFT_TOP,
                    &error.detail,
                    FontId::proportional(12.),
                    RED,
                );
            }
        }
        if changed || movement_started {
            self.remember(before);
        }
        if changed || geometry_changed {
            self.dirty = true;
            self.recalculate();
            ui.ctx().request_repaint();
        }
        if let Some((x, y)) = self.insertion {
            let p = paper.min + vec2(x as f32, y as f32);
            ui.painter()
                .line_segment([p - vec2(6., 0.), p + vec2(6., 0.)], Stroke::new(1., BLUE));
            ui.painter()
                .line_segment([p - vec2(0., 6.), p + vec2(0., 6.)], Stroke::new(1., BLUE));
        }
        if background.clicked()
            && !hit_region
            && let Some(pointer) = background.interact_pointer_pos()
        {
            let p = pointer - paper.min;
            let snap = |v: f64| {
                let r = v % self.document.grid;
                if r > self.document.grid / 2. {
                    v + self.document.grid - r
                } else {
                    v - r
                }
            };
            self.insertion = Some((snap(p.x as f64), snap(p.y as f64)));
            self.selected = None;
            if self.text_mode {
                self.insert(self.insertion.unwrap(), true);
            }
        }
        // Enter commits the current formula and advances to a new insertion point.
        if let Some(id) = self.selected
            && let Some(region) = self.document.regions.iter().find(|r| r.id == id)
            && matches!(region.content, CanvasContent::Math(_))
            && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter))
        {
            self.insertion = Some((region.left, region.top + self.document.grid * 4.));
            self.selected = None;
            ui.memory_mut(|m| {
                if let Some(focused) = m.focused() {
                    m.surrender_focus(focused)
                }
            });
        }
        if let Some(position) = self.insertion {
            let text: String = ui.input(|i| {
                i.events
                    .iter()
                    .filter_map(|e| match e {
                        egui::Event::Text(s) | egui::Event::Paste(s) => Some(s.as_str()),
                        _ => None,
                    })
                    .collect()
            });
            if !text.is_empty() {
                let id = self.insert(position, false);
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

fn value_text(value: &Value) -> String {
    match value {
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
    let package = NativePackage::open(path)?;
    let source = package.worksheet_xml()?;
    let xml = roxmltree::Document::parse(&source).map_err(|e| e.to_string())?;
    let mut document = CanvasDocument {
        title: path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        native_archive: Some(std::fs::read(path).map_err(|e| e.to_string())?),
        ..Default::default()
    };
    if let Some(presentation) = package
        .original_part("mathcad/settings/presentation.xml")
        .and_then(|b| std::str::from_utf8(b).ok())
        && let Ok(grid) = mathcraft_compat::PlacementGrid::from_presentation(
            presentation.trim_start_matches('\u{feff}'),
        )
    {
        document.grid = grid.spacing();
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
        if ctx.input(|i| i.viewport().close_requested()) && self.dirty && !self.closing {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::Close);
        }
        if self.closing {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
            self.save(false);
        }
        // Document history owns Ctrl+Z/Y so movement, text and math are one undo stream.
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z)) {
            self.undo();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Y)) {
            self.redo();
        }
        egui::Panel::top("worksheet-toolbar")
            .exact_size(48.)
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(
                        egui::RichText::new("Mathcraft")
                            .strong()
                            .size(19.)
                            .color(BLUE),
                    );
                    ui.separator();
                    if ui.button("New").clicked() {
                        self.request(Pending::New);
                    }
                    if ui.button("Open").clicked()
                        && let Some(path) = rfd::FileDialog::new()
                            .add_filter("Worksheets", &["mathcraft", "mcdx"])
                            .pick_file()
                    {
                        self.request(Pending::Open(path));
                    }
                    if ui.button("Save").clicked() {
                        self.save(false);
                    }
                    if ui.button("Save as").clicked() {
                        self.save(true);
                    }
                    ui.separator();
                    ui.selectable_value(&mut self.text_mode, false, "Math");
                    ui.selectable_value(&mut self.text_mode, true, "Text");
                    if ui.button("Recalculate").clicked() {
                        self.recalculate();
                    }
                    if ui
                        .add_enabled(self.selected.is_some(), egui::Button::new("Delete region"))
                        .clicked()
                        && let Some(id) = self.selected.take()
                    {
                        let before = self.snapshot();
                        self.document.regions.retain(|r| r.id != id);
                        self.editors.remove(&id);
                        self.remember(before);
                        self.recalculate();
                    }
                });
            });
        egui::Panel::bottom("worksheet-status")
            .exact_size(28.)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.small(format!(
                        "{} regions{}",
                        self.document.regions.len(),
                        if self.dirty { " · unsaved" } else { "" }
                    ));
                    ui.separator();
                    if let Some(error) = &self.calculation_error {
                        ui.colored_label(RED, error);
                    } else {
                        ui.small(&self.status);
                    }
                });
            });
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::both()
                .id_salt("free-worksheet")
                .show(ui, |ui| {
                    ui.add_space(12.);
                    self.page_ui(ui);
                    ui.add_space(24.);
                });
        });
        if self.pending.is_some() {
            egui::Window::new("Unsaved worksheet")
                .collapsible(false)
                .resizable(false)
                .show(&ctx, |ui| {
                    ui.label("Save this worksheet before continuing?");
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked()
                            && self.save(false)
                            && let Some(action) = self.pending.take()
                        {
                            self.apply(action);
                        }
                        if ui.button("Discard").clicked()
                            && let Some(action) = self.pending.take()
                        {
                            self.apply(action);
                        }
                        if ui.button("Cancel").clicked() {
                            self.pending = None;
                        }
                    });
                });
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
    #[test]
    fn typing_on_page_defining_evaluating_and_reopening_is_one_workflow() {
        let context = egui::Context::default();
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
}
