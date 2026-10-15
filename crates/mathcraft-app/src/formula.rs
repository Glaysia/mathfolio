//! Direct AST renderer and a bounded structural editing surface.
//!
//! Fraction formulas follow the recovered contract. Font/padding choices, script
//! scaling, keyboard wrapping, and navigation are independent policies awaiting
//! original-app comparison. This is not a complete Prime editor or text parser.

use eframe::egui::{
    self, Color32, FontId, Galley, Key, Painter, Pos2, Rect, Response, Sense, Stroke, StrokeKind,
    Ui, Vec2, pos2, vec2,
};
use mathcraft_editor::{
    Arena, BinaryOp, Direction, Editor, FractionMetrics, NodeId, NodeKind, Selection,
    layout_fraction,
};
use std::sync::Arc;

const INK: Color32 = Color32::from_rgb(32, 38, 48);
const CARET: Color32 = Color32::from_rgb(28, 102, 206);
const PADDING: f32 = 12.0;
const FONT_SIZE: f32 = 26.0;

struct FormulaBox {
    node: NodeId,
    size: Vec2,
    baseline: f32,
    primitives: Vec<Primitive>,
    children: Vec<(Vec2, FormulaBox)>,
    atom: Option<Arc<Galley>>,
    hit_bounds: Option<Rect>,
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
}

/// Paint and edit one structural formula. The caller supplies a stable egui ID
/// scope when rendering multiple editors (`ui.push_id(worksheet_region_id, …)`).
///
/// Supported: atomic text, / ^ ( ), + - *, Space, Tab, left/right, Backspace,
/// Delete, Ctrl/Cmd+Z and redo. ':' and '=' wrap the whole current root using
/// the bounded project policy below, not Prime's complete contextual dispatcher.
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
    let initial = measure_with_result(
        ui.painter(),
        editor.arena(),
        editor.root(),
        FONT_SIZE,
        result,
    );
    let desired = (initial.size + vec2(PADDING * 2.0, PADDING * 2.0)).max(vec2(76.0, 58.0));
    let (rect, mut response) = ui.allocate_exact_size(desired, Sense::click());
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
                let cursor = galley.cursor_from_pos(pointer - hit.rect.min);
                let offset = galley
                    .text()
                    .chars()
                    .take(cursor.index.into())
                    .map(char::len_utf16)
                    .sum();
                let _ = editor.place_text_caret(hit.node, offset);
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

    if response.has_focus() {
        ui.memory_mut(|memory| {
            memory.set_focus_lock_filter(
                response.id,
                egui::EventFilter {
                    tab: true,
                    horizontal_arrows: true,
                    ..Default::default()
                },
            );
        });
        // This widget owns keyboard events only while it has focus. Pointer and
        // viewport events remain available to the surrounding worksheet.
        let events = ui.input_mut(|input| {
            let mut ours = Vec::new();
            input.events.retain(|event| {
                let owned = match event {
                    egui::Event::Text(_) | egui::Event::Paste(_) => true,
                    egui::Event::Key { key, .. } => !matches!(key, Key::Enter | Key::Escape),
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
                egui::Event::Text(text) | egui::Event::Paste(text) => {
                    changed |= input_text(editor, &text);
                }
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    if modifiers.command && key == Key::Z {
                        changed |= if modifiers.shift {
                            editor.redo()
                        } else {
                            editor.undo()
                        };
                    } else if modifiers.command && key == Key::Y {
                        changed |= editor.redo();
                    } else if !modifiers.command && !modifiers.alt {
                        match key {
                            Key::Backspace => changed |= delete_character(editor, true),
                            Key::Delete => changed |= delete_character(editor, false),
                            Key::ArrowLeft => move_horizontal(editor, false),
                            Key::ArrowRight => move_horizontal(editor, true),
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
        if changed {
            response.mark_changed();
            ui.ctx().request_repaint();
        }
    }

    let layout = measure_with_result(
        ui.painter(),
        editor.arena(),
        editor.root(),
        FONT_SIZE,
        result,
    );
    let mut hits = vec![];
    collect_hits(&layout, origin, &mut hits);
    let paint = ui.painter();
    if let Selection::Subtree(node) = editor.selection()
        && let Some(hit) = hits.iter().find(|hit| hit.node == node)
    {
        paint.rect_filled(hit.rect.expand(3.0), 2.0, CARET.gamma_multiply(0.12));
        paint.rect_stroke(
            hit.rect.expand(3.0),
            2.0,
            Stroke::new(1.0, CARET),
            StrokeKind::Outside,
        );
    }
    paint_box(paint, &layout, origin);
    if response.has_focus()
        && let Some(hit) = hits.iter().find(|hit| hit.node == editor.caret().node)
    {
        paint_caret(paint, hit, editor);
    }
    response.on_hover_text(
        "수식 직접 입력 · / 분수 · ^ 지수 · ( ) 괄호 · Space 범위 선택 · Tab 입력 칸 이동",
    )
}

fn text_galley(painter: &Painter, text: &str, size: f32) -> Arc<Galley> {
    painter.layout_no_wrap(text.into(), FontId::proportional(size), INK)
}

fn text_baseline(galley: &Galley) -> f32 {
    galley
        .rows
        .first()
        .and_then(|row| row.glyphs.first().map(|glyph| row.pos.y + glyph.pos.y))
        .unwrap_or(galley.size().y * 0.8)
}

fn measure(painter: &Painter, arena: &Arena, id: NodeId, size: f32) -> FormulaBox {
    measure_with_result(painter, arena, id, size, None)
}

fn measure_with_result(
    painter: &Painter,
    arena: &Arena,
    id: NodeId,
    size: f32,
    result_text: Option<&str>,
) -> FormulaBox {
    let node = arena.get(id).expect("validated editor tree");
    let mut result = FormulaBox {
        node: id,
        size: Vec2::ZERO,
        baseline: 0.0,
        primitives: vec![],
        children: vec![],
        atom: None,
        hit_bounds: None,
    };
    match node.kind() {
        NodeKind::Identifier(text)
        | NodeKind::Number(text)
        | NodeKind::AnnotatedIdentifier { name: text, .. } => {
            let galley = text_galley(painter, if text.is_empty() { " " } else { text }, size);
            result.size = galley.size().max(vec2(4.0, size));
            result.baseline = text_baseline(&galley);
            result
                .primitives
                .push(Primitive::Text(Vec2::ZERO, galley.clone()));
            result.atom = Some(galley);
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
        NodeKind::Fraction => {
            let numerator = measure(painter, arena, node.children()[0], size);
            let denominator = measure(painter, arena, node.children()[1], size);
            // Independent visual style values; these are not recovered PTC defaults.
            let metrics = layout_fraction(FractionMetrics {
                numerator_width: f64::from(numerator.size.x),
                numerator_height: f64::from(numerator.size.y),
                denominator_width: f64::from(denominator.size.x),
                denominator_height: f64::from(denominator.size.y),
                overhang: f64::from(size * 0.12),
                vertical_pad: f64::from(size * 0.14),
                font_baseline_adjustment: f64::from(-size * 0.20),
                bar_height: 1.25,
            });
            result.size = vec2(metrics.width as f32, metrics.height as f32);
            result.baseline = metrics.text_baseline as f32;
            let line_y = metrics.bar_origin[1] as f32 + 0.625;
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
            let base = measure(painter, arena, node.children()[0], size);
            let exponent = measure(painter, arena, node.children()[1], size * 0.72);
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
            let child = measure(painter, arena, node.children()[0], size);
            let paren_size = size.max(child.size.y * 0.93);
            let left = text_galley(painter, "(", paren_size);
            let right = text_galley(painter, ")", paren_size);
            let height = child.size.y.max(left.size().y);
            let child_y = (height - child.size.y) / 2.0;
            let paren_y = (height - left.size().y) / 2.0;
            result.size = vec2(left.size().x + child.size.x + right.size().x + 4.0, height);
            result.baseline = child_y + child.baseline;
            result
                .primitives
                .push(Primitive::Text(vec2(0.0, paren_y), left.clone()));
            result.primitives.push(Primitive::Text(
                vec2(left.size().x + child.size.x + 4.0, paren_y),
                right,
            ));
            result
                .children
                .push((vec2(left.size().x + 2.0, child_y), child));
        }
        NodeKind::Binary(op) => {
            let left = measure(painter, arena, node.children()[0], size);
            let mut right = measure(painter, arena, node.children()[1], size);
            if *op == BinaryOp::Evaluate
                && let Some(value) = result_text
            {
                let value = text_galley(painter, value, size);
                match arena.get(node.children()[1]).unwrap().kind() {
                    NodeKind::Placeholder => {
                        right.size = value.size();
                        right.baseline = text_baseline(&value);
                        right.primitives = vec![Primitive::Text(Vec2::ZERO, value)];
                    }
                    _ => {
                        let unit_size = right.size;
                        let prefix = value.size().x + size * 0.18;
                        let baseline = right.baseline.max(text_baseline(&value));
                        let value_y = baseline - text_baseline(&value);
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
                        right
                            .primitives
                            .push(Primitive::Text(vec2(0.0, value_y), value.clone()));
                        right.hit_bounds =
                            Some(Rect::from_min_size(pos2(prefix, unit_y), unit_size));
                        right.size = vec2(
                            prefix + unit_size.x,
                            (unit_y + unit_size.y).max(value_y + value.size().y),
                        );
                        right.baseline = baseline;
                    }
                }
            }
            let symbol = match op {
                BinaryOp::Add => "+",
                BinaryOp::Subtract => "−",
                BinaryOp::Multiply => "·",
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

fn collect_hits(layout: &FormulaBox, origin: Pos2, hits: &mut Vec<HitNode>) {
    hits.push(HitNode {
        node: layout.node,
        rect: layout.hit_bounds.map_or_else(
            || Rect::from_min_size(origin, layout.size),
            |bounds| bounds.translate(origin.to_vec2()),
        ),
        atom: layout.atom.clone(),
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

fn paint_box(painter: &Painter, layout: &FormulaBox, origin: Pos2) {
    for primitive in &layout.primitives {
        match primitive {
            Primitive::Text(offset, galley) => {
                painter.galley(origin + *offset, galley.clone(), INK);
            }
            Primitive::Line(start, end) => {
                painter.line_segment([origin + *start, origin + *end], Stroke::new(1.25, INK));
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
    let caret = editor.caret();
    let mut x = if caret.caret_side == Direction::Left {
        hit.rect.left()
    } else {
        hit.rect.right()
    };
    if let Some(galley) = &hit.atom
        && let Some(range) = caret.value_range
    {
        let mut length = 0;
        let index = galley
            .text()
            .chars()
            .take_while(|ch| {
                let take = length < range.focus;
                length += ch.len_utf16();
                take
            })
            .count();
        let cursor = egui::text::CCursor::new(index);
        x = hit.rect.min.x + galley.pos_from_cursor(cursor).left();
    }
    painter.line_segment(
        [pos2(x, hit.rect.top()), pos2(x, hit.rect.bottom())],
        Stroke::new(1.5, CARET),
    );
    painter.line_segment(
        [pos2(x, hit.rect.bottom()), pos2(x + 5.0, hit.rect.bottom())],
        Stroke::new(1.5, CARET),
    );
}

pub(crate) fn input_text(editor: &mut Editor, text: &str) -> bool {
    let mut changed = false;
    for ch in text.chars() {
        match ch {
            '/' => {
                editor.insert_fraction();
                changed = true;
            }
            '^' => {
                editor.insert_power();
                changed = true;
            }
            '(' => {
                editor.insert_group();
                changed = true;
            }
            ')' => {
                editor.exit_group_right();
            }
            ' ' => {
                editor.expand_scope();
            }
            '+' => {
                editor.insert_binary(BinaryOp::Add);
                changed = true;
            }
            '-' => {
                editor.insert_binary(BinaryOp::Subtract);
                changed = true;
            }
            '*' => {
                editor.insert_binary(BinaryOp::Multiply);
                changed = true;
            }
            ':' => changed |= insert_root_command(editor, BinaryOp::Define),
            '=' => changed |= insert_root_command(editor, BinaryOp::Evaluate),
            // These need dedicated structural/context rules; do not disguise them as text.
            '[' | ']' | '\r' | '\n' | '\t' => {}
            ch if !ch.is_control() => {
                // A unit/name typed after a numeric atom becomes multiplication,
                // preserving two editable operands instead of the name "2m".
                // Full Prime implicit-operator/caret behavior still needs probes.
                if ch.is_alphabetic()
                    && matches!(
                        editor.arena().get(editor.caret().node).map(|n| n.kind()),
                        Some(NodeKind::Number(_))
                    )
                {
                    editor.insert_binary(BinaryOp::Multiply);
                }
                changed |= insert_character(editor, ch);
            }
            _ => {}
        }
    }
    changed
}

/// Independent bounded policy. REA shows that Prime's '=' first attempts a
/// local range definition and has evaluation/undo branches; this is not that
/// complete dispatcher. Definitions keep their own RHS and do not acquire a
/// second evaluation node. In particular ':' followed by '=' inserts only ':='.
fn insert_root_command(editor: &mut Editor, operation: BinaryOp) -> bool {
    match editor.arena().get(editor.root()).unwrap().kind() {
        NodeKind::Binary(BinaryOp::Define | BinaryOp::Evaluate) => false,
        _ => {
            let _ = editor.select_subtree(editor.root());
            editor.insert_binary(operation);
            true
        }
    }
}

fn atomic_text(editor: &Editor, node: NodeId) -> Option<String> {
    match editor.arena().get(node)?.kind() {
        NodeKind::Number(text) | NodeKind::Identifier(text) => Some(text.clone()),
        NodeKind::AnnotatedIdentifier { name, .. } => Some(name.clone()),
        NodeKind::Placeholder => Some(String::new()),
        _ => None,
    }
}

fn text_kind(text: String) -> NodeKind {
    if text.is_empty() {
        NodeKind::Placeholder
    } else if text.chars().all(|ch| ch.is_ascii_digit() || ch == '.') {
        NodeKind::Number(text)
    } else {
        NodeKind::Identifier(text)
    }
}

fn edited_text_kind(editor: &Editor, target: NodeId, text: String) -> NodeKind {
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
    let Some(mut text) = atomic_text(editor, target) else {
        return false;
    };
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
            return false;
        };
        (start, byte)
    } else {
        let Some(ch) = text[byte..].chars().next() else {
            return false;
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
    }
    cycle_leaf(editor, forward, false);
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(!input_text(&mut definition, "="));
        assert_eq!(definition.root(), root);
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
            assert!(bounds.min.x > 0.0);
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
}
