//! Direct AST renderer and a bounded structural editing surface.
//!
//! Prime font sizes, label styles and fraction padding follow static evidence.
//! Glyph outlines use licensed STIX faces; fences/root paths and some positioning
//! remain independent approximations. This is not a complete Prime editor.

use eframe::egui::{
    self, Color32, Galley, Key, Painter, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2,
    pos2, vec2,
};
use mathcraft_editor::{
    Arena, BinaryOp, Direction, Editor, FractionMetrics, NodeId, NodeKind, PortableTree, Selection,
    layout_fraction,
};
use std::sync::Arc;

use crate::typography::{self, INK, MathStyle};
use std::collections::HashMap;
const CARET: Color32 = Color32::from_rgb(0, 128, 255);
const PADDING: f32 = 2.0;
#[cfg(test)]
const FONT_SIZE: f32 = typography::MATH_SIZE;

#[derive(Clone, Copy, Debug)]
pub enum FormulaResult<'a> {
    Formatted(&'a crate::result_display::FormattedResult),
    Text(&'a str),
    #[allow(
        dead_code,
        reason = "Legacy display payload used by structural geometry regressions"
    )]
    Matrix {
        rows: usize,
        cols: usize,
        cells: &'a [String],
        brackets: bool,
    },
}

/// A calculation request is a command even when the editable tree is unchanged.
/// REA: EquationModel.ProcessEqualsKey invokes the evaluation callback for an
/// existing EVALUATION without adding a second operator or an undo entry.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct InputOutcome {
    pub document_changed: bool,
    pub evaluation_requested: bool,
}

pub(crate) struct FormulaResponse {
    pub response: Response,
    pub evaluation_requested: bool,
}

struct FormulaBox {
    node: NodeId,
    size: Vec2,
    baseline: f32,
    primitives: Vec<Primitive>,
    children: Vec<(Vec2, FormulaBox)>,
    atom: Option<Arc<Galley>>,
    hit_bounds: Option<Rect>,
    text_offset: Vec2,
}

enum Primitive {
    Text(Vec2, Arc<Galley>),
    Line(Vec2, Vec2),
    Placeholder(Rect),
}

struct HitNode {
    node: NodeId,
    rect: Rect,
    atom: Option<Arc<Galley>>,
    text_origin: Pos2,
}

#[derive(Clone)]
struct DragAnchor {
    node: NodeId,
    offset: usize,
}

/// Paint and edit one structural formula. The caller supplies a stable egui ID
/// scope when rendering multiple editors (`ui.push_id(worksheet_region_id, …)`).
///
/// Supported: atomic text, / ^ ( ), + - *, Space, Tab, left/right, Backspace,
/// Delete, Ctrl/Cmd+Z and redo. '=' evaluates the root or a definition's RHS;
/// repeated evaluation requests do not change the document or its undo history.
/// Enter/Escape events are left for the surrounding worksheet to handle.
#[allow(
    dead_code,
    reason = "Compatibility wrapper; worksheet uses the result-aware variant."
)]
pub fn formula_ui(ui: &mut Ui, editor: &mut Editor) -> Response {
    formula_ui_with_result(ui, editor, None)
}

/// Display a cached evaluation result without mutating the expression tree.
/// `result` excludes any editable RHS unit identifier. Unit hit/caret behavior
/// and result composition are independent policies, not recovered Prime geometry.
pub fn formula_ui_with_result(ui: &mut Ui, editor: &mut Editor, result: Option<&str>) -> Response {
    formula_ui_with_focus(ui, editor, result, false)
}

pub(crate) fn formula_ui_with_focus(
    ui: &mut Ui,
    editor: &mut Editor,
    result: Option<&str>,
    focus: bool,
) -> Response {
    formula_ui_with_display(ui, editor, result.map(FormulaResult::Text), focus)
}

/// Cached values are layout-only; every hit remains bound to an existing source
/// RHS/unit node. No temporary arena or editable result-cell nodes are created.
pub fn measured_size_styled(
    ui: &Ui,
    editor: &Editor,
    result: Option<FormulaResult<'_>>,
    font_points: f32,
) -> Vec2 {
    let layout = measure_with_display(
        ui.painter(),
        editor.arena(),
        editor.root(),
        font_points.clamp(1., 200.) * 96. / 72.,
        result,
    );
    (layout.size + vec2(PADDING * 2., PADDING * 2.)).max(vec2(12., 22.))
}

pub fn formula_ui_with_display(
    ui: &mut Ui,
    editor: &mut Editor,
    result: Option<FormulaResult<'_>>,
    focus: bool,
) -> Response {
    formula_ui_with_actions(ui, editor, result, focus).response
}

pub(crate) fn formula_ui_with_actions(
    ui: &mut Ui,
    editor: &mut Editor,
    result: Option<FormulaResult<'_>>,
    focus: bool,
) -> FormulaResponse {
    formula_ui_with_labels(ui, editor, result, focus, None)
}

pub(crate) fn formula_ui_with_labels(
    ui: &mut Ui,
    editor: &mut Editor,
    result: Option<FormulaResult<'_>>,
    focus: bool,
    labelled: Option<&PortableTree>,
) -> FormulaResponse {
    formula_ui_styled(ui, editor, result, focus, labelled, 11.)
}

pub(crate) fn formula_ui_styled(
    ui: &mut Ui,
    editor: &mut Editor,
    result: Option<FormulaResult<'_>>,
    focus: bool,
    labelled: Option<&PortableTree>,
    font_points: f32,
) -> FormulaResponse {
    let font_size = font_points.clamp(1., 200.) * 96. / 72.;
    let labels = resolved_labels(editor.arena(), editor.root(), labelled);
    let initial = Typesetter {
        painter: ui.painter(),
        arena: editor.arena(),
        labels: &labels,
    }
    .layout(editor.root(), font_size, result);
    let desired = (initial.size + vec2(PADDING * 2.0, PADDING * 2.0)).max(vec2(12., 22.));
    let (rect, mut response) = ui.allocate_exact_size(desired, Sense::click_and_drag());
    if focus {
        response.request_focus();
    }
    let origin = rect.min + vec2(PADDING, PADDING);
    let mut hits = vec![];
    collect_hits(&initial, origin, &mut hits);

    if response.clicked() {
        response.request_focus();
        if let Some(pointer) = response.interact_pointer_pos()
            && let Some(hit) = hit_node(&hits, pointer)
        {
            if let Some(galley) = &hit.atom {
                let cursor = galley.cursor_from_pos(pointer - hit.text_origin);
                let offset = galley
                    .text()
                    .chars()
                    .take(cursor.index.into())
                    .map(char::len_utf16)
                    .sum();
                let anchor = editor.caret().value_range.map_or(offset, |r| r.anchor);
                if ui.input(|i| i.modifiers.shift) && editor.caret().node == hit.node {
                    let _ = editor.place_text_selection(hit.node, anchor, offset);
                } else {
                    let _ = editor.place_text_caret(hit.node, offset);
                }
            } else {
                let side = if pointer.x < hit.rect.center().x {
                    Direction::Left
                } else {
                    Direction::Right
                };
                let _ = editor.place_caret(hit.node, side);
            }
            if response.double_clicked() {
                let _ = editor.select_subtree(hit.node);
            }
        }
    }

    let drag_id = response.id.with("text-drag");
    if response.drag_started() {
        response.request_focus();
        let from_global = ui
            .ctx()
            .layer_transform_from_global(ui.layer_id())
            .unwrap_or_default();
        if let Some(point) = ui
            .input(|i| i.pointer.press_origin())
            .map(|p| from_global * p)
            && let Some(hit) = hit_node(&hits, point)
        {
            let offset = hit_text_offset(hit, point);
            ui.data_mut(|data| {
                data.insert_temp(
                    drag_id,
                    DragAnchor {
                        node: hit.node,
                        offset,
                    },
                )
            });
        }
    }
    if response.dragged()
        && let Some(anchor) = ui.data(|data| data.get_temp::<DragAnchor>(drag_id))
        && let Some(point) = response.interact_pointer_pos()
    {
        if let Some(hit) = hits.iter().find(|h| h.node == anchor.node)
            && hit.atom.is_some()
            && (hit.rect.expand2(vec2(10000., 3.)).contains(point))
        {
            let _ = editor.place_text_selection(
                anchor.node,
                anchor.offset,
                hit_text_offset(hit, point),
            );
        } else if let Some(hit) = hit_node(&hits, point) {
            let mut node = anchor.node;
            loop {
                if node_contains(editor.arena(), node, hit.node) {
                    let _ = editor.select_subtree(node);
                    break;
                }
                let Some(parent) = editor.arena().get(node).and_then(|n| n.parent()) else {
                    break;
                };
                node = parent;
            }
        }
    }
    if response.drag_stopped() {
        ui.data_mut(|data| data.remove::<DragAnchor>(drag_id));
    }

    let mut evaluation_requested = false;
    let ime_id = response.id.with("ime-preedit");
    let mut preedit = ui
        .data(|data| data.get_temp::<String>(ime_id))
        .unwrap_or_default();
    if response.has_focus() {
        ui.memory_mut(|memory| {
            memory.set_focus_lock_filter(
                response.id,
                egui::EventFilter {
                    tab: true,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    ..Default::default()
                },
            );
        });
        // This widget owns keyboard events only while it has focus. Pointer and
        // viewport events remain available to the surrounding worksheet.
        let events = ui.input_mut(|input| {
            let composing = !preedit.is_empty() || input.events.iter().any(|e| matches!(e,
                egui::Event::Ime(egui::ImeEvent::Preedit { text, .. } | egui::ImeEvent::Commit(text)) if !text.is_empty()));
            let mut ours = Vec::new();
            input.events.retain(|event| {
                let owned = match event {
                    egui::Event::Text(_)
                    | egui::Event::Paste(_)
                    | egui::Event::Copy
                    | egui::Event::Cut => true,
                    egui::Event::Ime(_) => true,
                    egui::Event::Key { key, modifiers, .. } => composing || (modifiers.shift && *key == Key::Enter) || !matches!(key, Key::Enter | Key::Escape),
                    _ => false,
                };
                if owned {
                    ours.push(event.clone());
                    false
                } else {
                    true
                }
            });
            ours
        });
        let mut changed = false;
        for event in events {
            match event {
                egui::Event::Text(text) => {
                    if text == " " && ui.input(|i| i.modifiers.shift) {
                        continue;
                    }
                    if !preedit.is_empty() {
                        continue;
                    }
                    let outcome = input_text_with_actions(editor, &text);
                    changed |= outcome.document_changed;
                    evaluation_requested |= outcome.evaluation_requested;
                }
                egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => {
                    preedit = text;
                    ui.ctx().request_repaint();
                }
                egui::Event::Ime(egui::ImeEvent::Commit(text)) => {
                    preedit.clear();
                    let outcome = input_text_with_actions(editor, &text);
                    changed |= outcome.document_changed;
                    evaluation_requested |= outcome.evaluation_requested;
                }
                egui::Event::Ime(egui::ImeEvent::DeleteSurrounding {
                    before_chars,
                    after_chars,
                }) => {
                    changed |= delete_surrounding(editor, before_chars, after_chars);
                }
                egui::Event::Paste(text) => {
                    let outcome = paste_tree_or_text_with_actions(editor, &text);
                    changed |= outcome.document_changed;
                    evaluation_requested |= outcome.evaluation_requested;
                }
                egui::Event::Copy | egui::Event::Cut => {
                    if let Some(text) = copy_tree(editor) {
                        ui.ctx().copy_text(text);
                        if event == egui::Event::Cut {
                            if editor.selection() == Selection::None
                                && editor
                                    .caret()
                                    .value_range
                                    .is_some_and(|r| r.anchor != r.focus)
                            {
                                changed |= delete_character(editor, true);
                            } else {
                                if editor.selection() == Selection::None {
                                    let _ = editor.select_subtree(editor.caret().node);
                                }
                                changed |= editor.delete_structure(true);
                            }
                        }
                    }
                }
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    if !preedit.is_empty() {
                        continue;
                    }
                    if modifiers.command && key == Key::Z {
                        changed |= if modifiers.shift {
                            editor.redo()
                        } else {
                            editor.undo()
                        };
                    } else if modifiers.command && key == Key::Y {
                        changed |= editor.redo();
                    } else if modifiers.command && key == Key::A {
                        let _ = editor.select_subtree(editor.root());
                    } else if !modifiers.command && !modifiers.alt {
                        match key {
                            Key::Backspace => changed |= delete_character(editor, true),
                            Key::Delete => changed |= delete_character(editor, false),
                            Key::ArrowLeft | Key::ArrowRight if modifiers.shift => {
                                extend_text_selection(editor, key == Key::ArrowRight)
                            }
                            Key::ArrowLeft => move_horizontal(editor, false),
                            Key::ArrowRight => move_horizontal(editor, true),
                            Key::Enter if modifiers.shift => {
                                changed |= editor.insert_matrix_row(Direction::Indeterminate)
                            }
                            Key::Space if modifiers.shift => {
                                changed |= editor.insert_matrix_column(Direction::Indeterminate)
                            }
                            Key::ArrowUp | Key::ArrowDown => {
                                move_vertical(editor, &hits, key == Key::ArrowDown)
                            }
                            Key::Tab => cycle_leaf(editor, !modifiers.shift, true),
                            Key::Home => atom_extreme(editor, false),
                            Key::End => atom_extreme(editor, true),
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
        ui.data_mut(|data| data.insert_temp(ime_id, preedit.clone()));
        if changed {
            response.mark_changed();
            ui.ctx().request_repaint();
        }
    } else {
        ui.data_mut(|data| data.remove::<String>(ime_id));
        preedit.clear();
    }

    let labels = resolved_labels(editor.arena(), editor.root(), labelled);
    let layout = Typesetter {
        painter: ui.painter(),
        arena: editor.arena(),
        labels: &labels,
    }
    .layout(editor.root(), font_size, result);
    let mut hits = vec![];
    collect_hits(&layout, origin, &mut hits);
    let paint = ui.painter();
    if response.has_focus()
        && let Selection::Subtree(node) = editor.selection()
        && let Some(hit) = hits.iter().find(|hit| hit.node == node)
    {
        paint.rect_filled(
            hit.rect.expand(1.0),
            0.,
            Color32::from_gray(169).gamma_multiply(0.25),
        );
    }
    paint_box(paint, &layout, origin);
    let caret_node = match editor.selection() {
        Selection::Subtree(id) => id,
        Selection::None => editor.caret().node,
    };
    if response.has_focus()
        && let Some(hit) = hits.iter().find(|hit| hit.node == caret_node)
    {
        if let Some(range) = editor.caret().value_range
            && range.anchor != range.focus
        {
            let a = text_caret_rect(hit, range.anchor);
            let b = text_caret_rect(hit, range.focus);
            paint.rect_filled(a.union(b).expand(1.), 0., CARET.gamma_multiply(0.18));
        }
        paint_caret(paint, hit, editor);
        let cursor = caret_rect(hit, editor);
        if !preedit.is_empty() {
            let preview = text_galley(paint, &preedit, font_size);
            let preview_rect = Rect::from_min_size(cursor.left_top(), preview.size());
            paint.rect_filled(preview_rect.expand(2.), 0., Color32::WHITE);
            paint.galley(preview_rect.min, preview, INK);
            paint.line_segment(
                [preview_rect.left_bottom(), preview_rect.right_bottom()],
                Stroke::new(1., CARET),
            );
        }
        let to_global = ui
            .ctx()
            .layer_transform_to_global(ui.layer_id())
            .unwrap_or_default();
        if ui.memory(|m| m.owns_ime_events(response.id)) {
            ui.output_mut(|output| {
                output.ime = Some(egui::output::IMEOutput {
                    purpose: egui::IMEPurpose::Normal,
                    rect: to_global * rect,
                    cursor_rect: to_global * cursor,
                    should_interrupt_composition: false,
                })
            });
        }
    }
    FormulaResponse {
        response: response.on_hover_cursor(egui::CursorIcon::Text),
        evaluation_requested,
    }
}

fn text_galley(painter: &Painter, text: &str, size: f32) -> Arc<Galley> {
    painter.layout_no_wrap(text.into(), MathStyle::Operator.font(size), INK)
}

fn text_baseline(galley: &Galley) -> f32 {
    galley
        .rows
        .first()
        .and_then(|row| row.glyphs.first().map(|glyph| row.pos.y + glyph.pos.y))
        .unwrap_or(galley.size().y * 0.8)
}

#[cfg(test)]
fn measure(painter: &Painter, arena: &Arena, id: NodeId, size: f32) -> FormulaBox {
    measure_with_result(painter, arena, id, size, None)
}

#[cfg(test)]
fn measure_with_result(
    painter: &Painter,
    arena: &Arena,
    id: NodeId,
    size: f32,
    result_text: Option<&str>,
) -> FormulaBox {
    measure_with_display(
        painter,
        arena,
        id,
        size,
        result_text.map(FormulaResult::Text),
    )
}

fn measure_with_display(
    painter: &Painter,
    arena: &Arena,
    id: NodeId,
    size: f32,
    result_text: Option<FormulaResult<'_>>,
) -> FormulaBox {
    Typesetter {
        painter,
        arena,
        labels: &HashMap::new(),
    }
    .layout(id, size, result_text)
}

fn resolved_labels(
    arena: &Arena,
    id: NodeId,
    labelled: Option<&PortableTree>,
) -> HashMap<NodeId, String> {
    fn visit(arena: &Arena, id: NodeId, tree: &PortableTree, out: &mut HashMap<NodeId, String>) {
        let node = arena.get(id).unwrap();
        if node.children().len() != tree.children.len() {
            return;
        }
        let source_name = match node.kind() {
            NodeKind::Identifier(n)
            | NodeKind::AnnotatedIdentifier { name: n, .. }
            | NodeKind::NativeIdentifier { name: n, .. } => Some(n),
            _ => None,
        };
        if let Some(source) = source_name {
            if let NodeKind::AnnotatedIdentifier { name, labels }
            | NodeKind::NativeIdentifier { name, labels, .. } = &tree.kind
                && name == source
            {
                out.insert(id, labels.clone());
            }
            return;
        }
        // A stale cached tree must never style a structurally different expression.
        if std::mem::discriminant(node.kind()) != std::mem::discriminant(&tree.kind) {
            return;
        }
        for (child, labelled) in node.children().iter().zip(&tree.children) {
            visit(arena, *child, labelled, out);
        }
    }
    let mut out = HashMap::new();
    if let Some(tree) = labelled {
        visit(arena, id, tree, &mut out);
    }
    out
}

struct Typesetter<'a> {
    painter: &'a Painter,
    arena: &'a Arena,
    labels: &'a HashMap<NodeId, String>,
}
impl Typesetter<'_> {
    fn measure(&self, id: NodeId, size: f32) -> FormulaBox {
        self.layout(id, size, None)
    }
    fn identifier_style(&self, id: NodeId) -> MathStyle {
        let node = self.arena.get(id).unwrap();
        if let Some(label) = self.labels.get(&id) {
            return MathStyle::from_label(label);
        }
        match node.kind() {
            NodeKind::AnnotatedIdentifier { labels, .. }
            | NodeKind::NativeIdentifier { labels, .. } => MathStyle::from_label(labels),
            NodeKind::Identifier(_) => {
                if node
                    .parent()
                    .and_then(|p| self.arena.get(p))
                    .is_some_and(|p| {
                        matches!(p.kind(), NodeKind::Call { .. } | NodeKind::Function { .. })
                            && p.children()[0] == id
                    })
                {
                    MathStyle::Function
                } else {
                    MathStyle::Variable
                }
            }
            _ => MathStyle::Operator,
        }
    }
    fn layout(&self, id: NodeId, size: f32, result_text: Option<FormulaResult<'_>>) -> FormulaBox {
        let painter = self.painter;
        let arena = self.arena;
        let node = arena.get(id).expect("validated editor tree");
        let mut result = FormulaBox {
            node: id,
            size: Vec2::ZERO,
            baseline: 0.0,
            primitives: vec![],
            children: vec![],
            atom: None,
            hit_bounds: None,
            text_offset: Vec2::ZERO,
        };
        match node.kind() {
            NodeKind::Identifier(text)
            | NodeKind::Number(text)
            | NodeKind::AnnotatedIdentifier { name: text, .. }
            | NodeKind::NativeIdentifier { name: text, .. }
            | NodeKind::Imaginary(text)
            | NodeKind::ImaginarySymbol { value: text, .. } => {
                let style = self.identifier_style(id);
                let galley = painter.layout_no_wrap(
                    if text.is_empty() { " " } else { text }.into(),
                    style.font(size),
                    style.color(),
                );
                result.size = galley.size().max(vec2(4.0, size));
                result.baseline = text_baseline(&galley);
                result
                    .primitives
                    .push(Primitive::Text(Vec2::ZERO, galley.clone()));
                result.atom = Some(galley);
                let suffix = match node.kind() {
                    NodeKind::NativeIdentifier {
                        subscript: Some(text),
                        ..
                    } => Some((
                        text.as_str(),
                        typography::script_size(size, 0.7),
                        size * 0.42,
                    )),
                    NodeKind::Imaginary(_) => Some(("i", size, 0.0)),
                    NodeKind::ImaginarySymbol { symbol, .. } => Some((symbol.as_str(), size, 0.0)),
                    _ => None,
                };
                if let Some((suffix, font_size, y)) = suffix {
                    let suffix =
                        painter.layout_no_wrap(suffix.into(), style.font(font_size), style.color());
                    let original = result.size;
                    result
                        .primitives
                        .push(Primitive::Text(vec2(original.x, y), suffix.clone()));
                    result.size = vec2(
                        original.x + suffix.size().x,
                        original.y.max(y + suffix.size().y),
                    );
                    result.hit_bounds = Some(Rect::from_min_size(Pos2::ZERO, original));
                }
            }
            NodeKind::Placeholder => {
                result.size = vec2(size * 0.46, size * 0.80);
                result.baseline = size * 0.64;
                result
                    .primitives
                    .push(Primitive::Placeholder(Rect::from_min_size(
                        pos2(1.0, size * 0.12),
                        vec2(size * 0.34, size * 0.55),
                    )));
            }
            NodeKind::UnaryNeg => {
                let mut child = self.measure(node.children()[0], size);
                if arena.get(node.children()[0]).unwrap().kind().precedence()
                    < node.kind().precedence()
                {
                    child = visual_parentheses(painter, child, size);
                }
                result = row_layout(
                    id,
                    vec![
                        RowPiece::Text(text_galley(painter, "−", size)),
                        RowPiece::Child(child),
                    ],
                );
            }
            NodeKind::Absolute => {
                let child = self.measure(node.children()[0], size);
                let height = child.size.y;
                result.size = child.size + vec2(10.0, 0.0);
                result.baseline = child.baseline;
                result
                    .primitives
                    .push(Primitive::Line(vec2(2.0, 0.0), vec2(2.0, height)));
                result.primitives.push(Primitive::Line(
                    vec2(result.size.x - 2.0, 0.0),
                    vec2(result.size.x - 2.0, height),
                ));
                result.children.push((vec2(5.0, 0.0), child));
            }
            NodeKind::Root => {
                let square = matches!(arena.get(node.children()[0]).unwrap().kind(), NodeKind::Number(n) if n == "2");
                let mut degree =
                    self.measure(node.children()[0], typography::script_size(size, 7. / 11.));
                if square {
                    degree.size = Vec2::ZERO;
                    degree.primitives.clear();
                    degree.atom = None;
                    degree.baseline = 0.;
                }
                let radicand = self.measure(node.children()[1], size);
                let top = 2.;
                let hook = size * 0.65;
                let lead = if square {
                    0.
                } else {
                    degree.size.x.max(hook * 0.45)
                };
                let x = lead + hook;
                let height = radicand.size.y + top;
                result.size = vec2(x + radicand.size.x + 1., height);
                result.baseline = radicand.baseline + top;
                let points = [
                    vec2(lead, height * 0.58),
                    vec2(lead + hook * 0.22, height * 0.52),
                    vec2(lead + hook * 0.43, height - 1.),
                    vec2(x - 1., 0.8),
                    vec2(result.size.x, 0.8),
                ];
                for pair in points.windows(2) {
                    result.primitives.push(Primitive::Line(pair[0], pair[1]));
                }
                result.children.push((vec2(0., 0.), degree));
                result.children.push((vec2(x, top), radicand));
            }
            NodeKind::Call { .. } | NodeKind::Function { .. } => {
                let name = self.measure(node.children()[0], size);
                let mut arguments = vec![];
                for (index, child) in node.children()[1..].iter().enumerate() {
                    if index > 0 {
                        arguments.push(RowPiece::Text(text_galley(painter, ", ", size)));
                    }
                    arguments.push(RowPiece::Child(self.measure(*child, size)));
                }
                let arguments = visual_parentheses(painter, row_layout(id, arguments), size);
                result = row_layout(id, vec![RowPiece::Child(name), RowPiece::Child(arguments)]);
                let (offset, arguments) = result.children.pop().unwrap();
                result.primitives.extend(
                    arguments
                        .primitives
                        .into_iter()
                        .map(|p| translate_primitive(p, offset)),
                );
                result
                    .children
                    .extend(arguments.children.into_iter().map(|(p, b)| (p + offset, b)));
            }
            NodeKind::Range { has_step } => {
                let mut pieces = vec![];
                for (index, child) in node.children().iter().enumerate() {
                    if index > 0 {
                        pieces.push(RowPiece::Text(text_galley(
                            painter,
                            if *has_step && index == 1 {
                                ", "
                            } else {
                                " .. "
                            },
                            size,
                        )));
                    }
                    pieces.push(RowPiece::Child(self.measure(*child, size)));
                }
                result = row_layout(id, pieces);
            }
            NodeKind::Index { .. } => {
                let base = self.measure(node.children()[0], size);
                let mut pieces = vec![];
                for (index, child) in node.children()[1..].iter().enumerate() {
                    if index > 0 {
                        pieces.push(RowPiece::Text(text_galley(
                            painter,
                            ",",
                            typography::script_size(size, 7. / 11.),
                        )));
                    }
                    pieces.push(RowPiece::Child(
                        self.measure(*child, typography::script_size(size, 7. / 11.)),
                    ));
                }
                let index = row_layout(id, pieces);
                let baseline = base.baseline;
                let offset = vec2(base.size.x + 1.0, baseline * 0.63);
                result.size = vec2(
                    offset.x + index.size.x,
                    base.size.y.max(offset.y + index.size.y),
                );
                result.baseline = baseline;
                result.children.push((Vec2::ZERO, base));
                result
                    .primitives
                    .extend(index.primitives.into_iter().map(|p| match p {
                        Primitive::Text(x, g) => Primitive::Text(x + offset, g),
                        other => other,
                    }));
                result
                    .children
                    .extend(index.children.into_iter().map(|(x, b)| (x + offset, b)));
            }
            NodeKind::Matrix { rows, cols } => {
                let cells: Vec<_> = node
                    .children()
                    .iter()
                    .map(|child| self.measure(*child, size))
                    .collect();
                let mut widths = vec![0.0_f32; *cols];
                let mut ascents = vec![0.0_f32; *rows];
                let mut descents = vec![0.0_f32; *rows];
                for (i, cell) in cells.iter().enumerate() {
                    widths[i % cols] = widths[i % cols].max(cell.size.x);
                    ascents[i / cols] = ascents[i / cols].max(cell.baseline);
                    descents[i / cols] = descents[i / cols].max(cell.size.y - cell.baseline);
                }
                let gap = size * 0.4;
                let width = widths.iter().sum::<f32>() + gap * (*cols as f32 - 1.0) + 14.0;
                let height = ascents.iter().sum::<f32>()
                    + descents.iter().sum::<f32>()
                    + gap * (*rows as f32 - 1.0);
                result.size = vec2(width, height);
                result.baseline = height / 2.0 + size * 0.2;
                let mut y = 0.0;
                let mut cells = cells.into_iter();
                for row in 0..*rows {
                    let mut x = 7.0;
                    for width in &widths {
                        let cell = cells.next().unwrap();
                        let offset = vec2(
                            x + (width - cell.size.x) / 2.0,
                            y + ascents[row] - cell.baseline,
                        );
                        result.children.push((offset, cell));
                        x += width + gap;
                    }
                    y += ascents[row] + descents[row] + gap;
                }
                for x in [1.0, width - 1.0] {
                    let direction = if x < width / 2.0 { 1.0 } else { -1.0 };
                    result
                        .primitives
                        .push(Primitive::Line(vec2(x, 0.0), vec2(x, height)));
                    result.primitives.push(Primitive::Line(
                        vec2(x, 0.0),
                        vec2(x + 4.0 * direction, 0.0),
                    ));
                    result.primitives.push(Primitive::Line(
                        vec2(x, height),
                        vec2(x + 4.0 * direction, height),
                    ));
                }
            }
            NodeKind::Fraction => {
                let numerator = self.measure(node.children()[0], size);
                let denominator = self.measure(node.children()[1], size);
                // REA: 2.4 DIP overhang, settings-bound 2 DIP vertical margin.
                // Baseline follows the hyphen ink bottom in the substitute font.
                let metrics = layout_fraction(FractionMetrics {
                    numerator_width: f64::from(numerator.size.x),
                    numerator_height: f64::from(numerator.size.y),
                    denominator_width: f64::from(denominator.size.x),
                    denominator_height: f64::from(denominator.size.y),
                    overhang: 2.4,
                    vertical_pad: 2.,
                    font_baseline_adjustment: {
                        let hyphen = text_galley(painter, "-", size);
                        f64::from(hyphen.mesh_bounds.bottom() - text_baseline(&hyphen))
                    },
                    bar_height: 0.8,
                });
                result.size = vec2(metrics.width as f32, metrics.height as f32);
                result.baseline = metrics.text_baseline as f32;
                let line_y = metrics.bar_origin[1] as f32 + 0.4;
                result.primitives.push(Primitive::Line(
                    vec2(0.0, line_y),
                    vec2(result.size.x, line_y),
                ));
                result.children.push((
                    vec2(
                        metrics.numerator_origin[0] as f32,
                        metrics.numerator_origin[1] as f32,
                    ),
                    numerator,
                ));
                result.children.push((
                    vec2(
                        metrics.denominator_origin[0] as f32,
                        metrics.denominator_origin[1] as f32,
                    ),
                    denominator,
                ));
            }
            NodeKind::Power => {
                let mut base = self.measure(node.children()[0], size);
                if arena.get(node.children()[0]).unwrap().kind().precedence()
                    <= node.kind().precedence()
                {
                    base = visual_parentheses(painter, base, size);
                }
                let exponent =
                    self.measure(node.children()[1], typography::script_size(size, 7. / 11.));
                // Raised script geometry is a project policy; exact Prime metrics unknown.
                let base_y = exponent.size.y * 0.62;
                result.size = vec2(
                    base.size.x + exponent.size.x + 1.0,
                    (base_y + base.size.y).max(exponent.size.y),
                );
                result.baseline = base_y + base.baseline;
                let exponent_x = base.size.x + 1.0;
                result.children.push((vec2(0.0, base_y), base));
                result.children.push((vec2(exponent_x, 0.0), exponent));
            }
            NodeKind::Group => {
                let child = self.measure(node.children()[0], size);
                let width = fence_width(size);
                let height = child.size.y.max(size);
                let offset = vec2(width + 1., (height - child.size.y) / 2.);
                result.size = vec2(child.size.x + 2. * (width + 1.), height);
                result.baseline = child.baseline + offset.y;
                result.primitives = parentheses(result.size.x, height, width);
                result.children.push((offset, child));
            }
            NodeKind::Binary(op) => {
                let mut left = self.measure(node.children()[0], size);
                let mut right = if *op == BinaryOp::Define {
                    self.layout(node.children()[1], size, result_text)
                } else {
                    self.measure(node.children()[1], size)
                };
                let precedence = node.kind().precedence();
                if arena.get(node.children()[0]).unwrap().kind().precedence() < precedence {
                    left = visual_parentheses(painter, left, size);
                }
                let right_precedence = arena.get(node.children()[1]).unwrap().kind().precedence();
                if right_precedence < precedence
                    || (right_precedence == precedence
                        && matches!(op, BinaryOp::Subtract | BinaryOp::InlineDivide))
                {
                    right = visual_parentheses(painter, right, size);
                }
                if *op == BinaryOp::Evaluate
                    && let Some(value) = result_text
                {
                    let value = measure_result(painter, node.children()[1], value, size);
                    match arena.get(node.children()[1]).unwrap().kind() {
                        NodeKind::Placeholder => {
                            right = value;
                        }
                        _ => {
                            let unit_size = right.size;
                            let prefix = value.size.x + size * 0.18;
                            let baseline = right.baseline.max(value.baseline);
                            let value_y = baseline - value.baseline;
                            let unit_y = baseline - right.baseline;
                            for primitive in &mut right.primitives {
                                match primitive {
                                    Primitive::Text(offset, _) => *offset += vec2(prefix, unit_y),
                                    Primitive::Line(a, b) => {
                                        *a += vec2(prefix, unit_y);
                                        *b += vec2(prefix, unit_y);
                                    }
                                    Primitive::Placeholder(rect) => {
                                        *rect = rect.translate(vec2(prefix, unit_y))
                                    }
                                }
                            }
                            for (offset, _) in &mut right.children {
                                *offset += vec2(prefix, unit_y);
                            }
                            right.text_offset += vec2(prefix, unit_y);
                            for primitive in value.primitives {
                                right
                                    .primitives
                                    .push(translate_primitive(primitive, vec2(0.0, value_y)));
                            }
                            right.size = vec2(
                                prefix + unit_size.x,
                                (unit_y + unit_size.y).max(value_y + value.size.y),
                            );
                            right.hit_bounds = Some(Rect::from_min_size(Pos2::ZERO, right.size));
                            right.baseline = baseline;
                        }
                    }
                }
                let symbol = match op {
                    BinaryOp::Add => "+",
                    BinaryOp::Subtract => "−",
                    BinaryOp::Multiply => "·",
                    BinaryOp::Scale => " ",
                    BinaryOp::InlineDivide => "÷",
                    BinaryOp::Define => ":=",
                    BinaryOp::Evaluate => "=",
                };
                let operator = text_galley(painter, symbol, size);
                let operator_baseline = text_baseline(&operator);
                let baseline = left.baseline.max(right.baseline).max(operator_baseline);
                let descent = (left.size.y - left.baseline)
                    .max(right.size.y - right.baseline)
                    .max(operator.size().y - operator_baseline);
                let gap = size * 0.18;
                let operator_x = left.size.x + gap;
                let right_x = operator_x + operator.size().x + gap;
                result.size = vec2(right_x + right.size.x, baseline + descent);
                result.baseline = baseline;
                result.primitives.push(Primitive::Text(
                    vec2(operator_x, baseline - operator_baseline),
                    operator,
                ));
                result
                    .children
                    .push((vec2(0.0, baseline - left.baseline), left));
                result
                    .children
                    .push((vec2(right_x, baseline - right.baseline), right));
            }
        }
        result
    }
}

enum RowPiece {
    Child(FormulaBox),
    Text(Arc<Galley>),
}

fn translate_primitive(primitive: Primitive, by: Vec2) -> Primitive {
    match primitive {
        Primitive::Text(p, g) => Primitive::Text(p + by, g),
        Primitive::Line(a, b) => Primitive::Line(a + by, b + by),
        Primitive::Placeholder(rect) => Primitive::Placeholder(rect.translate(by)),
    }
}

fn empty_display(node: NodeId) -> FormulaBox {
    FormulaBox {
        node,
        size: Vec2::ZERO,
        baseline: 0.,
        primitives: vec![],
        children: vec![],
        atom: None,
        hit_bounds: None,
        text_offset: Vec2::ZERO,
    }
}
fn flatten_display(mut layout: FormulaBox) -> FormulaBox {
    for (offset, child) in std::mem::take(&mut layout.children) {
        let child = flatten_display(child);
        layout.primitives.extend(
            child
                .primitives
                .into_iter()
                .map(|p| translate_primitive(p, offset)),
        );
    }
    layout.atom = None;
    layout
}
fn measure_display_math(
    painter: &Painter,
    node: NodeId,
    math: &crate::result_display::DisplayMath,
    size: f32,
) -> FormulaBox {
    use crate::result_display::DisplayMath;
    let mut result = empty_display(node);
    match math {
        DisplayMath::Text(text, style) => {
            let galley = painter.layout_no_wrap(text.clone(), style.font(size), style.color());
            result.size = galley.size();
            result.baseline = text_baseline(&galley);
            result.primitives.push(Primitive::Text(Vec2::ZERO, galley));
        }
        DisplayMath::Row(parts) => {
            result = row_layout(
                node,
                parts
                    .iter()
                    .map(|p| RowPiece::Child(measure_display_math(painter, node, p, size)))
                    .collect(),
            );
        }
        DisplayMath::Power(base, exp) => {
            let base = measure_display_math(painter, node, base, size);
            let exp =
                measure_display_math(painter, node, exp, typography::script_size(size, 7. / 11.));
            let y = exp.size.y * 0.62;
            result.size = vec2(
                base.size.x + exp.size.x + 1.,
                (base.size.y + y).max(exp.size.y),
            );
            result.baseline = base.baseline + y;
            let x = base.size.x + 1.;
            result.children = vec![(vec2(0., y), base), (vec2(x, 0.), exp)];
        }
        DisplayMath::Subscript(base, index) => {
            let base = measure_display_math(painter, node, base, size);
            let index =
                measure_display_math(painter, node, index, typography::script_size(size, 0.7));
            let x = base.size.x;
            let y = base.baseline * 0.63;
            result.size = vec2(x + index.size.x, base.size.y.max(y + index.size.y));
            result.baseline = base.baseline;
            result.children = vec![(Vec2::ZERO, base), (vec2(x, y), index)];
        }
        DisplayMath::Fraction(top, bottom) => {
            let top = measure_display_math(painter, node, top, size);
            let bottom = measure_display_math(painter, node, bottom, size);
            let hyphen = text_galley(painter, "-", size);
            let metrics = layout_fraction(FractionMetrics {
                numerator_width: f64::from(top.size.x),
                numerator_height: f64::from(top.size.y),
                denominator_width: f64::from(bottom.size.x),
                denominator_height: f64::from(bottom.size.y),
                overhang: 2.4,
                vertical_pad: 2.,
                font_baseline_adjustment: f64::from(
                    hyphen.mesh_bounds.bottom() - text_baseline(&hyphen),
                ),
                bar_height: 0.8,
            });
            result.size = vec2(metrics.width as f32, metrics.height as f32);
            result.baseline = metrics.text_baseline as f32;
            let y = metrics.bar_origin[1] as f32 + 0.4;
            result
                .primitives
                .push(Primitive::Line(vec2(0., y), vec2(result.size.x, y)));
            result.children = vec![
                (
                    vec2(
                        metrics.numerator_origin[0] as f32,
                        metrics.numerator_origin[1] as f32,
                    ),
                    top,
                ),
                (
                    vec2(
                        metrics.denominator_origin[0] as f32,
                        metrics.denominator_origin[1] as f32,
                    ),
                    bottom,
                ),
            ];
        }
        DisplayMath::Group(child) => {
            result = visual_parentheses(
                painter,
                measure_display_math(painter, node, child, size),
                size,
            );
        }
        DisplayMath::Root(child) => {
            let child = measure_display_math(painter, node, child, size);
            let width = size * 0.65;
            let height = child.size.y + 2.;
            result.size = vec2(width + child.size.x + 1., height);
            result.baseline = child.baseline + 2.;
            let points = [
                vec2(0., height * 0.58),
                vec2(width * 0.22, height * 0.52),
                vec2(width * 0.43, height - 1.),
                vec2(width - 1., 0.8),
                vec2(result.size.x, 0.8),
            ];
            for pair in points.windows(2) {
                result.primitives.push(Primitive::Line(pair[0], pair[1]));
            }
            result.children.push((vec2(width, 2.), child));
        }
    }
    flatten_display(result)
}
fn matrix_display(
    painter: &Painter,
    node: NodeId,
    rows: usize,
    cols: usize,
    cells: Vec<FormulaBox>,
    brackets: bool,
    size: f32,
) -> FormulaBox {
    if rows == 0
        || cols == 0
        || rows > 12
        || cols > 12
        || rows.checked_mul(cols) != Some(cells.len())
    {
        return measure_result(painter, node, FormulaResult::Text("…"), size);
    }
    let mut output = empty_display(node);
    let mut widths = vec![0_f32; cols];
    let mut ascents = vec![0_f32; rows];
    let mut descents = vec![0_f32; rows];
    for (i, cell) in cells.iter().enumerate() {
        widths[i % cols] = widths[i % cols].max(cell.size.x);
        ascents[i / cols] = ascents[i / cols].max(cell.baseline);
        descents[i / cols] = descents[i / cols].max(cell.size.y - cell.baseline);
    }
    let gap = size * 0.4;
    let margin = if brackets { 7. } else { 0. };
    let width = widths.iter().sum::<f32>() + gap * (cols as f32 - 1.) + 2. * margin;
    let height =
        ascents.iter().sum::<f32>() + descents.iter().sum::<f32>() + gap * (rows as f32 - 1.);
    output.size = vec2(width, height);
    output.baseline = height / 2. + size * 0.2;
    let mut cells = cells.into_iter();
    let mut y = 0.;
    for row in 0..rows {
        let mut x = margin;
        for width in &widths {
            let cell = cells.next().unwrap();
            let offset = vec2(
                x + (width - cell.size.x) / 2.,
                y + ascents[row] - cell.baseline,
            );
            output.primitives.extend(
                cell.primitives
                    .into_iter()
                    .map(|p| translate_primitive(p, offset)),
            );
            x += width + gap;
        }
        y += ascents[row] + descents[row] + gap;
    }
    if brackets {
        for x in [1., width - 1.] {
            let direction = if x < width / 2. { 1. } else { -1. };
            output
                .primitives
                .push(Primitive::Line(vec2(x, 0.), vec2(x, height)));
            output
                .primitives
                .push(Primitive::Line(vec2(x, 0.), vec2(x + 4. * direction, 0.)));
            output.primitives.push(Primitive::Line(
                vec2(x, height),
                vec2(x + 4. * direction, height),
            ));
        }
    }
    output
}
fn measure_result(
    painter: &Painter,
    node: NodeId,
    result: FormulaResult<'_>,
    size: f32,
) -> FormulaBox {
    use crate::result_display::DisplayMath;
    match result {
        FormulaResult::Text(text) => measure_display_math(
            painter,
            node,
            &DisplayMath::Text(text.into(), MathStyle::Operator),
            size,
        ),
        FormulaResult::Matrix {
            rows,
            cols,
            cells,
            brackets,
        } => matrix_display(
            painter,
            node,
            rows,
            cols,
            cells
                .iter()
                .map(|c| {
                    measure_display_math(
                        painter,
                        node,
                        &DisplayMath::Text(c.clone(), MathStyle::Operator),
                        size,
                    )
                })
                .collect(),
            brackets,
            size,
        ),
        FormulaResult::Formatted(result) => {
            if result.rows == 1 && result.cols == 1 && !result.brackets && result.cells.len() == 1 {
                measure_display_math(painter, node, &result.cells[0], size)
            } else {
                matrix_display(
                    painter,
                    node,
                    result.rows,
                    result.cols,
                    result
                        .cells
                        .iter()
                        .map(|c| measure_display_math(painter, node, c, size))
                        .collect(),
                    result.brackets,
                    size,
                )
            }
        }
    }
}

/// Preserve the AST's meaning when a selected subtree was wrapped by a tighter
/// operator. These are visual parentheses, not an unobserved tree mutation.
fn fence_width(size: f32) -> f32 {
    size * 0.30
}

/// Fixed-width curves stretch vertically, preserving a light stroke for tall math.
/// This is an independent STIX fallback construction, not copied Prime outlines.
fn parentheses(width: f32, height: f32, fence: f32) -> Vec<Primitive> {
    let mut out = Vec::new();
    for right in [false, true] {
        let point = |i: usize| {
            let t = i as f32 / 24.;
            let x = fence * (2. * t - 1.).powi(2);
            vec2(if right { width - x } else { x }, t * height)
        };
        for i in 0..24 {
            out.push(Primitive::Line(point(i), point(i + 1)));
        }
    }
    out
}

fn visual_parentheses(_painter: &Painter, mut child: FormulaBox, size: f32) -> FormulaBox {
    let fence = fence_width(size);
    let height = child.size.y.max(size);
    let offset = vec2(fence + 1., (height - child.size.y) / 2.);
    child.primitives = child
        .primitives
        .into_iter()
        .map(|p| translate_primitive(p, offset))
        .collect();
    for (p, _) in &mut child.children {
        *p += offset;
    }
    child.text_offset += offset;
    child.size = vec2(child.size.x + 2. * (fence + 1.), height);
    child.baseline += offset.y;
    child
        .primitives
        .extend(parentheses(child.size.x, height, fence));
    child
}

fn row_layout(node: NodeId, pieces: Vec<RowPiece>) -> FormulaBox {
    let baseline = pieces
        .iter()
        .map(|piece| match piece {
            RowPiece::Child(child) => child.baseline,
            RowPiece::Text(text) => text_baseline(text),
        })
        .fold(0.0_f32, f32::max);
    let descent = pieces
        .iter()
        .map(|piece| match piece {
            RowPiece::Child(child) => child.size.y - child.baseline,
            RowPiece::Text(text) => text.size().y - text_baseline(text),
        })
        .fold(0.0_f32, f32::max);
    let mut result = FormulaBox {
        node,
        size: vec2(0.0, baseline + descent),
        baseline,
        primitives: vec![],
        children: vec![],
        atom: None,
        hit_bounds: None,
        text_offset: Vec2::ZERO,
    };
    for piece in pieces {
        match piece {
            RowPiece::Child(child) => {
                let width = child.size.x;
                result
                    .children
                    .push((vec2(result.size.x, baseline - child.baseline), child));
                result.size.x += width;
            }
            RowPiece::Text(text) => {
                let width = text.size().x;
                result.primitives.push(Primitive::Text(
                    vec2(result.size.x, baseline - text_baseline(&text)),
                    text,
                ));
                result.size.x += width;
            }
        }
    }
    result
}

fn collect_hits(layout: &FormulaBox, origin: Pos2, hits: &mut Vec<HitNode>) {
    hits.push(HitNode {
        node: layout.node,
        rect: layout.hit_bounds.map_or_else(
            || Rect::from_min_size(origin, layout.size),
            |bounds| bounds.translate(origin.to_vec2()),
        ),
        atom: layout.atom.clone(),
        text_origin: origin + layout.text_offset,
    });
    for (offset, child) in &layout.children {
        collect_hits(child, origin + *offset, hits);
    }
}

fn hit_node(hits: &[HitNode], point: Pos2) -> Option<&HitNode> {
    hits.iter()
        .filter(|hit| hit.rect.expand(2.0).contains(point))
        .min_by(|left, right| left.rect.area().total_cmp(&right.rect.area()))
}

fn hit_text_offset(hit: &HitNode, point: Pos2) -> usize {
    hit.atom.as_ref().map_or(0, |galley| {
        let cursor = galley.cursor_from_pos(point - hit.text_origin);
        galley
            .text()
            .chars()
            .take(cursor.index.into())
            .map(char::len_utf16)
            .sum()
    })
}

fn node_contains(arena: &Arena, ancestor: NodeId, mut node: NodeId) -> bool {
    loop {
        if ancestor == node {
            return true;
        }
        let Some(parent) = arena.get(node).and_then(|n| n.parent()) else {
            return false;
        };
        node = parent;
    }
}

fn text_caret_rect(hit: &HitNode, offset: usize) -> Rect {
    let Some(galley) = &hit.atom else {
        return hit.rect;
    };
    let mut units = 0;
    let index = galley
        .text()
        .chars()
        .take_while(|ch| {
            let take = units < offset;
            units += ch.len_utf16();
            take
        })
        .count();
    galley
        .pos_from_cursor(egui::text::CCursor::new(index))
        .translate(hit.text_origin.to_vec2())
}

fn caret_rect(hit: &HitNode, editor: &Editor) -> Rect {
    if matches!(editor.selection(), Selection::Subtree(id) if id == hit.node) {
        let x = if editor.scope_direction() == Direction::Left {
            hit.rect.left()
        } else {
            hit.rect.right()
        };
        return Rect::from_min_max(pos2(x, hit.rect.top()), pos2(x + 1., hit.rect.bottom()));
    }
    if let Some(range) = editor.caret().value_range {
        return text_caret_rect(hit, range.focus);
    }
    if let Some(galley) = &hit.atom {
        return text_caret_rect(
            hit,
            if editor.caret().caret_side == Direction::Left {
                0
            } else {
                galley.text().encode_utf16().count()
            },
        );
    }
    let x = if editor.caret().caret_side == Direction::Left {
        hit.rect.left()
    } else {
        hit.rect.right()
    };
    Rect::from_min_max(pos2(x, hit.rect.top()), pos2(x + 1., hit.rect.bottom()))
}

fn move_vertical(editor: &mut Editor, hits: &[HitNode], down: bool) {
    let Some(here) = hits.iter().find(|h| h.node == editor.caret().node) else {
        return;
    };
    let caret = caret_rect(here, editor);
    let mut leaves = Vec::new();
    self::leaves(editor.arena(), editor.root(), &mut leaves);
    let candidate = hits
        .iter()
        .filter(|h| {
            h.node != here.node
                && leaves.contains(&h.node)
                && if down {
                    h.rect.center().y > here.rect.center().y + 3.
                } else {
                    h.rect.center().y < here.rect.center().y - 3.
                }
        })
        .min_by(|a, b| {
            let score = |h: &HitNode| {
                let dx = if caret.center().x < h.rect.left() {
                    h.rect.left() - caret.center().x
                } else if caret.center().x > h.rect.right() {
                    caret.center().x - h.rect.right()
                } else {
                    0.
                };
                (h.rect.center().y - here.rect.center().y).abs() + dx * 2.
            };
            score(a).total_cmp(&score(b))
        });
    if let Some(hit) = candidate {
        if hit.atom.is_some() {
            let offset = hit_text_offset(hit, pos2(caret.center().x, hit.rect.center().y));
            let _ = editor.place_text_caret(hit.node, offset);
        } else {
            let _ = editor.place_caret(hit.node, Direction::Right);
        }
    }
}

fn extend_text_selection(editor: &mut Editor, forward: bool) {
    let node = editor.caret().node;
    let Some(text) = atomic_text(editor, node) else {
        return;
    };
    let range = editor.caret().value_range;
    let offset = range.map_or_else(
        || {
            if editor.caret().caret_side == Direction::Left {
                0
            } else {
                text.encode_utf16().count()
            }
        },
        |r| r.focus,
    );
    let Some(byte) = byte_at_utf16(&text, offset) else {
        return;
    };
    let next = if forward {
        text[byte..]
            .chars()
            .next()
            .map(|ch| offset + ch.len_utf16())
    } else {
        text[..byte]
            .chars()
            .next_back()
            .map(|ch| offset - ch.len_utf16())
    };
    if let Some(focus) = next {
        let _ = editor.place_text_selection(node, range.map_or(offset, |r| r.anchor), focus);
    }
}

fn paint_box(painter: &Painter, layout: &FormulaBox, origin: Pos2) {
    for primitive in &layout.primitives {
        match primitive {
            Primitive::Text(offset, galley) => {
                painter.galley(origin + *offset, galley.clone(), INK);
            }
            Primitive::Line(start, end) => {
                painter.line_segment([origin + *start, origin + *end], Stroke::new(0.8, INK));
            }
            Primitive::Placeholder(rect) => {
                painter.rect_stroke(
                    rect.translate(origin.to_vec2()),
                    0.0,
                    Stroke::new(1.0, CARET),
                    StrokeKind::Inside,
                );
            }
        }
    }
    for (offset, child) in &layout.children {
        paint_box(painter, child, origin + *offset);
    }
}

fn paint_caret(painter: &Painter, hit: &HitNode, editor: &Editor) {
    let caret_rect = caret_rect(hit, editor);
    let x = caret_rect.left();
    painter.line_segment(
        [pos2(x, caret_rect.top()), pos2(x, caret_rect.bottom())],
        Stroke::new(1.5, CARET),
    );
    painter.line_segment(
        [
            pos2(x, caret_rect.bottom()),
            pos2(x + 5.0, caret_rect.bottom()),
        ],
        Stroke::new(1.5, CARET),
    );
}

pub(crate) fn input_text(editor: &mut Editor, text: &str) -> bool {
    input_text_with_actions(editor, text).document_changed
}

pub(crate) fn input_text_with_actions(editor: &mut Editor, text: &str) -> InputOutcome {
    // One committed text event may require several tree edits (e.g. 2m).
    // Keep the user command intact when undoing those internal operations.
    editor.edit_group(|editor| input_text_operations(editor, text))
}

fn input_text_operations(editor: &mut Editor, text: &str) -> InputOutcome {
    let mut changed = false;
    let mut evaluation_requested = false;
    for ch in text.chars() {
        let current = editor.caret().node;
        let kind = editor.arena().get(current).unwrap().kind().clone();
        match ch {
            '/' => {
                editor.insert_fraction_infix();
                changed = true;
            }
            '^' => {
                editor.insert_power_infix();
                changed = true;
            }
            '(' => {
                match atomic_text(editor, current).as_deref() {
                    Some("sqrt") => {
                        editor.edit_group(|editor| {
                            let _ = editor.set_atom(current, NodeKind::Placeholder);
                            editor.insert_root();
                        });
                    }
                    Some("abs") => {
                        editor.edit_group(|editor| {
                            let _ = editor.set_atom(current, NodeKind::Placeholder);
                            editor.insert_absolute();
                        });
                    }
                    _ if matches!(
                        kind,
                        NodeKind::Identifier(_)
                            | NodeKind::AnnotatedIdentifier { .. }
                            | NodeKind::NativeIdentifier { .. }
                    ) =>
                    {
                        editor.insert_call();
                    }
                    _ => {
                        editor.insert_group();
                    }
                }
                changed = true;
            }
            ')' | ']' => {
                editor.exit_group_right();
            }
            '[' => {
                if kind == NodeKind::Placeholder {
                    editor.insert_matrix(1, 1);
                } else {
                    editor.insert_index();
                }
                changed = true;
            }
            ' ' => {
                editor.expand_scope();
            }
            '+' => {
                editor.insert_infix(BinaryOp::Add);
                changed = true;
            }
            '-' if matches!(&kind,NodeKind::Number(value) if value.ends_with('e') || value.ends_with('E')) => {
                changed |= insert_character(editor, ch)
            }
            '-' => {
                if kind == NodeKind::Placeholder {
                    editor.insert_unary_neg();
                } else {
                    editor.insert_infix(BinaryOp::Subtract);
                }
                changed = true;
            }
            '*' => {
                editor.insert_infix(BinaryOp::Multiply);
                changed = true;
            }
            ':' => changed |= insert_root_command(editor, BinaryOp::Define),
            '=' => {
                let outcome = process_equals(editor);
                changed |= outcome.document_changed;
                evaluation_requested |= outcome.evaluation_requested;
            }
            ',' => {
                if !editor.append_argument() {
                    editor.insert_step_range();
                }
                changed = true;
            }
            ';' => changed |= editor.append_matrix_row(),
            '\\' | '√' => {
                editor.insert_root();
                changed = true;
            }
            '|' => {
                if ancestor_kind(editor, current, |kind| matches!(kind, NodeKind::Absolute))
                    .is_some()
                {
                    editor.exit_group_right();
                } else {
                    editor.insert_absolute();
                    changed = true;
                }
            }
            '.' if matches!(&kind,NodeKind::Number(value) if value.ends_with('.')) => {
                if let NodeKind::Number(mut value) = kind {
                    value.pop();
                    let _ = editor.set_atom(current, NodeKind::Number(value));
                }
                if let Some(range) = ancestor_kind(editor, current, |kind| {
                    matches!(kind, NodeKind::Range { has_step: true })
                }) {
                    let end = editor.arena().get(range).unwrap().children()[2];
                    let _ = editor.place_caret(end, Direction::Indeterminate);
                } else {
                    editor.insert_range();
                }
                changed = true;
            }
            '\r' | '\n' | '\t' => {}
            ch if !ch.is_control() => {
                if editor.selection() != Selection::None
                    || editor
                        .caret()
                        .value_range
                        .is_some_and(|r| r.anchor != r.focus)
                {
                    changed |= insert_character(editor, ch);
                    continue;
                }
                if let NodeKind::Number(value) = &kind {
                    if matches!(ch, 'i' | 'j') {
                        let _ = editor.set_atom(
                            current,
                            NodeKind::ImaginarySymbol {
                                value: value.clone(),
                                symbol: ch.to_string(),
                            },
                        );
                        changed = true;
                        continue;
                    }
                    if ch.is_alphabetic() && !matches!(ch, 'e' | 'E') {
                        editor.insert_infix(BinaryOp::Multiply);
                    }
                } else if !matches!(
                    kind,
                    NodeKind::Placeholder
                        | NodeKind::Identifier(_)
                        | NodeKind::AnnotatedIdentifier { .. }
                        | NodeKind::NativeIdentifier { .. }
                        | NodeKind::Imaginary(_)
                        | NodeKind::ImaginarySymbol { .. }
                ) && editor.caret().caret_side == Direction::Right
                {
                    editor.insert_infix(BinaryOp::Multiply);
                }
                changed |= insert_character(editor, ch);
            }
            _ => {}
        }
    }
    InputOutcome {
        document_changed: changed,
        evaluation_requested,
    }
}

fn ancestor_kind(
    editor: &Editor,
    mut node: NodeId,
    predicate: impl Fn(&NodeKind) -> bool,
) -> Option<NodeId> {
    loop {
        let value = editor.arena().get(node)?;
        if predicate(value.kind()) {
            return Some(node);
        }
        node = value.parent()?;
    }
}

/// Bounded definition shortcut; the separate '=' command handles evaluation.
fn insert_root_command(editor: &mut Editor, operation: BinaryOp) -> bool {
    match editor.arena().get(editor.root()).unwrap().kind() {
        NodeKind::Binary(BinaryOp::Define | BinaryOp::Evaluate) => false,
        _ => {
            if operation == BinaryOp::Define {
                editor.insert_definition();
            } else {
                editor.insert_evaluation();
            }
            true
        }
    }
}

/// REA get_EvaluationRoot recognizes both a root evaluation and the evaluation
/// on a definition's RHS. Local range binding and expression-only contexts are
/// not represented by the current portable editor and remain outside this path.
fn process_equals(editor: &mut Editor) -> InputOutcome {
    let root = editor.arena().get(editor.root()).unwrap();
    let target = if root.kind() == &NodeKind::Binary(BinaryOp::Define) {
        root.children()[1]
    } else {
        editor.root()
    };
    let node = editor.arena().get(target).unwrap();
    if node.kind() == &NodeKind::Binary(BinaryOp::Evaluate) {
        return InputOutcome {
            document_changed: false,
            evaluation_requested: true,
        };
    }
    // ':' already created the definition; the '=' in ':=' must not evaluate
    // its still-empty RHS. This is the bounded text-entry shortcut policy.
    if target != editor.root() && node.kind() == &NodeKind::Placeholder {
        return InputOutcome::default();
    }
    editor.insert_evaluation();
    InputOutcome {
        document_changed: true,
        evaluation_requested: true,
    }
}

const TREE_CLIPBOARD_PREFIX: &str = "mathcraft-tree:";

fn copy_tree(editor: &Editor) -> Option<String> {
    if editor.selection() == Selection::None
        && let Some(range) = editor.caret().value_range
        && range.anchor != range.focus
    {
        let text = atomic_text(editor, editor.caret().node)?;
        return Some(
            text[byte_at_utf16(&text, range.anchor.min(range.focus))?
                ..byte_at_utf16(&text, range.anchor.max(range.focus))?]
                .to_owned(),
        );
    }
    let target = match editor.selection() {
        Selection::None => editor.caret().node,
        Selection::Subtree(id) => id,
    };
    let tree = editor.export_subtree(target).ok()?;
    Some(format!(
        "{TREE_CLIPBOARD_PREFIX}{}",
        serde_json::to_string(&tree).ok()?
    ))
}

#[cfg(test)]
fn paste_tree_or_text(editor: &mut Editor, text: &str) -> bool {
    paste_tree_or_text_with_actions(editor, text).document_changed
}

fn paste_tree_or_text_with_actions(editor: &mut Editor, text: &str) -> InputOutcome {
    if let Some(json) = text.strip_prefix(TREE_CLIPBOARD_PREFIX) {
        if json.len() > 8 * 1024 * 1024 {
            return InputOutcome::default();
        }
        let Ok(tree) = serde_json::from_str::<PortableTree>(json) else {
            return InputOutcome::default();
        };
        InputOutcome {
            document_changed: editor.replace_selection(&tree).is_ok(),
            evaluation_requested: false,
        }
    } else {
        // The original builds one compound Paste command. Text can expand to
        // several structural mutations, but undo must restore the whole paste.
        editor.edit_group(|editor| input_text_with_actions(editor, text))
    }
}

fn atomic_text(editor: &Editor, node: NodeId) -> Option<String> {
    match editor.arena().get(node)?.kind() {
        NodeKind::Number(text) | NodeKind::Identifier(text) | NodeKind::Imaginary(text) => {
            Some(text.clone())
        }
        NodeKind::NativeIdentifier { name, .. } => Some(name.clone()),
        NodeKind::ImaginarySymbol { value, .. } => Some(value.clone()),
        NodeKind::AnnotatedIdentifier { name, .. } => Some(name.clone()),
        NodeKind::Placeholder => Some(String::new()),
        _ => None,
    }
}

fn text_kind(text: String) -> NodeKind {
    if text.is_empty() {
        NodeKind::Placeholder
    } else if text
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_digit() || ch == '.')
        && text
            .chars()
            .all(|ch| ch.is_ascii_digit() || matches!(ch, '.' | 'e' | 'E' | '+' | '-'))
    {
        NodeKind::Number(text)
    } else {
        NodeKind::Identifier(text)
    }
}

fn edited_text_kind(editor: &Editor, target: NodeId, text: String) -> NodeKind {
    match editor.arena().get(target).map(|node| node.kind()) {
        Some(NodeKind::NativeIdentifier {
            labels,
            contextual,
            subscript,
            ..
        }) => {
            return NodeKind::NativeIdentifier {
                name: text,
                labels: labels.clone(),
                contextual: *contextual,
                subscript: subscript.clone(),
            };
        }
        Some(NodeKind::Imaginary(_)) => return NodeKind::Imaginary(text),
        Some(NodeKind::ImaginarySymbol { symbol, .. }) => {
            return NodeKind::ImaginarySymbol {
                value: text,
                symbol: symbol.clone(),
            };
        }
        _ => {}
    }

    if let Some(NodeKind::AnnotatedIdentifier { labels, .. }) =
        editor.arena().get(target).map(|node| node.kind())
    {
        NodeKind::AnnotatedIdentifier {
            name: text,
            labels: labels.clone(),
        }
    } else {
        text_kind(text)
    }
}

fn byte_at_utf16(text: &str, offset: usize) -> Option<usize> {
    let mut units = 0;
    for (byte, ch) in text.char_indices() {
        if units == offset {
            return Some(byte);
        }
        units += ch.len_utf16();
    }
    (units == offset).then_some(text.len())
}

fn insert_character(editor: &mut Editor, ch: char) -> bool {
    let target = match editor.selection() {
        Selection::Subtree(node) => node,
        Selection::None => editor.caret().node,
    };
    let Some(mut text) = atomic_text(editor, target) else {
        if editor.selection() == Selection::Subtree(target) {
            return editor
                .replace_selection(&PortableTree {
                    kind: text_kind(ch.to_string()),
                    children: vec![],
                })
                .is_ok();
        }
        return false;
    };
    let replace = editor.selection() == Selection::Subtree(target);
    let offset = if replace {
        0
    } else {
        editor.caret().value_range.map_or_else(
            || {
                if editor.caret().caret_side == Direction::Left {
                    0
                } else {
                    text.encode_utf16().count()
                }
            },
            |range| range.focus,
        )
    };
    if replace {
        text.clear();
    }
    let offset = if !replace
        && let Some(range) = editor.caret().value_range
        && range.anchor != range.focus
    {
        let start = range.anchor.min(range.focus);
        let end = range.anchor.max(range.focus);
        let (Some(a), Some(b)) = (byte_at_utf16(&text, start), byte_at_utf16(&text, end)) else {
            return false;
        };
        text.replace_range(a..b, "");
        start
    } else {
        offset
    };
    let Some(byte) = byte_at_utf16(&text, offset) else {
        return false;
    };
    text.insert(byte, ch);
    let kind = edited_text_kind(editor, target, text);
    if editor.set_atom(target, kind).is_err() {
        return false;
    }
    let _ = editor.place_text_caret(target, offset + ch.len_utf16());
    true
}

fn delete_character(editor: &mut Editor, backwards: bool) -> bool {
    let target = match editor.selection() {
        Selection::Subtree(node) => node,
        Selection::None => editor.caret().node,
    };
    if editor.selection() != Selection::None {
        return editor.delete_structure(backwards);
    }
    let Some(mut text) = atomic_text(editor, target) else {
        return editor.delete_structure(backwards);
    };
    if let Some(range) = editor.caret().value_range
        && range.anchor != range.focus
    {
        let start = range.anchor.min(range.focus);
        let end = range.anchor.max(range.focus);
        let (Some(a), Some(b)) = (byte_at_utf16(&text, start), byte_at_utf16(&text, end)) else {
            return false;
        };
        text.replace_range(a..b, "");
        let kind = edited_text_kind(editor, target, text);
        if editor.set_atom(target, kind).is_err() {
            return false;
        }
        let _ = editor.place_text_caret(target, start);
        return true;
    }
    if editor.selection() == Selection::Subtree(target) {
        let kind = edited_text_kind(editor, target, String::new());
        return editor.set_atom(target, kind).is_ok();
    }
    let offset = editor.caret().value_range.map_or_else(
        || {
            if editor.caret().caret_side == Direction::Left {
                0
            } else {
                text.encode_utf16().count()
            }
        },
        |range| range.focus,
    );
    let Some(byte) = byte_at_utf16(&text, offset) else {
        return false;
    };
    let (start, end) = if backwards {
        let Some((start, _)) = text[..byte].char_indices().next_back() else {
            return editor.delete_structure(backwards);
        };
        (start, byte)
    } else {
        let Some(ch) = text[byte..].chars().next() else {
            return editor.delete_structure(backwards);
        };
        (byte, byte + ch.len_utf8())
    };
    let next_offset = text[..start].encode_utf16().count();
    text.replace_range(start..end, "");
    let kind = edited_text_kind(editor, target, text);
    if editor.set_atom(target, kind).is_err() {
        return false;
    }
    let _ = editor.place_text_caret(target, next_offset);
    true
}

fn delete_surrounding(editor: &mut Editor, before: usize, after: usize) -> bool {
    if editor.selection() != Selection::None {
        return false;
    }
    let node = editor.caret().node;
    let Some(text) = atomic_text(editor, node) else {
        return false;
    };
    let focus = editor.caret().value_range.map_or_else(
        || {
            if editor.caret().caret_side == Direction::Left {
                0
            } else {
                text.encode_utf16().count()
            }
        },
        |r| r.focus,
    );
    let anchor = editor.caret().value_range.map_or(focus, |r| r.anchor);
    let (Some(a), Some(b)) = (
        byte_at_utf16(&text, anchor.min(focus)),
        byte_at_utf16(&text, anchor.max(focus)),
    ) else {
        return false;
    };
    let chars: Vec<_> = text.chars().collect();
    let start = text[..a].chars().count().saturating_sub(before);
    let end = text[..b]
        .chars()
        .count()
        .saturating_add(after)
        .min(chars.len());
    if start == end {
        return false;
    }
    let start_utf16 = chars[..start].iter().map(|ch| ch.len_utf16()).sum();
    let end_utf16 = chars[..end].iter().map(|ch| ch.len_utf16()).sum();
    if editor
        .place_text_selection(node, start_utf16, end_utf16)
        .is_err()
    {
        return false;
    }
    delete_character(editor, true)
}

fn leaves(arena: &Arena, node: NodeId, out: &mut Vec<NodeId>) {
    let value = arena.get(node).unwrap();
    if value.children().is_empty() {
        out.push(node);
    }
    for child in value.children() {
        leaves(arena, *child, out);
    }
}

fn cycle_leaf(editor: &mut Editor, forward: bool, placeholders_first: bool) {
    let mut nodes = vec![];
    leaves(editor.arena(), editor.root(), &mut nodes);
    let here = nodes
        .iter()
        .position(|id| *id == editor.caret().node)
        .unwrap_or(0);
    for step in 1..=nodes.len() {
        let index = if forward {
            (here + step) % nodes.len()
        } else {
            (here + nodes.len() - step % nodes.len()) % nodes.len()
        };
        let id = nodes[index];
        if !placeholders_first || editor.arena().get(id).unwrap().kind() == &NodeKind::Placeholder {
            let _ = editor.place_caret(
                id,
                if forward {
                    Direction::Left
                } else {
                    Direction::Right
                },
            );
            return;
        }
    }
    if placeholders_first {
        cycle_leaf(editor, forward, false);
    }
}

fn atom_extreme(editor: &mut Editor, end: bool) {
    let node = editor.caret().node;
    if let Some(text) = atomic_text(editor, node) {
        let _ = editor.place_text_caret(node, if end { text.encode_utf16().count() } else { 0 });
    }
}

fn move_horizontal(editor: &mut Editor, forward: bool) {
    if let Some(range) = editor.caret().value_range
        && range.anchor != range.focus
    {
        let offset = if forward {
            range.anchor.max(range.focus)
        } else {
            range.anchor.min(range.focus)
        };
        let _ = editor.place_text_caret(editor.caret().node, offset);
        return;
    }
    if let Selection::Subtree(node) = editor.selection() {
        let _ = editor.place_caret(
            node,
            if forward {
                Direction::Right
            } else {
                Direction::Left
            },
        );
        return;
    }
    let node = editor.caret().node;
    if let Some(text) = atomic_text(editor, node) {
        let offset = editor.caret().value_range.map_or_else(
            || {
                if editor.caret().caret_side == Direction::Left {
                    0
                } else {
                    text.encode_utf16().count()
                }
            },
            |range| range.focus,
        );
        if let Some(byte) = byte_at_utf16(&text, offset) {
            let next = if forward {
                text[byte..]
                    .chars()
                    .next()
                    .map(|ch| offset + ch.len_utf16())
            } else {
                text[..byte]
                    .chars()
                    .next_back()
                    .map(|ch| offset - ch.len_utf16())
            };
            if let Some(next) = next {
                let _ = editor.place_text_caret(node, next);
                return;
            }
        }
    } else {
        let side = editor.caret().caret_side;
        if (forward && side == Direction::Left) || (!forward && side == Direction::Right) {
            let mut leaf = node;
            loop {
                let children = editor.arena().get(leaf).unwrap().children();
                if children.is_empty() {
                    break;
                }
                leaf = if forward {
                    children[0]
                } else {
                    *children.last().unwrap()
                };
            }
            let _ = editor.place_caret(
                leaf,
                if forward {
                    Direction::Left
                } else {
                    Direction::Right
                },
            );
            return;
        }
    }
    if let Some(parent) = editor.arena().get(node).unwrap().parent() {
        let children = editor.arena().get(parent).unwrap().children();
        let index = children.iter().position(|child| *child == node).unwrap();
        let sibling = if forward {
            children.get(index + 1).copied()
        } else {
            index.checked_sub(1).map(|i| children[i])
        };
        if let Some(mut leaf) = sibling {
            loop {
                let children = editor.arena().get(leaf).unwrap().children();
                if children.is_empty() {
                    break;
                }
                leaf = if forward {
                    children[0]
                } else {
                    *children.last().unwrap()
                };
            }
            let _ = editor.place_caret(
                leaf,
                if forward {
                    Direction::Left
                } else {
                    Direction::Right
                },
            );
        } else {
            let _ = editor.place_caret(
                parent,
                if forward {
                    Direction::Right
                } else {
                    Direction::Left
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_selection_replaces_copies_and_deletes_without_splitting_unicode() {
        let mut editor = Editor::new();
        input_text(&mut editor, "a😀bc");
        let id = editor.root();
        editor.place_text_caret(id, 5).unwrap();
        for _ in 0..3 {
            extend_text_selection(&mut editor, false);
        }
        assert_eq!(copy_tree(&editor).as_deref(), Some("😀bc"));
        assert!(input_text(&mut editor, "한"));
        assert_eq!(atomic_text(&editor, id).as_deref(), Some("a한"));
        assert!(editor.undo());
        assert_eq!(editor.caret().value_range.unwrap().focus, 1);
        assert!(delete_character(&mut editor, false));
        assert_eq!(atomic_text(&editor, id).as_deref(), Some("a"));
        assert!(editor.undo());
        move_horizontal(&mut editor, true);
        assert_eq!(editor.caret().value_range.unwrap().focus, 5);
        assert_eq!(editor.caret().value_range.unwrap().anchor, 5);
    }

    #[test]
    fn pointer_drag_selects_text_without_editing_the_expression() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut editor = typed_one_character_at_a_time("hello");
        let before = editor.to_portable().unwrap();
        let mut points = [Pos2::ZERO; 2];
        let output = context.run_ui(egui::RawInput::default(), |ui| {
            let response = formula_ui(ui, &mut editor);
            let layout = measure(ui.painter(), editor.arena(), editor.root(), FONT_SIZE);
            let mut hits = vec![];
            collect_hits(
                &layout,
                response.rect.min + vec2(PADDING, PADDING),
                &mut hits,
            );
            let atom = &hits[0];
            for (i, offset) in [1, 4].into_iter().enumerate() {
                points[i] = text_caret_rect(atom, offset).center();
            }
        });
        output.drop_without_applying_deltas();
        for events in [
            vec![
                egui::Event::PointerMoved(points[0]),
                egui::Event::PointerButton {
                    pos: points[0],
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            vec![egui::Event::PointerMoved(points[1])],
            vec![egui::Event::PointerButton {
                pos: points[1],
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        ] {
            let output = context.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    assert!(!formula_ui(ui, &mut editor).changed());
                },
            );
            output.drop_without_applying_deltas();
        }
        assert_eq!(editor.to_portable().unwrap(), before);
        assert_eq!(copy_tree(&editor).as_deref(), Some("ell"));
    }

    #[test]
    fn vertical_navigation_follows_fraction_and_matrix_geometry() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut fraction = typed_one_character_at_a_time("12/34");
        let root = fraction.root();
        let children = fraction.arena().get(root).unwrap().children().to_vec();
        let mut matrix = typed_one_character_at_a_time("[1,2;3,4]");
        let matrix_root = matrix.root();
        let cells = matrix.arena().get(matrix_root).unwrap().children().to_vec();
        let output = context.run_ui(egui::RawInput::default(), |ui| {
            let layout = measure(ui.painter(), fraction.arena(), root, FONT_SIZE);
            let mut hits = vec![];
            collect_hits(&layout, pos2(0., 0.), &mut hits);
            move_vertical(&mut fraction, &hits, false);
            assert_eq!(fraction.caret().node, children[0]);
            move_vertical(&mut fraction, &hits, true);
            assert_eq!(fraction.caret().node, children[1]);
            let layout = measure(ui.painter(), matrix.arena(), matrix_root, FONT_SIZE);
            hits.clear();
            collect_hits(&layout, pos2(0., 0.), &mut hits);
            matrix.place_text_caret(cells[0], 1).unwrap();
            move_vertical(&mut matrix, &hits, true);
            assert_eq!(matrix.caret().node, cells[2]);
            move_vertical(&mut matrix, &hits, false);
            assert_eq!(matrix.caret().node, cells[0]);
        });
        output.drop_without_applying_deltas();
    }

    #[test]
    fn ime_surrounding_deletion_stays_inside_the_current_identifier() {
        let mut editor = typed_one_character_at_a_time("7+abc");
        let before = editor.to_portable().unwrap();
        let root = editor.root();
        let left = editor.arena().get(root).unwrap().children()[0];
        assert!(delete_surrounding(&mut editor, usize::MAX, usize::MAX));
        assert_eq!(
            editor.arena().get(left).unwrap().kind(),
            &NodeKind::Number("7".into())
        );
        assert_eq!(editor.root(), root);
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), before);
    }

    #[test]
    fn ime_preedit_is_transient_and_commit_is_one_undo_command() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut editor = typed_one_character_at_a_time("x");
        let before = editor.to_portable().unwrap();
        let output = context.run_ui(egui::RawInput::default(), |ui| {
            formula_ui_with_focus(ui, &mut editor, None, true);
        });
        output.drop_without_applying_deltas();
        let output = context.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Ime(egui::ImeEvent::Preedit {
                    text: "한".into(),
                    active_range_chars: Some(0..1),
                })],
                ..Default::default()
            },
            |ui| {
                assert!(!formula_ui(ui, &mut editor).changed());
            },
        );
        assert!(output.platform_output.ime.is_some());
        output.drop_without_applying_deltas();
        assert_eq!(editor.to_portable().unwrap(), before);
        let output = context.run_ui(
            egui::RawInput {
                events: vec![
                    egui::Event::Ime(egui::ImeEvent::Commit("한글".into())),
                    egui::Event::Key {
                        key: Key::Enter,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..Default::default()
            },
            |ui| {
                assert!(formula_ui(ui, &mut editor).changed());
                assert!(!ui.input(|i| i.key_pressed(Key::Enter)));
            },
        );
        output.drop_without_applying_deltas();
        assert_eq!(
            atomic_text(&editor, editor.root()).as_deref(),
            Some("x한글")
        );
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), before);
    }

    #[test]
    fn text_input_edits_ast_operands_and_keeps_unicode_boundaries() {
        let mut editor = Editor::new();
        assert!(input_text(&mut editor, "a/2^3"));
        let fraction = editor.arena().get(editor.root()).unwrap();
        assert_eq!(fraction.kind(), &NodeKind::Fraction);
        let power = fraction.children()[1];
        assert_eq!(editor.arena().get(power).unwrap().kind(), &NodeKind::Power);
        assert_eq!(
            editor.arena().get(editor.caret().node).unwrap().kind(),
            &NodeKind::Number("3".into())
        );
        let mut unicode = Editor::new();
        input_text(&mut unicode, "a𝛼b");
        move_horizontal(&mut unicode, false);
        assert!(delete_character(&mut unicode, true));
        assert_eq!(atomic_text(&unicode, unicode.root()).unwrap(), "ab");
        assert_eq!(unicode.validate(), Ok(()));
        assert_eq!(editor.validate(), Ok(()));
    }

    #[test]
    fn headless_geometry_separates_fraction_operands_and_raises_exponent() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut editor = Editor::new();
        input_text(&mut editor, "1/2^3");
        let output = context.run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let layout = measure(ui.painter(), editor.arena(), editor.root(), FONT_SIZE);
                assert_eq!(layout.children.len(), 2);
                let (numerator_origin, numerator) = &layout.children[0];
                let (denominator_origin, power) = &layout.children[1];
                assert!(denominator_origin.y > numerator_origin.y + numerator.size.y);
                assert!(power.children[1].0.y < power.children[0].0.y);
                assert!(power.children[1].1.size.y < power.children[0].1.size.y);
                let mut hits = vec![];
                collect_hits(&layout, pos2(0.0, 0.0), &mut hits);
                let numerator_hit =
                    hit_node(&hits, numerator_origin.to_pos2() + numerator.size / 2.0).unwrap();
                assert_eq!(numerator_hit.node, numerator.node);
                formula_ui(ui, &mut editor);
            });
        });
        let meshes = context.tessellate(output.shapes.clone(), output.pixels_per_point);
        output.drop_without_applying_deltas();
        assert!(!meshes.is_empty());
    }

    #[test]
    fn focused_widget_handles_text_in_a_headless_frame() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut editor = Editor::new();
        let first_frame = context.run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                formula_ui(ui, &mut editor).request_focus();
            });
        });
        first_frame.drop_without_applying_deltas();
        let input = egui::RawInput {
            events: vec![egui::Event::Text("4/5".into())],
            ..Default::default()
        };
        let second_frame = context.run_ui(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                formula_ui(ui, &mut editor);
            });
        });
        second_frame.drop_without_applying_deltas();
        assert_eq!(
            editor.arena().get(editor.root()).unwrap().kind(),
            &NodeKind::Fraction
        );
        assert_eq!(atomic_text(&editor, editor.caret().node).unwrap(), "5");
        let tab = egui::RawInput {
            events: vec![egui::Event::Key {
                key: Key::Tab,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::default(),
            }],
            ..Default::default()
        };
        let third_frame = context.run_ui(tab, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(formula_ui(ui, &mut editor).has_focus());
            });
        });
        third_frame.drop_without_applying_deltas();
        assert_eq!(atomic_text(&editor, editor.caret().node).unwrap(), "4");
    }

    #[test]
    fn root_definition_and_evaluation_keys_do_not_duplicate_operators() {
        let mut definition = Editor::new();
        input_text(&mut definition, "x:=2");
        let root = definition.root();
        let node = definition.arena().get(root).unwrap();
        assert_eq!(node.kind(), &NodeKind::Binary(BinaryOp::Define));
        assert_eq!(atomic_text(&definition, node.children()[0]).unwrap(), "x");
        assert_eq!(atomic_text(&definition, node.children()[1]).unwrap(), "2");
        assert!(input_text(&mut definition, "="));
        assert_eq!(definition.root(), root);
        let rhs = definition.arena().get(root).unwrap().children()[1];
        assert_eq!(
            definition.arena().get(rhs).unwrap().kind(),
            &NodeKind::Binary(BinaryOp::Evaluate)
        );
        assert!(!input_text(&mut definition, "="));
        let mut evaluation = Editor::new();
        input_text(&mut evaluation, "1+2=");
        let root = evaluation.root();
        let node = evaluation.arena().get(root).unwrap();
        assert_eq!(node.kind(), &NodeKind::Binary(BinaryOp::Evaluate));
        assert_eq!(
            evaluation.arena().get(node.children()[0]).unwrap().kind(),
            &NodeKind::Binary(BinaryOp::Add)
        );
        assert!(!input_text(&mut evaluation, "="));
        assert_eq!(evaluation.root(), root);
    }

    #[test]
    fn equals_requests_calculation_without_another_edit_or_undo_step() {
        let mut editor = typed_one_character_at_a_time("2+3");
        let original = editor.to_portable().unwrap();
        assert_eq!(
            input_text_with_actions(&mut editor, "="),
            InputOutcome {
                document_changed: true,
                evaluation_requested: true
            }
        );
        let evaluated = editor.to_portable().unwrap();
        let caret = editor.caret();
        let selection = editor.selection();
        assert_eq!(
            input_text_with_actions(&mut editor, "="),
            InputOutcome {
                document_changed: false,
                evaluation_requested: true
            }
        );
        assert_eq!(editor.to_portable().unwrap(), evaluated);
        assert_eq!(editor.caret(), caret);
        assert_eq!(editor.selection(), selection);
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), original);
        assert!(editor.redo());
        assert_eq!(editor.to_portable().unwrap(), evaluated);
    }

    #[test]
    fn equals_on_definition_keeps_assignment_and_editable_display_unit_separate() {
        let mut editor = typed_one_character_at_a_time("distance:=2m");
        let defined = editor.to_portable().unwrap();
        let outcome = input_text_with_actions(&mut editor, "=");
        assert!(outcome.document_changed && outcome.evaluation_requested);
        let evaluated = editor.to_portable().unwrap();
        assert_eq!(evaluated.kind, NodeKind::Binary(BinaryOp::Define));
        assert_eq!(
            evaluated.children[1].kind,
            NodeKind::Binary(BinaryOp::Evaluate)
        );
        input_text(&mut editor, "c");
        input_text(&mut editor, "m");
        let fragment =
            mathcraft_compat::editor_bridge::tree_to_mathml(&editor.to_portable().unwrap())
                .unwrap();
        let xml = mathcraft_compat::editor_bridge::worksheet_xml(&[(0, 0., 0., fragment)]).unwrap();
        let result = mathcraft_compat::evaluate_worksheet(&xml).unwrap();
        assert!(result[0].error.is_none(), "{:?}", result[0].error);
        let Some(mathcraft_compat::Value::Scalar { quantity }) = result[0].value else {
            panic!()
        };
        assert_eq!(quantity.value.re, 2.);
        assert_eq!(quantity.dimension[0], 1.);
        let Some(mathcraft_compat::Value::Scalar { quantity }) = result[0].display_value else {
            panic!()
        };
        assert_eq!(quantity.value.re, 200.);
        assert!(editor.undo()); // remove m
        assert!(editor.undo()); // remove c
        assert_eq!(editor.to_portable().unwrap(), evaluated);
        assert!(editor.undo()); // remove evaluation, preserving the definition
        assert_eq!(editor.to_portable().unwrap(), defined);
    }

    #[test]
    fn definition_shortcut_does_not_request_evaluation_of_empty_rhs() {
        let mut editor = Editor::new();
        assert!(input_text_with_actions(&mut editor, "x:").document_changed);
        let before = editor.to_portable().unwrap();
        assert_eq!(
            input_text_with_actions(&mut editor, "="),
            InputOutcome::default()
        );
        assert_eq!(editor.to_portable().unwrap(), before);
        input_text(&mut editor, "2");
        assert!(input_text_with_actions(&mut editor, "=").evaluation_requested);
    }

    #[test]
    fn annotated_identifier_edit_preserves_labels() {
        let mut editor = Editor::new();
        let id = editor.root();
        editor
            .set_atom(
                id,
                NodeKind::AnnotatedIdentifier {
                    name: "mass".into(),
                    labels: "VARIABLE".into(),
                },
            )
            .unwrap();
        input_text(&mut editor, "2");
        assert_eq!(
            editor.arena().get(id).unwrap().kind(),
            &NodeKind::AnnotatedIdentifier {
                name: "mass2".into(),
                labels: "VARIABLE".into()
            }
        );
    }

    #[test]
    fn cached_result_composition_does_not_mutate_tree_and_unit_remains_hittable() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut editor = Editor::new();
        input_text(&mut editor, "2+3=");
        let before = editor.to_portable().unwrap();
        let frame = context.run_ui(egui::RawInput::default(), |ui| {
            let layout = measure_with_result(
                ui.painter(),
                editor.arena(),
                editor.root(),
                FONT_SIZE,
                Some("5"),
            );
            let right = &layout.children[1].1;
            assert!(right.primitives.iter().any(
                |primitive| matches!(primitive, Primitive::Text(_, text) if text.text() == "5")
            ));
            assert!(
                !right
                    .primitives
                    .iter()
                    .any(|primitive| matches!(primitive, Primitive::Placeholder(_)))
            );
            formula_ui_with_result(ui, &mut editor, Some("5"));
        });
        frame.drop_without_applying_deltas();
        assert_eq!(editor.to_portable().unwrap(), before);
        let rhs = editor.arena().get(editor.root()).unwrap().children()[1];
        editor
            .set_atom(
                rhs,
                NodeKind::AnnotatedIdentifier {
                    name: "m".into(),
                    labels: "UNIT".into(),
                },
            )
            .unwrap();
        let before = editor.to_portable().unwrap();
        let frame = context.run_ui(egui::RawInput::default(), |ui| {
            let layout = measure_with_result(
                ui.painter(),
                editor.arena(),
                editor.root(),
                FONT_SIZE,
                Some("5"),
            );
            let right = &layout.children[1].1;
            let bounds = right.hit_bounds.unwrap();
            assert!(right.text_offset.x > 0.0);
            let mut hits = vec![];
            collect_hits(&layout, Pos2::ZERO, &mut hits);
            let point = layout.children[1].0.to_pos2() + bounds.center().to_vec2();
            assert_eq!(hit_node(&hits, point).unwrap().node, rhs);
        });
        frame.drop_without_applying_deltas();
        assert_eq!(editor.to_portable().unwrap(), before);
    }
    #[test]
    fn compound_unit_keeps_numeric_result_and_unit_children() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut editor = Editor::new();
        input_text(&mut editor, "1=m/s");
        let before = editor.to_portable().unwrap();
        let frame = context.run_ui(egui::RawInput::default(), |ui| {
            let layout = measure_with_result(
                ui.painter(),
                editor.arena(),
                editor.root(),
                FONT_SIZE,
                Some("123.456"),
            );
            let right = &layout.children[1].1;
            assert!(
                right
                    .primitives
                    .iter()
                    .any(|p| matches!(p,Primitive::Text(_,g) if g.text()=="123.456"))
            );
            assert_eq!(right.children.len(), 2);
            assert!(right.children.iter().all(|(offset, _)| offset.x > 0.));
        });
        frame.drop_without_applying_deltas();
        assert_eq!(editor.to_portable().unwrap(), before);
    }

    #[test]
    fn enter_and_escape_events_remain_available_to_worksheet() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut editor = Editor::new();
        let frame = context.run_ui(egui::RawInput::default(), |ui| {
            formula_ui(ui, &mut editor).request_focus();
        });
        frame.drop_without_applying_deltas();
        let events = [Key::Enter, Key::Escape]
            .into_iter()
            .map(|key| egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::default(),
            })
            .collect();
        let frame = context.run_ui(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ui| {
                formula_ui(ui, &mut editor);
                let remaining = ui.input(|input| {
                    input
                        .events
                        .iter()
                        .filter(|event| {
                            matches!(
                                event,
                                egui::Event::Key {
                                    key: Key::Enter | Key::Escape,
                                    ..
                                }
                            )
                        })
                        .count()
                });
                assert_eq!(remaining, 2);
            },
        );
        frame.drop_without_applying_deltas();
    }
    fn typed_one_character_at_a_time(text: &str) -> Editor {
        let mut editor = Editor::new();
        for ch in text.chars() {
            input_text(&mut editor, &ch.to_string());
            assert_eq!(editor.validate(), Ok(()), "after {ch:?} in {text}");
        }
        editor
    }

    fn calculated(texts: &[&str]) -> serde_json::Value {
        use mathcraft_compat::editor_bridge::{tree_to_mathml, worksheet_xml};
        let regions: Vec<_> = texts
            .iter()
            .enumerate()
            .map(|(index, text)| {
                let editor = typed_one_character_at_a_time(text);
                let tree = editor.to_portable().unwrap();
                (
                    index as u64,
                    10.0,
                    10.0 + index as f64 * 80.0,
                    tree_to_mathml(&tree).unwrap(),
                )
            })
            .collect();
        let xml = worksheet_xml(&regions).unwrap();
        let results = mathcraft_compat::evaluate_worksheet(&xml).unwrap();
        let result = results.last().unwrap();
        assert!(
            result.error.is_none(),
            "{texts:?}: {:?} XML: {xml}",
            result.error
        );
        result
            .value
            .as_ref()
            .expect("evaluation result")
            .numeric_json()
    }

    #[test]
    fn character_entry_through_mathml_calculator_respects_precedence_and_functions() {
        for (text, expected) in [
            ("2*3+4=", 10.0),
            ("2+3*4=", 14.0),
            ("(2+3)*4=", 20.0),
            ("2*(3+4)=", 14.0),
            ("8/4*2=", 4.0),
            ("2^3^2=", 512.0),
            ("-2^2=", -4.0),
            ("2*-3+4=", -2.0),
            ("sin(0)=", 0.0),
            ("sqrt(9)=", 3.0),
            ("abs(-3)=", 3.0),
            ("1e-3+2=", 2.001),
        ] {
            let value = calculated(&[text]);
            let actual = value["real"].as_f64().unwrap();
            assert!(
                (actual - expected).abs() < 1e-12,
                "{text}: {actual} != {expected}"
            );
        }
        assert_eq!(calculated(&["f(x):=x^2", "f(3)="])["real"], 9.0);
        assert_eq!(calculated(&["f(x,y):=x*y+x", "f(2,3)="])["real"], 8.0);
        let imaginary = calculated(&["2j+3="]);
        assert_eq!(imaginary["real"], 3.0);
        assert_eq!(imaginary["imag"], 2.0);
    }

    #[test]
    fn character_entry_builds_rectangular_matrices_indices_and_ranges() {
        let matrix = typed_one_character_at_a_time("[1,2;3,4]")
            .to_portable()
            .unwrap();
        assert_eq!(matrix.kind, NodeKind::Matrix { rows: 2, cols: 2 });
        assert_eq!(
            matrix
                .children
                .iter()
                .map(|cell| &cell.kind)
                .collect::<Vec<_>>(),
            vec![
                &NodeKind::Number("1".into()),
                &NodeKind::Number("2".into()),
                &NodeKind::Number("3".into()),
                &NodeKind::Number("4".into())
            ]
        );
        let index = typed_one_character_at_a_time("A[1,0]")
            .to_portable()
            .unwrap();
        assert_eq!(index.kind, NodeKind::Index { indices: 2 });
        assert_eq!(
            typed_one_character_at_a_time("1..3")
                .to_portable()
                .unwrap()
                .kind,
            NodeKind::Range { has_step: false }
        );
        let stepped = typed_one_character_at_a_time("1,3..7")
            .to_portable()
            .unwrap();
        assert_eq!(stepped.kind, NodeKind::Range { has_step: true });
        assert_eq!(stepped.children[2].kind, NodeKind::Number("7".into()));
        let empty = typed_one_character_at_a_time("f()").to_portable().unwrap();
        assert_eq!(empty.kind, NodeKind::Call { argument_count: 0 });
        assert_eq!(empty.children.len(), 1);
    }

    #[test]
    fn committed_text_event_undoes_implicit_multiplication_as_one_command() {
        let mut editor = typed_one_character_at_a_time("2");
        let before = editor.to_portable().unwrap();
        assert!(input_text(&mut editor, "m"));
        assert_eq!(
            editor.to_portable().unwrap().kind,
            NodeKind::Binary(BinaryOp::Multiply)
        );
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), before);
        assert!(editor.redo());
        assert!(input_text(&mut editor, "eters"));
        assert_eq!(
            atomic_text(&editor, editor.caret().node).as_deref(),
            Some("meters")
        );
        assert!(editor.undo());
        assert_eq!(
            atomic_text(&editor, editor.caret().node).as_deref(),
            Some("m")
        );
    }

    #[test]
    fn paste_is_one_edit_and_repeated_equals_still_requests_calculation() {
        let mut editor = typed_one_character_at_a_time("8+");
        let before = editor.to_portable().unwrap();
        let root = editor.root();
        let untouched = editor.arena().get(root).unwrap().children()[0];
        let outcome = paste_tree_or_text_with_actions(&mut editor, "sqrt(4)+3=");
        assert!(outcome.document_changed && outcome.evaluation_requested);
        let after = editor.to_portable().unwrap();
        assert!(editor.undo());
        assert_eq!(editor.root(), root);
        assert_eq!(editor.to_portable().unwrap(), before);
        assert_eq!(editor.arena().get(root).unwrap().children()[0], untouched);
        assert!(editor.redo());
        assert_eq!(editor.to_portable().unwrap(), after);
        let caret = editor.caret();
        assert_eq!(
            paste_tree_or_text_with_actions(&mut editor, "="),
            InputOutcome {
                document_changed: false,
                evaluation_requested: true,
            }
        );
        assert_eq!(editor.caret(), caret);
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), before);
    }

    #[test]
    fn typing_over_selected_compound_replaces_only_that_branch() {
        let mut editor = typed_one_character_at_a_time("1+2/3");
        let root = editor.root();
        let children = editor.arena().get(root).unwrap().children().to_vec();
        assert_eq!(
            editor.arena().get(children[1]).unwrap().kind(),
            &NodeKind::Fraction
        );
        editor.select_subtree(children[1]).unwrap();
        let before = editor.to_portable().unwrap();
        assert!(input_text(&mut editor, "x"));
        assert_eq!(editor.root(), root);
        let children_after = editor.arena().get(root).unwrap().children();
        assert_eq!(children_after[0], children[0]);
        assert_eq!(
            editor.arena().get(children_after[1]).unwrap().kind(),
            &NodeKind::Identifier("x".into())
        );
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), before);
        assert_eq!(editor.selection(), Selection::Subtree(children[1]));
        assert!(input_text(&mut editor, "7"));
        assert_eq!(
            editor.arena().get(editor.caret().node).unwrap().kind(),
            &NodeKind::Number("7".into())
        );
        assert!(!editor.redo());
    }

    #[test]
    fn typing_over_selected_number_does_not_make_a_unit_or_imaginary_product() {
        for text in ["m", "i", "j"] {
            let mut editor = typed_one_character_at_a_time("23");
            editor.select_subtree(editor.root()).unwrap();
            assert!(input_text(&mut editor, text));
            assert_eq!(
                editor.to_portable().unwrap().kind,
                NodeKind::Identifier(text.into())
            );
            assert!(editor.undo());
            assert_eq!(
                editor.to_portable().unwrap().kind,
                NodeKind::Number("23".into())
            );
        }
    }

    #[test]
    fn structural_clipboard_and_delete_preserve_undo_and_other_branches() {
        let source = typed_one_character_at_a_time("sin(2)+3");
        let mut source = source;
        source.select_subtree(source.root()).unwrap();
        let clipboard = copy_tree(&source).unwrap();
        let mut target = typed_one_character_at_a_time("1+4");
        assert!(paste_tree_or_text(&mut target, &clipboard));
        assert_eq!(
            target.arena().get(target.root()).unwrap().kind(),
            &NodeKind::Binary(BinaryOp::Add)
        );
        assert_eq!(
            target
                .arena()
                .get(target.arena().get(target.root()).unwrap().children()[0])
                .unwrap()
                .kind(),
            &NodeKind::Number("1".into())
        );
        let before = target.to_portable().unwrap();
        assert!(!paste_tree_or_text(&mut target, "mathcraft-tree:{bad"));
        assert_eq!(target.to_portable().unwrap(), before);
        target.select_subtree(target.root()).unwrap();
        assert!(delete_character(&mut target, true));
        assert_eq!(
            target.arena().get(target.root()).unwrap().kind(),
            &NodeKind::Placeholder
        );
        assert!(target.undo());
        assert_eq!(target.to_portable().unwrap(), before);
        let mut fraction = typed_one_character_at_a_time("1/");
        assert!(delete_character(&mut fraction, true));
        assert_eq!(
            fraction.to_portable().unwrap().kind,
            NodeKind::Number("1".into())
        );
    }
    #[test]
    fn structural_navigation_exits_containers_and_never_wraps_at_formula_edges() {
        let mut editor = typed_one_character_at_a_time("sin(12)");
        let call = editor.root();
        move_horizontal(&mut editor, false);
        assert_eq!(atomic_text(&editor, editor.caret().node).unwrap(), "12");
        move_horizontal(&mut editor, true);
        assert_eq!(editor.caret().node, call);
        assert_eq!(editor.caret().caret_side, Direction::Right);
        move_horizontal(&mut editor, true);
        assert_eq!(editor.caret().node, call);
        let before = editor.to_portable().unwrap();
        assert!(!delete_character(&mut editor, true));
        assert_eq!(editor.to_portable().unwrap(), before);
        assert_eq!(atomic_text(&editor, editor.caret().node).unwrap(), "12");
        let mut sum = typed_one_character_at_a_time("2+34");
        let node = sum.caret().node;
        sum.place_text_caret(node, 0).unwrap();
        let before = sum.to_portable().unwrap();
        assert!(!delete_character(&mut sum, true));
        assert_eq!(sum.to_portable().unwrap(), before);
    }

    #[test]
    fn native_identifier_subscript_and_i_j_survive_edit_and_clipboard() {
        let mut editor = Editor::new();
        let id = editor.root();
        editor
            .set_atom(
                id,
                NodeKind::NativeIdentifier {
                    name: "f".into(),
                    labels: "VARIABLE".into(),
                    contextual: Some(true),
                    subscript: Some("0".into()),
                },
            )
            .unwrap();
        input_text(&mut editor, "x");
        assert_eq!(
            editor.arena().get(id).unwrap().kind(),
            &NodeKind::NativeIdentifier {
                name: "fx".into(),
                labels: "VARIABLE".into(),
                contextual: Some(true),
                subscript: Some("0".into())
            }
        );
        let clipboard = copy_tree(&editor).unwrap();
        let mut pasted = Editor::new();
        assert!(paste_tree_or_text(&mut pasted, &clipboard));
        assert_eq!(pasted.to_portable().unwrap(), editor.to_portable().unwrap());
        assert_eq!(
            typed_one_character_at_a_time("2j")
                .to_portable()
                .unwrap()
                .kind,
            NodeKind::ImaginarySymbol {
                value: "2".into(),
                symbol: "j".into()
            }
        );
    }
    #[test]
    fn readonly_matrix_and_range_columns_keep_original_tree_and_hit_ids() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut editor = typed_one_character_at_a_time("1=");
        let rhs = editor.arena().get(editor.root()).unwrap().children()[1];
        let before = editor.to_portable().unwrap();
        let before_caret = editor.caret();
        let cells = vec!["1".into(), "200".into(), "3.5".into(), "4".into()];
        let frame = context.run_ui(egui::RawInput::default(), |ui| {
            let layout = measure_with_display(
                ui.painter(),
                editor.arena(),
                editor.root(),
                FONT_SIZE,
                Some(FormulaResult::Matrix {
                    rows: 2,
                    cols: 2,
                    cells: &cells,
                    brackets: true,
                }),
            );
            let result = &layout.children[1].1;
            assert!(result.children.is_empty());
            assert!(result.atom.is_none());
            let glyphs: Vec<_> = result
                .primitives
                .iter()
                .filter_map(|p| match p {
                    Primitive::Text(position, text) => {
                        Some((*position, text.text().to_owned(), text_baseline(text)))
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(glyphs.len(), 4);
            assert_eq!(glyphs[0].0.y + glyphs[0].2, glyphs[1].0.y + glyphs[1].2);
            assert!(glyphs[2].0.y > glyphs[0].0.y);
            assert!(glyphs[1].0.x > glyphs[0].0.x);
            assert_eq!(
                result
                    .primitives
                    .iter()
                    .filter(|p| matches!(p, Primitive::Line(..)))
                    .count(),
                6
            );
            let mut hits = vec![];
            collect_hits(&layout, Pos2::ZERO, &mut hits);
            assert_eq!(hits.iter().filter(|hit| hit.node == rhs).count(), 1);
            for (position, _, _) in &glyphs {
                let point = layout.children[1].0.to_pos2() + *position + vec2(1.0, 1.0);
                assert_eq!(hit_node(&hits, point).unwrap().node, rhs);
            }
            formula_ui_with_display(
                ui,
                &mut editor,
                Some(FormulaResult::Matrix {
                    rows: 2,
                    cols: 2,
                    cells: &cells,
                    brackets: true,
                }),
                false,
            );
        });
        frame.drop_without_applying_deltas();
        assert_eq!(editor.to_portable().unwrap(), before);
        assert_eq!(editor.caret(), before_caret);
        let column = vec!["1".into(), "2".into(), "3".into()];
        let frame = context.run_ui(egui::RawInput::default(), |ui| {
            let output = measure_result(
                ui.painter(),
                rhs,
                FormulaResult::Matrix {
                    rows: 3,
                    cols: 1,
                    cells: &column,
                    brackets: false,
                },
                FONT_SIZE,
            );
            assert_eq!(
                output
                    .primitives
                    .iter()
                    .filter(|p| matches!(p, Primitive::Text(..)))
                    .count(),
                3
            );
            assert!(
                !output
                    .primitives
                    .iter()
                    .any(|p| matches!(p, Primitive::Line(..)))
            );
            assert!(output.children.is_empty());
        });
        frame.drop_without_applying_deltas();
    }

    #[test]
    fn readonly_matrix_prefix_keeps_unit_caret_at_its_actual_text_origin() {
        let context = egui::Context::default();
        crate::configure(&context);
        let editor = typed_one_character_at_a_time("1=m");
        let before = editor.to_portable().unwrap();
        let rhs = editor.arena().get(editor.root()).unwrap().children()[1];
        let cells = vec!["1".into(), "2".into()];
        let frame = context.run_ui(egui::RawInput::default(), |ui| {
            let layout = measure_with_display(
                ui.painter(),
                editor.arena(),
                editor.root(),
                FONT_SIZE,
                Some(FormulaResult::Matrix {
                    rows: 2,
                    cols: 1,
                    cells: &cells,
                    brackets: true,
                }),
            );
            let unit = &layout.children[1].1;
            assert!(unit.text_offset.x > 0.0);
            assert!(unit.text_offset.y > 0.0);
            let mut hits = vec![];
            collect_hits(&layout, Pos2::ZERO, &mut hits);
            let hit = hits.iter().find(|hit| hit.node == rhs).unwrap();
            assert_eq!(
                hit.text_origin,
                layout.children[1].0.to_pos2() + unit.text_offset
            );
            assert_eq!(
                hit_node(&hits, layout.children[1].0.to_pos2() + vec2(5.0, 5.0))
                    .unwrap()
                    .node,
                rhs
            );
            let unit_point = hit.text_origin + vec2(1.0, 1.0);
            assert_eq!(hit_node(&hits, unit_point).unwrap().node, rhs);
        });
        frame.drop_without_applying_deltas();
        assert_eq!(editor.to_portable().unwrap(), before);
    }
    #[test]
    fn nested_definition_evaluation_receives_matrix_result_at_its_own_unit_slot() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut tree = typed_one_character_at_a_time("x:=2").to_portable().unwrap();
        let value = tree.children.pop().unwrap();
        tree.children.push(PortableTree {
            kind: NodeKind::Binary(BinaryOp::Evaluate),
            children: vec![
                value,
                PortableTree {
                    kind: NodeKind::AnnotatedIdentifier {
                        name: "m".into(),
                        labels: "UNIT".into(),
                    },
                    children: vec![],
                },
            ],
        });
        let editor = Editor::from_portable(&tree).unwrap();
        let before = editor.to_portable().unwrap();
        let cells = vec!["2".into(), "4".into()];
        let frame = context.run_ui(egui::RawInput::default(), |ui| {
            let layout = measure_with_display(
                ui.painter(),
                editor.arena(),
                editor.root(),
                FONT_SIZE,
                Some(FormulaResult::Matrix {
                    rows: 2,
                    cols: 1,
                    cells: &cells,
                    brackets: true,
                }),
            );
            let evaluation = &layout.children[1].1;
            let unit = &evaluation.children[1].1;
            let displayed: Vec<_> = unit
                .primitives
                .iter()
                .filter_map(|p| match p {
                    Primitive::Text(_, text) => Some(text.text()),
                    _ => None,
                })
                .collect();
            assert!(displayed.contains(&"2"));
            assert!(displayed.contains(&"4"));
            assert!(displayed.contains(&"m"));
            assert!(unit.text_offset.x > 0.0);
            let mut hits = vec![];
            collect_hits(&layout, Pos2::ZERO, &mut hits);
            assert!(
                hits.iter()
                    .all(|hit| editor.arena().get(hit.node).is_some())
            );
            let mut ids = std::collections::HashSet::new();
            assert!(hits.iter().all(|hit| ids.insert(hit.node)));
        });
        frame.drop_without_applying_deltas();
        assert_eq!(editor.to_portable().unwrap(), before);
    }
    #[test]
    fn tall_function_fences_stay_narrow_and_every_hit_is_a_source_node() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut editor = Editor::new();
        input_text(&mut editor, "sin(1/2)");
        let output = context.run_ui(egui::RawInput::default(), |ui| {
            let layout = measure(ui.painter(), editor.arena(), editor.root(), FONT_SIZE);
            let mut hits = vec![];
            collect_hits(&layout, Pos2::ZERO, &mut hits);
            let mut ids = std::collections::HashSet::new();
            assert!(hits.iter().all(|hit| ids.insert(hit.node)));
            assert!(
                hits.iter()
                    .all(|hit| editor.arena().get(hit.node).is_some())
            );
            let mut child = measure(ui.painter(), editor.arena(), editor.root(), FONT_SIZE);
            child.size.y *= 5.;
            let width = child.size.x;
            let tall = visual_parentheses(ui.painter(), child, FONT_SIZE);
            assert!((tall.size.x - width - 2. * (fence_width(FONT_SIZE) + 1.)).abs() < 0.001);
        });
        output.drop_without_applying_deltas();
    }

    #[test]
    fn square_root_hides_default_degree_without_changing_source_tree() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut editor = Editor::new();
        input_text(&mut editor, "√9");
        let before = editor.to_portable().unwrap();
        let output = context.run_ui(egui::RawInput::default(), |ui| {
            let layout = measure(ui.painter(), editor.arena(), editor.root(), FONT_SIZE);
            assert_eq!(layout.children[0].1.size, Vec2::ZERO);
            let mut hits = vec![];
            collect_hits(&layout, Pos2::ZERO, &mut hits);
            assert!(
                hits.iter()
                    .any(|h| h.node == editor.arena().get(editor.root()).unwrap().children()[0])
            );
        });
        output.drop_without_applying_deltas();
        assert_eq!(editor.to_portable().unwrap(), before);
    }

    #[test]
    fn same_name_variable_and_unit_get_occurrence_styles_without_mutating_metadata() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut editor = Editor::new();
        input_text(&mut editor, "m:=2m");
        let before = editor.to_portable().unwrap();
        let source = crate::worksheet_math::tree_to_mathml(&before).unwrap();
        let worksheet = crate::worksheet_math::worksheet_xml(&[(1, 0., 0., source)]).unwrap();
        let xml = roxmltree::Document::parse(&worksheet).unwrap();
        let math = xml
            .descendants()
            .find(|n| n.has_tag_name(("http://schemas.mathsoft.com/worksheet50", "math")))
            .unwrap();
        let labelled = crate::worksheet_math::tree_from_mathml(&worksheet[math.range()]).unwrap();
        let labels = resolved_labels(editor.arena(), editor.root(), Some(&labelled));
        let output = context.run_ui(egui::RawInput::default(), |ui| {
            let typesetter = Typesetter {
                painter: ui.painter(),
                arena: editor.arena(),
                labels: &labels,
            };
            let layout = typesetter.layout(editor.root(), FONT_SIZE, None);
            let mut hits = vec![];
            collect_hits(&layout, Pos2::ZERO, &mut hits);
            let styles: Vec<_> = hits
                .iter()
                .filter_map(|h| h.atom.as_ref())
                .filter(|g| g.text() == "m")
                .map(|g| &g.job.sections[0].format)
                .collect();
            assert_eq!(styles.len(), 2);
            assert_eq!(styles[0].font_id, MathStyle::Variable.font(FONT_SIZE));
            assert_eq!(styles[0].color, Color32::BLACK);
            assert_eq!(styles[1].font_id, MathStyle::Unit.font(FONT_SIZE));
            assert_eq!(styles[1].color, Color32::from_rgb(0, 0, 139));
        });
        output.drop_without_applying_deltas();
        assert_eq!(editor.to_portable().unwrap(), before);
    }
    #[test]
    fn shifted_matrix_keys_insert_axes_without_advancing_the_worksheet_or_scoping_space() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut editor = Editor::new();
        input_text(&mut editor, "[1,2;3,4]");
        let matrix = editor.root();
        let cell = editor.arena().get(matrix).unwrap().children()[3];
        editor.place_text_caret(cell, 1).unwrap();
        for (key, text, rows, cols) in [(Key::Space, Some(" "), 2, 3), (Key::Enter, None, 3, 3)] {
            let mut events = vec![
                egui::Event::ModifiersChanged(egui::Modifiers {
                    shift: true,
                    ..egui::Modifiers::NONE
                }),
                egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers {
                        shift: true,
                        ..egui::Modifiers::NONE
                    },
                },
            ];
            if let Some(text) = text {
                events.push(egui::Event::Text(text.into()));
            }
            let output = context.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    formula_ui_with_focus(ui, &mut editor, None, true);
                },
            );
            output.drop_without_applying_deltas();
            assert_eq!(
                editor.arena().get(matrix).unwrap().kind(),
                &NodeKind::Matrix { rows, cols }
            );
            assert_eq!(editor.selection(), Selection::None);
        }
        assert!(editor.undo());
        assert!(editor.undo());
        assert_eq!(
            editor.arena().get(matrix).unwrap().kind(),
            &NodeKind::Matrix { rows: 2, cols: 2 }
        );
    }
    #[test]
    fn structured_quantity_results_keep_source_hits_and_font_scaled_ime() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut editor = Editor::new();
        input_text(&mut editor, "2m=");
        let before = editor.to_portable().unwrap();
        let q = mathcraft_core::Quantity {
            dimension: [1., 0., -2., 0., 0., 0., 0.],
            ..mathcraft_core::Quantity::scalar(2.)
        };
        let result = crate::result_display::format(
            &mathcraft_compat::Value::Scalar { quantity: q },
            &mathcraft_document::canvas::MathFormat::default(),
        );
        let output = context.run_ui(egui::RawInput::default(), |ui| {
            let small =
                measured_size_styled(ui, &editor, Some(FormulaResult::Formatted(&result)), 11.);
            let large =
                measured_size_styled(ui, &editor, Some(FormulaResult::Formatted(&result)), 24.);
            assert!(large.y > small.y * 1.5);
            formula_ui_styled(
                ui,
                &mut editor,
                Some(FormulaResult::Formatted(&result)),
                true,
                None,
                24.,
            );
            let layout = measure_with_display(
                ui.painter(),
                editor.arena(),
                editor.root(),
                32.,
                Some(FormulaResult::Formatted(&result)),
            );
            let mut hits = vec![];
            collect_hits(&layout, Pos2::ZERO, &mut hits);
            let mut ids = std::collections::HashSet::new();
            assert!(
                hits.iter()
                    .all(|h| editor.arena().get(h.node).is_some() && ids.insert(h.node))
            );
        });
        assert!(
            output
                .platform_output
                .ime
                .as_ref()
                .unwrap()
                .cursor_rect
                .height()
                > 25.
        );
        output.drop_without_applying_deltas();
        assert_eq!(editor.to_portable().unwrap(), before);
    }
}
