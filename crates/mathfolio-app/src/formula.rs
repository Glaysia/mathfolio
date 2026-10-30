//! Direct AST renderer and a bounded structural editing surface.
//!
//! Prime font sizes, label styles and fraction padding follow static evidence.
//! Glyph outlines use licensed STIX faces; fences/root paths and some positioning
//! remain independent approximations. This is not a complete Prime editor.

use eframe::egui::{
    self, Color32, Galley, Key, Painter, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2,
    pos2, vec2,
};
use mathfolio_editor::{
    Arena, BinaryOp, Direction, Editor, FractionMetrics, InsertionMode, NodeId, NodeKind,
    PortableTree, Selection, layout_fraction,
};
mod matrix_viewport;
mod radix_input;
mod source_matrix;
pub use matrix_viewport::MatrixIndexLabels;
pub(crate) use matrix_viewport::MatrixViewGesture;
use std::sync::Arc;

use crate::typography::{self, INK, MathStyle};
use std::collections::HashMap;
const CARET: Color32 = Color32::from_rgb(0, 128, 255);
const PADDING: f32 = 2.0;
#[cfg(test)]
const FONT_SIZE: f32 = typography::MATH_SIZE;

#[derive(Clone, Copy, Debug)]
pub enum FormulaResult<'a> {
    MatrixViewport(&'a crate::result_display::MatrixProjection),
    MatrixViewportWithIndices {
        projection: &'a crate::result_display::MatrixProjection,
        indices: MatrixIndexLabels,
    },
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
    // Intrinsic worksheet DIP size before absolute-position float rounding.
    pub layout_size: Vec2,
    pub evaluation_requested: bool,
    pub matrix_view_gesture: Option<MatrixViewGesture>,
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
    subscript: Option<(Vec2, Arc<Galley>)>,
    operator_hits: Vec<(OperatorTarget, Rect)>,
    matrix_viewport: Option<Box<matrix_viewport::Geometry>>,
}

#[derive(Clone, Copy)]
enum OperatorTarget {
    Part(usize),
    Side(Direction),
}

#[derive(Clone, PartialEq)]
enum OwnedFormulaResult {
    MatrixViewport(crate::result_display::MatrixProjection),
    MatrixViewportWithIndices {
        projection: crate::result_display::MatrixProjection,
        indices: MatrixIndexLabels,
    },
    Formatted(crate::result_display::FormattedResult),
    Text(String),
    Matrix {
        rows: usize,
        cols: usize,
        cells: Vec<String>,
        brackets: bool,
    },
}
impl OwnedFormulaResult {
    fn owns(result: FormulaResult<'_>) -> Self {
        match result {
            FormulaResult::MatrixViewport(result) => Self::MatrixViewport(result.clone()),
            FormulaResult::MatrixViewportWithIndices {
                projection,
                indices,
            } => Self::MatrixViewportWithIndices {
                projection: projection.clone(),
                indices,
            },
            FormulaResult::Formatted(result) => Self::Formatted(result.clone()),
            FormulaResult::Text(text) => Self::Text(text.into()),
            FormulaResult::Matrix {
                rows,
                cols,
                cells,
                brackets,
            } => Self::Matrix {
                rows,
                cols,
                cells: cells.into(),
                brackets,
            },
        }
    }
    fn matches(&self, result: FormulaResult<'_>) -> bool {
        match (self, result) {
            (Self::MatrixViewport(a), FormulaResult::MatrixViewport(b)) => a == b,
            (
                Self::MatrixViewportWithIndices {
                    projection: a,
                    indices: ai,
                },
                FormulaResult::MatrixViewportWithIndices {
                    projection: b,
                    indices: bi,
                },
            ) => a == b && *ai == bi,
            (Self::Formatted(a), FormulaResult::Formatted(b)) => a == b,
            (Self::Text(a), FormulaResult::Text(b)) => a == b,
            (
                Self::Matrix {
                    rows: ar,
                    cols: ac,
                    cells: av,
                    brackets: ab,
                },
                FormulaResult::Matrix {
                    rows,
                    cols,
                    cells,
                    brackets,
                },
            ) => *ar == rows && *ac == cols && av == cells && *ab == brackets,
            _ => false,
        }
    }
}

/// Retain geometry and glyph galleys, while caret, selection, hits and input
/// continue to run on every frame. Arena revisions include node identities;
/// restoring/replacing an editor cannot leave hits attached to retired nodes.
struct CachedFormulaLayout {
    arena_revision: u64,
    root: NodeId,
    labelled: Option<PortableTree>,
    result: Option<OwnedFormulaResult>,
    font_size: f32,
    font_key: typography::FontCacheKey,
    layout: Arc<FormulaBox>,
}

fn retained_layout(
    ui: &Ui,
    editor: &Editor,
    labelled: Option<&PortableTree>,
    font_size: f32,
    result: Option<FormulaResult<'_>>,
) -> Arc<FormulaBox> {
    retained_layout_mode(ui, editor, labelled, font_size, result, false)
}
fn retained_layout_mode(
    ui: &Ui,
    editor: &Editor,
    labelled: Option<&PortableTree>,
    font_size: f32,
    result: Option<FormulaResult<'_>>,
    readonly: bool,
) -> Arc<FormulaBox> {
    retained_layout_mode_for_id(ui, ui.id(), editor, labelled, font_size, result, readonly)
}

fn retained_layout_mode_for_id(
    ui: &Ui,
    owner: egui::Id,
    editor: &Editor,
    labelled: Option<&PortableTree>,
    font_size: f32,
    result: Option<FormulaResult<'_>>,
    readonly: bool,
) -> Arc<FormulaBox> {
    #[cfg(test)]
    let lookup_timing = crate::frame_profile::scope(crate::frame_profile::Phase::FormulaLookup);
    let id = owner.with(("formula-layout", readonly));
    let font_key = typography::font_cache_key(ui.ctx());
    if let Some(saved) = ui.data(|data| data.get_temp::<Arc<CachedFormulaLayout>>(id))
        && saved.root == editor.root()
        && saved.arena_revision == editor.arena().revision()
        && saved.labelled.as_ref() == labelled
        && saved.font_size == font_size
        && saved.font_key == font_key
        && match (&saved.result, result) {
            (None, None) => true,
            (Some(saved), Some(result)) => saved.matches(result),
            _ => false,
        }
    {
        return saved.layout.clone();
    }
    #[cfg(test)]
    drop(lookup_timing);
    #[cfg(test)]
    let labels_timing = crate::frame_profile::scope(crate::frame_profile::Phase::FormulaLabels);
    let labels = resolved_labels(editor.arena(), editor.root(), labelled);
    #[cfg(test)]
    drop(labels_timing);
    #[cfg(test)]
    let typeset_timing = crate::frame_profile::scope(crate::frame_profile::Phase::FormulaTypeset);
    let mut layout = Typesetter {
        painter: ui.painter(),
        arena: editor.arena(),
        labels: &labels,
        readonly,
    }
    .layout(editor.root(), font_size, result);
    if matches!(
        result,
        Some(FormulaResult::MatrixViewport(_) | FormulaResult::MatrixViewportWithIndices { .. })
    ) {
        layout.matrix_viewport = matrix_viewport::find(&layout, Pos2::ZERO).map(Box::new);
    }
    let layout = Arc::new(layout);
    #[cfg(test)]
    drop(typeset_timing);
    #[cfg(test)]
    let _payload_timing = crate::frame_profile::scope(crate::frame_profile::Phase::FormulaPayload);
    let saved = Arc::new(CachedFormulaLayout {
        arena_revision: editor.arena().revision(),
        root: editor.root(),
        labelled: labelled.cloned(),
        result: result.map(OwnedFormulaResult::owns),
        font_size,
        font_key,
        layout: layout.clone(),
    });
    ui.data_mut(|data| data.insert_temp(id, saved));
    layout
}

enum Primitive {
    Text(Vec2, Arc<Galley>),
    FocusedText(Vec2, Arc<Galley>),
    Line(Vec2, Vec2),
    MinorFence(Vec2, Vec2),
    Placeholder(Rect),
}

struct HitNode {
    node: NodeId,
    rect: Rect,
    atom: Option<Arc<Galley>>,
    text_origin: Pos2,
    insertion: InsertionMode,
    operator_part: Option<usize>,
    operator_side: Option<Direction>,
}
struct CachedHits {
    layout: Arc<FormulaBox>,
    origin: Pos2,
    hits: Arc<Vec<HitNode>>,
}
fn retained_hits(ui: &Ui, layout: &Arc<FormulaBox>, origin: Pos2) -> Arc<Vec<HitNode>> {
    let id = ui.id().with("formula-hits");
    if let Some(saved) = ui.data(|d| d.get_temp::<Arc<CachedHits>>(id))
        && saved.origin == origin
        && Arc::ptr_eq(&saved.layout, layout)
    {
        return saved.hits.clone();
    }
    let mut hits = vec![];
    collect_hits(layout, origin, &mut hits);
    let hits = Arc::new(hits);
    ui.data_mut(|d| {
        d.insert_temp(
            id,
            Arc::new(CachedHits {
                layout: layout.clone(),
                origin,
                hits: hits.clone(),
            }),
        )
    });
    hits
}

#[derive(Clone)]
struct DragAnchor {
    node: NodeId,
    offset: usize,
    insertion: InsertionMode,
}

/// Paint and edit one structural formula. The caller supplies a stable egui ID
/// scope when rendering multiple editors (`ui.push_id(worksheet_region_id, …)`).
///
/// Supported: atomic text, / ^ ( ), + - *, Space, Tab, left/right, Backspace,
/// Delete, Ctrl/Cmd+Z and redo. '=' evaluates the root or a definition's RHS;
/// repeated evaluation requests do not change the document or its undo history.
/// Enter inserts a program line inside programs; otherwise Enter/Escape remain
/// available to the surrounding worksheet.
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

/// Measure the same non-interactive layout that printing paints. An absent root
/// degree is an optional editor slot; its editing width is not part of output.
pub(crate) fn measured_size_readonly_styled(
    ui: &Ui,
    editor: &Editor,
    result: Option<FormulaResult<'_>>,
    labelled: Option<&PortableTree>,
    font_points: f32,
) -> Vec2 {
    measured_geometry_readonly_styled(ui, editor, result, labelled, font_points).0
}

/// Native worksheet locations align the equation's text baseline, rather than
/// its top edge. Keep geometry and that baseline in the same layout operation.
pub(crate) fn measured_geometry_readonly_styled(
    ui: &Ui,
    editor: &Editor,
    result: Option<FormulaResult<'_>>,
    labelled: Option<&PortableTree>,
    font_points: f32,
) -> (Vec2, f32) {
    let layout = readonly_layout(ui, editor, result, labelled, font_points);
    (
        (layout.size + vec2(PADDING * 2., PADDING * 2.)).max(vec2(12., 22.)),
        layout.baseline + PADDING,
    )
}

/// Measure before allocating a child Ui while sharing that exact child's paint
/// cache. The supplied identity must also be used by the eventual formula Ui.
pub(crate) fn measured_geometry_retained_styled_for_id(
    ui: &Ui,
    owner: egui::Id,
    editor: &Editor,
    result: Option<FormulaResult<'_>>,
    labelled: Option<&PortableTree>,
    font_points: f32,
) -> (Vec2, f32) {
    let layout = retained_layout_mode_for_id(
        ui,
        owner,
        editor,
        labelled,
        font_points.clamp(1., 200.) * 96. / 72.,
        result,
        false,
    );
    (
        (layout.size + vec2(PADDING * 2., PADDING * 2.)).max(vec2(12., 22.)),
        layout.baseline + PADDING,
    )
}

pub(crate) fn baseline_offset_styled(
    ui: &Ui,
    editor: &Editor,
    result: Option<FormulaResult<'_>>,
    labelled: Option<&PortableTree>,
    font_points: f32,
) -> f32 {
    retained_layout(
        ui,
        editor,
        labelled,
        font_points.clamp(1., 200.) * 96. / 72.,
        result,
    )
    .baseline
        + PADDING
}

/// Paint source and cached results without focus, selection, IME or input
/// handling. Required empty operands remain visible; only an optional empty
/// radical degree is omitted, as opposed to hiding every placeholder.
pub(crate) fn formula_readonly_ui_styled(
    ui: &mut Ui,
    editor: &Editor,
    result: Option<FormulaResult<'_>>,
    labelled: Option<&PortableTree>,
    font_points: f32,
) -> Response {
    let layout = readonly_layout(ui, editor, result, labelled, font_points);
    let desired = (layout.size + vec2(PADDING * 2., PADDING * 2.)).max(vec2(12., 22.));
    let (rect, response) = ui.allocate_exact_size(desired, Sense::hover());
    paint_box(ui.painter(), &layout, rect.min + vec2(PADDING, PADDING));
    response
}

fn readonly_layout(
    ui: &Ui,
    editor: &Editor,
    result: Option<FormulaResult<'_>>,
    labelled: Option<&PortableTree>,
    font_points: f32,
) -> Arc<FormulaBox> {
    retained_layout_mode(
        ui,
        editor,
        labelled,
        font_points.clamp(1., 200.) * 96. / 72.,
        result,
        true,
    )
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

pub(crate) fn set_numeric_input_mode(ctx: &egui::Context, enabled: bool) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("worksheet-numeric-input-mode"), enabled));
}

pub(crate) fn formula_ui_styled(
    ui: &mut Ui,
    editor: &mut Editor,
    result: Option<FormulaResult<'_>>,
    focus: bool,
    labelled: Option<&PortableTree>,
    font_points: f32,
) -> FormulaResponse {
    if let Some(mode) = ui
        .ctx()
        .data(|data| data.get_temp::<bool>(egui::Id::new("worksheet-numeric-input-mode")))
    {
        editor.set_bin_oct_hex_mode(mode);
    }
    #[cfg(test)]
    let layout_timing = crate::frame_profile::scope(crate::frame_profile::Phase::FormulaLayout);
    let font_size = font_points.clamp(1., 200.) * 96. / 72.;
    editor.clamp_source_matrix_caret();
    let mut initial = retained_layout(ui, editor, labelled, font_size, result);
    #[cfg(test)]
    drop(layout_timing);
    #[cfg(test)]
    let pointer_timing = crate::frame_profile::scope(crate::frame_profile::Phase::FormulaPointer);
    let desired = (initial.size + vec2(PADDING * 2.0, PADDING * 2.0)).max(vec2(12., 22.));
    let (rect, mut response) = ui.allocate_exact_size(desired, Sense::click_and_drag());
    if focus {
        response.request_focus();
    }
    // egui only installs a focus filter after a widget had focus last frame.
    // The first Tab/arrow after clicking an equation can therefore start
    // framework focus traversal before this widget sees its keyboard event.
    let navigation = ui.input(|input| {
        !input.pointer.any_pressed()
            && input.events.iter().any(|event| matches!(event,
                egui::Event::Key {key, pressed:true, modifiers, ..}
                if (modifiers.is_none() || modifiers.shift_only())
                    && matches!(key,Key::Tab|Key::ArrowLeft|Key::ArrowRight|Key::ArrowUp|Key::ArrowDown)))
    });
    if navigation && !ui.ctx().any_popup_open() {
        if !response.has_focus() && ui.memory(|memory| memory.had_focus_last_frame(response.id)) {
            response.request_focus();
        }
        if response.has_focus() {
            ui.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
        }
    }
    let origin = rect.min + vec2(PADDING, PADDING);
    let mut hits = retained_hits(ui, &initial, origin);
    let matrix_geometry = if matches!(
        result,
        Some(FormulaResult::MatrixViewport(_) | FormulaResult::MatrixViewportWithIndices { .. })
    ) {
        matrix_viewport::find(&initial, origin)
    } else {
        None
    };
    let matrix_interaction = matrix_viewport::interact(ui, response.id, editor, matrix_geometry);

    if response.clicked() && !matrix_interaction.owns_pointer {
        response.request_focus();
        if let Some(pointer) = response.interact_pointer_pos()
            && let Some(hit) = hit_node(&hits, pointer)
        {
            if let Some(part) = hit.operator_part {
                if matches!(editor.arena().get(hit.node).unwrap().kind(), NodeKind::Try) {
                    // VisualTryNode uses NodeSide Left/Right and a null
                    // native operator-part index. The hit list's local index
                    // identifies the header; do not copy it to the caret.
                    let side = match part {
                        0 => Direction::Left,
                        1 => Direction::Right,
                        _ => Direction::Indeterminate,
                    };
                    let _ = editor.place_try_operator(hit.node, side);
                } else {
                    let _ = editor.place_if_operator(hit.node, part);
                }
            } else if let Some(side) = hit.operator_side {
                let _ = editor.place_operator_caret(hit.node, side);
            } else if let Some(galley) = &hit.atom {
                let cursor = galley.cursor_from_pos(pointer - hit.text_origin);
                let offset = galley
                    .text()
                    .chars()
                    .take(cursor.index.into())
                    .map(char::len_utf16)
                    .sum();
                let anchor = editor.caret().value_range.map_or(offset, |r| r.anchor);
                if ui.input(|i| i.modifiers.shift)
                    && editor.caret().node == hit.node
                    && editor.caret().insertion == hit.insertion
                {
                    place_hit_selection(editor, hit, anchor, offset);
                } else {
                    place_hit_selection(editor, hit, offset, offset);
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
    if response.drag_started() && !matrix_interaction.owns_pointer {
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
                        insertion: hit.insertion,
                    },
                )
            });
        }
    }
    if response.dragged()
        && !matrix_interaction.owns_pointer
        && let Some(anchor) = ui.data(|data| data.get_temp::<DragAnchor>(drag_id))
        && let Some(point) = response.interact_pointer_pos()
    {
        let matrix_range = hit_node(&hits, point)
            .and_then(|hit| matrix_cell_corners(editor.arena(), anchor.node, hit.node));
        if let Some((matrix, first, last)) = matrix_range {
            editor.select_matrix_cells(matrix, first, last);
        } else if let Some(hit) = hits
            .iter()
            .find(|h| h.node == anchor.node && h.insertion == anchor.insertion)
            && hit.atom.is_some()
            && (hit.rect.expand2(vec2(10000., 3.)).contains(point))
        {
            place_hit_selection(editor, hit, anchor.offset, hit_text_offset(hit, point));
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

    #[cfg(test)]
    drop(pointer_timing);
    #[cfg(test)]
    let event_timing = crate::frame_profile::scope(crate::frame_profile::Phase::FormulaEvent);
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
                    egui::Event::Key { key, modifiers, .. } => composing || (*key == Key::Enter && (modifiers.shift || (!modifiers.command && !modifiers.alt && editor.is_in_program()))) || !matches!(key, Key::Enter | Key::Escape),
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
        let revision_before_input = editor.arena().revision();
        let mut changed = false;
        let mut batch_geometry_dirty = false;
        for event in events {
            if batch_geometry_dirty
                && matches!(
                    &event,
                    egui::Event::Key {
                        key: Key::ArrowUp | Key::ArrowDown,
                        pressed: true,
                        ..
                    }
                )
            {
                initial = retained_layout(ui, editor, labelled, font_size, result);
                hits = retained_hits(ui, &initial, origin);
                batch_geometry_dirty = false;
            }
            let before_event_caret = editor.caret();
            let before_event_revision = editor.arena().revision();
            let source_navigation = matches!(&event, egui::Event::Key { key, pressed: true, modifiers, .. }
                if !modifiers.command && !modifiers.alt && !modifiers.shift
                    && matches!(key, Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown | Key::Tab));
            let source_back_tab = matches!(&event, egui::Event::Key { key: Key::Tab, pressed: true, modifiers, .. } if modifiers.shift_only());
            match event {
                egui::Event::Text(text) => {
                    if text == " "
                        && ui.input(|i| i.modifiers.shift)
                        && !editor.is_inside_string_literal()
                    {
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
                #[allow(deprecated)] // Accept cancellation from older input adapters too.
                egui::Event::Ime(egui::ImeEvent::Disabled) => {
                    preedit.clear();
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
                            if editor.matrix_selection().is_some() {
                                changed |= editor.delete_matrix_selection(true);
                            } else if editor.selection() == Selection::None
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
                    // Original STRING caret values disable mathematical
                    // gestures. Editing/navigation commands remain available.
                    if editor.is_inside_string_literal()
                        && (!matches!(
                            key,
                            Key::Backspace
                                | Key::Delete
                                | Key::ArrowLeft
                                | Key::ArrowRight
                                | Key::ArrowUp
                                | Key::ArrowDown
                                | Key::Home
                                | Key::End
                                | Key::Tab
                                | Key::A
                                | Key::Z
                                | Key::Y
                        ) || (key == Key::Z
                            && modifiers.command
                            && modifiers.shift
                            && !modifiers.alt))
                    {
                        continue;
                    }
                    if modifiers.command && !modifiers.alt && key == Key::Minus && !modifiers.shift
                    {
                        changed |= editor.toggle_literal_subscript();
                    } else if modifiers.command
                        && !modifiers.alt
                        && !modifiers.shift
                        && key == Key::J
                    {
                        changed |= editor.convert_program_keyword();
                    } else if modifiers.command
                        && !modifiers.alt
                        && !modifiers.shift
                        && key == Key::Q
                    {
                        changed |= editor.cycle_identifier_label();
                    } else if modifiers.command
                        && !modifiers.alt
                        && modifiers.shift
                        && key == Key::U
                    {
                        let revision = editor.arena().revision();
                        editor.insert_infix(BinaryOp::Scale);
                        changed |= editor.arena().revision() != revision;
                    } else if modifiers.command
                        && !modifiers.alt
                        && modifiers.shift
                        && key == Key::P
                    {
                        let revision = editor.arena().revision();
                        editor.insert_polar();
                        changed |= editor.arena().revision() != revision;
                    } else if modifiers.command
                        && !modifiers.alt
                        && modifiers.shift
                        && key == Key::Backtick
                    {
                        changed |= insert_root_command(editor, BinaryOp::GlobalDefine);
                    } else if modifiers.command
                        && !modifiers.alt
                        && modifiers.shift
                        && key == Key::Num6
                    {
                        editor.insert_vectorize();
                        changed = true;
                    } else if modifiers.command
                        && !modifiers.alt
                        && !modifiers.shift
                        && key == Key::Num8
                    {
                        editor.insert_cross_product();
                        changed = true;
                    } else if modifiers.command
                        && !modifiers.alt
                        && modifiers.shift
                        && matches!(key, Key::T | Key::R | Key::C | Key::Num9)
                    {
                        match key {
                            Key::T => {
                                editor.insert_transpose();
                            }
                            Key::R => {
                                editor.insert_row_operator();
                            }
                            Key::C => {
                                editor.insert_column_operator();
                            }
                            _ => {
                                editor.insert_conjugate();
                            }
                        }
                        changed = true;
                    } else if modifiers.command
                        && !modifiers.alt
                        && modifiers.shift
                        && matches!(key, Key::Pipe | Key::Backslash)
                    {
                        editor.insert_norm();
                        changed = true;
                    } else if modifiers.command
                        && !modifiers.alt
                        && key == Key::M
                        && !modifiers.shift
                    {
                        changed |= editor.insert_native_matrix();
                    } else if modifiers.command
                        && !modifiers.alt
                        && !modifiers.shift
                        && matches!(key, Key::Equals | Key::Num9 | Key::Num0)
                    {
                        let op = match key {
                            Key::Equals => BinaryOp::Equal,
                            Key::Num9 => BinaryOp::LessOrEqual,
                            _ => BinaryOp::GreaterOrEqual,
                        };
                        changed |= editor.insert_typed_operator(NodeKind::Binary(op));
                    } else if modifiers.command
                        && !modifiers.alt
                        && modifiers.shift
                        && matches!(key, Key::Num1 | Key::Num2 | Key::Num5 | Key::Num7)
                    {
                        if key == Key::Num1 {
                            changed |= editor.insert_typed_operator(NodeKind::LogicalNot);
                        } else {
                            changed |= editor.insert_typed_operator(NodeKind::Binary(match key {
                                Key::Num2 => BinaryOp::Or,
                                Key::Num5 => BinaryOp::Xor,
                                _ => BinaryOp::And,
                            }));
                        }
                    } else if modifiers.command
                        && modifiers.shift
                        && !modifiers.alt
                        && key == Key::Z
                    {
                        changed |= input_text(editor, "∞");
                    } else if modifiers.command && key == Key::Z {
                        changed |= if modifiers.shift {
                            editor.redo()
                        } else {
                            editor.undo()
                        };
                    } else if modifiers.command && key == Key::Y {
                        changed |= editor.redo();
                    } else if modifiers.command && key == Key::A {
                        let _ = editor.select_subtree(editor.root());
                    } else if modifiers.command
                        && !modifiers.alt
                        && !modifiers.shift
                        && matches!(key, Key::ArrowLeft | Key::ArrowRight)
                    {
                        move_by_term(editor, key == Key::ArrowRight);
                    } else if !modifiers.command && !modifiers.alt {
                        match key {
                            Key::Backspace => changed |= delete_character(editor, true),
                            Key::Delete => changed |= delete_character(editor, false),
                            Key::ArrowLeft | Key::ArrowRight if modifiers.shift => {
                                extend_selection(editor, key == Key::ArrowRight)
                            }
                            Key::ArrowLeft => move_horizontal(editor, false),
                            Key::ArrowRight => move_horizontal(editor, true),
                            Key::Enter if modifiers.shift => {
                                changed |= editor.insert_matrix_row(Direction::Indeterminate)
                            }
                            Key::Enter => changed |= editor.insert_program_line(),
                            Key::Space if modifiers.shift => {
                                changed |= editor.insert_matrix_column(Direction::Indeterminate)
                            }
                            Key::ArrowUp | Key::ArrowDown => {
                                if !source_matrix::move_vertical(
                                    editor,
                                    &hits,
                                    key == Key::ArrowDown,
                                ) {
                                    move_vertical(editor, &hits, key == Key::ArrowDown);
                                }
                            }
                            Key::Tab => {
                                if let Some(mutated) = editor.matrix_tab(!modifiers.shift) {
                                    changed |= mutated;
                                } else {
                                    cycle_leaf(editor, !modifiers.shift, true);
                                }
                            }
                            Key::Home => {
                                editor.move_to_expression_extreme(false);
                            }
                            Key::End => {
                                editor.move_to_expression_extreme(true);
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
            if source_navigation || source_back_tab {
                changed |= editor.reveal_source_matrix_caret_after_navigation(before_event_caret);
            }
            editor.clamp_source_matrix_caret();
            // Refresh lazily before the next geometry-dependent key, avoiding
            // extra layouts for ordinary scalar Text batches. Final paint already
            // uses the current Arena revision through retained_layout.
            batch_geometry_dirty |= editor.arena().revision() != before_event_revision;
        }
        ui.data_mut(|data| data.insert_temp(ime_id, preedit.clone()));
        if changed && editor.arena().revision() != revision_before_input {
            response.mark_changed();
            ui.ctx().request_repaint();
        }
    } else {
        ui.data_mut(|data| data.remove::<String>(ime_id));
        preedit.clear();
    }

    #[cfg(test)]
    drop(event_timing);
    #[cfg(test)]
    let relayout_timing = crate::frame_profile::scope(crate::frame_profile::Phase::FormulaRelayout);
    let layout = if response.changed() {
        retained_layout(ui, editor, labelled, font_size, result)
    } else {
        initial
    };
    let hits = retained_hits(ui, &layout, origin);
    #[cfg(test)]
    drop(relayout_timing);
    #[cfg(test)]
    let _paint_timing = crate::frame_profile::scope(crate::frame_profile::Phase::FormulaPaint);
    let paint = ui.painter();
    if response.has_focus() {
        if let Selection::Subtree(node) = editor.selection()
            && let Some(hit) = hits.iter().find(|hit| hit.node == node)
        {
            paint.rect_filled(
                hit.rect.expand(1.0),
                0.,
                Color32::from_gray(169).gamma_multiply(0.25),
            );
        } else if let Some(selection) = editor.matrix_selection()
            && let Some(matrix) = editor.arena().get(selection.matrix)
            && let NodeKind::Matrix { cols, .. } = matrix.kind()
        {
            let (first_row, last_row) = (
                (selection.anchor / cols).min(selection.focus / cols),
                (selection.anchor / cols).max(selection.focus / cols),
            );
            let (first_col, last_col) = (
                (selection.anchor % cols).min(selection.focus % cols),
                (selection.anchor % cols).max(selection.focus % cols),
            );
            let selected: std::collections::HashSet<_> = (first_row..=last_row)
                .flat_map(|row| {
                    (first_col..=last_col).map(move |col| matrix.children()[row * cols + col])
                })
                .collect();
            for hit in hits.iter().filter(|hit| {
                hit.insertion == InsertionMode::Normal && selected.contains(&hit.node)
            }) {
                paint.rect_filled(hit.rect.expand(1.), 0., CARET.gamma_multiply(0.18));
            }
        }
    }
    paint_box_with_focus(paint, &layout, origin, response.has_focus());
    if response.has_focus()
        && let Some(target) = editor.typeover_target()
        && let Some(hit) = hits.iter().find(|hit| {
            hit.node == target && hit.operator_side.is_none() && hit.operator_part.is_none()
        })
    {
        // Native VisualScoper uses a TypeoverColor bounds highlighter, not
        // a glyph foreground override. Defaults are Red and opacity0.25.
        let mut bounds = hit.rect;
        if let Some(excluded) = editor.typeover_excluded_operand()
            && let Some(base) = hits.iter().find(|hit| {
                hit.node == excluded && hit.operator_side.is_none() && hit.operator_part.is_none()
            })
        {
            // Power's selected analysis handle includes its script, not the
            // preceding base. Keep that retained operand outside the overlay.
            bounds.min.x = bounds.min.x.max(base.rect.right());
        }
        paint.rect_filled(bounds, 0., Color32::from_rgba_unmultiplied(255, 0, 0, 64));
    }
    let caret_node = match editor.selection() {
        Selection::Subtree(id) => id,
        Selection::MatrixCells(_) => editor.caret().node,
        Selection::None => editor.caret().node,
    };
    let ordinary_operator = if editor.selection() == Selection::None
        && editor.caret().caret_side == Direction::Indeterminate
        && editor.caret().value_range.is_none()
        && editor.caret().operator_part.is_none()
    {
        hits.iter().find(|hit| {
            hit.node == caret_node && hit.operator_side == Some(editor.caret().node_side)
        })
    } else {
        None
    };
    if response.has_focus()
        && ordinary_operator.is_some()
        && matches!(editor.arena().get(caret_node).unwrap().kind(), NodeKind::Binary(op)
            if !matches!(op, BinaryOp::Define | BinaryOp::GlobalDefine | BinaryOp::Evaluate))
    {
        paint_operator_selection(paint, &layout, origin, caret_node);
    }
    if response.has_focus()
        && editor.typeover_target().is_none()
        && let Some(hit) = ordinary_operator.or_else(|| {
            hits.iter().find(|hit| {
                hit.node == caret_node
                    && hit.insertion == editor.caret().insertion
                    && hit.operator_side.is_none()
                    && hit.operator_part
                        == if matches!(
                            editor.arena().get(caret_node).unwrap().kind(),
                            NodeKind::Try
                        ) && editor.caret().caret_side == Direction::Indeterminate
                        {
                            match editor.caret().node_side {
                                Direction::Left => Some(0),
                                Direction::Right => Some(1),
                                _ => None,
                            }
                        } else {
                            editor.caret().operator_part
                        }
            })
        })
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
        layout_size: desired,
        evaluation_requested,
        matrix_view_gesture: matrix_interaction.gesture,
    }
}

fn text_galley(painter: &Painter, text: &str, size: f32) -> Arc<Galley> {
    typography::layout_no_wrap(painter, text.into(), MathStyle::Operator.font(size), INK)
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
        readonly: false,
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
    readonly: bool,
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
            subscript: None,
            operator_hits: vec![],
            matrix_viewport: None,
        };
        match node.kind() {
            NodeKind::StringLiteral(text) => {
                let font = MathStyle::Operator.font(size);
                let opening = typography::layout_no_wrap(painter, "“".into(), font.clone(), INK);
                let closing = typography::layout_no_wrap(painter, "”".into(), font.clone(), INK);
                let body = typography::layout_no_wrap(painter, text.clone(), font, INK);
                let body_width = body.size().x.max(4.0);
                result.size = vec2(
                    opening.size().x + body_width + closing.size().x,
                    opening.size().y.max(body.size().y).max(closing.size().y),
                );
                result.baseline = text_baseline(&opening);
                result.text_offset = vec2(opening.size().x, 0.0);
                result
                    .primitives
                    .push(Primitive::Text(Vec2::ZERO, opening.clone()));
                result
                    .primitives
                    .push(Primitive::Text(result.text_offset, body.clone()));
                result.primitives.push(Primitive::Text(
                    vec2(opening.size().x + body_width, 0.0),
                    closing,
                ));
                result.atom = Some(body);
            }
            NodeKind::Identifier(text)
            | NodeKind::BasedNumber { literal: text, .. }
            | NodeKind::Number(text)
            | NodeKind::AnnotatedIdentifier { name: text, .. }
            | NodeKind::NativeIdentifier { name: text, .. }
            | NodeKind::Imaginary(text)
            | NodeKind::ImaginarySymbol { value: text, .. } => {
                let style = self.identifier_style(id);
                let galley = typography::layout_no_wrap(
                    painter,
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
                    let suffix = typography::layout_no_wrap(
                        painter,
                        if suffix.is_empty() {
                            " ".into()
                        } else {
                            suffix.into()
                        },
                        style.font(font_size),
                        style.color(),
                    );
                    let original = result.size;
                    if matches!(
                        node.kind(),
                        NodeKind::NativeIdentifier {
                            subscript: Some(_),
                            ..
                        }
                    ) {
                        result.subscript = Some((vec2(original.x, y), suffix.clone()));
                    }
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
            NodeKind::Program { .. } => {
                let lines: Vec<_> = node
                    .children()
                    .iter()
                    .map(|child| self.measure(*child, size))
                    .collect();
                let indent = size * 0.55;
                let gap = size * 0.28;
                let width = lines.iter().map(|line| line.size.x).fold(0.0_f32, f32::max) + indent;
                let height = lines.iter().map(|line| line.size.y).sum::<f32>()
                    + gap * lines.len().saturating_sub(1) as f32;
                result.size = vec2(width.max(size), height.max(size));
                result.baseline = lines.first().map_or(size * 0.7, |line| line.baseline);
                // Original PROGRAM/BODY both use the double vertical fence.
                // These two vector strokes approximate its licensed font
                // glyph; BODY suppresses only the right minor fence.
                for x in [indent * 0.22, indent * 0.44] {
                    result
                        .primitives
                        .push(Primitive::Line(vec2(x, 0.), vec2(x, result.size.y)));
                }
                let is_body = node
                    .parent()
                    .and_then(|parent| arena.get(parent))
                    .is_some_and(|parent| {
                        let index = parent
                            .children()
                            .iter()
                            .position(|child| *child == id)
                            .unwrap();
                        match parent.kind() {
                            NodeKind::If {
                                branch_count,
                                has_else,
                                ..
                            } => index % 2 == 1 || (*has_else && index == branch_count * 2),
                            NodeKind::For => index == 2,
                            NodeKind::While => index == 1,
                            NodeKind::Try => true,
                            _ => false,
                        }
                    });
                if !is_body {
                    result.primitives.push(Primitive::MinorFence(
                        vec2(result.size.x, 0.),
                        vec2(result.size.x, result.size.y),
                    ));
                }
                let mut y = 0.0;
                for line in lines {
                    let line_height = line.size.y;
                    result.children.push((vec2(indent, y), line));
                    y += line_height + gap;
                }
            }
            NodeKind::LocalDefine => {
                result = row_layout(
                    id,
                    vec![
                        RowPiece::Child(self.measure(node.children()[0], size)),
                        RowPiece::Text(text_galley(painter, " ← ", size)),
                        RowPiece::Child(self.measure(node.children()[1], size)),
                    ],
                );
            }
            NodeKind::Try => {
                // VisualTryNode has two headers and two BODY rows, in this
                // order. The native node name is tryCatch; "catch" is not
                // its displayed second keyword. Static contract 0103.
                let indent = 8. * size / typography::MATH_SIZE;
                let mut y = 0.;
                for (index, word) in ["try", "on error"].iter().enumerate() {
                    let header = text_galley(painter, word, size);
                    result.operator_hits.push((
                        OperatorTarget::Part(index),
                        Rect::from_min_size(pos2(0., y), header.size()),
                    ));
                    if index == 0 {
                        result.baseline = text_baseline(&header);
                    }
                    result
                        .primitives
                        .push(Primitive::Text(vec2(0., y), header.clone()));
                    result.size.x = result.size.x.max(header.size().x);
                    y += header.size().y;
                    let body = self.measure(node.children()[index], size);
                    result.size.x = result.size.x.max(indent + body.size.x);
                    let height = body.size.y;
                    result.children.push((vec2(indent, y), body));
                    y += height;
                }
                result.size.y = y;
                result.primitives.push(Primitive::MinorFence(
                    vec2(result.size.x, 0.),
                    vec2(result.size.x, y),
                ));
            }
            NodeKind::If {
                has_else,
                branch_count,
                alternate,
            } => {
                // Original VisualPairNaryNode lays each keyword/test on one
                // baseline, then its BODY on the next row. "then" is a native
                // XML wrapper, not visible operator text (REA contract 0099).
                let scale = size / typography::MATH_SIZE;
                let test_gap = 4. * scale;
                let body_indent = 8. * scale;
                let mut y = 0.;
                for index in 0..*branch_count {
                    let word = if index == 0 {
                        "if"
                    } else if *alternate {
                        "also if"
                    } else {
                        "else if"
                    };
                    let text = text_galley(painter, word, size);
                    let test = self.measure(node.children()[index * 2], size);
                    let baseline = text_baseline(&text).max(test.baseline);
                    if index == 0 {
                        result.baseline = baseline;
                    }
                    let text_y = y + baseline - text_baseline(&text);
                    let test_y = y + baseline - test.baseline;
                    let height = (text_y + text.size().y).max(test_y + test.size.y) - y;
                    let width = text.size().x + test_gap + test.size.x;
                    result.operator_hits.push((
                        OperatorTarget::Part(index),
                        Rect::from_min_size(pos2(0., text_y), text.size()),
                    ));
                    result
                        .primitives
                        .push(Primitive::Text(vec2(0., text_y), text.clone()));
                    result
                        .children
                        .push((vec2(text.size().x + test_gap, test_y), test));
                    y += height;
                    let body = self.measure(node.children()[index * 2 + 1], size);
                    result.size.x = result.size.x.max(width).max(body_indent + body.size.x);
                    let body_height = body.size.y;
                    result.children.push((vec2(body_indent, y), body));
                    y += body_height;
                }
                if *has_else {
                    let text = text_galley(painter, "else", size);
                    result.operator_hits.push((
                        OperatorTarget::Part(*branch_count),
                        Rect::from_min_size(pos2(0., y), text.size()),
                    ));
                    result
                        .primitives
                        .push(Primitive::Text(vec2(0., y), text.clone()));
                    y += text.size().y;
                    let body = self.measure(node.children()[branch_count * 2], size);
                    result.size.x = result
                        .size
                        .x
                        .max(text.size().x)
                        .max(body_indent + body.size.x);
                    let body_height = body.size.y;
                    result.children.push((vec2(body_indent, y), body));
                    y += body_height;
                }
                result.size.y = y;
                result.primitives.push(Primitive::MinorFence(
                    vec2(result.size.x, 0.),
                    vec2(result.size.x, y),
                ));
            }
            NodeKind::For => {
                result = row_layout(
                    id,
                    vec![
                        RowPiece::Text(text_galley(painter, "for ", size)),
                        RowPiece::Child(self.measure(node.children()[0], size)),
                        RowPiece::Text(text_galley(painter, " in ", size)),
                        RowPiece::Child(self.measure(node.children()[1], size)),
                        RowPiece::Text(text_galley(painter, " ", size)),
                        RowPiece::Child(self.measure(node.children()[2], size)),
                    ],
                );
            }
            NodeKind::While => {
                result = row_layout(
                    id,
                    vec![
                        RowPiece::Text(text_galley(painter, "while ", size)),
                        RowPiece::Child(self.measure(node.children()[0], size)),
                        RowPiece::Text(text_galley(painter, " ", size)),
                        RowPiece::Child(self.measure(node.children()[1], size)),
                    ],
                );
            }
            NodeKind::Break | NodeKind::Continue => {
                result = row_layout(
                    id,
                    vec![RowPiece::Text(text_galley(
                        painter,
                        if matches!(node.kind(), NodeKind::Break) {
                            "break"
                        } else {
                            "continue"
                        },
                        size,
                    ))],
                );
            }
            NodeKind::Return => {
                result = row_layout(
                    id,
                    vec![
                        RowPiece::Text(text_galley(painter, "return ", size)),
                        RowPiece::Child(self.measure(node.children()[0], size)),
                    ],
                );
            }
            NodeKind::Integral | NodeKind::Summation | NodeKind::Product => {
                let operator = match node.kind() {
                    NodeKind::Integral => "∫",
                    NodeKind::Summation => "∑",
                    NodeKind::Product => "∏",
                    _ => unreachable!(),
                };
                let suffix = if matches!(node.kind(), NodeKind::Integral) {
                    " d"
                } else {
                    ", "
                };
                result = row_layout(
                    id,
                    vec![
                        RowPiece::Text(text_galley(painter, operator, size * 1.25)),
                        RowPiece::Text(text_galley(painter, "[", size)),
                        RowPiece::Child(self.measure(node.children()[2], size * 0.72)),
                        RowPiece::Text(text_galley(painter, ",", size * 0.72)),
                        RowPiece::Child(self.measure(node.children()[3], size * 0.72)),
                        RowPiece::Text(text_galley(painter, "] ", size)),
                        RowPiece::Child(self.measure(node.children()[0], size)),
                        RowPiece::Text(text_galley(painter, suffix, size)),
                        RowPiece::Child(self.measure(node.children()[1], size)),
                    ],
                );
            }
            NodeKind::Derivative { partial } => {
                let d = if *partial { "∂" } else { "d" };
                result = row_layout(
                    id,
                    vec![
                        RowPiece::Text(text_galley(painter, d, size)),
                        RowPiece::Child(self.measure(node.children()[2], size * 0.72)),
                        RowPiece::Text(text_galley(painter, &format!("/{d}"), size)),
                        RowPiece::Child(self.measure(node.children()[1], size)),
                        RowPiece::Text(text_galley(painter, " ", size)),
                        RowPiece::Child(self.measure(node.children()[0], size)),
                    ],
                );
            }
            NodeKind::SymbolicEvaluation { .. } => {
                let mut pieces = vec![
                    RowPiece::Child(self.measure(node.children()[0], size)),
                    RowPiece::Text(text_galley(painter, " → ", size)),
                ];
                for (index, command) in node.children()[1..].iter().enumerate() {
                    if index > 0 {
                        pieces.push(RowPiece::Text(text_galley(painter, ", ", size)));
                    }
                    pieces.push(RowPiece::Child(self.measure(*command, size * 0.82)));
                }
                if let Some(value) = result_text {
                    pieces.push(RowPiece::Text(text_galley(painter, "  ", size)));
                    pieces.push(RowPiece::Child(measure_result(painter, id, value, size)));
                }
                result = row_layout(id, pieces);
            }
            NodeKind::SymbolicCommand { .. } => {
                let mut pieces = vec![];
                for (index, child) in node.children().iter().enumerate() {
                    if index > 0 {
                        pieces.push(RowPiece::Text(text_galley(painter, ", ", size)));
                    }
                    pieces.push(RowPiece::Child(self.measure(*child, size)));
                }
                result = row_layout(id, pieces);
            }
            NodeKind::UnaryNeg | NodeKind::LogicalNot => {
                let mut child = self.measure(node.children()[0], size);
                if arena.get(node.children()[0]).unwrap().kind().precedence()
                    < node.kind().precedence()
                {
                    child = visual_parentheses(painter, child, size);
                }
                result = row_layout(
                    id,
                    vec![
                        RowPiece::Text(text_galley(
                            painter,
                            if matches!(node.kind(), NodeKind::LogicalNot) {
                                "¬"
                            } else {
                                "−"
                            },
                            size,
                        )),
                        RowPiece::Child(child),
                    ],
                );
            }
            NodeKind::Absolute | NodeKind::Determinant => {
                result = visual_bars(
                    id,
                    self.measure(node.children()[0], size),
                    matches!(node.kind(), NodeKind::Determinant),
                );
            }
            NodeKind::Conjugate => {
                result = visual_overline(id, self.measure(node.children()[0], size));
            }
            NodeKind::Vectorize => {
                result =
                    visual_overarrow(painter, id, self.measure(node.children()[0], size), size);
            }
            NodeKind::Factorial | NodeKind::Percent | NodeKind::Degree => {
                let mut child = self.measure(node.children()[0], size);
                if (arena.get(node.children()[0]).unwrap().kind().precedence()
                    < node.kind().precedence()
                    && !(matches!(node.kind(), NodeKind::Degree)
                        && matches!(
                            arena.get(node.children()[0]).unwrap().kind(),
                            NodeKind::Power
                        )))
                    || (matches!(node.kind(), NodeKind::Percent)
                        && matches!(
                            arena.get(node.children()[0]).unwrap().kind(),
                            NodeKind::Power | NodeKind::Root
                        ))
                {
                    child = visual_parentheses(painter, child, size);
                }
                let symbol = match node.kind() {
                    NodeKind::Percent => "%",
                    NodeKind::Degree => "°",
                    _ => "!",
                };
                result = row_layout(
                    id,
                    vec![
                        RowPiece::Child(child),
                        RowPiece::Text(text_galley(painter, symbol, size)),
                    ],
                );
            }
            NodeKind::Transpose => {
                let mut child = self.measure(node.children()[0], size);
                if array_script_requires_group(
                    node.kind(),
                    arena.get(node.children()[0]).unwrap().kind(),
                ) {
                    child = visual_parentheses(painter, child, size);
                }
                let script = text_galley(painter, "T", typography::script_size(size, 7. / 11.));
                let y = script.size().y * 0.62;
                result.size = vec2(
                    child.size.x + script.size().x + 1.,
                    (y + child.size.y).max(script.size().y),
                );
                result.baseline = y + child.baseline;
                result
                    .primitives
                    .push(Primitive::Text(vec2(child.size.x + 1., 0.), script));
                result.children.push((vec2(0., y), child));
            }
            NodeKind::Row | NodeKind::Column => {
                let mut base = self.measure(node.children()[0], size);
                if array_script_requires_group(
                    node.kind(),
                    arena.get(node.children()[0]).unwrap().kind(),
                ) {
                    base = visual_parentheses(painter, base, size);
                }
                let script_size = typography::script_size(size, 7. / 11.);
                let index = self.measure(node.children()[1], script_size);
                let index = visual_angle_fences(
                    id,
                    index,
                    matches!(node.kind(), NodeKind::Row),
                    script_size,
                );
                let y = index.size.y * 0.62;
                result.size = vec2(
                    base.size.x + index.size.x + 1.,
                    (base.size.y + y).max(index.size.y),
                );
                result.baseline = base.baseline + y;
                let x = base.size.x + 1.;
                result.children = vec![(vec2(0., y), base), (vec2(x, 0.), index)];
            }
            NodeKind::Root => {
                let degree_kind = arena.get(node.children()[0]).unwrap().kind();
                let square = matches!(degree_kind, NodeKind::Number(n) if n == "2")
                    || (self.readonly && degree_kind == &NodeKind::Placeholder);
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
                        Primitive::FocusedText(x, g) => Primitive::FocusedText(x + offset, g),
                        other => other,
                    }));
                result
                    .children
                    .extend(index.children.into_iter().map(|(x, b)| (x + offset, b)));
            }
            NodeKind::Matrix {
                rows,
                cols,
                display: Some(display),
            } if display.window([*rows, *cols]) != Ok(([0, 0], [*rows, *cols])) => {
                result = source_matrix::measure(self, id, [*rows, *cols], *display, size);
            }
            NodeKind::Matrix { rows, cols, .. } => {
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
                result.operator_hits.push((
                    OperatorTarget::Side(Direction::Left),
                    Rect::from_min_size(Pos2::ZERO, vec2(width, height)),
                ));
                result.operator_hits.push((
                    OperatorTarget::Side(Direction::Right),
                    Rect::from_min_size(pos2(result.size.x - width, 0.), vec2(width, height)),
                ));
                result.children.push((offset, child));
            }
            NodeKind::Binary(op) => {
                let mut left = self.measure(node.children()[0], size);
                let mut right = if matches!(op, BinaryOp::Define | BinaryOp::GlobalDefine) {
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
                                    Primitive::Text(offset, _)
                                    | Primitive::FocusedText(offset, _) => {
                                        *offset += vec2(prefix, unit_y)
                                    }
                                    Primitive::Line(a, b) | Primitive::MinorFence(a, b) => {
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
                            for (_, rect) in &mut right.operator_hits {
                                *rect = rect.translate(vec2(prefix, unit_y));
                            }
                            if let Some((offset, _)) = &mut right.subscript {
                                *offset += vec2(prefix, unit_y);
                            }
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
                if cross_operand_requires_group(
                    *op,
                    arena.get(node.children()[0]).unwrap().kind(),
                    true,
                ) {
                    left = visual_parentheses(painter, left, size);
                }
                if cross_operand_requires_group(
                    *op,
                    arena.get(node.children()[1]).unwrap().kind(),
                    false,
                ) {
                    right = visual_parentheses(painter, right, size);
                }
                let symbol = match op {
                    BinaryOp::Add => "+",
                    BinaryOp::Subtract => "−",
                    BinaryOp::Multiply => "·",
                    BinaryOp::Scale => " ",
                    BinaryOp::InlineDivide => "÷",
                    BinaryOp::Define => ":=",
                    BinaryOp::GlobalDefine => "≡",
                    BinaryOp::CrossProduct => "⨯",
                    BinaryOp::Polar => "∠",
                    BinaryOp::Evaluate => "=",
                    BinaryOp::Equal => "＝",
                    BinaryOp::NotEqual => "≠",
                    BinaryOp::LessThan => "<",
                    BinaryOp::LessOrEqual => "≤",
                    BinaryOp::GreaterThan => ">",
                    BinaryOp::GreaterOrEqual => "≥",
                    BinaryOp::And => "∧",
                    BinaryOp::Or => "∨",
                    BinaryOp::Xor => "⊕",
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
                // Native linear binary hit testing uses the symbol's x bounds
                // across this node, rather than half the whole expression.
                result.operator_hits.push((
                    OperatorTarget::Side(Direction::Indeterminate),
                    Rect::from_min_size(
                        pos2(operator_x, 0.),
                        vec2(operator.size().x, result.size.y),
                    ),
                ));
                result.primitives.push(Primitive::Text(
                    vec2(operator_x, baseline - operator_baseline),
                    operator,
                ));
                if *op == BinaryOp::Scale {
                    // Original SCALE uses U+2218 and ShowOnlyWhenFocused.
                    // Retain the existing implicit spacing when hidden and
                    // overlay the focus marker in that slot; exact native
                    // font/margin geometry remains an independent approximation.
                    let marker = text_galley(painter, "∘", size);
                    let slot_width = right_x - operator_x - gap;
                    result.primitives.push(Primitive::FocusedText(
                        vec2(
                            operator_x + (slot_width - marker.size().x) * 0.5,
                            baseline - text_baseline(&marker),
                        ),
                        marker,
                    ));
                }
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
        Primitive::FocusedText(p, g) => Primitive::FocusedText(p + by, g),
        Primitive::Line(a, b) => Primitive::Line(a + by, b + by),
        Primitive::MinorFence(a, b) => Primitive::MinorFence(a + by, b + by),
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
        subscript: None,
        operator_hits: vec![],
        matrix_viewport: None,
    }
}

fn array_script_requires_group(operator: &NodeKind, child: &NodeKind) -> bool {
    child.precedence() < operator.precedence()
        || matches!(
            (operator, child),
            (NodeKind::Transpose, NodeKind::Row | NodeKind::Column)
                | (
                    NodeKind::Row | NodeKind::Column,
                    NodeKind::Transpose | NodeKind::Row | NodeKind::Column
                )
        )
}

fn cross_operand_requires_group(operator: BinaryOp, child: &NodeKind, left: bool) -> bool {
    if matches!(operator, BinaryOp::CrossProduct | BinaryOp::Polar) {
        child.precedence() < NodeKind::Binary(operator).precedence()
            || (left
                && operator == BinaryOp::CrossProduct
                && matches!(
                    child,
                    NodeKind::Binary(BinaryOp::CrossProduct | BinaryOp::Polar)
                ))
            || (!left && child == &NodeKind::Binary(BinaryOp::Polar))
    } else {
        matches!(
            child,
            NodeKind::Binary(BinaryOp::CrossProduct | BinaryOp::Polar)
        ) && child.precedence() < NodeKind::Binary(operator).precedence()
    }
}

fn visual_bars(node: NodeId, child: FormulaBox, double: bool) -> FormulaBox {
    let mut output = empty_display(node);
    let margin = if double { 7. } else { 5. };
    output.size = child.size + vec2(2. * margin, 0.);
    output.baseline = child.baseline;
    let mut positions = vec![2., output.size.x - 2.];
    if double {
        positions.extend([4., output.size.x - 4.]);
    }
    for x in positions {
        output
            .primitives
            .push(Primitive::Line(vec2(x, 0.), vec2(x, child.size.y)));
    }
    output.children.push((vec2(margin, 0.), child));
    output
}

fn visual_angle_fences(node: NodeId, child: FormulaBox, vertical: bool, size: f32) -> FormulaBox {
    let mut output = empty_display(node);
    let depth = (size * 0.22).max(1.5);
    let gap = (size * 0.1).max(1.);
    if vertical {
        output.size = child.size + vec2(2. * gap, 2. * (depth + gap));
        output.baseline = child.baseline + depth + gap;
        let w = output.size.x;
        let h = output.size.y;
        for points in [
            [vec2(0., depth), vec2(w / 2., 0.), vec2(w, depth)],
            [vec2(0., h - depth), vec2(w / 2., h), vec2(w, h - depth)],
        ] {
            for pair in points.windows(2) {
                output.primitives.push(Primitive::Line(pair[0], pair[1]));
            }
        }
        output.children.push((vec2(gap, depth + gap), child));
    } else {
        output.size = child.size + vec2(2. * (depth + gap), 0.);
        output.baseline = child.baseline;
        let w = output.size.x;
        let h = output.size.y;
        for points in [
            [vec2(depth, 0.), vec2(0., h / 2.), vec2(depth, h)],
            [vec2(w - depth, 0.), vec2(w, h / 2.), vec2(w - depth, h)],
        ] {
            for pair in points.windows(2) {
                output.primitives.push(Primitive::Line(pair[0], pair[1]));
            }
        }
        output.children.push((vec2(depth + gap, 0.), child));
    }
    output
}

fn visual_overline(node: NodeId, child: FormulaBox) -> FormulaBox {
    let mut output = empty_display(node);
    output.size = child.size + vec2(0., 2.);
    output.baseline = child.baseline + 2.;
    output
        .primitives
        .push(Primitive::Line(vec2(0., 0.5), vec2(child.size.x, 0.5)));
    output.children.push((vec2(0., 2.), child));
    output
}

fn visual_overarrow(painter: &Painter, node: NodeId, child: FormulaBox, size: f32) -> FormulaBox {
    let mut output = empty_display(node);
    let width = child.size.x.max(text_galley(painter, "→", size).size().x);
    let head = (size * 0.2).min(width).max(0.);
    let top = 4.;
    let y = top * 0.5;
    output.size = vec2(width, child.size.y + top);
    output.baseline = child.baseline + top;
    output.primitives.extend([
        Primitive::Line(vec2(0., y), vec2(width, y)),
        Primitive::Line(vec2(width - head, 0.), vec2(width, y)),
        Primitive::Line(vec2(width - head, top), vec2(width, y)),
    ]);
    output
        .children
        .push((vec2((width - child.size.x) * 0.5, top), child));
    output
}
fn flatten_display(mut layout: FormulaBox) -> FormulaBox {
    for (offset, child) in std::mem::take(&mut layout.children) {
        let child = flatten_display(child);
        if layout.matrix_viewport.is_none() {
            layout.matrix_viewport = child
                .matrix_viewport
                .map(|g| Box::new(g.translated(offset)));
        }
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
            let galley =
                typography::layout_no_wrap(painter, text.clone(), style.font(size), style.color());
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
        DisplayMath::Overline(child) => {
            result = visual_overline(node, measure_display_math(painter, node, child, size));
        }
        DisplayMath::OverArrow(child) => {
            result = visual_overarrow(
                painter,
                node,
                measure_display_math(painter, node, child, size),
                size,
            );
        }
        DisplayMath::Bars(child, double) => {
            result = visual_bars(
                node,
                measure_display_math(painter, node, child, size),
                *double,
            );
        }
        DisplayMath::AngleFence(child, vertical) => {
            result = visual_angle_fences(
                node,
                measure_display_math(painter, node, child, size),
                *vertical,
                size,
            );
        }
    }
    flatten_display(result)
}
pub(crate) fn readonly_result_ui(
    ui: &mut Ui,
    result: &crate::result_display::FormattedResult,
    font_points: f32,
) -> Response {
    let editor = Editor::new();
    let layout = measure_result(
        ui.painter(),
        editor.root(),
        FormulaResult::Formatted(result),
        font_points.clamp(1., 200.) * 96. / 72.,
    );
    let (rect, response) = ui.allocate_exact_size(layout.size + vec2(4., 4.), Sense::hover());
    paint_box(ui.painter(), &layout, rect.min + vec2(2., 2.));
    response
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
        || rows.saturating_mul(cols) > 65_536
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
        FormulaResult::MatrixViewport(result) => {
            matrix_viewport::measure(painter, node, result, size)
        }
        FormulaResult::MatrixViewportWithIndices {
            projection,
            indices,
        } => matrix_viewport::measure_indexed(painter, node, projection, indices, size),
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
    for (_, rect) in &mut child.operator_hits {
        *rect = rect.translate(offset);
    }
    if let Some((subscript_offset, _)) = &mut child.subscript {
        *subscript_offset += offset;
    }
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
        subscript: None,
        operator_hits: vec![],
        matrix_viewport: None,
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
        insertion: InsertionMode::Normal,
        operator_part: None,
        operator_side: None,
    });
    if let Some((offset, galley)) = &layout.subscript {
        let text_origin = origin + *offset;
        hits.push(HitNode {
            node: layout.node,
            rect: Rect::from_min_size(text_origin, galley.size().max(vec2(4., 8.))),
            atom: Some(galley.clone()),
            text_origin,
            insertion: InsertionMode::Subscript,
            operator_part: None,
            operator_side: None,
        });
    }
    for (target, rect) in &layout.operator_hits {
        hits.push(HitNode {
            node: layout.node,
            rect: rect.translate(origin.to_vec2()),
            atom: None,
            text_origin: origin,
            insertion: InsertionMode::Normal,
            operator_part: match target {
                OperatorTarget::Part(part) => Some(*part),
                _ => None,
            },
            operator_side: match target {
                OperatorTarget::Side(side) => Some(*side),
                _ => None,
            },
        });
    }
    for (offset, child) in &layout.children {
        collect_hits(child, origin + *offset, hits);
    }
}

fn hit_node(hits: &[HitNode], point: Pos2) -> Option<&HitNode> {
    hits.iter()
        .filter(|hit| {
            if hit.operator_side.is_some() {
                point.x >= hit.rect.left()
                    && point.x < hit.rect.right()
                    && point.y >= hit.rect.top()
                    && point.y <= hit.rect.bottom()
            } else {
                hit.rect.expand(2.0).contains(point)
            }
        })
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

fn place_hit_selection(editor: &mut Editor, hit: &HitNode, anchor: usize, focus: usize) {
    if hit.insertion == InsertionMode::Subscript {
        let _ = editor.place_subscript_selection(hit.node, anchor, focus);
    } else {
        let _ = editor.place_text_selection(hit.node, anchor, focus);
    }
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

/// Resolve drag endpoints to cells of their nearest shared matrix. An inner
/// matrix remains one operand of an outer cell until a drag crosses that cell.
fn matrix_cell_corners(
    arena: &Arena,
    anchor: NodeId,
    focus: NodeId,
) -> Option<(NodeId, usize, usize)> {
    let mut child = anchor;
    while let Some(parent) = arena.get(child)?.parent() {
        let node = arena.get(parent)?;
        if matches!(node.kind(), NodeKind::Matrix { .. }) {
            let first = node.children().iter().position(|id| *id == child)?;
            if let Some(last) = node
                .children()
                .iter()
                .position(|id| node_contains(arena, *id, focus))
            {
                if first != last {
                    return Some((parent, first, last));
                }
                // Both endpoints are in the same cell of this matrix; a higher
                // matrix also sees one cell and must not replace the operand.
                return None;
            }
        }
        child = parent;
    }
    None
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
    if matches!(
        editor.arena().get(hit.node).map(|n| n.kind()),
        Some(NodeKind::StringLiteral(_))
    ) && matches!(
        editor.caret().caret_side,
        Direction::Left | Direction::Right
    ) {
        let x = if editor.caret().caret_side == Direction::Left {
            hit.rect.left()
        } else {
            hit.rect.right()
        };
        return Rect::from_min_max(pos2(x, hit.rect.top()), pos2(x + 1., hit.rect.bottom()));
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
    let Some(here) = hits
        .iter()
        .find(|h| h.node == editor.caret().node && h.insertion == editor.caret().insertion)
    else {
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
            place_hit_selection(editor, hit, offset, offset);
        } else {
            let _ = editor.place_caret(hit.node, Direction::Right);
        }
    }
}

fn extend_text_selection(editor: &mut Editor, forward: bool) -> bool {
    let node = editor.caret().node;
    let Some(text) = atomic_text(editor, node) else {
        return false;
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
        return false;
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
        let anchor = range.map_or(offset, |range| range.anchor);
        if editor.caret().insertion == InsertionMode::Subscript {
            let _ = editor.place_subscript_selection(node, anchor, focus);
        } else {
            let _ = editor.place_text_selection(node, anchor, focus);
        }
        true
    } else {
        false
    }
}

/// Grow a character selection into a structural operand selection at its
/// boundary. The whole-subtree step is the bounded representation supported by
/// the current editor; adjacent partial operand ranges remain future work.
fn extend_selection(editor: &mut Editor, forward: bool) {
    if editor
        .move_literal_caret(
            if forward {
                Direction::Right
            } else {
                Direction::Left
            },
            true,
        )
        .is_some()
    {
        return;
    }
    // Native matrix-cell typeover inherits empty SelectLeft/SelectRight
    // handlers. Shift+Arrow must not promote the rectangle to a whole subtree.
    if editor.matrix_selection().is_some() {
        return;
    }
    if editor.selection() != Selection::None {
        editor.expand_scope();
        return;
    }
    if extend_text_selection(editor, forward) {
        return;
    }
    let node = editor.caret().node;
    if let Some(text) = atomic_text(editor, node) {
        let length = text.encode_utf16().count();
        let range = editor.caret().value_range;
        let whole_atom = range.is_some_and(|range| {
            range.anchor.min(range.focus) == 0 && range.anchor.max(range.focus) == length
        });
        if whole_atom || length == 0 {
            let origin = range.map_or_else(
                || {
                    if editor.caret().caret_side == Direction::Left {
                        0
                    } else {
                        length
                    }
                },
                |range| range.anchor,
            );
            let side = if origin.saturating_mul(2) < length {
                Direction::Left
            } else {
                Direction::Right
            };
            let _ = editor.select_subtree_from(node, side);
            return;
        }
    }
    editor.expand_scope();
}

fn paint_box(painter: &Painter, layout: &FormulaBox, origin: Pos2) {
    paint_box_with_focus(painter, layout, origin, false);
}

// C 05cd/052e: selected operator geometry uses CaretColor, while its operands
// receive rectangular NearestNeighborColor highlights (default opacity 0.25).
// Keep the retained galleys shared: selection must not rebuild glyph meshes.
fn paint_operator_selection(painter: &Painter, layout: &FormulaBox, origin: Pos2, node: NodeId) {
    if layout.node != node {
        for (offset, child) in &layout.children {
            paint_operator_selection(painter, child, origin + *offset, node);
        }
        return;
    }
    for (offset, child) in &layout.children {
        painter.rect_filled(
            Rect::from_min_size(origin + *offset, child.size),
            0.,
            Color32::from_rgba_unmultiplied(169, 169, 169, 64),
        );
    }
    for primitive in &layout.primitives {
        if let Primitive::Text(offset, galley) | Primitive::FocusedText(offset, galley) = primitive
        {
            typography::paint_galley_override(painter, origin + *offset, galley.clone(), CARET);
        }
    }
}

fn paint_box_with_focus(painter: &Painter, layout: &FormulaBox, origin: Pos2, focused: bool) {
    let clip = painter.clip_rect();
    for primitive in &layout.primitives {
        match primitive {
            Primitive::Text(offset, galley) | Primitive::FocusedText(offset, galley) => {
                if matches!(primitive, Primitive::FocusedText(..)) && !focused {
                    continue;
                }
                if clip.intersects(
                    galley
                        .mesh_bounds
                        .union(galley.rect)
                        .translate((origin + *offset).to_vec2()),
                ) {
                    typography::paint_galley(painter, origin + *offset, galley.clone(), INK);
                }
            }
            Primitive::Line(start, end) | Primitive::MinorFence(start, end) => {
                if clip.intersects(Rect::from_two_pos(origin + *start, origin + *end).expand(0.8)) {
                    let color = if matches!(primitive, Primitive::MinorFence(..)) {
                        INK.gamma_multiply(0.3)
                    } else {
                        INK
                    };
                    painter.line_segment([origin + *start, origin + *end], Stroke::new(0.8, color));
                }
            }
            Primitive::Placeholder(rect) => {
                if !clip.intersects(rect.translate(origin.to_vec2()).expand(1.)) {
                    continue;
                }
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
        paint_box_with_focus(painter, child, origin + *offset, focused);
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
    // Native matrix-cell typeover does not dispatch ordinary expression input
    // or Paste. A rectangular selection must not replace its whole matrix or
    // fabricate a distribution over cells.
    if editor.matrix_selection().is_some() {
        return InputOutcome::default();
    }
    let mut changed = false;
    let mut evaluation_requested = false;
    for ch in text.chars() {
        if let Some(literal_changed) = editor.insert_literal_text(&ch.to_string()) {
            changed |= literal_changed;
            continue;
        }
        if let Some(radix_changed) = radix_input::input_suffix(editor, ch) {
            changed |= radix_changed;
            continue;
        }
        if editor.typeover_target().is_some() {
            if ch == '=' {
                let outcome = process_equals(editor);
                changed |= outcome.document_changed;
                evaluation_requested |= outcome.evaluation_requested;
                continue;
            }
            if ch == ':' {
                // The bare whole-term case follows n.u -> DEFINITION. A
                // definition under another command needs the native reparse.
                if editor.typeover_target() == Some(editor.root())
                    && editor.typeover_excluded_operand().is_none()
                {
                    let tree = PortableTree {
                        kind: NodeKind::Binary(BinaryOp::Define),
                        children: vec![
                            PortableTree {
                                kind: NodeKind::Placeholder,
                                children: vec![]
                            };
                            2
                        ],
                    };
                    if let Ok(mut template) = Editor::from_portable(&tree) {
                        let lhs = template.arena().get(template.root()).unwrap().children()[0];
                        let _ = template.place_caret(lhs, Direction::Indeterminate);
                        changed |= editor.replace_typeover_editor(&template);
                    }
                }
                continue;
            }
            let template_kind = match ch {
                '%' => Some(NodeKind::Percent),
                '°' => Some(NodeKind::Degree),
                '!' => Some(NodeKind::Factorial),
                '(' => Some(NodeKind::Group),
                '|' => Some(NodeKind::Absolute),
                _ => None,
            };
            if let Some(kind) = template_kind {
                let tree = PortableTree {
                    kind,
                    children: vec![PortableTree {
                        kind: NodeKind::Placeholder,
                        children: vec![],
                    }],
                };
                if let Ok(mut template) = Editor::from_portable(&tree) {
                    let child = template.arena().get(template.root()).unwrap().children()[0];
                    let _ = template.place_caret(child, Direction::Indeterminate);
                    changed |= editor.replace_typeover_editor(&template);
                }
                continue;
            }
            // CloseParen's native Typeover base implementation is a no-op.
            // The other public/special dispatches still need their own proof;
            // do not accidentally wrap the old selected term through them.
            if matches!(ch, ')' | '[' | '{' | '∠') {
                continue;
            }
        }
        if editor.caret().insertion == InsertionMode::Subscript {
            if !ch.is_control() {
                changed |= insert_character(editor, ch);
            }
            continue;
        }
        let current = editor.caret().node;
        let kind = editor.arena().get(current).unwrap().kind().clone();
        match ch {
            '"' => {
                changed |= editor.insert_string_literal();
            }
            '{' => {
                let revision = editor.arena().revision();
                editor.insert_local_definition();
                changed |= editor.arena().revision() != revision;
            }
            '%' => {
                let revision = editor.arena().revision();
                editor.insert_percent();
                changed |= editor.arena().revision() != revision;
            }
            '°' => {
                let revision = editor.arena().revision();
                editor.insert_degree();
                changed |= editor.arena().revision() != revision;
            }
            '∠' => {
                let revision = editor.arena().revision();
                editor.insert_polar();
                changed |= editor.arena().revision() != revision;
            }
            '!' => {
                editor.insert_factorial();
                changed = true;
            }
            '/' => {
                changed |= editor.insert_typed_operator(NodeKind::Fraction);
            }
            '^' => {
                changed |= editor.insert_typed_operator(NodeKind::Power);
            }
            '(' => {
                changed |= editor.insert_open_parenthesis();
            }
            ')' => {
                changed |= editor.insert_close_parenthesis();
            }
            ']' => {
                editor.exit_group_right();
            }
            '[' => {
                if kind == NodeKind::Placeholder {
                    changed |= editor.insert_native_matrix();
                } else {
                    editor.insert_index();
                    changed = true;
                }
            }
            ' ' => {
                editor.expand_scope();
            }
            '+' => {
                changed |= editor.insert_typed_operator(NodeKind::Binary(BinaryOp::Add));
            }
            '-' if matches!(&kind,NodeKind::Number(value) if value.ends_with('e') || value.ends_with('E')) => {
                changed |= insert_character(editor, ch)
            }
            '-' => {
                changed |= editor.insert_typed_operator(NodeKind::Binary(BinaryOp::Subtract));
            }
            '*' => {
                changed |= editor.insert_typed_operator(NodeKind::Binary(BinaryOp::Multiply));
            }
            ':' => changed |= insert_root_command(editor, BinaryOp::Define),
            '=' => {
                if upgrade_comparison(editor, ch) {
                    changed = true;
                    continue;
                }
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
            '<' | '>' | '≤' | '≥' | '≠' | '＝' | '∧' | '∨' | '⊕' => {
                if ch == '>' && upgrade_comparison(editor, ch) {
                    changed = true;
                    continue;
                }
                changed |= editor.insert_typed_operator(NodeKind::Binary(match ch {
                    '<' => BinaryOp::LessThan,
                    '>' => BinaryOp::GreaterThan,
                    '≤' => BinaryOp::LessOrEqual,
                    '≥' => BinaryOp::GreaterOrEqual,
                    '≠' => BinaryOp::NotEqual,
                    '＝' => BinaryOp::Equal,
                    '∧' => BinaryOp::And,
                    '∨' => BinaryOp::Or,
                    _ => BinaryOp::Xor,
                }));
            }
            '¬' => {
                changed |= editor.insert_typed_operator(NodeKind::LogicalNot);
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
                if editor.typeover_target().is_some() {
                    changed |= insert_character(editor, ch);
                    continue;
                }
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
                        editor.insert_infix(BinaryOp::Scale);
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
                    editor.insert_infix(BinaryOp::Scale);
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

fn upgrade_comparison(editor: &mut Editor, second: char) -> bool {
    let node = editor.caret().node;
    let Some(current) = editor.arena().get(node) else {
        return false;
    };
    if !matches!(current.kind(), NodeKind::Placeholder) {
        return false;
    }
    let Some(parent) = current.parent() else {
        return false;
    };
    let Some(operator) = editor.arena().get(parent) else {
        return false;
    };
    if operator.children().get(1) != Some(&node) {
        return false;
    }
    let op = match (operator.kind(), second) {
        (NodeKind::Binary(BinaryOp::LessThan), '=') => BinaryOp::LessOrEqual,
        (NodeKind::Binary(BinaryOp::GreaterThan), '=') => BinaryOp::GreaterOrEqual,
        (NodeKind::Binary(BinaryOp::LessThan), '>') => BinaryOp::NotEqual,
        _ => return false,
    };
    editor.replace_infix_operator(parent, op)
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
        NodeKind::Binary(BinaryOp::Define | BinaryOp::GlobalDefine | BinaryOp::Evaluate) => false,
        _ => {
            let revision = editor.arena().revision();
            if operation == BinaryOp::Define {
                editor.insert_definition();
            } else if operation == BinaryOp::GlobalDefine {
                editor.insert_global_definition();
            } else {
                editor.insert_evaluation();
            }
            editor.arena().revision() != revision
        }
    }
}

/// REA get_EvaluationRoot recognizes both a root evaluation and the evaluation
/// on a definition's RHS. The editor enforces expression-only contexts before
/// a definition can be inserted.
fn process_equals(editor: &mut Editor) -> InputOutcome {
    let root = editor.arena().get(editor.root()).unwrap();
    let target = if matches!(
        root.kind(),
        NodeKind::Binary(BinaryOp::Define | BinaryOp::GlobalDefine)
    ) {
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
    if editor.typeover_target().is_some() {
        // ProcessEqualsKey first adds EVALUATION, then deletes the selected
        // opaque Fraction segment. Power's postfix segment has another repair.
        if editor.typeover_excluded_operand().is_some() {
            return InputOutcome::default();
        }
        let tree = PortableTree {
            kind: NodeKind::Binary(BinaryOp::Evaluate),
            children: vec![
                PortableTree {
                    kind: NodeKind::Placeholder,
                    children: vec![]
                };
                2
            ],
        };
        let Ok(mut template) = Editor::from_portable(&tree) else {
            return InputOutcome::default();
        };
        let lhs = template.arena().get(template.root()).unwrap().children()[0];
        let _ = template.place_caret(lhs, Direction::Left);
        let changed = editor.replace_typeover_editor(&template);
        return InputOutcome {
            document_changed: changed,
            evaluation_requested: changed,
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
    if let Some(tree) = editor.copy_matrix_selection() {
        return Some(format!(
            "{TREE_CLIPBOARD_PREFIX}{}",
            serde_json::to_string(&tree).ok()?
        ));
    }
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
        Selection::MatrixCells(_) => return None,
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
    if editor.is_inside_string_literal() && text.starts_with(TREE_CLIPBOARD_PREFIX) {
        // Internal tree clipboard dispatch inside STRING is unproven. Do not
        // silently replace the containing expression with a decoded tree.
        return InputOutcome::default();
    }
    if let Some(json) = text.strip_prefix(TREE_CLIPBOARD_PREFIX) {
        if json.len() > 8 * 1024 * 1024 {
            return InputOutcome::default();
        }
        let Ok(tree) = serde_json::from_str::<PortableTree>(json) else {
            return InputOutcome::default();
        };
        InputOutcome {
            document_changed: if editor.matrix_selection().is_some() {
                editor.paste_matrix_selection(&tree).unwrap_or(false)
            } else {
                editor.replace_selection(&tree).is_ok()
            },
            evaluation_requested: false,
        }
    } else {
        // The original builds one compound Paste command. Text can expand to
        // several structural mutations, but undo must restore the whole paste.
        editor.edit_group(|editor| input_text_with_actions(editor, text))
    }
}

fn atomic_text(editor: &Editor, node: NodeId) -> Option<String> {
    if editor.caret().node == node && editor.caret().insertion == InsertionMode::Subscript {
        return editor.editing_text(node).map(str::to_owned);
    }
    match editor.arena().get(node)?.kind() {
        NodeKind::Number(text)
        | NodeKind::BasedNumber { literal: text, .. }
        | NodeKind::Identifier(text)
        | NodeKind::Imaginary(text)
        | NodeKind::StringLiteral(text) => Some(text.clone()),
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

fn edited_text_kind(editor: &Editor, target: NodeId, text: String) -> Option<NodeKind> {
    if let Some(kind) = radix_input::edited_kind(editor, target, &text) {
        return Some(kind);
    }
    if matches!(
        editor.arena().get(target)?.kind(),
        NodeKind::BasedNumber { .. }
    ) && editor.selection() == Selection::None
        && editor.typeover_target().is_none()
    {
        // A partial based value requiring structural reparse is unsupported;
        // never silently turn a numeric-looking hex fragment into a name.
        return None;
    }
    Some(ordinary_edited_text_kind(editor, target, text))
}

fn ordinary_edited_text_kind(editor: &Editor, target: NodeId, text: String) -> NodeKind {
    match editor.arena().get(target).map(|node| node.kind()) {
        Some(NodeKind::StringLiteral(_)) => return NodeKind::StringLiteral(text),
        Some(NodeKind::NativeIdentifier {
            name,
            labels,
            contextual,
            subscript,
            ..
        }) => {
            return NodeKind::NativeIdentifier {
                name: if editor.caret().insertion == InsertionMode::Subscript {
                    name.clone()
                } else {
                    text.clone()
                },
                labels: labels.clone(),
                contextual: *contextual,
                subscript: if editor.caret().insertion == InsertionMode::Subscript {
                    Some(text)
                } else {
                    subscript.clone()
                },
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
    if let Some(target) = editor.typeover_target() {
        let Some(kind) = edited_text_kind(editor, target, ch.to_string()) else {
            return false;
        };
        return editor.replace_typeover_value(kind);
    }
    let target = match editor.selection() {
        Selection::Subtree(node) => node,
        Selection::MatrixCells(range) => range.matrix,
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
    let Some(kind) = edited_text_kind(editor, target, text) else {
        return false;
    };
    let subscript = editor.caret().insertion == InsertionMode::Subscript;
    if editor.set_atom(target, kind).is_err() {
        return false;
    }
    if subscript {
        let offset = offset + ch.len_utf16();
        let _ = editor.place_subscript_selection(target, offset, offset);
    } else {
        let _ = editor.place_text_caret(target, offset + ch.len_utf16());
    }
    true
}

fn set_edited_text(editor: &mut Editor, node: NodeId, text: String, offset: usize) -> bool {
    let subscript = editor.caret().insertion == InsertionMode::Subscript;
    let Some(kind) = edited_text_kind(editor, node, text) else {
        return false;
    };
    if editor.set_atom(node, kind).is_err() {
        return false;
    }
    if subscript {
        let _ = editor.place_subscript_selection(node, offset, offset);
    } else {
        let _ = editor.place_text_caret(node, offset);
    }
    true
}

fn delete_character(editor: &mut Editor, backwards: bool) -> bool {
    if let Some(changed) = editor.delete_literal_text(backwards) {
        return changed;
    }
    if editor.matrix_selection().is_some() {
        return editor.delete_matrix_selection(backwards);
    }
    let target = match editor.selection() {
        Selection::Subtree(node) => node,
        Selection::MatrixCells(_) => unreachable!("matrix deletion handled above"),
        Selection::None => editor.caret().node,
    };
    if editor.selection() != Selection::None {
        return editor.delete_structure(backwards);
    }
    if matches!(
        editor.arena().get(target).unwrap().kind(),
        NodeKind::NativeIdentifier { .. }
            | NodeKind::AnnotatedIdentifier { .. }
            | NodeKind::Imaginary(_)
            | NodeKind::ImaginarySymbol { .. }
    ) {
        return editor
            .delete_formatted_value(backwards)
            .unwrap_or_else(|| editor.delete_structure(backwards));
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
        return set_edited_text(editor, target, text, start);
    }
    if editor.selection() == Selection::Subtree(target) {
        let Some(kind) = edited_text_kind(editor, target, String::new()) else {
            return false;
        };
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
    if editor.caret().insertion == InsertionMode::Subscript
        && ((backwards && byte == 0) || (!backwards && byte == text.len()))
    {
        if text.is_empty()
            && let NodeKind::NativeIdentifier {
                name,
                labels,
                contextual,
                ..
            } = editor.arena().get(target).unwrap().kind().clone()
        {
            let length = name.encode_utf16().count();
            let _ = editor.set_atom(
                target,
                NodeKind::NativeIdentifier {
                    name,
                    labels,
                    contextual,
                    subscript: None,
                },
            );
            let _ = editor.place_text_caret(target, length);
            return true;
        }
        let _ = editor.toggle_literal_subscript();
        return false;
    }
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
    set_edited_text(editor, target, text, next_offset)
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
    let selection = if editor.caret().insertion == InsertionMode::Subscript {
        editor.place_subscript_selection(node, start_utf16, end_utf16)
    } else {
        editor.place_text_selection(node, start_utf16, end_utf16)
    };
    if selection.is_err() {
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

/// Prime exposes Ctrl+Left/Right as a term command distinct from ordinary
/// character navigation. In the represented subset a selected subtree is one
/// term, while an atom is one word.
fn move_by_term(editor: &mut Editor, forward: bool) {
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
    if editor.caret().insertion == InsertionMode::Subscript {
        let length = atomic_text(editor, node).map_or(0, |text| text.encode_utf16().count());
        if !forward {
            let _ = editor.place_text_caret(node, 0);
            return;
        }
        if editor
            .caret()
            .value_range
            .is_some_and(|range| range.focus == length)
        {
            let _ = editor.place_caret(node, Direction::Right);
            move_horizontal(editor, true);
            let next = editor.caret().node;
            if next != node
                && let Some(text) = atomic_text(editor, next)
            {
                let offset = text.encode_utf16().count();
                let _ = editor.place_text_caret(next, offset);
            }
            return;
        }
    }
    if let Some(text) = atomic_text(editor, node) {
        let target = if forward {
            text.encode_utf16().count()
        } else {
            0
        };
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
        if offset == target {
            move_horizontal(editor, forward);
            let next = editor.caret().node;
            if let Some(text) = atomic_text(editor, next) {
                let offset = if forward {
                    text.encode_utf16().count()
                } else {
                    0
                };
                if editor.caret().insertion == InsertionMode::Subscript {
                    let _ = editor.place_subscript_selection(next, offset, offset);
                } else {
                    let _ = editor.place_text_caret(next, offset);
                }
            }
        } else if editor.caret().insertion == InsertionMode::Subscript {
            let _ = editor.place_subscript_selection(node, target, target);
        } else {
            let _ = editor.place_text_caret(node, target);
        }
    } else {
        move_horizontal(editor, forward);
    }
}

fn move_horizontal(editor: &mut Editor, forward: bool) {
    if editor
        .move_literal_caret(
            if forward {
                Direction::Right
            } else {
                Direction::Left
            },
            false,
        )
        .is_some()
    {
        return;
    }
    if editor.cancel_typeover() {
        return;
    }
    let insertion = editor.caret().insertion;
    if let Some(range) = editor.caret().value_range
        && range.anchor != range.focus
    {
        let offset = if forward {
            range.anchor.max(range.focus)
        } else {
            range.anchor.min(range.focus)
        };
        if insertion == InsertionMode::Subscript {
            let _ = editor.place_subscript_selection(editor.caret().node, offset, offset);
        } else {
            let _ = editor.place_text_caret(editor.caret().node, offset);
        }
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
    if let NodeKind::NativeIdentifier {
        name,
        subscript: Some(suffix),
        ..
    } = editor.arena().get(node).unwrap().kind()
    {
        let base_len = name.encode_utf16().count();
        let suffix_len = suffix.encode_utf16().count();
        if insertion == InsertionMode::Normal
            && ((forward
                && editor
                    .caret()
                    .value_range
                    .is_some_and(|range| range.focus == base_len))
                || (!forward
                    && editor.caret().value_range.is_none()
                    && editor.caret().caret_side == Direction::Right))
        {
            let offset = if forward { 0 } else { suffix_len };
            let _ = editor.place_subscript_selection(node, offset, offset);
            return;
        }
        if insertion == InsertionMode::Subscript
            && editor.caret().value_range.is_some_and(|range| {
                if forward {
                    range.focus == suffix_len
                } else {
                    range.focus == 0
                }
            })
        {
            if forward {
                let _ = editor.place_caret(node, Direction::Right);
            } else {
                let _ = editor.place_text_caret(node, base_len);
            }
            return;
        }
    }
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
                if insertion == InsertionMode::Subscript {
                    let _ = editor.place_subscript_selection(node, next, next);
                } else {
                    let _ = editor.place_text_caret(node, next);
                }
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
#[path = "formula/matrix_selection_tests.rs"]
mod matrix_selection_tests;

#[cfg(test)]
#[path = "formula/source_matrix_reshape_cut_tests.rs"]
mod source_matrix_reshape_cut_tests;

#[cfg(test)]
#[path = "formula/array_operator_tests.rs"]
mod array_operator_tests;

#[cfg(test)]
#[path = "formula/global_vector_operator_tests.rs"]
mod global_vector_operator_tests;

#[cfg(test)]
#[path = "formula/scalar_notation_tests.rs"]
mod scalar_notation_tests;

#[cfg(test)]
#[path = "formula/local_function_input.rs"]
mod local_function_input;

#[cfg(test)]
#[path = "formula/conditional_layout_tests.rs"]
mod conditional_layout_tests;

#[cfg(test)]
#[path = "formula/deletion_tests.rs"]
mod deletion_tests;

#[cfg(test)]
#[path = "formula/replacement_tests.rs"]
mod replacement_tests;

#[cfg(test)]
#[path = "formula/typeover_tests.rs"]
mod typeover_tests;

#[cfg(test)]
#[path = "formula/formatted_deletion_tests.rs"]
mod formatted_deletion_tests;
#[cfg(test)]
#[path = "formula/infix_entry_tests.rs"]
mod infix_entry_tests;
#[cfg(test)]
#[path = "formula/matrix_authoring_tests.rs"]
mod matrix_authoring_tests;
#[cfg(test)]
#[path = "formula/parenthesis_tests.rs"]
mod parenthesis_tests;
#[cfg(test)]
#[path = "formula/scaling_tests.rs"]
mod scaling_tests;

#[cfg(test)]
mod tests {
    use super::*;
    fn calculated_number(editor: &Editor) -> Result<f64, String> {
        let fragment = crate::worksheet_math::tree_to_mathml(&editor.to_portable().unwrap())?;
        let xml = crate::worksheet_math::worksheet_xml(&[(1, 0., 0., fragment)])?;
        let rows = mathfolio_compat::evaluate_worksheet(&xml)?;
        if let Some(error) = &rows[0].error {
            return Err(error.detail.clone());
        }
        let Some(mathfolio_compat::Value::Scalar { quantity }) = &rows[0].value else {
            return Err(format!("{rows:?}"));
        };
        Ok(quantity.value.re)
    }

    #[test]
    fn comparison_input_roundtrips_native_mathml_and_evaluates_without_confusing_equals_command() {
        for (source, answer) in [
            ("1<2=", 1.),
            ("2>3=", 0.),
            ("2<=2=", 1.),
            ("2>=3=", 0.),
            ("2<>3=", 1.),
            ("1+2<4*2=", 1.),
            ("2m<3m=", 1.),
            ("1∧0∨1=", 1.),
            ("1⊕1=", 0.),
            ("¬0=", 1.),
        ] {
            let mut editor = Editor::new();
            let outcome = input_text_with_actions(&mut editor, source);
            assert!(outcome.evaluation_requested, "{source}");
            assert_eq!(calculated_number(&editor).unwrap(), answer, "{source}");
            let tree = editor.to_portable().unwrap();
            let xml = crate::worksheet_math::tree_to_mathml(&tree).unwrap();
            let restored = crate::worksheet_math::tree_from_mathml(&xml).unwrap();
            assert_eq!(restored, tree, "{source}");
        }
        let mut editor = Editor::new();
        input_text(&mut editor, "2m<3s=");
        assert!(calculated_number(&editor).is_err());
        let mut editor = Editor::new();
        input_text(&mut editor, "2<");
        let before = editor.to_portable().unwrap();
        let outcome = input_text_with_actions(&mut editor, "=");
        assert!(!outcome.evaluation_requested);
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), before);
    }

    #[test]
    fn widget_routes_literal_matrix_comparison_and_logic_shortcuts_without_zoom_or_implicit_evaluation()
     {
        let context = egui::Context::default();
        crate::configure(&context);
        let command = egui::Modifiers {
            ctrl: true,
            command: true,
            ..egui::Modifiers::NONE
        };
        let send = |editor: &mut Editor, key: Key, modifiers: egui::Modifiers| {
            let output = context.run_ui(
                egui::RawInput {
                    events: vec![egui::Event::Key {
                        key,
                        physical_key: Some(key),
                        pressed: true,
                        repeat: false,
                        modifiers,
                    }],
                    ..Default::default()
                },
                |ui| {
                    let response = formula_ui_with_actions(ui, editor, None, true);
                    assert!(!response.evaluation_requested);
                },
            );
            output.drop_without_applying_deltas();
        };
        let mut editor = Editor::new();
        input_text(&mut editor, "V");
        send(&mut editor, Key::Minus, command);
        assert_eq!(editor.caret().insertion, InsertionMode::Subscript);
        input_text(&mut editor, "in");
        send(
            &mut editor,
            Key::Z,
            egui::Modifiers {
                shift: true,
                ..command
            },
        );
        assert_eq!(atomic_text(&editor, editor.root()).as_deref(), Some("in∞"));
        send(&mut editor, Key::Minus, command);
        assert_eq!(editor.caret().insertion, InsertionMode::Normal);
        for (key, op) in [
            (Key::Equals, BinaryOp::Equal),
            (Key::Num0, BinaryOp::GreaterOrEqual),
            (Key::Num9, BinaryOp::LessOrEqual),
        ] {
            let mut editor = Editor::new();
            input_text(&mut editor, "2");
            send(&mut editor, key, command);
            input_text(&mut editor, "2=");
            assert_eq!(
                editor.to_portable().unwrap().children[0].kind,
                NodeKind::Binary(op)
            );
            assert_eq!(calculated_number(&editor).unwrap(), 1.);
        }
        let mut editor = Editor::new();
        send(&mut editor, Key::M, command);
        assert!(matches!(
            editor.to_portable().unwrap().kind,
            NodeKind::Matrix {
                rows: 1,
                cols: 1,
                display: None
            }
        ));
        let mut editor = Editor::new();
        send(
            &mut editor,
            Key::Num1,
            egui::Modifiers {
                shift: true,
                ..command
            },
        );
        input_text(&mut editor, "0=");
        assert_eq!(calculated_number(&editor).unwrap(), 1.);
    }
    #[test]
    fn literal_subscript_edits_name_suffix_without_making_array_or_calculation_operators() {
        let mut editor = Editor::new();
        input_text(&mut editor, "I");
        assert!(editor.toggle_literal_subscript());
        let outcome = input_text_with_actions(&mut editor, "dc-2=∞");
        assert!(outcome.document_changed);
        assert!(!outcome.evaluation_requested);
        assert!(
            matches!(editor.arena().get(editor.root()).unwrap().kind(),NodeKind::NativeIdentifier{name,subscript:Some(suffix),..}if name=="I"&&suffix=="dc-2=∞")
        );
        assert_eq!(editor.caret().insertion, InsertionMode::Subscript);
        assert!(delete_character(&mut editor, true));
        assert_eq!(
            atomic_text(&editor, editor.root()).as_deref(),
            Some("dc-2=")
        );
        assert!(editor.undo());
        assert_eq!(
            atomic_text(&editor, editor.root()).as_deref(),
            Some("dc-2=∞")
        );
        editor.toggle_literal_subscript();
        let outcome = input_text_with_actions(&mut editor, ":2");
        assert!(!outcome.evaluation_requested);
        let xml = crate::worksheet_math::tree_to_mathml(&editor.to_portable().unwrap()).unwrap();
        assert!(xml.contains("Subscript"));
        assert!(!xml.contains("indexer"));
    }

    #[test]
    fn literal_subscript_selection_delete_uses_native_baseline_insertion_at_suffix_start() {
        let mut editor = Editor::new();
        input_text(&mut editor, "V");
        editor.toggle_literal_subscript();
        input_text(&mut editor, "ab𝛼");
        move_horizontal(&mut editor, false);
        assert_eq!(editor.caret().value_range.unwrap().focus, 2);
        extend_selection(&mut editor, false);
        assert_eq!(copy_tree(&editor).as_deref(), Some("b"));
        assert!(delete_character(&mut editor, true));
        assert_eq!(atomic_text(&editor, editor.root()).as_deref(), Some("a𝛼"));
        editor
            .place_subscript_selection(editor.root(), 0, 3)
            .unwrap();
        assert!(!input_text(&mut editor, ""));
        assert!(delete_character(&mut editor, true));
        // Native0435 retains DefaultCaretInsertionType at the selection start.
        // At suffix start1329 reads the preceding baseline character, so a
        // whole-suffix range differs from deleting its last character at end.
        assert_eq!(atomic_text(&editor, editor.root()).as_deref(), Some("V"));
        assert_eq!(editor.caret().insertion, InsertionMode::Normal);
        assert_eq!(editor.caret().value_range.unwrap().focus, 1);
        let baseline = editor.to_portable().unwrap();
        assert!(delete_character(&mut editor, true));
        assert_eq!(editor.to_portable().unwrap().kind, NodeKind::Placeholder);
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), baseline);
        assert!(editor.undo());
        assert!(
            matches!(editor.arena().get(editor.root()).unwrap().kind(),NodeKind::NativeIdentifier{name,subscript:Some(suffix),..}if name=="V"&&suffix=="a𝛼")
        );
        assert_eq!(editor.caret().insertion, InsertionMode::Subscript);
        let range = editor.caret().value_range.unwrap();
        assert_eq!((range.anchor, range.focus), (0, 3));
    }

    #[test]
    fn repeated_control_arrows_cross_atoms_and_literal_suffixes_instead_of_sticking() {
        let mut editor = Editor::new();
        input_text(&mut editor, "I");
        editor.toggle_literal_subscript();
        input_text(&mut editor, "dc");
        editor.toggle_literal_subscript();
        input_text(&mut editor, "+123");
        let root = editor.to_portable().unwrap();
        assert_eq!(root.kind, NodeKind::Binary(BinaryOp::Add));
        let left = editor.arena().get(editor.root()).unwrap().children()[0];
        let right = editor.caret().node;
        editor.place_text_caret(left, 0).unwrap();
        move_by_term(&mut editor, true);
        assert_eq!(editor.caret().node, left);
        assert_eq!(editor.caret().value_range.unwrap().focus, 1);
        move_by_term(&mut editor, true);
        assert_eq!(editor.caret().insertion, InsertionMode::Subscript);
        assert_eq!(editor.caret().value_range.unwrap().focus, 2);
        move_by_term(&mut editor, true);
        assert_eq!(editor.caret().node, right);
        assert_eq!(editor.caret().value_range.unwrap().focus, 3);
        move_by_term(&mut editor, false);
        assert_eq!(editor.caret().value_range.unwrap().focus, 0);
        move_by_term(&mut editor, false);
        assert_eq!(editor.caret().node, left);
        assert_eq!(editor.caret().value_range.unwrap().focus, 0);
    }

    #[test]
    fn literal_subscript_has_separate_headless_hit_and_lower_caret_geometry() {
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut editor = Editor::new();
        input_text(&mut editor, "I");
        editor.toggle_literal_subscript();
        input_text(&mut editor, "dc");
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let layout = measure(ui.painter(), editor.arena(), editor.root(), FONT_SIZE);
            let mut hits = vec![];
            collect_hits(&layout, Pos2::ZERO, &mut hits);
            let base = hits
                .iter()
                .find(|hit| hit.insertion == InsertionMode::Normal)
                .unwrap();
            let suffix = hits
                .iter()
                .find(|hit| hit.insertion == InsertionMode::Subscript)
                .unwrap();
            assert_eq!(
                hit_node(&hits, suffix.rect.center()).unwrap().insertion,
                InsertionMode::Subscript
            );
            assert!(suffix.text_origin.y > base.text_origin.y);
            assert!(suffix.text_origin.x > base.text_origin.x);
            place_hit_selection(&mut editor, suffix, 1, 1);
            assert_eq!(editor.caret().insertion, InsertionMode::Subscript);
            assert!(caret_rect(suffix, &editor).center().y > base.rect.top());
        });
        output.drop_without_applying_deltas();
    }
    #[test]
    fn retained_hits_follow_layout_revisions_and_region_motion() {
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut editor = Editor::new();
        input_text(&mut editor, "12/3");
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let first = retained_layout(ui, &editor, None, 16., None);
            let hits = retained_hits(ui, &first, Pos2::ZERO);
            let same = retained_hits(ui, &first, Pos2::ZERO);
            assert!(Arc::ptr_eq(&hits, &same));
            let moved = retained_hits(ui, &first, pos2(20., 30.));
            assert!(!Arc::ptr_eq(&hits, &moved));
            for (a, b) in hits.iter().zip(moved.iter()) {
                let expected = a.rect.translate(vec2(20., 30.));
                assert!(
                    b.rect.min.distance(expected.min) < 0.0001
                        && b.rect.max.distance(expected.max) < 0.0001
                );
            }
            let mut branch = editor.clone();
            let leaf = branch.caret().node;
            branch
                .set_atom(leaf, NodeKind::Number("999999999".into()))
                .unwrap();
            let changed = retained_layout(ui, &branch, None, 16., None);
            assert!(!Arc::ptr_eq(&first, &changed));
            let changed_hits = retained_hits(ui, &changed, Pos2::ZERO);
            assert!(!Arc::ptr_eq(&hits, &changed_hits));
            assert!(
                changed_hits
                    .iter()
                    .all(|h| branch.arena().get(h.node).is_some())
            );
        });
        output.drop_without_applying_deltas();
    }
    #[test]
    fn matrix_paint_skips_offscreen_glyph_primitives() {
        let tree = PortableTree {
            kind: NodeKind::Matrix {
                rows: 64,
                cols: 64,
                display: None,
            },
            children: (0..4096)
                .map(|n| PortableTree {
                    kind: NodeKind::Number(n.to_string()),
                    children: vec![],
                })
                .collect(),
        };
        let editor = Editor::from_portable(&tree).unwrap();
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.set_clip_rect(Rect::from_min_size(Pos2::ZERO, vec2(140., 70.)));
            let layout = retained_layout(ui, &editor, None, 16., None);
            paint_box(ui.painter(), &layout, Pos2::ZERO);
        });
        let texts = output
            .shapes
            .iter()
            .filter(|s| matches!(s.shape, egui::Shape::Text(_)))
            .count();
        assert!(
            texts > 0 && texts < 100,
            "only visible cells are painted, got {texts}"
        );
        output.drop_without_applying_deltas();
    }
    #[test]
    #[ignore = "CPU-only formula cache and matrix paint benchmark"]
    fn benchmark_retained_formula_matrix() {
        let tree = PortableTree {
            kind: NodeKind::Matrix {
                rows: 64,
                cols: 64,
                display: None,
            },
            children: (0..4096)
                .map(|n| PortableTree {
                    kind: NodeKind::Number(n.to_string()),
                    children: vec![],
                })
                .collect(),
        };
        let mut editor = Editor::from_portable(&tree).unwrap();
        let context = egui::Context::default();
        crate::configure(&context);
        let output = context.run_ui(egui::RawInput::default(), |ui| {
            let first = retained_layout(ui, &editor, None, 16., None);
            let started = std::time::Instant::now();
            for _ in 0..2000 {
                std::hint::black_box(retained_layout(ui, &editor, None, 16., None));
            }
            println!(
                "PERF formula_nodes=4097 retained_queries=2000 elapsed_us={}",
                started.elapsed().as_micros()
            );
            let started = std::time::Instant::now();
            for _ in 0..120 {
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(Rect::from_min_size(Pos2::ZERO, vec2(900., 700.))),
                    |ui| {
                        std::hint::black_box(formula_ui_styled(
                            ui,
                            &mut editor,
                            None,
                            false,
                            None,
                            12.,
                        ));
                    },
                );
            }
            println!(
                "PERF formula_nodes=4097 interactive_paints=120 elapsed_us={}",
                started.elapsed().as_micros()
            );
            assert!(first.size.x > 900.);
        });
        output.drop_without_applying_deltas();
    }
    #[test]
    fn retained_geometry_updates_for_source_result_and_style_changes() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut editor = Editor::new();
        input_text(&mut editor, "1=");
        let output = context.run_ui(egui::RawInput::default(), |ui| {
            let first = retained_layout(ui, &editor, None, 16., Some(FormulaResult::Text("2")));
            let idle = retained_layout(ui, &editor, None, 16., Some(FormulaResult::Text("2")));
            assert!(Arc::ptr_eq(&first, &idle));
            editor.place_caret(editor.root(), Direction::Left).unwrap();
            let caret_only =
                retained_layout(ui, &editor, None, 16., Some(FormulaResult::Text("2")));
            assert!(
                Arc::ptr_eq(&idle, &caret_only),
                "Caret motion must reuse glyph geometry"
            );
            let answer = retained_layout(
                ui,
                &editor,
                None,
                16.,
                Some(FormulaResult::Text("222222222")),
            );
            assert!(answer.size.x > first.size.x);
            let larger = retained_layout(
                ui,
                &editor,
                None,
                32.,
                Some(FormulaResult::Text("222222222")),
            );
            assert!(larger.size.x > answer.size.x * 1.5);
            let operand = editor.arena().get(editor.root()).unwrap().children()[0];
            editor.place_text_caret(operand, 0).unwrap();
            assert!(input_text(&mut editor, "987654321"));
            let edited = retained_layout(
                ui,
                &editor,
                None,
                32.,
                Some(FormulaResult::Text("222222222")),
            );
            assert!(!Arc::ptr_eq(&larger, &edited));
            let mut hits = Vec::new();
            collect_hits(&edited, Pos2::ZERO, &mut hits);
            assert!(
                hits.iter()
                    .all(|hit| editor.arena().get(hit.node).is_some())
            );
        });
        output.drop_without_applying_deltas();
    }
    #[test]
    fn symbolic_evaluation_lays_out_its_computed_answer() {
        let source = "<ml:symEval xmlns:ml=\"http://schemas.mathsoft.com/math50\"><ml:apply><ml:plus/><ml:id labels=\"VARIABLE\">x</ml:id><ml:real>0</ml:real></ml:apply></ml:symEval>";
        let tree = crate::worksheet_math::tree_from_mathml(source).unwrap();
        let editor = Editor::from_portable(&tree).unwrap();
        let context = egui::Context::default();
        crate::configure(&context);
        let output = context.run_ui(egui::RawInput::default(), |ui| {
            let without =
                measure_with_result(ui.painter(), editor.arena(), editor.root(), FONT_SIZE, None);
            let with = measure_with_result(
                ui.painter(),
                editor.arena(),
                editor.root(),
                FONT_SIZE,
                Some("123456789"),
            );
            assert!(with.size.x > without.size.x + FONT_SIZE * 3.);
        });
        output.drop_without_applying_deltas();
        assert_eq!(editor.to_portable().unwrap(), tree);
    }

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
    fn program_calculus_and_symbolic_nodes_render_and_tab_through_placeholders() {
        fn p(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
            PortableTree { kind, children }
        }
        let placeholder = || p(NodeKind::Placeholder, vec![]);
        let tree = p(
            NodeKind::Matrix {
                rows: 3,
                cols: 1,
                display: None,
            },
            vec![
                p(
                    NodeKind::Program { statement_count: 1 },
                    vec![p(NodeKind::LocalDefine, vec![placeholder(), placeholder()])],
                ),
                p(
                    NodeKind::Integral,
                    vec![placeholder(), placeholder(), placeholder(), placeholder()],
                ),
                p(
                    NodeKind::SymbolicEvaluation { command_count: 1 },
                    vec![
                        placeholder(),
                        p(
                            NodeKind::SymbolicCommand { argument_count: 0 },
                            vec![placeholder()],
                        ),
                    ],
                ),
            ],
        );
        let mut editor = Editor::from_portable(&tree).unwrap();
        let first = editor.caret().node;
        cycle_leaf(&mut editor, true, true);
        assert_ne!(editor.caret().node, first);

        let context = egui::Context::default();
        crate::configure(&context);
        let output = context.run_ui(egui::RawInput::default(), |ui| {
            let layout = measure(ui.painter(), editor.arena(), editor.root(), FONT_SIZE);
            assert!(layout.size.x > FONT_SIZE);
            assert!(layout.size.y > FONT_SIZE * 2.0);
            let mut hits = vec![];
            collect_hits(&layout, Pos2::ZERO, &mut hits);
            let mut source_leaves = vec![];
            leaves(editor.arena(), editor.root(), &mut source_leaves);
            assert!(
                source_leaves
                    .iter()
                    .all(|leaf| hits.iter().any(|hit| hit.node == *leaf))
            );
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
    #[allow(deprecated)]
    fn cancelled_ime_preedit_never_blocks_later_plain_text_or_adds_source_history() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut editor = typed_one_character_at_a_time("x");
        let before = editor.to_portable().unwrap();
        let run = |editor: &mut Editor, events: Vec<egui::Event>| {
            context
                .run_ui(
                    egui::RawInput {
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        formula_ui_with_focus(ui, editor, None, true);
                    },
                )
                .drop_without_applying_deltas();
        };
        run(&mut editor, vec![]);
        run(
            &mut editor,
            vec![egui::Event::Ime(egui::ImeEvent::Preedit {
                text: "한".into(),
                active_range_chars: None,
            })],
        );
        assert_eq!(editor.to_portable().unwrap(), before);
        let revision = editor.arena().revision();
        run(
            &mut editor,
            vec![egui::Event::Ime(egui::ImeEvent::Disabled)],
        );
        assert_eq!(editor.arena().revision(), revision);
        run(&mut editor, vec![egui::Event::Text("7".into())]);
        assert_eq!(atomic_text(&editor, editor.root()).as_deref(), Some("x7"));
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
            mathfolio_compat::editor_bridge::tree_to_mathml(&editor.to_portable().unwrap())
                .unwrap();
        let xml = mathfolio_compat::editor_bridge::worksheet_xml(&[(0, 0., 0., fragment)]).unwrap();
        let result = mathfolio_compat::evaluate_worksheet(&xml).unwrap();
        assert!(result[0].error.is_none(), "{:?}", result[0].error);
        let Some(mathfolio_compat::Value::Scalar { quantity }) = result[0].value else {
            panic!()
        };
        assert_eq!(quantity.value.re, 2.);
        assert_eq!(quantity.dimension[0], 1.);
        let Some(mathfolio_compat::Value::Scalar { quantity }) = result[0].display_value else {
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
        use mathfolio_compat::editor_bridge::{tree_to_mathml, worksheet_xml};
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
        let results = mathfolio_compat::evaluate_worksheet(&xml).unwrap();
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
            // The denominator remains the active container until ArrowRight.
            ("8/4*2=", 1.0),
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
    fn indexed_definition_keyboard_input_roundtrips_calculates_and_undoes_as_one_text_event() {
        let mut editor = typed_one_character_at_a_time("A[2");
        let before = editor.to_portable().unwrap();
        let outcome = input_text_with_actions(&mut editor, ":=7A=");
        assert!(outcome.document_changed);
        assert!(outcome.evaluation_requested);
        let defined = editor.to_portable().unwrap();
        assert_eq!(defined.kind, NodeKind::Binary(BinaryOp::Define));
        assert_eq!(defined.children[0].kind, NodeKind::Index { indices: 1 });
        let xml = crate::worksheet_math::tree_to_mathml(&defined).unwrap();
        assert_eq!(
            crate::worksheet_math::tree_from_mathml(&xml).unwrap(),
            defined
        );
        assert_eq!(calculated_number(&editor).unwrap(), 7.);
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), before);
        assert!(editor.redo());
        assert_eq!(editor.to_portable().unwrap(), defined);

        let mut fragments = Vec::new();
        for (i, text) in ["ORIGIN:=1", "i:=1..3", "j:=1..2", "M[i,j:=i^2+j", "M[3,2="]
            .into_iter()
            .enumerate()
        {
            let editor = if text == "M[i,j:=i^2+j" {
                let mut editor = typed_one_character_at_a_time("M[i,j:=i^2");
                move_horizontal(&mut editor, true);
                input_text(&mut editor, "+j");
                editor
            } else {
                typed_one_character_at_a_time(text)
            };
            let tree = editor.to_portable().unwrap();
            let xml = crate::worksheet_math::tree_to_mathml(&tree).unwrap();
            assert_eq!(
                crate::worksheet_math::tree_from_mathml(&xml).unwrap(),
                tree,
                "{text}"
            );
            fragments.push((i as u64 + 1, 0., i as f64 * 40., xml));
        }
        let xml = crate::worksheet_math::worksheet_xml(&fragments).unwrap();
        let rows = mathfolio_compat::evaluate_worksheet(&xml).unwrap();
        assert!(
            rows.iter().all(|row| row.error.is_none()),
            "{rows:?}\n{xml}"
        );
        let Some(mathfolio_compat::Value::Scalar { quantity }) = rows.last().unwrap().value else {
            panic!("{rows:?}")
        };
        assert_eq!(quantity.value.re, 11.);
    }

    #[test]
    fn character_entry_builds_rectangular_matrices_indices_and_ranges() {
        let matrix = typed_one_character_at_a_time("[1,2;3,4]")
            .to_portable()
            .unwrap();
        assert_eq!(
            matrix.kind,
            NodeKind::Matrix {
                rows: 2,
                cols: 2,
                display: None
            }
        );
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
        assert_eq!(empty.kind, NodeKind::Call { argument_count: 1 });
        assert_eq!(empty.children.len(), 2);
        assert_eq!(empty.children[1].kind, NodeKind::Placeholder);
    }

    #[test]
    fn committed_text_event_undoes_implicit_multiplication_as_one_command() {
        let mut editor = typed_one_character_at_a_time("2");
        let before = editor.to_portable().unwrap();
        assert!(input_text(&mut editor, "m"));
        assert_eq!(
            editor.to_portable().unwrap().kind,
            NodeKind::Binary(BinaryOp::Scale)
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
        let before_caret = editor.caret();
        assert!(!delete_character(&mut editor, true));
        assert_eq!(editor.to_portable().unwrap(), before);
        // Unresolved outer Typeover is atomic, rather than an arbitrary jump
        // into the last leaf. Native ordinary binary boundary deletion below
        // is a separate, confirmed parse-and-join command.
        assert_eq!(editor.caret(), before_caret);
        let mut sum = typed_one_character_at_a_time("2+34");
        let node = sum.caret().node;
        sum.place_text_caret(node, 0).unwrap();
        let before = sum.to_portable().unwrap();
        assert!(delete_character(&mut sum, true));
        assert_eq!(atomic_text(&sum, sum.root()).as_deref(), Some("234"));
        assert_eq!(sum.caret().value_range.unwrap().focus, 1);
        assert!(sum.undo());
        assert_eq!(sum.to_portable().unwrap(), before);
    }

    #[test]
    fn term_navigation_and_shift_selection_preserve_structure() {
        let mut word = typed_one_character_at_a_time("alpha");
        word.place_text_caret(word.root(), 2).unwrap();
        move_by_term(&mut word, true);
        assert_eq!(word.caret().value_range.unwrap().focus, 5);
        move_by_term(&mut word, false);
        assert_eq!(word.caret().value_range.unwrap().focus, 0);

        let mut expression = typed_one_character_at_a_time("a+b");
        let root = expression.root();
        let right = expression.arena().get(root).unwrap().children()[1];
        expression.place_text_caret(right, 1).unwrap();
        extend_selection(&mut expression, false);
        assert_eq!(
            expression.caret().value_range,
            Some(mathfolio_editor::ValueRange {
                anchor: 1,
                focus: 0
            })
        );
        extend_selection(&mut expression, false);
        assert_eq!(expression.selection(), Selection::Subtree(right));
        assert_eq!(expression.scope_direction(), Direction::Right);
        extend_selection(&mut expression, false);
        assert_eq!(expression.selection(), Selection::Subtree(root));
        let before = expression.to_portable().unwrap();
        assert!(input_text(&mut expression, "x"));
        assert_eq!(
            atomic_text(&expression, expression.root()).as_deref(),
            Some("x")
        );
        assert!(expression.undo());
        assert_eq!(expression.to_portable().unwrap(), before);
        assert_eq!(expression.selection(), Selection::Subtree(root));
    }

    #[test]
    fn widget_routes_home_end_and_control_arrows_to_expression_commands() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut editor = typed_one_character_at_a_time("a+bc");
        let root = editor.root();
        let left = editor.arena().get(root).unwrap().children()[0];
        let right = editor.arena().get(root).unwrap().children()[1];
        let frame = context.run_ui(egui::RawInput::default(), |ui| {
            formula_ui_with_focus(ui, &mut editor, None, true);
        });
        frame.drop_without_applying_deltas();

        let send =
            |context: &egui::Context, editor: &mut Editor, key: Key, modifiers: egui::Modifiers| {
                let output = context.run_ui(
                    egui::RawInput {
                        events: vec![egui::Event::Key {
                            key,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers,
                        }],
                        ..Default::default()
                    },
                    |ui| {
                        formula_ui_with_focus(ui, editor, None, true);
                    },
                );
                output.drop_without_applying_deltas();
            };
        send(&context, &mut editor, Key::Home, egui::Modifiers::NONE);
        assert_eq!(editor.caret().node, left);
        assert_eq!(editor.caret().value_range.unwrap().focus, 0);
        send(&context, &mut editor, Key::End, egui::Modifiers::NONE);
        assert_eq!(editor.caret().node, right);
        assert_eq!(editor.caret().value_range.unwrap().focus, 2);
        editor.place_text_caret(right, 1).unwrap();
        send(
            &context,
            &mut editor,
            Key::ArrowLeft,
            egui::Modifiers {
                command: true,
                ctrl: true,
                ..egui::Modifiers::NONE
            },
        );
        assert_eq!(editor.caret().value_range.unwrap().focus, 0);
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
            assert!(
                hits.iter()
                    .filter(|hit| hit.operator_side.is_none())
                    .all(|hit| ids.insert(hit.node))
            );
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
            assert!(
                hits.iter()
                    .filter(|hit| hit.operator_side.is_none())
                    .all(|hit| ids.insert(hit.node))
            );
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

    fn root_editor(degree: NodeKind, radicand: NodeKind) -> Editor {
        Editor::from_portable(&PortableTree {
            kind: NodeKind::Root,
            children: vec![
                PortableTree {
                    kind: degree,
                    children: vec![],
                },
                PortableTree {
                    kind: radicand,
                    children: vec![],
                },
            ],
        })
        .unwrap()
    }

    #[test]
    fn readonly_optional_root_degree_matches_square_root_without_editing_marker() {
        let context = egui::Context::default();
        crate::configure(&context);
        let editor = root_editor(NodeKind::Placeholder, NodeKind::Number("9".into()));
        let square = root_editor(NodeKind::Number("2".into()), NodeKind::Number("9".into()));
        let before = editor.to_portable().unwrap();
        let output = context.run_ui(egui::RawInput::default(), |ui| {
            let editable = measure(ui.painter(), editor.arena(), editor.root(), FONT_SIZE);
            assert!(matches!(
                editable.children[0].1.primitives.as_slice(),
                [Primitive::Placeholder(_)]
            ));
            let readonly = readonly_layout(ui, &editor, None, None, 11.);
            let square = readonly_layout(ui, &square, None, None, 11.);
            assert_eq!(readonly.size, square.size);
            assert_eq!(readonly.baseline, square.baseline);
            assert_eq!(readonly.children[0].1.size, Vec2::ZERO);
            assert!(readonly.children[0].1.primitives.is_empty());
            assert!(readonly.size.x < editable.size.x);
            formula_readonly_ui_styled(ui, &editor, None, None, 11.);
        });
        assert!(!output.shapes.iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Rect(rect) if rect.stroke.color == CARET)
        }));
        output.drop_without_applying_deltas();
        assert_eq!(editor.to_portable().unwrap(), before);
    }

    #[test]
    fn readonly_root_preserves_explicit_zero_degree_and_required_empty_operand() {
        let context = egui::Context::default();
        crate::configure(&context);
        let zero = root_editor(NodeKind::Number("0".into()), NodeKind::Placeholder);
        let cube = root_editor(NodeKind::Number("3".into()), NodeKind::Number("9".into()));
        let empty = Editor::new();
        let output = context.run_ui(egui::RawInput::default(), |ui| {
            for (editor, degree) in [(&zero, "0"), (&cube, "3")] {
                let layout = readonly_layout(ui, editor, None, None, 11.);
                assert_eq!(layout.children[0].1.atom.as_ref().unwrap().text(), degree);
                assert!(layout.children[0].1.size.x > 0.);
            }
            let layout = readonly_layout(ui, &zero, None, None, 11.);
            assert!(matches!(
                layout.children[1].1.primitives.as_slice(),
                [Primitive::Placeholder(_)]
            ));
            let layout = readonly_layout(ui, &empty, None, None, 11.);
            assert!(matches!(
                layout.primitives.as_slice(),
                [Primitive::Placeholder(_)]
            ));
        });
        output.drop_without_applying_deltas();
    }

    #[test]
    fn readonly_formula_does_not_consume_input_and_measurement_matches_paint() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut editor = root_editor(NodeKind::Placeholder, NodeKind::Number("9".into()));
        editor.select_subtree(editor.root()).unwrap();
        let before = editor.to_portable().unwrap();
        let selection = editor.selection();
        let events = vec![egui::Event::Text("123=".into())];
        let output = context.run_ui(
            egui::RawInput {
                events: events.clone(),
                ..Default::default()
            },
            |ui| {
                let measured = measured_size_readonly_styled(ui, &editor, None, None, 17.);
                let response = formula_readonly_ui_styled(ui, &editor, None, None, 17.);
                // egui snaps the allocated rectangle to physical pixels.
                let delta = response.rect.size() - measured;
                let pixel = ui.ctx().pixels_per_point().recip();
                assert!(delta.x.abs() <= pixel && delta.y.abs() <= pixel);
                assert_eq!(response.sense, Sense::hover());
                assert!(!response.changed());
                assert!(!response.has_focus());
                assert_eq!(ui.input(|input| input.events.clone()), events);
            },
        );
        output.drop_without_applying_deltas();
        assert_eq!(editor.to_portable().unwrap(), before);
        assert_eq!(editor.selection(), selection);
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
                readonly: false,
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
    fn program_enter_and_blank_delete_are_consumed_and_calculate_after_mathml_roundtrip() {
        use mathfolio_editor::PortableTree;
        let node = |kind, children| PortableTree { kind, children };
        let tree = node(
            NodeKind::Program { statement_count: 1 },
            vec![node(
                NodeKind::LocalDefine,
                vec![
                    node(NodeKind::Identifier("s".into()), vec![]),
                    node(NodeKind::Number("2".into()), vec![]),
                ],
            )],
        );
        let context = egui::Context::default();
        crate::configure(&context);
        let mut editor = Editor::from_portable(&tree).unwrap();
        let program = editor.root();
        let assignment = editor.arena().get(program).unwrap().children()[0];
        editor.place_caret(assignment, Direction::Right).unwrap();
        for key in [Key::Enter, Key::Backspace, Key::Enter] {
            let output = context.run_ui(
                egui::RawInput {
                    events: vec![egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    }],
                    ..Default::default()
                },
                |ui| {
                    formula_ui_with_focus(ui, &mut editor, None, true);
                    assert!(!ui.input(|i| i.key_pressed(Key::Enter)));
                },
            );
            output.drop_without_applying_deltas();
        }
        assert_eq!(editor.arena().get(program).unwrap().children().len(), 2);
        input_text(&mut editor, "s+3");
        editor.insert_evaluation();
        let tree = editor.to_portable().unwrap();
        let portable: PortableTree =
            serde_json::from_str(&serde_json::to_string(&tree).unwrap()).unwrap();
        let math = mathfolio_compat::editor_bridge::tree_to_mathml(&portable).unwrap();
        let parsed = roxmltree::Document::parse(&math).unwrap();
        for name in ["program", "localDefine"] {
            assert!(
                parsed
                    .descendants()
                    .any(|n| n.has_tag_name(("http://schemas.mathsoft.com/math50", name)))
            );
        }
        let xml = mathfolio_compat::editor_bridge::worksheet_xml(&[(1, 0., 0., math)]).unwrap();
        let results = mathfolio_compat::evaluate_worksheet(&xml).unwrap();
        assert!(results[0].error.is_none(), "{:?}", results[0].error);
        assert_eq!(
            results[0].value.as_ref().unwrap().numeric_json()["real"],
            5.
        );
    }

    #[test]
    fn matrix_tab_keys_visit_filled_cells_and_grow_only_at_last_cell() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut editor = Editor::new();
        input_text(&mut editor, "[1,2;3,4]");
        let matrix = editor.root();
        let cells = editor.arena().get(matrix).unwrap().children().to_vec();
        editor.place_caret(cells[0], Direction::Right).unwrap();
        for (step, target) in [cells[1], cells[2], cells[3]].into_iter().enumerate() {
            let output = context.run_ui(
                egui::RawInput {
                    events: vec![egui::Event::Key {
                        key: Key::Tab,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    }],
                    ..Default::default()
                },
                |ui| {
                    formula_ui_with_focus(ui, &mut editor, None, true);
                },
            );
            output.drop_without_applying_deltas();
            assert_eq!(editor.caret().node, target, "step {step}");
        }
        let output = context.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: Key::Tab,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ui| {
                assert!(formula_ui_with_focus(ui, &mut editor, None, true).changed());
            },
        );
        output.drop_without_applying_deltas();
        assert_eq!(
            editor.arena().get(matrix).unwrap().kind(),
            &NodeKind::Matrix {
                rows: 3,
                cols: 2,
                display: None
            }
        );
        assert!(editor.undo());
        assert_eq!(
            editor.arena().get(matrix).unwrap().kind(),
            &NodeKind::Matrix {
                rows: 2,
                cols: 2,
                display: None
            }
        );
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
                &NodeKind::Matrix {
                    rows,
                    cols,
                    display: None
                }
            );
            assert_eq!(editor.selection(), Selection::None);
        }
        assert!(editor.undo());
        assert!(editor.undo());
        assert_eq!(
            editor.arena().get(matrix).unwrap().kind(),
            &NodeKind::Matrix {
                rows: 2,
                cols: 2,
                display: None
            }
        );
    }
    #[test]
    fn structured_quantity_results_keep_source_hits_and_font_scaled_ime() {
        let context = egui::Context::default();
        crate::configure(&context);
        let mut editor = Editor::new();
        input_text(&mut editor, "2m=");
        let before = editor.to_portable().unwrap();
        let q = mathfolio_core::Quantity {
            dimension: [1., 0., -2., 0., 0., 0., 0.],
            ..mathfolio_core::Quantity::scalar(2.)
        };
        let result = crate::result_display::format(
            &mathfolio_compat::Value::Scalar { quantity: q },
            &mathfolio_document::canvas::MathFormat::default(),
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
            assert!(hits.iter().all(|h| editor.arena().get(h.node).is_some()
                && (h.operator_side.is_some() || ids.insert(h.node))));
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
#[cfg(test)]
#[path = "formula/string_literal_tests.rs"]
mod string_literal_tests;

#[cfg(test)]
#[path = "formula/scope_modifier_tests.rs"]
mod scope_modifier_tests;
