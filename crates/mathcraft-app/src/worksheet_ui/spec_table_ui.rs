//! Specification-table drafts share the structural math editor. A cell is
//! serialized and recalculated only at an explicit commit or focus boundary.
use super::*;
use mathcraft_document::spec_table::{CellSlot, SpecTable, SpecTableClipboard};

pub(super) struct TableEditor {
    source: String,
    table: SpecTable,
    active: Option<CellEditor>,
    selection: Option<CellRange>,
    drag_start: Option<(usize, usize)>,
    error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CellRange {
    start: (usize, usize), // column, display row
    end: (usize, usize),
}

impl CellRange {
    fn bounds(self) -> (std::ops::Range<usize>, std::ops::Range<usize>) {
        (
            self.start.1.min(self.end.1)..self.start.1.max(self.end.1) + 1,
            self.start.0.min(self.end.0)..self.start.0.max(self.end.0) + 1,
        )
    }
}

struct CellEditor {
    column: usize,
    row: usize,
    editor: Editor,
    focus: bool,
    changed: bool,
    widget: Option<egui::Id>,
}

pub(super) struct TableResponse {
    pub response: egui::Response,
    pub committed: bool,
    pub draft_changed: bool,
    pub focused: bool,
}

fn slot(row: usize) -> CellSlot {
    match row {
        0 => CellSlot::Header,
        1 => CellSlot::Unit,
        row => CellSlot::Body(row - 2),
    }
}

/// The native indeterminate insertion chooses the preceding row/column only
/// at the entire cell's left boundary, including a nested first operand.
fn at_cell_start(editor: &Editor) -> bool {
    let caret = editor.caret();
    if editor.selection() == mathcraft_editor::Selection::Subtree(editor.root()) {
        return caret.caret_side == Direction::Left;
    }
    if editor.selection() != mathcraft_editor::Selection::None {
        return false;
    }
    let mut first = editor.root();
    while let Some(child) = editor.arena().get(first).and_then(|n| n.children().first()) {
        first = *child;
    }
    caret.node == first
        && caret
            .value_range
            .map_or(caret.caret_side == Direction::Left, |range| {
                range.anchor == 0 && range.focus == 0
            })
}

pub(super) fn native_saved_row(
    region: &CanvasRegion,
    original: &str,
    caches: &[mathcraft_document::native::NativeCache],
) -> Option<SavedCalculationRow> {
    let CanvasContent::Opaque { xml, kind } = &region.content else {
        return None;
    };
    if kind != "spec-table" || xml != original {
        return None;
    }
    let table = SpecTable::parse(xml).ok()?;
    let columns = table
        .columns()
        .iter()
        .map(|column| {
            let reference = column.result_ref.as_deref()?;
            caches
                .iter()
                .find(|cache| cache.matches_result_ref(reference))
        })
        .collect::<Option<Vec<_>>>()?;
    let synchronized = columns
        .iter()
        .all(|cache| cache.calculation_status.as_deref() == Some("Synchronized"));
    let error = columns
        .iter()
        .flat_map(|cache| &cache.errors)
        .next()
        .map(|error| {
            error
                .resource_string
                .clone()
                .or_else(|| error.code.clone())
                .unwrap_or_else(|| "저장된 표 계산 오류".into())
        });
    Some(SavedCalculationRow {
        region_id: region.id,
        status: if synchronized {
            SavedCalculationStatus::Synchronized
        } else {
            SavedCalculationStatus::Pending
        },
        display: None,
        error,
        warnings: columns
            .iter()
            .flat_map(|cache| &cache.warnings)
            .map(|_| "저장된 재정의 경고".into())
            .collect(),
        symbolic_mathml: None,
        numeric_source: None,
    })
}

fn clipboard_text(payload: SpecTableClipboard) -> Result<String, String> {
    match payload {
        SpecTableClipboard::Matrix(tree) => serde_json::to_string(&tree)
            .map(|json| format!("mathcraft-tree:{json}"))
            .map_err(|e| e.to_string()),
        SpecTableClipboard::TableXml(xml) => Ok(format!("mathcraft-table:{xml}")),
    }
}

impl TableEditor {
    pub(super) fn new(source: &str) -> Result<Self, String> {
        Ok(Self {
            source: source.into(),
            table: SpecTable::parse(source)?,
            active: None,
            selection: None,
            drag_start: None,
            error: None,
        })
    }

    fn activate(&mut self, column: usize, row: usize) -> Result<(), String> {
        let tree = self.table.cell(column, slot(row))?;
        let mut editor = Editor::from_portable(&tree).map_err(|e| format!("{e:?}"))?;
        editor.set_expressions_only(true);
        self.active = Some(CellEditor {
            column,
            row,
            editor,
            focus: true,
            changed: false,
            widget: None,
        });
        self.error = None;
        self.selection = None;
        Ok(())
    }

    pub(super) fn commit(&mut self, xml: &mut String) -> Result<bool, String> {
        let Some(active) = self.active.as_mut().filter(|cell| cell.changed) else {
            return Ok(false);
        };
        let tree = active.editor.to_portable().map_err(|e| format!("{e:?}"))?;
        self.table
            .set_cell(active.column, slot(active.row), &tree)?;
        let changed = self.table.xml() != xml;
        if changed {
            *xml = self.table.xml().into();
            self.source.clone_from(xml);
        }
        active.changed = false;
        self.error = None;
        Ok(changed)
    }

    pub(super) fn deactivate(&mut self) {
        self.active = None;
    }

    pub(super) fn has_active(&self) -> bool {
        self.active.is_some()
    }

    pub(super) fn has_draft_changes(&self) -> bool {
        self.active.as_ref().is_some_and(|cell| cell.changed)
    }

    pub(super) fn clear_range_selection(&mut self) {
        self.selection = None;
        self.drag_start = None;
    }

    pub(super) fn active_editor(&self) -> Option<&Editor> {
        self.active.as_ref().map(|cell| &cell.editor)
    }

    fn apply_command(&mut self, command: impl FnOnce(&mut Editor)) -> bool {
        let Some(active) = &mut self.active else {
            return false;
        };
        let revision = active.editor.arena().revision();
        active.editor.edit_group(command);
        let changed = active.editor.arena().revision() != revision;
        active.changed |= changed;
        active.focus = true;
        changed
    }

    pub(super) fn history(&mut self, redo: bool) -> bool {
        let Some(active) = &mut self.active else {
            return false;
        };
        // Draft undo never rewinds another worksheet region.
        let changed = if redo {
            active.editor.redo()
        } else {
            active.editor.undo()
        };
        if !changed {
            return false;
        }
        active.changed = match (
            active.editor.to_portable(),
            self.table.cell(active.column, slot(active.row)),
        ) {
            (Ok(draft), Ok(committed)) => draft != committed,
            _ => true,
        };
        active.focus = true;
        true
    }

    fn structural_command(
        &mut self,
        action: RibbonAction,
        xml: &mut String,
    ) -> Result<bool, String> {
        if action == RibbonAction::ClearCells {
            if let Some(selection) = self.selection {
                let (rows, columns) = selection.bounds();
                let changed = self.table.clear_selection(rows, columns)?;
                if changed {
                    *xml = self.table.xml().into();
                    self.source.clone_from(xml);
                }
                self.activate(selection.start.0, selection.start.1)?;
                return Ok(changed);
            }
            if self.active.is_none() {
                self.activate(0, 0)?;
            }
            self.apply_command(|editor| {
                let _ = editor.select_subtree(editor.root());
                let _ = editor.replace_selection(&PortableTree {
                    kind: NodeKind::Placeholder,
                    children: vec![],
                });
            });
            let changed = self.commit(xml)?;
            if changed {
                let active = self.active.as_ref().unwrap();
                self.activate(active.column, active.row)?;
            }
            return Ok(changed);
        }
        if self.active.is_none() {
            self.activate(0, 0)?;
        }
        let active = self.active.as_ref().unwrap();
        let (column, row) = (active.column, active.row);
        let before = at_cell_start(&active.editor);
        let original = xml.clone();
        self.commit(xml)?;
        let mut target = (column, row);
        match action {
            RibbonAction::InsertMatrixRow if row >= 2 => {
                let index = row - 2 + usize::from(!before);
                self.table.insert_row(index)?;
                target.1 = index + 2;
            }
            RibbonAction::InsertMatrixColumn => {
                let index = column + usize::from(!before);
                self.table.insert_column(index)?;
                target.0 = index;
            }
            RibbonAction::DeleteMatrixRow if row >= 2 => {
                self.table.delete_row(row - 2)?;
                target.1 = row.min(self.table.row_count() + 1);
            }
            RibbonAction::DeleteMatrixColumn => {
                self.table.delete_column(column)?;
                target.0 = column.min(self.table.col_count() - 1);
            }
            _ => {}
        }
        *xml = self.table.xml().into();
        self.source.clone_from(xml);
        self.activate(target.0, target.1)?;
        Ok(*xml != original)
    }

    fn remove_selection(&mut self, xml: &mut String) -> Result<bool, String> {
        let Some(selection) = self.selection else {
            return Ok(false);
        };
        let (rows, columns) = selection.bounds();
        let changed = self.table.remove_or_clear_selection(rows, columns)?;
        if changed {
            *xml = self.table.xml().into();
            self.source.clone_from(xml);
        }
        let column = selection.start.0.min(self.table.col_count() - 1);
        let row = selection.start.1.min(self.table.row_count() + 1);
        self.activate(column, row)?;
        Ok(changed)
    }

    pub(super) fn show(
        &mut self,
        ui: &mut egui::Ui,
        xml: &mut String,
        visual: &mathcraft_document::native_visual::RegionVisual,
        width: f32,
        focus: bool,
        font_points: f32,
    ) -> TableResponse {
        if *xml != self.source {
            match Self::new(xml) {
                Ok(state) => *self = state,
                Err(error) => self.error = Some(error),
            }
        }
        if focus {
            if let Some(active) = &mut self.active {
                active.focus = true;
            } else if self.selection.is_none()
                && let Err(error) = self.activate(0, 0)
            {
                self.error = Some(error);
            }
        }
        let editing = self.active.as_ref().map(|cell| (cell.row, cell.column));
        let (drawn, cells) = crate::native_presentation::draw_table(ui, visual, width, editing);
        let mut response = ui.interact(drawn.rect, ui.id().with("table"), Sense::click());
        let mut clicked = None;
        for cell in &cells {
            let hit = ui.interact(
                cell.rect,
                ui.id().with(("cell", cell.column, cell.row)),
                Sense::click(),
            );
            if hit.clicked() {
                clicked = Some((cell.column, cell.row));
            }
            response = response.union(hit);
        }
        let mut committed = false;
        let from_global = ui.ctx().layer_transform_from_global(ui.layer_id());
        let pointer_cell = ui.input(|i| i.pointer.interact_pos()).and_then(|pos| {
            let pos = from_global.map_or(pos, |transform| transform * pos);
            cells
                .iter()
                .find(|cell| cell.rect.contains(pos))
                .map(|cell| (cell.column, cell.row))
        });
        if ui.input(|i| i.pointer.primary_pressed()) {
            self.drag_start = pointer_cell;
            if pointer_cell.is_some() {
                self.selection = None;
            }
        }
        if ui.input(|i| i.pointer.primary_down())
            && let (Some(start), Some(end)) = (self.drag_start, pointer_cell)
            && (start != end || self.selection.is_some())
        {
            match self.commit(xml) {
                Ok(changed) => {
                    committed |= changed;
                    self.deactivate();
                    self.selection = Some(CellRange { start, end });
                }
                Err(error) => self.error = Some(error),
            }
        }
        if ui.input(|i| i.pointer.primary_released()) {
            self.drag_start = None;
            if self.selection.is_some() {
                clicked = None;
            }
        }
        if let Some(selection) = self.selection {
            let (rows, columns) = selection.bounds();
            for cell in &cells {
                if rows.contains(&cell.row) && columns.contains(&cell.column) {
                    ui.painter()
                        .rect_filled(cell.rect, 0., BLUE.gamma_multiply(0.14));
                }
            }
            let (copy, cut) = ui.input_mut(|i| {
                let copy = i
                    .events
                    .iter()
                    .any(|event| matches!(event, egui::Event::Copy));
                let cut = i
                    .events
                    .iter()
                    .any(|event| matches!(event, egui::Event::Cut));
                if copy || cut {
                    i.events
                        .retain(|event| !matches!(event, egui::Event::Copy | egui::Event::Cut));
                }
                (copy, cut)
            });
            if copy || cut {
                match self
                    .table
                    .copy_selection(rows, columns)
                    .and_then(clipboard_text)
                {
                    Ok(text) => {
                        ui.ctx().copy_text(text);
                        if cut {
                            match self.remove_selection(xml) {
                                Ok(changed) => committed |= changed,
                                Err(error) => self.error = Some(error),
                            }
                        }
                    }
                    Err(error) => self.error = Some(error),
                }
            }
            if ui.input_mut(|i| {
                consume_exact_key(i, egui::Modifiers::NONE, egui::Key::Delete)
                    || consume_exact_key(i, egui::Modifiers::NONE, egui::Key::Backspace)
            }) {
                match self.remove_selection(xml) {
                    Ok(changed) => committed |= changed,
                    Err(error) => self.error = Some(error),
                }
            }
        }
        if let Some((column, row)) = clicked
            && self
                .active
                .as_ref()
                .is_none_or(|c| c.column != column || c.row != row)
        {
            match self.commit(xml) {
                Ok(changed) => {
                    committed |= changed;
                    if let Err(error) = self.activate(column, row) {
                        self.error = Some(error);
                    }
                }
                Err(error) => self.error = Some(error),
            }
        }
        let mut draft_changed = false;
        let mut focused = self.selection.is_some();
        let mut tab = None;
        let mut arrow = None;
        let mut structure = None;
        if self.active.as_ref().is_some_and(|cell| {
            cell.focus || cell.widget.is_some_and(|id| ui.memory(|m| m.has_focus(id)))
        }) {
            structure = ui.input_mut(|i| {
                if consume_exact_key(i, egui::Modifiers::SHIFT, egui::Key::Enter) {
                    Some(RibbonAction::InsertMatrixRow)
                } else if consume_exact_key(i, egui::Modifiers::SHIFT, egui::Key::Space) {
                    Some(RibbonAction::InsertMatrixColumn)
                } else {
                    None
                }
            });
            tab = ui.input_mut(|i| {
                if consume_exact_key(i, egui::Modifiers::SHIFT, egui::Key::Tab) {
                    Some(false)
                } else if consume_exact_key(i, egui::Modifiers::NONE, egui::Key::Tab) {
                    Some(true)
                } else {
                    None
                }
            });
            let active = self.active.as_ref().unwrap();
            let caret = active.editor.caret();
            if caret.node == active.editor.root()
                && let Some(node) = active.editor.arena().get(caret.node)
                && node.children().is_empty()
            {
                let length = active
                    .editor
                    .editing_text(caret.node)
                    .map_or(0, |text| text.encode_utf16().count());
                let left = caret
                    .value_range
                    .map_or(caret.caret_side == Direction::Left, |range| {
                        range.focus == 0
                    });
                let right = caret
                    .value_range
                    .map_or(caret.caret_side == Direction::Right, |range| {
                        range.focus == length
                    });
                let columns = self.table.col_count() as isize;
                arrow = ui.input_mut(|i| {
                    for (key, allowed, step) in [
                        (egui::Key::ArrowLeft, left, -1),
                        (egui::Key::ArrowRight, right, 1),
                        (egui::Key::ArrowUp, true, -columns),
                        (egui::Key::ArrowDown, true, columns),
                    ] {
                        if allowed && consume_exact_key(i, egui::Modifiers::NONE, key) {
                            return Some(step);
                        }
                    }
                    None
                });
            }
        }
        if let Some(action) = structure {
            match self.structural_command(action, xml) {
                Ok(changed) => committed |= changed,
                Err(error) => self.error = Some(error),
            }
        }
        if let Some(active) = &mut self.active
            && let Some(cell) = cells
                .iter()
                .find(|c| c.row == active.row && c.column == active.column)
        {
            ui.painter().rect_stroke(
                cell.rect.shrink(0.5),
                0.,
                Stroke::new(0.7, BLUE),
                egui::StrokeKind::Inside,
            );
            let mut child = ui.new_child(
                UiBuilder::new()
                    .id_salt(("table-editor", active.column, active.row))
                    .max_rect(Rect::from_min_size(
                        cell.content_origin.to_pos2(),
                        vec2(cell.rect.width().max(100.), 1000.),
                    ))
                    .layout(egui::Layout::left_to_right(egui::Align::Min)),
            );
            child.set_clip_rect(ui.clip_rect());
            let outcome = formula::formula_ui_styled(
                &mut child,
                &mut active.editor,
                None,
                active.focus,
                None,
                font_points,
            );
            active.focus = false;
            draft_changed = outcome.response.changed();
            active.changed |= draft_changed;
            focused = outcome.response.has_focus();
            active.widget = Some(outcome.response.id);
            response = response.union(outcome.response);
        }
        let leave = focused
            && ui.input_mut(|i| {
                i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                    || i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
            });
        if tab.is_some() || arrow.is_some() || leave {
            match self.commit(xml) {
                Ok(changed) => {
                    committed |= changed;
                    if tab.is_some() || arrow.is_some() {
                        let moved = if let Some(forward) = tab {
                            self.advance(forward, xml)
                        } else {
                            self.move_cell(arrow.unwrap(), false, xml)
                        };
                        if let Err(error) = moved {
                            self.error = Some(error);
                        }
                        // Advancing past the final cell serializes an empty row.
                        committed |= *xml != self.source;
                        self.source.clone_from(xml);
                        focused = true;
                    } else {
                        if let Some(id) = self.active.as_ref().and_then(|cell| cell.widget) {
                            ui.memory_mut(|m| m.surrender_focus(id));
                        }
                        self.deactivate();
                        focused = false;
                    }
                }
                Err(error) => self.error = Some(error),
            }
        }
        if let Some(error) = &self.error {
            response = response.on_hover_text(error);
        }
        if committed || draft_changed || clicked.is_some() {
            ui.ctx().request_repaint();
        }
        TableResponse {
            response,
            committed,
            draft_changed,
            focused,
        }
    }

    fn advance(&mut self, forward: bool, xml: &mut String) -> Result<(), String> {
        self.move_cell(if forward { 1 } else { -1 }, forward, xml)
    }

    fn move_cell(&mut self, step: isize, append: bool, xml: &mut String) -> Result<(), String> {
        let Some(active) = &self.active else {
            return Ok(());
        };
        let count = self.table.col_count();
        let index = active.row * count + active.column;
        let Some(next) = index.checked_add_signed(step) else {
            return Ok(());
        };
        let (row, column) = (next / count, next % count);
        if row == self.table.row_count() + 2 {
            if !append {
                return Ok(());
            }
            self.table.insert_row(self.table.row_count())?;
            *xml = self.table.xml().into();
        }
        if row > self.table.row_count() + 1 {
            return Ok(());
        }
        self.activate(column, row)
    }
}

impl WorksheetApp {
    pub(super) fn worksheet_math_defaults(&self) -> MathFormat {
        self.document
            .native_archive
            .as_ref()
            .and_then(|bytes| NativePackage::from_bytes(bytes).ok())
            .and_then(|package| {
                mathcraft_document::formatting::read_worksheet_math_format_defaults(&package).ok()
            })
            .unwrap_or_default()
    }

    pub(super) fn insert_spec_table(&mut self) {
        let table = match SpecTable::new(1, 1) {
            Ok(table) => table,
            Err(error) => {
                self.status = error;
                return;
            }
        };
        if let Err(error) = self.commit_table_edits(None) {
            self.status = error;
            return;
        }
        let before = self.snapshot();
        let position = self.insertion.unwrap_or_else(|| {
            self.selected
                .and_then(|id| self.document.regions.iter().find(|r| r.id == id))
                .map(|r| (r.left, r.top + self.document.grid * 5.))
                .unwrap_or((0., self.document.grid))
        });
        let id = self
            .document
            .add_opaque(position.0, position.1, table.xml(), "spec-table");
        let format = self.worksheet_math_defaults();
        if let Some(region) = self.document.regions.iter_mut().find(|r| r.id == id) {
            region.format = format;
        }
        let mut editor = TableEditor::new(table.xml()).expect("new table is valid");
        editor.activate(0, 0).expect("new table has a header");
        self.table_editors.insert(id, editor);
        self.refresh_table_visual(id);
        self.selected = Some(id);
        self.insertion = None;
        self.focus_new = Some(id);
        self.reveal = Some(position);
        self.remember(before);
        self.calculation_changed(false);
    }

    pub(super) fn apply_table_editor_command(&mut self, command: impl FnOnce(&mut Editor)) -> bool {
        let Some(id) = self.selected.filter(|id| {
            self.document
                .regions
                .iter()
                .any(|r| r.id == *id && is_spec_table(r))
        }) else {
            return false;
        };
        if let std::collections::hash_map::Entry::Vacant(entry) = self.table_editors.entry(id) {
            let CanvasContent::Opaque { xml, .. } = &self
                .document
                .regions
                .iter()
                .find(|r| r.id == id)
                .unwrap()
                .content
            else {
                unreachable!();
            };
            match TableEditor::new(xml) {
                Ok(editor) => {
                    entry.insert(editor);
                }
                Err(error) => {
                    self.status = error;
                    return true;
                }
            }
        }
        let table = self.table_editors.get_mut(&id).unwrap();
        if table.active.is_none()
            && let Err(error) = table.activate(0, 0)
        {
            self.status = error;
            return true;
        }
        self.dirty |= table.apply_command(command);
        self.editing_region = Some(id);
        self.focus_new = Some(id);
        true
    }

    pub(super) fn table_matrix_command(&mut self, action: RibbonAction) -> bool {
        let Some(index) = self.selected.and_then(|id| {
            self.document
                .regions
                .iter()
                .position(|r| r.id == id && is_spec_table(r))
        }) else {
            return false;
        };
        let id = self.document.regions[index].id;
        self.retain_committed_input();
        let before = self.snapshot_with_document(DocumentHistory::Region(Box::new(
            self.document.regions[index].clone(),
        )));
        let CanvasContent::Opaque { xml, .. } = &mut self.document.regions[index].content else {
            unreachable!();
        };
        let editor = match self.table_editors.entry(id) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => match TableEditor::new(xml) {
                Ok(editor) => entry.insert(editor),
                Err(error) => {
                    self.status = error;
                    return true;
                }
            },
        };
        // Undo/redo can restore the XML while the previous cell editor survives.
        if editor.source != *xml {
            match TableEditor::new(xml) {
                Ok(state) => *editor = state,
                Err(error) => {
                    self.status = error;
                    return true;
                }
            }
        }
        match editor.structural_command(action, xml) {
            Ok(true) => {
                self.remember(before);
                self.refresh_table_visual(id);
                self.stage_math_edit(id, false);
                self.commit_math_edits();
            }
            Ok(false) => {}
            Err(error) => self.status = error,
        }
        self.editing_region = Some(id);
        self.focus_new = Some(id);
        true
    }

    /// Commit cell drafts before Save, calculation commands and leaving a table.
    pub(super) fn commit_table_edits(&mut self, except: Option<u64>) -> Result<bool, String> {
        let ids: Vec<_> = self
            .table_editors
            .iter()
            .filter(|(id, editor)| Some(**id) != except && editor.has_active())
            .map(|(id, _)| *id)
            .collect();
        let mut changed = false;
        for id in ids {
            let Some(index) = self.document.regions.iter().position(|r| r.id == id) else {
                continue;
            };
            let before = self.snapshot_with_document(DocumentHistory::Region(Box::new(
                self.document.regions[index].clone(),
            )));
            let CanvasContent::Opaque { xml, .. } = &mut self.document.regions[index].content
            else {
                continue;
            };
            let table = self.table_editors.get_mut(&id).unwrap();
            let committed = table.commit(xml)?;
            table.deactivate();
            if committed {
                self.remember(before);
                self.refresh_table_visual(id);
                self.stage_math_edit(id, false);
                changed = true;
            }
        }
        Ok(changed)
    }

    pub(super) fn refresh_table_visual(&mut self, id: u64) {
        if let Some(region) = self.document.regions.iter().find(|r| r.id == id)
            && let Err(error) = Arc::make_mut(&mut self.native_visuals).update_spec_table(region)
        {
            self.status = error;
        }
        self.region_widths.remove(&id);
        self.region_heights.remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SOURCE: &str = r#"<region xmlns="http://schemas.mathsoft.com/worksheet50" xmlns:ml="http://schemas.mathsoft.com/math50" region-id="10" actualWidth="150" actualHeight="108"><spec-table><math resultRef="18"><ml:define><ml:id labels="VARIABLE">lengths</ml:id><ml:apply><ml:scale/><ml:matrix rows="2" cols="1"><ml:real>1.00</ml:real><ml:real>2</ml:real></ml:matrix><ml:id labels="UNIT">mm</ml:id></ml:apply></ml:define></math></spec-table></region>"#;

    fn app() -> (WorksheetApp, u64) {
        let mut app = WorksheetApp::default();
        let id = app.document.add_opaque(0., 100., SOURCE, "spec-table");
        app.document.regions[0].source_region = Some("10".into());
        app.refresh_table_visual(id);
        app.selected = Some(id);
        app.insertion = None;
        let mut editor = TableEditor::new(SOURCE).unwrap();
        editor.activate(0, 2).unwrap();
        app.table_editors.insert(id, editor);
        (app, id)
    }

    fn frame(app: &mut WorksheetApp, ctx: &egui::Context, events: Vec<egui::Event>) {
        let input = egui::RawInput {
            events,
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1200., 900.))),
            ..Default::default()
        };
        ctx.run_ui(input, |ui| app.page_ui(ui))
            .drop_without_applying_deltas();
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
    fn every_table_cell_rejects_definitions_without_mutating_or_committing() {
        for row in 0..=3 {
            let mut table = TableEditor::new(SOURCE).unwrap();
            table.activate(0, row).unwrap();
            let original = table.active_editor().unwrap().to_portable().unwrap();
            let definition = PortableTree {
                kind: NodeKind::Binary(BinaryOp::GlobalDefine),
                children: vec![original.clone(), original.clone()],
            };
            assert!(table.active_editor().unwrap().expressions_only());
            assert!(!table.apply_command(|editor| {
                editor.insert_definition();
            }));
            assert!(!table.apply_command(|editor| {
                editor.insert_global_definition();
            }));
            assert!(!table.apply_command(|editor| {
                editor.select_subtree(editor.root()).unwrap();
                let _ = editor.replace_selection(&definition);
            }));
            assert_eq!(
                table.active_editor().unwrap().to_portable().unwrap(),
                original
            );
            assert!(!table.has_draft_changes());
            assert!(!table.history(false));
            let mut xml = SOURCE.to_owned();
            assert!(!table.commit(&mut xml).unwrap());
            assert_eq!(xml, SOURCE);
            assert!(table.apply_command(|editor| {
                editor.select_subtree(editor.root()).unwrap();
                editor
                    .replace_selection(&PortableTree {
                        kind: NodeKind::Number("7".into()),
                        children: vec![],
                    })
                    .unwrap();
            }));
            assert!(table.history(false));
            assert!(table.active_editor().unwrap().expressions_only());
            assert_eq!(
                table.active_editor().unwrap().to_portable().unwrap(),
                original
            );
        }
    }

    #[test]
    fn rejected_table_definition_keys_do_not_dirty_the_document_or_recalculate() {
        for row in 0..=3 {
            let (mut app, id) = app();
            app.table_editors
                .get_mut(&id)
                .unwrap()
                .activate(0, row)
                .unwrap();
            let ctx = egui::Context::default();
            crate::configure(&ctx);
            frame(&mut app, &ctx, vec![]);
            let dirty = app.dirty;
            let original = app.table_editors[&id]
                .active_editor()
                .unwrap()
                .to_portable()
                .unwrap();
            frame(&mut app, &ctx, vec![egui::Event::Text(":".into())]);
            frame(
                &mut app,
                &ctx,
                vec![key(
                    egui::Key::Backtick,
                    egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
                )],
            );
            assert_eq!(
                app.table_editors[&id]
                    .active_editor()
                    .unwrap()
                    .to_portable()
                    .unwrap(),
                original
            );
            assert_eq!(app.dirty, dirty);
            assert!(!app.table_editors[&id].has_draft_changes());
            assert!(app.last_executed_regions.is_empty());
            assert!(app.undo.is_empty());
            assert_eq!(
                app.document.regions[0].content,
                CanvasContent::Opaque {
                    xml: SOURCE.into(),
                    kind: "spec-table".into()
                }
            );
        }
    }

    #[test]
    fn input_only_native_table_restores_column_status_and_rejects_changed_source() {
        use mathcraft_document::native::NativeCache;
        let (app, _) = app();
        let region = &app.document.regions[0];
        let mut caches = vec![NativeCache {
            part: "mathcad/result.xml".into(),
            result_id: Some("18".into()),
            calculation_status: Some("Synchronized".into()),
            tag: None,
            result_kind: None,
            result_mathml: None,
            attributes: vec![],
            errors: vec![],
            warnings: vec![],
            raw_xml: String::new(),
        }];
        let row = native_saved_row(region, SOURCE, &caches).unwrap();
        assert_eq!(row.status, SavedCalculationStatus::Synchronized);
        assert!(row.display.is_none());
        assert!(native_saved_row(region, &SOURCE.replace("1.00", "42"), &caches).is_none());
        assert!(native_saved_row(region, SOURCE, &[]).is_none());
        caches[0].calculation_status = Some("Pending".into());
        assert_eq!(
            native_saved_row(region, SOURCE, &caches).unwrap().status,
            SavedCalculationStatus::Pending
        );
    }

    #[test]
    fn cell_keystrokes_remain_drafts_until_enter_then_recalculate_and_undo() {
        let (mut app, id) = app();
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        frame(&mut app, &ctx, vec![]);
        let active = app
            .table_editors
            .get_mut(&id)
            .unwrap()
            .active
            .as_mut()
            .unwrap();
        active.editor.select_subtree(active.editor.root()).unwrap();
        frame(&mut app, &ctx, vec![egui::Event::Text("3".into())]);
        let CanvasContent::Opaque { xml, .. } = &app.document.regions[0].content else {
            panic!()
        };
        assert_eq!(xml, SOURCE);
        assert_eq!(app.last_executed_regions.len(), 0);
        assert!(app.dirty);
        frame(
            &mut app,
            &ctx,
            vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
        );
        let CanvasContent::Opaque { xml, .. } = &app.document.regions[0].content else {
            panic!()
        };
        assert_eq!(
            SpecTable::parse(xml)
                .unwrap()
                .cell(0, CellSlot::Body(0))
                .unwrap()
                .kind,
            NodeKind::Number("3".into())
        );
        assert!(
            app.calculation_error.is_none(),
            "{:?}",
            app.calculation_error
        );
        assert!(
            app.results.get(&id).is_some_and(|row| row.error.is_none()),
            "status={} errors={:?}",
            app.status,
            app.results
        );
        assert!(!app.last_executed_regions.is_empty());
        assert_eq!(app.undo.len(), 1);
        app.editing_region = None;
        app.undo();
        assert_eq!(
            app.document.regions[0].content,
            CanvasContent::Opaque {
                xml: SOURCE.into(),
                kind: "spec-table".into()
            }
        );
        app.redo();
        let CanvasContent::Opaque { xml, .. } = &app.document.regions[0].content else {
            panic!()
        };
        assert_eq!(
            SpecTable::parse(xml)
                .unwrap()
                .cell(0, CellSlot::Body(0))
                .unwrap()
                .kind,
            NodeKind::Number("3".into())
        );
    }

    #[test]
    fn tab_commits_in_row_order_and_appends_a_blank_row_at_the_end() {
        let mut state = TableEditor::new(SOURCE).unwrap();
        let mut xml = SOURCE.to_owned();
        state.activate(0, 3).unwrap();
        state.advance(true, &mut xml).unwrap();
        assert_eq!(state.table.row_count(), 3);
        assert_eq!(state.active.as_ref().unwrap().row, 4);
        assert_eq!(
            state.table.cell(0, CellSlot::Body(2)).unwrap().kind,
            NodeKind::Placeholder
        );
        state.advance(false, &mut xml).unwrap();
        assert_eq!(state.active.as_ref().unwrap().row, 3);
        assert!(xml.contains("1.00"));
    }

    #[test]
    fn structural_ribbon_commits_draft_as_one_region_undo_and_recalculates_once() {
        let (mut app, id) = app();
        let active = app
            .table_editors
            .get_mut(&id)
            .unwrap()
            .active
            .as_mut()
            .unwrap();
        active.editor = Editor::from_portable(&PortableTree {
            kind: NodeKind::Number("42".into()),
            children: vec![],
        })
        .unwrap();
        active.editor.move_to_expression_extreme(true);
        active.changed = true;
        assert!(app.table_matrix_command(RibbonAction::InsertMatrixRow));
        let CanvasContent::Opaque { xml, .. } = &app.document.regions[0].content else {
            panic!()
        };
        let table = SpecTable::parse(xml).unwrap();
        assert_eq!(table.row_count(), 3);
        assert_eq!(
            table.cell(0, CellSlot::Body(0)).unwrap().kind,
            NodeKind::Number("42".into())
        );
        assert_eq!(
            table.cell(0, CellSlot::Body(1)).unwrap().kind,
            NodeKind::Placeholder
        );
        assert_eq!(app.undo.len(), 1);
        assert_eq!(app.last_executed_regions.len(), 1);
        assert!(app.uncommitted_math.is_empty());
        assert!(
            app.native_visuals
                .region(&app.document.regions[0])
                .unwrap()
                .is_ok()
        );
        assert_eq!(app.editing_region, Some(id));
        app.undo();
        assert!(
            matches!(&app.document.regions[0].content,CanvasContent::Opaque {xml,..} if xml == SOURCE)
        );
        app.redo();
        let CanvasContent::Opaque { xml, .. } = &app.document.regions[0].content else {
            panic!()
        };
        assert_eq!(SpecTable::parse(xml).unwrap().row_count(), 3);
    }

    #[test]
    fn structural_insertion_uses_whole_cell_edge_and_header_row_guard() {
        let mut state = TableEditor::new(SOURCE).unwrap();
        let mut xml = SOURCE.to_owned();
        state.activate(0, 2).unwrap();
        let active = state.active.as_mut().unwrap();
        active.editor.move_to_expression_extreme(false);
        state
            .structural_command(RibbonAction::InsertMatrixRow, &mut xml)
            .unwrap();
        assert_eq!(
            state.table.cell(0, CellSlot::Body(0)).unwrap().kind,
            NodeKind::Placeholder
        );
        assert_eq!(
            state.table.cell(0, CellSlot::Body(1)).unwrap().kind,
            NodeKind::Number("1.00".into())
        );
        state.activate(0, 0).unwrap();
        let saved = xml.clone();
        assert!(
            !state
                .structural_command(RibbonAction::InsertMatrixRow, &mut xml)
                .unwrap()
        );
        assert_eq!(xml, saved);
        state
            .structural_command(RibbonAction::InsertMatrixColumn, &mut xml)
            .unwrap();
        assert_eq!(state.table.col_count(), 2);
    }

    #[test]
    fn shift_insert_shortcuts_commit_once_without_becoming_cell_text() {
        for (shortcut, rows, columns) in [(egui::Key::Enter, 3, 1), (egui::Key::Space, 2, 2)] {
            let (mut app, id) = app();
            let ctx = egui::Context::default();
            crate::configure(&ctx);
            frame(&mut app, &ctx, vec![]);
            frame(&mut app, &ctx, vec![key(shortcut, egui::Modifiers::SHIFT)]);
            assert_eq!(
                (
                    app.table_editors[&id].table.row_count(),
                    app.table_editors[&id].table.col_count()
                ),
                (rows, columns)
            );
            assert_eq!(app.undo.len(), 1);
            assert_eq!(app.last_executed_regions.len(), 1);
        }
    }

    #[test]
    fn pointer_drag_selects_rectangle_and_copy_carries_header_unit_and_body() {
        let mut state = TableEditor::new(SOURCE).unwrap();
        let mut xml = SOURCE.to_owned();
        let visual =
            mathcraft_document::native_visual::parse_table_visual(SOURCE, MathFormat::default())
                .unwrap();
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut positions = Vec::new();
        ctx.run_ui(egui::RawInput::default(), |ui| {
            let (_, cells) = crate::native_presentation::draw_table(ui, &visual, 150., None);
            positions = cells.iter().map(|cell| cell.rect.center()).collect();
        })
        .drop_without_applying_deltas();
        assert_eq!(positions.len(), 4);
        let mut run = |events| {
            ctx.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    state.show(ui, &mut xml, &visual, 150., false, 11.);
                },
            )
        };
        run(vec![
            egui::Event::PointerMoved(positions[0]),
            egui::Event::PointerButton {
                pos: positions[0],
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ])
        .drop_without_applying_deltas();
        run(vec![egui::Event::PointerMoved(positions[3])]).drop_without_applying_deltas();
        run(vec![egui::Event::PointerButton {
            pos: positions[3],
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }])
        .drop_without_applying_deltas();
        let copied = run(vec![egui::Event::Copy]);
        let copied = copied
            .platform_output
            .commands
            .iter()
            .find_map(|command| {
                let egui::OutputCommand::CopyText(text) = command else {
                    return None;
                };
                text.strip_prefix("mathcraft-table:").map(str::to_owned)
            })
            .expect("drag rectangle owns Copy");
        let copied = SpecTable::parse(&copied).unwrap();
        assert_eq!((copied.row_count(), copied.col_count()), (2, 1));
        assert!(
            matches!(copied.cell(0, CellSlot::Header).unwrap().kind, NodeKind::AnnotatedIdentifier {ref name,..} if name == "lengths")
        );
        assert!(
            matches!(copied.cell(0, CellSlot::Unit).unwrap().kind, NodeKind::AnnotatedIdentifier {ref name,..} if name == "mm")
        );
        assert_eq!(
            copied.cell(0, CellSlot::Body(0)).unwrap().kind,
            NodeKind::Number("1.00".into())
        );
        assert_eq!(
            copied.cell(0, CellSlot::Body(1)).unwrap().kind,
            NodeKind::Number("2".into())
        );
        assert_eq!(xml, SOURCE);
    }

    #[test]
    fn selection_delete_removes_data_row_and_groups_one_worksheet_commit() {
        let (mut app, id) = app();
        let state = app.table_editors.get_mut(&id).unwrap();
        state.deactivate();
        state.selection = Some(CellRange {
            start: (0, 2),
            end: (0, 2),
        });
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        frame(
            &mut app,
            &ctx,
            vec![key(egui::Key::Delete, egui::Modifiers::NONE)],
        );
        assert_eq!(app.table_editors[&id].table.row_count(), 1);
        assert_eq!(
            app.table_editors[&id]
                .table
                .cell(0, CellSlot::Body(0))
                .unwrap()
                .kind,
            NodeKind::Number("2".into())
        );
        assert_eq!(app.undo.len(), 1);
        assert_eq!(app.last_executed_regions.len(), 1);
    }

    #[test]
    fn translated_scaled_layer_drag_and_ribbon_copy_preserve_range() {
        let mut state = TableEditor::new(SOURCE).unwrap();
        let mut xml = SOURCE.to_owned();
        let visual =
            mathcraft_document::native_visual::parse_table_visual(SOURCE, MathFormat::default())
                .unwrap();
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let layer = egui::LayerId::new(egui::Order::Background, egui::Id::new("scaled-table"));
        let transform = egui::emath::TSTransform::from_translation(vec2(237., 119.))
            * egui::emath::TSTransform::from_scaling(2.);
        let mut positions = Vec::new();
        ctx.run_ui(egui::RawInput::default(), |ui| {
            ctx.set_sublayer(ui.layer_id(), layer);
            ctx.set_transform_layer(layer, transform);
            let mut child = ui.new_child(
                UiBuilder::new()
                    .id_salt("layer-table")
                    .layer_id(layer)
                    .max_rect(Rect::from_min_size(egui::Pos2::ZERO, vec2(500., 500.))),
            );
            let (_, cells) =
                crate::native_presentation::draw_table(&mut child, &visual, 150., None);
            positions = cells
                .iter()
                .map(|cell| transform * cell.rect.center())
                .collect();
        })
        .drop_without_applying_deltas();
        let mut run = |events, focus| {
            ctx.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    ctx.set_sublayer(ui.layer_id(), layer);
                    ctx.set_transform_layer(layer, transform);
                    let mut child = ui.new_child(
                        UiBuilder::new()
                            .id_salt("layer-table")
                            .layer_id(layer)
                            .max_rect(Rect::from_min_size(egui::Pos2::ZERO, vec2(500., 500.))),
                    );
                    state.show(&mut child, &mut xml, &visual, 150., focus, 11.);
                },
            )
        };
        run(
            vec![
                egui::Event::PointerMoved(positions[0]),
                egui::Event::PointerButton {
                    pos: positions[0],
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            false,
        )
        .drop_without_applying_deltas();
        run(vec![egui::Event::PointerMoved(positions[3])], false).drop_without_applying_deltas();
        run(
            vec![egui::Event::PointerButton {
                pos: positions[3],
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
            false,
        )
        .drop_without_applying_deltas();
        // Ribbon Copy requests focus_new on the selected table. That focus
        // request must not replace the rectangle with cell (0, 0).
        let output = run(vec![egui::Event::Copy], true);
        let copied = output
            .platform_output
            .commands
            .iter()
            .find_map(|command| {
                let egui::OutputCommand::CopyText(text) = command else {
                    return None;
                };
                text.strip_prefix("mathcraft-table:")
            })
            .expect("transformed table range owns Ribbon Copy");
        let copied = SpecTable::parse(copied).unwrap();
        assert_eq!((copied.row_count(), copied.col_count()), (2, 1));
        assert!(state.active.is_none());
        assert_eq!(state.selection.unwrap().bounds(), (0..4, 0..1));
        assert_eq!(xml, SOURCE);
        state.clear_range_selection();
        assert!(state.selection.is_none());
        assert!(state.drag_start.is_none());
    }

    #[test]
    fn native_identifier_name_and_literal_suffix_right_edge_advance_to_next_cell() {
        for suffix in [false, true] {
            let (mut app, id) = app();
            let mut table = SpecTable::parse(SOURCE).unwrap();
            table
                .set_cell(
                    0,
                    CellSlot::Body(0),
                    &PortableTree {
                        kind: NodeKind::NativeIdentifier {
                            name: "current".into(),
                            labels: "VARIABLE".into(),
                            contextual: None,
                            subscript: Some("rms".into()),
                        },
                        children: vec![],
                    },
                )
                .unwrap();
            let mut state = TableEditor::new(table.xml()).unwrap();
            state.activate(0, 2).unwrap();
            app.document.regions[0].content = CanvasContent::Opaque {
                xml: table.xml().into(),
                kind: "spec-table".into(),
            };
            app.table_editors.insert(id, state);
            app.refresh_table_visual(id);
            let ctx = egui::Context::default();
            crate::configure(&ctx);
            frame(&mut app, &ctx, vec![]);
            let active = app
                .table_editors
                .get_mut(&id)
                .unwrap()
                .active
                .as_mut()
                .unwrap();
            let root = active.editor.root();
            if suffix {
                active.editor.place_subscript_selection(root, 3, 3).unwrap();
            } else {
                active.editor.place_text_caret(root, 7).unwrap();
            }
            frame(
                &mut app,
                &ctx,
                vec![key(egui::Key::ArrowRight, egui::Modifiers::NONE)],
            );
            assert_eq!(
                app.table_editors[&id].active.as_ref().unwrap().row,
                3,
                "suffix={suffix}"
            );
            assert!(app.last_executed_regions.is_empty());
            assert!(app.undo.is_empty());
        }
    }

    #[test]
    fn pointer_click_other_cell_commits_draft_and_one_undo_restores_original() {
        let (mut app, id) = app();
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut next = egui::Pos2::ZERO;
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1200., 900.))),
                ..Default::default()
            },
            |ui| {
                let paper_min = ui.cursor().min;
                app.page_ui(ui);
                let region = &app.document.regions[0];
                let visual = app.native_visuals.region(region).unwrap().as_ref().unwrap();
                let mut origin = paper_min
                    + app
                        .page_geometry()
                        .to_paper(region.left, region.top)
                        .to_vec2();
                origin.y += crate::native_presentation::alignment_offset(ui, visual, 150., true);
                let mut child = ui.new_child(
                    UiBuilder::new()
                        .id_salt("test-table-geometry")
                        .max_rect(Rect::from_min_size(origin, vec2(150., 1000.))),
                );
                let (_, cells) =
                    crate::native_presentation::draw_table(&mut child, visual, 150., None);
                next = cells
                    .iter()
                    .find(|cell| cell.row == 3 && cell.column == 0)
                    .unwrap()
                    .rect
                    .center();
            },
        )
        .drop_without_applying_deltas();
        let active = app
            .table_editors
            .get_mut(&id)
            .unwrap()
            .active
            .as_mut()
            .unwrap();
        active.editor = Editor::from_portable(&PortableTree {
            kind: NodeKind::Number("42".into()),
            children: vec![],
        })
        .unwrap();
        active.changed = true;
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(next),
                egui::Event::PointerButton {
                    pos: next,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        assert!(app.last_executed_regions.is_empty());
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerButton {
                pos: next,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert_eq!(app.table_editors[&id].active.as_ref().unwrap().row, 3);
        assert_eq!(
            app.table_editors[&id]
                .table
                .cell(0, CellSlot::Body(0))
                .unwrap()
                .kind,
            NodeKind::Number("42".into())
        );
        assert_eq!(app.last_executed_regions.len(), 1);
        assert_eq!(app.undo.len(), 1);
        assert_eq!(app.editing_region, Some(id));
        app.undo();
        assert!(
            matches!(&app.document.regions[0].content,CanvasContent::Opaque {xml,..} if xml==SOURCE)
        );
    }

    #[test]
    fn minimum_explicit_delete_clears_but_rectangle_delete_retains_native_minimum() {
        let mut table = SpecTable::new(1, 1).unwrap();
        table
            .set_cell(
                0,
                CellSlot::Body(0),
                &PortableTree {
                    kind: NodeKind::Number("8".into()),
                    children: vec![],
                },
            )
            .unwrap();
        let mut xml = table.xml().to_owned();
        let mut state = TableEditor::new(&xml).unwrap();
        state.selection = Some(CellRange {
            start: (0, 0),
            end: (0, 2),
        });
        assert!(!state.remove_selection(&mut xml).unwrap());
        state.activate(0, 2).unwrap();
        assert!(
            state
                .structural_command(RibbonAction::DeleteMatrixRow, &mut xml)
                .unwrap()
        );
        assert_eq!((state.table.row_count(), state.table.col_count()), (1, 1));
        assert_eq!(
            state.table.cell(0, CellSlot::Body(0)).unwrap().kind,
            NodeKind::Placeholder
        );
    }

    #[test]
    fn matrix_paste_uses_the_active_equation_without_distributing_into_other_cells() {
        let (mut app, id) = app();
        let matrix = PortableTree {
            kind: NodeKind::Matrix {
                rows: 2,
                cols: 1,
                display: None,
            },
            children: vec![
                PortableTree {
                    kind: NodeKind::Number("8".into()),
                    children: vec![],
                },
                PortableTree {
                    kind: NodeKind::Number("9".into()),
                    children: vec![],
                },
            ],
        };
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        frame(&mut app, &ctx, vec![]);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::Paste(format!(
                "mathcraft-tree:{}",
                serde_json::to_string(&matrix).unwrap()
            ))],
        );
        assert_eq!(
            app.table_editors[&id]
                .active_editor()
                .unwrap()
                .to_portable()
                .unwrap(),
            matrix
        );
        // Paste is an equation draft, not an invented grid overwrite command.
        assert_eq!(
            app.table_editors[&id]
                .table
                .cell(0, CellSlot::Body(0))
                .unwrap()
                .kind,
            NodeKind::Number("1.00".into())
        );
        assert_eq!(
            app.table_editors[&id]
                .table
                .cell(0, CellSlot::Body(1))
                .unwrap()
                .kind,
            NodeKind::Number("2".into())
        );
        assert!(app.undo.is_empty());
        assert!(app.last_executed_regions.is_empty());
        frame(
            &mut app,
            &ctx,
            vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
        );
        assert_eq!(
            app.table_editors[&id]
                .table
                .cell(0, CellSlot::Body(0))
                .unwrap(),
            matrix
        );
        assert_eq!(
            app.table_editors[&id]
                .table
                .cell(0, CellSlot::Body(1))
                .unwrap()
                .kind,
            NodeKind::Number("2".into())
        );
        assert_eq!(app.undo.len(), 1);
    }

    #[test]
    fn body_cut_copies_a_matrix_before_one_structural_commit_and_undo() {
        let (mut app, id) = app();
        let state = app.table_editors.get_mut(&id).unwrap();
        state.deactivate();
        state.selection = Some(CellRange {
            start: (0, 2),
            end: (0, 2),
        });
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let output = ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Cut],
                ..Default::default()
            },
            |ui| app.page_ui(ui),
        );
        let copied = output
            .platform_output
            .commands
            .iter()
            .find_map(|command| {
                if let egui::OutputCommand::CopyText(text) = command {
                    text.strip_prefix("mathcraft-tree:")
                } else {
                    None
                }
            })
            .unwrap();
        let matrix: PortableTree = serde_json::from_str(copied).unwrap();
        assert_eq!(
            matrix.kind,
            NodeKind::Matrix {
                rows: 1,
                cols: 1,
                display: None
            }
        );
        assert_eq!(matrix.children[0].kind, NodeKind::Number("1.00".into()));
        output.drop_without_applying_deltas();
        assert_eq!(app.table_editors[&id].table.row_count(), 1);
        assert_eq!(
            app.table_editors[&id]
                .table
                .cell(0, CellSlot::Body(0))
                .unwrap()
                .kind,
            NodeKind::Number("2".into())
        );
        assert_eq!(app.undo.len(), 1);
        assert_eq!(app.last_executed_regions.len(), 1);
        app.undo();
        assert!(
            matches!(&app.document.regions[0].content, CanvasContent::Opaque {xml,..} if xml == SOURCE)
        );
        app.redo();
        assert!(
            matches!(&app.document.regions[0].content, CanvasContent::Opaque {xml,..} if SpecTable::parse(xml).unwrap().row_count() == 1)
        );
    }

    #[test]
    fn clear_cells_preserves_dimensions_and_metadata_but_releases_the_selection() {
        let (mut app, id) = app();
        let state = app.table_editors.get_mut(&id).unwrap();
        state.deactivate();
        state.selection = Some(CellRange {
            start: (0, 0),
            end: (0, 3),
        });
        assert!(app.table_matrix_command(RibbonAction::ClearCells));
        let table = &app.table_editors[&id];
        assert!(table.selection.is_none());
        assert_eq!((table.table.row_count(), table.table.col_count()), (2, 1));
        for row in 0..4 {
            assert_eq!(
                table.table.cell(0, slot(row)).unwrap().kind,
                NodeKind::Placeholder
            );
        }
        assert!(table.table.xml().contains("resultRef=\"18\""));
        assert_eq!(app.undo.len(), 1);
        app.undo();
        assert!(
            matches!(&app.document.regions[0].content, CanvasContent::Opaque {xml,..} if xml == SOURCE)
        );
    }

    #[test]
    fn clear_active_cell_is_one_committed_undo_and_accepts_an_incomplete_draft() {
        for incomplete in [false, true] {
            let (mut app, id) = app();
            if incomplete {
                let active = app
                    .table_editors
                    .get_mut(&id)
                    .unwrap()
                    .active
                    .as_mut()
                    .unwrap();
                active.editor = Editor::new();
                formula::input_text(&mut active.editor, "1e");
                active.changed = true;
            }
            app.table_matrix_command(RibbonAction::ClearCells);
            assert_eq!(
                app.table_editors[&id]
                    .table
                    .cell(0, CellSlot::Body(0))
                    .unwrap()
                    .kind,
                NodeKind::Placeholder
            );
            assert_eq!(app.undo.len(), 1);
            app.undo();
            assert!(
                matches!(&app.document.regions[0].content, CanvasContent::Opaque {xml,..} if xml == SOURCE)
            );
        }
    }

    #[test]
    fn header_table_payload_pastes_as_a_new_table_even_with_a_focused_cell() {
        let (mut app, id) = app();
        let payload = app.table_editors[&id]
            .table
            .copy_selection(0..4, 0..1)
            .unwrap();
        let text = clipboard_text(payload).unwrap();
        app.document.regions[0].format.font_points = 23.;
        app.focus_new = Some(id);
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![egui::Event::Paste(text)]);
        assert_eq!(app.document.regions.len(), 2);
        let pasted = app.selected.unwrap();
        assert_ne!(pasted, id);
        let region = app
            .document
            .regions
            .iter()
            .find(|r| r.id == pasted)
            .unwrap();
        assert_eq!(region.format, MathFormat::default());
        assert!(region.source_region.is_none());
        assert!(app.native_visuals.region(region).unwrap().is_ok());
        let CanvasContent::Opaque { xml, .. } = &region.content else {
            panic!()
        };
        let table = SpecTable::parse(xml).unwrap();
        assert_eq!((table.row_count(), table.col_count()), (2, 1));
        assert_eq!(
            table.cell(0, CellSlot::Body(0)).unwrap().kind,
            NodeKind::Number("1.00".into())
        );
        assert!(table.columns()[0].result_ref.is_none());
        assert_eq!(app.undo.len(), 1);
        app.undo();
        assert_eq!(app.document.regions.len(), 1);
    }

    #[test]
    fn rejected_table_object_paste_does_not_commit_the_active_cell_draft() {
        let (mut app, id) = app();
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        frame(&mut app, &ctx, vec![]);
        let active = app
            .table_editors
            .get_mut(&id)
            .unwrap()
            .active
            .as_mut()
            .unwrap();
        active.editor = Editor::from_portable(&PortableTree {
            kind: NodeKind::Number("42".into()),
            children: vec![],
        })
        .unwrap();
        active.changed = true;
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::Paste("mathcraft-table:invalid".into())],
        );
        assert_eq!(app.document.regions.len(), 1);
        assert!(
            matches!(&app.document.regions[0].content, CanvasContent::Opaque {xml,..} if xml == SOURCE)
        );
        assert!(app.table_editors[&id].has_draft_changes());
        assert!(app.undo.is_empty());
        assert!(app.last_executed_regions.is_empty());
    }

    #[test]
    fn explicit_calculation_commits_a_table_draft_once_and_disabled_table_stays_disabled() {
        for disabled in [false, true] {
            let (mut app, id) = app();
            app.document.regions[0].calculation_disabled = disabled;
            let active = app
                .table_editors
                .get_mut(&id)
                .unwrap()
                .active
                .as_mut()
                .unwrap();
            active.editor = Editor::from_portable(&PortableTree {
                kind: NodeKind::Number("4".into()),
                children: vec![],
            })
            .unwrap();
            active.changed = true;
            app.recalculate();
            assert!(
                app.calculation_error.is_none(),
                "{:?}",
                app.calculation_error
            );
            assert_eq!(
                app.last_executed_regions.len(),
                if disabled { 0 } else { 1 }
            );
            assert!(!app.table_editors[&id].has_active());
        }
    }

    #[test]
    #[ignore = "Read-only user source selected through environment variables"]
    fn native_spec_table_edit_calculate_save_export_reopen_probe() {
        let input = PathBuf::from(
            std::env::var_os("MATHCRAFT_TABLE_INPUT").expect("MATHCRAFT_TABLE_INPUT"),
        );
        let output = PathBuf::from(
            std::env::var_os("MATHCRAFT_TABLE_OUTPUT").expect("MATHCRAFT_TABLE_OUTPUT"),
        );
        std::fs::create_dir_all(&output).unwrap();
        let mut app = WorksheetApp::default();
        app.open(&input);
        let region = app
            .document
            .regions
            .iter()
            .find(
                |r| matches!(&r.content, CanvasContent::Opaque {kind, ..} if kind == "spec-table"),
            )
            .unwrap()
            .clone();
        let CanvasContent::Opaque { xml, .. } = &region.content else {
            panic!()
        };
        let original = SpecTable::parse(xml).unwrap();
        assert_eq!((original.col_count(), original.row_count()), (6, 10));
        assert!(app.last_executed_regions.is_empty());
        assert!(
            !app.pending_regions.contains(&region.id),
            "Synchronized input-only columns do not need numeric cache payloads"
        );
        let mut editor = TableEditor::new(xml).unwrap();
        editor.activate(0, 2).unwrap();
        let active = editor.active.as_mut().unwrap();
        active.editor = Editor::from_portable(&PortableTree {
            kind: NodeKind::Number("42".into()),
            children: vec![],
        })
        .unwrap();
        active.changed = true;
        app.table_editors.insert(region.id, editor);
        app.selected = Some(region.id);
        app.editing_region = Some(region.id);
        app.recalculate();
        assert!(
            app.calculation_error.is_none(),
            "{:?}",
            app.calculation_error
        );
        assert!(
            app.results.values().all(|row| row.error.is_none()),
            "{:?}",
            app.results
                .values()
                .filter_map(|r| r.error.as_ref())
                .collect::<Vec<_>>()
        );
        let context = app
            .calculator
            .context_before(region.left, region.top + app.document.grid)
            .unwrap();
        let query = format!(
            "<ml:eval xmlns:ml=\"http://schemas.mathsoft.com/math50\">{}<ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
            original.columns()[0].header.mathml
        );
        let value = context.evaluate_mathml(&query).unwrap();
        let Value::Matrix {
            rows,
            cols,
            elements,
        } = value
        else {
            panic!("First native table column must remain a vector");
        };
        assert_eq!((rows, cols), (10, 1));
        assert_eq!(elements[0].value.re, 42.);
        app.sync_editors().unwrap();
        app.persist_calculation_cache();
        let saved_presentations = app
            .document
            .calculation_cache
            .as_ref()
            .unwrap()
            .rows
            .iter()
            .filter(|row| row.display.is_some())
            .count();
        let saved = output.join("edited.mathcraft");
        let exported = output.join("edited.mcdx");
        app.document.save(&saved).unwrap();
        mathcraft_document::export::export_mcdx(&app.document, &exported).unwrap();
        for path in [&saved, &exported] {
            let mut reopened = WorksheetApp::default();
            reopened.open(path);
            assert!(reopened.last_executed_regions.is_empty());
            if path == &saved {
                assert!(
                    reopened.document.saved_calculation_cache().is_some(),
                    "Unchanged saved source must retain its calculation identity"
                );
                assert_eq!(
                    reopened
                        .saved_results
                        .values()
                        .filter(|row| row.display.is_some())
                        .count(),
                    saved_presentations
                );
                assert!(reopened.pending_regions.is_empty());
            }
            let table = reopened.document.regions.iter().find(|r| matches!(&r.content, CanvasContent::Opaque {kind, ..} if kind == "spec-table")).unwrap();
            let CanvasContent::Opaque { xml, .. } = &table.content else {
                panic!()
            };
            let changed = SpecTable::parse(xml).unwrap();
            assert_eq!(
                changed.cell(0, CellSlot::Body(0)).unwrap().kind,
                NodeKind::Number("42".into())
            );
            for column in 1..original.col_count() {
                for row in 0..original.row_count() + 2 {
                    assert_eq!(
                        changed.cell(column, slot(row)).unwrap(),
                        original.cell(column, slot(row)).unwrap()
                    );
                }
            }
            assert!(reopened.native_visuals.region(table).unwrap().is_ok());
        }
        println!(
            "{}",
            serde_json::json!({"regions":app.document.regions.len(),"calculation_rows":app.results.len(),"saved_presentations":saved_presentations,"table_columns":original.col_count(),"table_rows":original.row_count(),"edited_first_current_ampere":elements[0].value.re,"errors":0,"saved":saved,"exported":exported})
        );
    }
}
