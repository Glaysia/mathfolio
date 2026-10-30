//! Source-only visible cells. Every child keeps its full AST NodeId/index.
//! Continuation glyphs are independent chrome, never replacement data cells.
use super::*;
use mathfolio_editor::SourceMatrixDisplay;

pub(super) fn measure(
    typesetter: &Typesetter<'_>,
    id: NodeId,
    shape: [usize; 2],
    display: SourceMatrixDisplay,
    size: f32,
) -> FormulaBox {
    let Ok((offset, window)) = display.window(shape) else {
        return row_layout(
            id,
            vec![RowPiece::Text(text_galley(
                typesetter.painter,
                "invalid source window",
                size,
            ))],
        );
    };
    let node = typesetter.arena.get(id).unwrap();
    let mut cells = Vec::with_capacity(window[0] * window[1]);
    for row in offset[0]..offset[0] + window[0] {
        for col in offset[1]..offset[1] + window[1] {
            cells.push(typesetter.measure(node.children()[row * shape[1] + col], size));
        }
    }
    let mut widths = vec![0_f32; window[1]];
    let mut ascents = vec![0_f32; window[0]];
    let mut descents = vec![0_f32; window[0]];
    for (index, cell) in cells.iter().enumerate() {
        widths[index % window[1]] = widths[index % window[1]].max(cell.size.x);
        ascents[index / window[1]] = ascents[index / window[1]].max(cell.baseline);
        descents[index / window[1]] = descents[index / window[1]].max(cell.size.y - cell.baseline);
    }
    let gap = size * 0.4;
    let margin = size * 0.55;
    let width = widths.iter().sum::<f32>() + gap * (window[1] - 1) as f32 + 14.;
    let height =
        ascents.iter().sum::<f32>() + descents.iter().sum::<f32>() + gap * (window[0] - 1) as f32;
    let mut result = empty_display(id);
    result.size = vec2(width + margin * 2., height + margin * 2.);
    result.baseline = margin + height / 2. + size * 0.2;
    let mut cells = cells.into_iter();
    let mut y = margin;
    for row in 0..window[0] {
        let mut x = margin + 7.;
        for width in &widths {
            let cell = cells.next().unwrap();
            let position = vec2(
                x + (width - cell.size.x) / 2.,
                y + ascents[row] - cell.baseline,
            );
            result.children.push((position, cell));
            x += width + gap;
        }
        y += ascents[row] + descents[row] + gap;
    }
    for x in [margin + 1., margin + width - 1.] {
        let direction = if x < result.size.x / 2. { 1. } else { -1. };
        result
            .primitives
            .push(Primitive::Line(vec2(x, margin), vec2(x, margin + height)));
        for y in [margin, margin + height] {
            result
                .primitives
                .push(Primitive::Line(vec2(x, y), vec2(x + 4. * direction, y)));
        }
    }
    for (shown, glyph, position) in [
        (offset[0] > 0, "⋮", vec2(margin + width / 2., 0.)),
        (
            offset[0] + window[0] < shape[0],
            "⋮",
            vec2(margin + width / 2., margin + height),
        ),
        (offset[1] > 0, "…", vec2(0., margin + height / 2.)),
        (
            offset[1] + window[1] < shape[1],
            "…",
            vec2(margin + width, margin + height / 2.),
        ),
    ] {
        if shown {
            result.primitives.push(Primitive::Text(
                position,
                text_galley(typesetter.painter, glyph, size * 0.55),
            ));
        }
    }
    result
}

/// Keep fraction/script navigation inside a source cell before stepping a full
/// source row. Hidden target cells need no fabricated hit geometry.
pub(super) fn move_vertical(editor: &mut Editor, hits: &[HitNode], down: bool) -> bool {
    if editor.selection() != Selection::None {
        return false;
    }
    let Some(here) = hits
        .iter()
        .find(|hit| hit.node == editor.caret().node && hit.insertion == editor.caret().insertion)
    else {
        return false;
    };
    let caret = caret_rect(here, editor);
    for (matrix, cell) in editor.source_matrix_cells() {
        let candidate = hits
            .iter()
            .filter(|hit| {
                hit.node != here.node
                    // Same leaf predicate as `leaves`, evaluated only for
                    // actual visible hits. Never scan the full hidden source.
                    && editor.arena().get(hit.node).is_some_and(|node| node.children().is_empty())
                    && node_contains(editor.arena(), cell, hit.node)
                    && if down {
                        hit.rect.center().y > here.rect.center().y + 3.
                    } else {
                        hit.rect.center().y < here.rect.center().y - 3.
                    }
            })
            .min_by(|a, b| {
                let score = |hit: &HitNode| {
                    let dx = (caret.center().x - hit.rect.center().x).abs();
                    (hit.rect.center().y - here.rect.center().y).abs() + dx * 2.
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
            return true;
        }
        if editor.source_matrix_step_row(matrix, down) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod full_window_tests;
