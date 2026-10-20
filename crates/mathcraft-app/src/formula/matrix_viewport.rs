//! Bounded flat result window, separate continuation chrome, no source mutation.
//!
//! Original contract: matrix-result-display-next-20261020. The bottom-right
//! bracket gesture is supported here; this renderer's hit tolerance, cell-step
//! quantization and single directional continuation glyphs are implementation
//! policies, not recovered Prime pixel geometry. A continuation popup supports
//! window panning. Proven per-result ORIGIN optionally enables bounded headings;
//! inner navigator handles resize the same retained result window.
mod indices;
mod navigator;
use super::*;
use crate::result_display::MatrixProjection;
pub use indices::MatrixIndexLabels;

const MAX_CELLS: usize = 65_536;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MatrixViewGesture {
    Begin {
        size: [usize; 2],
        offset: [usize; 2],
    },
    Preview {
        size: [usize; 2],
        offset: [usize; 2],
    },
    Commit {
        size: [usize; 2],
        offset: [usize; 2],
    },
    // Explicit original inner-widget Size THEN Offset command pair. Kept
    // distinct from legacy outer-bracket and Move/pan sparse requested policy.
    BeginInnerResize {
        size: [usize; 2],
        offset: [usize; 2],
    },
    PreviewInnerResize {
        size: [usize; 2],
        offset: [usize; 2],
    },
    CommitInnerResize {
        size: [usize; 2],
        offset: [usize; 2],
    },
    Cancel,
}

#[derive(Clone, Copy)]
pub(super) struct Geometry {
    bracket: Rect,
    step: Vec2,
    shape: [usize; 2],
    size: [usize; 2],
    offset: [usize; 2],
    markers: [Option<Rect>; 4],
}
impl Geometry {
    pub(super) fn translated(mut self, by: Vec2) -> Self {
        self.bracket = self.bracket.translate(by);
        for marker in self.markers.iter_mut().flatten() {
            *marker = marker.translate(by);
        }
        self
    }
    fn handle(self) -> Rect {
        Rect::from_center_size(self.bracket.right_bottom(), vec2(12., 12.))
    }
}

pub(super) fn find(layout: &FormulaBox, origin: Pos2) -> Option<Geometry> {
    layout
        .matrix_viewport
        .as_deref()
        .map(|g| g.translated(origin.to_vec2()))
        .or_else(|| {
            layout
                .children
                .iter()
                .find_map(|(by, child)| find(child, origin + *by))
        })
}

fn valid(projection: &MatrixProjection) -> bool {
    projection.rows > 0
        && projection.cols > 0
        && projection.rows.checked_mul(projection.cols) == Some(projection.cells.len())
        && projection.cells.len() <= MAX_CELLS
        && projection
            .row_offset
            .checked_add(projection.rows)
            .is_some_and(|n| n <= projection.full_rows)
        && projection
            .col_offset
            .checked_add(projection.cols)
            .is_some_and(|n| n <= projection.full_cols)
        && projection.hidden_above == (projection.row_offset > 0)
        && projection.hidden_left == (projection.col_offset > 0)
        && projection.hidden_below
            == (projection.row_offset + projection.rows < projection.full_rows)
        && projection.hidden_right
            == (projection.col_offset + projection.cols < projection.full_cols)
}

pub(super) fn measure(
    painter: &Painter,
    node: NodeId,
    p: &MatrixProjection,
    size: f32,
) -> FormulaBox {
    measure_inner(painter, node, p, None, size)
}

pub(super) fn measure_indexed(
    painter: &Painter,
    node: NodeId,
    p: &MatrixProjection,
    indices: MatrixIndexLabels,
    size: f32,
) -> FormulaBox {
    // Construction is fallible so controllers can preflight without discarding
    // the existing result. Recheck here: a token cannot authorize a changed view.
    if MatrixIndexLabels::for_projection(p, indices.origin(), indices.show_indices()).is_err() {
        return measure_result(
            painter,
            node,
            FormulaResult::Text("[matrix indices unavailable]"),
            16.,
        );
    }
    measure_inner(painter, node, p, Some(indices), size)
}

fn measure_inner(
    painter: &Painter,
    node: NodeId,
    p: &MatrixProjection,
    indices: Option<MatrixIndexLabels>,
    size: f32,
) -> FormulaBox {
    if !valid(p) || !size.is_finite() || size <= 0. {
        // Explicit failed presentation; never a plausible silently truncated matrix.
        return measure_result(
            painter,
            node,
            FormulaResult::Text("[matrix view unavailable]"),
            if size.is_finite() && size > 0. {
                size
            } else {
                16.
            },
        );
    }
    let cells: Vec<_> = p
        .cells
        .iter()
        .map(|c| measure_display_math(painter, node, c, size))
        .collect();
    if cells
        .iter()
        .any(|c| !c.size.is_finite() || !c.baseline.is_finite())
    {
        return measure_result(
            painter,
            node,
            FormulaResult::Text("[matrix view unavailable]"),
            size,
        );
    }
    let mut widths = vec![0_f32; p.cols];
    let mut ascents = vec![0_f32; p.rows];
    let mut descents = vec![0_f32; p.rows];
    for (i, c) in cells.iter().enumerate() {
        widths[i % p.cols] = widths[i % p.cols].max(c.size.x);
        ascents[i / p.cols] = ascents[i / p.cols].max(c.baseline);
        descents[i / p.cols] = descents[i / p.cols].max(c.size.y - c.baseline);
    }
    let horizontal = text_galley(painter, "…", size);
    let vertical = text_galley(painter, "⋮", size);
    let gap = size * 0.4;
    let mut before = vec2(
        if p.hidden_left {
            horizontal.size().x + gap
        } else {
            0.
        },
        if p.hidden_above {
            vertical.size().y + gap
        } else {
            0.
        },
    );
    let mut after = vec2(
        if p.hidden_right {
            horizontal.size().x + gap
        } else {
            0.
        },
        if p.hidden_below {
            vertical.size().y + gap
        } else {
            0.
        },
    );
    let headings = indices.map(|labels| indices::Prepared::new(painter, p, labels, size));
    if let Some(headings) = &headings {
        headings.reserve(
            &mut widths,
            &mut ascents,
            &mut descents,
            &mut before,
            &mut after,
            gap,
        );
    }
    let data_size = vec2(
        widths.iter().sum::<f32>() + gap * (p.cols - 1) as f32,
        ascents.iter().sum::<f32>() + descents.iter().sum::<f32>() + gap * (p.rows - 1) as f32,
    );
    let mut output = empty_display(node);
    output.size = data_size + before + after + vec2(14., 0.);
    if !output.size.is_finite() {
        return measure_result(
            painter,
            node,
            FormulaResult::Text("[matrix view unavailable]"),
            16.,
        );
    }
    output.baseline = output.size.y / 2. + size * 0.2;
    let mut y = before.y;
    let mut cells = cells.into_iter();
    for row in 0..p.rows {
        let mut x = 7. + before.x;
        for width in &widths {
            let cell = cells.next().unwrap();
            let by = vec2(
                x + (width - cell.size.x) / 2.,
                y + ascents[row] - cell.baseline,
            );
            // DisplayMath is already flattened by measure_display_math.
            output.primitives.extend(
                cell.primitives
                    .into_iter()
                    .map(|p| translate_primitive(p, by)),
            );
            x += width + gap;
        }
        y += ascents[row] + descents[row] + gap;
    }
    // Four independent chrome markers, never inserted into/replacing p.cells.
    let center = vec2(
        7. + before.x + data_size.x / 2.,
        before.y + data_size.y / 2.,
    );
    let mut markers = [None; 4];
    if p.hidden_above {
        markers[0] = Some(Rect::from_min_size(
            pos2(center.x - vertical.size().x / 2., 0.),
            vertical.size(),
        ));
        output.primitives.push(Primitive::Text(
            vec2(center.x - vertical.size().x / 2., 0.),
            vertical.clone(),
        ));
    }
    if p.hidden_below {
        markers[1] = Some(Rect::from_min_size(
            pos2(
                center.x - vertical.size().x / 2.,
                before.y + data_size.y + gap,
            ),
            vertical.size(),
        ));
        output.primitives.push(Primitive::Text(
            vec2(
                center.x - vertical.size().x / 2.,
                before.y + data_size.y + gap,
            ),
            vertical,
        ));
    }
    if p.hidden_left {
        markers[2] = Some(Rect::from_min_size(
            pos2(7., center.y - horizontal.size().y / 2.),
            horizontal.size(),
        ));
        output.primitives.push(Primitive::Text(
            vec2(7., center.y - horizontal.size().y / 2.),
            horizontal.clone(),
        ));
    }
    if p.hidden_right {
        markers[3] = Some(Rect::from_min_size(
            pos2(
                7. + before.x + data_size.x + gap,
                center.y - horizontal.size().y / 2.,
            ),
            horizontal.size(),
        ));
        output.primitives.push(Primitive::Text(
            vec2(
                7. + before.x + data_size.x + gap,
                center.y - horizontal.size().y / 2.,
            ),
            horizontal,
        ));
    }
    for x in [1., output.size.x - 1.] {
        let d = if x < output.size.x / 2. { 4. } else { -4. };
        output.primitives.extend([
            Primitive::Line(vec2(x, 0.), vec2(x, output.size.y)),
            Primitive::Line(vec2(x, 0.), vec2(x + d, 0.)),
            Primitive::Line(vec2(x, output.size.y), vec2(x + d, output.size.y)),
        ]);
    }
    output.matrix_viewport = Some(Box::new(Geometry {
        bracket: Rect::from_min_size(pos2(1., 0.), output.size - vec2(2., 0.)),
        step: vec2(
            (data_size.x + gap) / p.cols as f32,
            (data_size.y + gap) / p.rows as f32,
        )
        .max(vec2(1., 1.)),
        shape: [p.full_rows, p.full_cols],
        size: [p.rows, p.cols],
        offset: [p.row_offset, p.col_offset],
        markers,
    }));
    if let Some(headings) = headings {
        headings.attach(
            &mut output,
            &widths,
            &ascents,
            &descents,
            before,
            after,
            gap,
        );
        if !output.size.is_finite() || !output.baseline.is_finite() {
            return measure_result(
                painter,
                node,
                FormulaResult::Text("[matrix indices unavailable]"),
                16.,
            );
        }
    }
    output
}

#[derive(Clone)]
struct Drag {
    geometry: Geometry,
    start: Pos2,
    last: [usize; 2],
    root: NodeId,
    revision: u64,
}
impl Drag {
    fn compatible(&self, editor: &Editor, now: Option<Geometry>) -> bool {
        let expected_offset = [
            self.geometry.offset[0].min(self.geometry.shape[0] - self.last[0]),
            self.geometry.offset[1].min(self.geometry.shape[1] - self.last[1]),
        ];
        editor.root() == self.root
            && editor.arena().revision() == self.revision
            && now.is_some_and(|n| {
                n.shape == self.geometry.shape
                    && (n.offset == expected_offset || n.offset == self.geometry.offset)
            })
    }
    fn requested(&self, pointer: Pos2) -> [usize; 2] {
        if !pointer.is_finite() {
            return self.last;
        }
        let delta = pointer - self.start;
        let rows = (self.geometry.size[0] as f64 + (delta.y / self.geometry.step.y).round() as f64)
            .clamp(1., self.geometry.shape[0].min(MAX_CELLS) as f64) as usize;
        let cols = (self.geometry.size[1] as f64 + (delta.x / self.geometry.step.x).round() as f64)
            .clamp(1., self.geometry.shape[1].min(MAX_CELLS / rows) as f64)
            as usize;
        [rows, cols]
    }
}

#[derive(Default)]
pub(super) struct Interaction {
    pub gesture: Option<MatrixViewGesture>,
    pub owns_pointer: bool,
}

/// Controller must also discard its transaction when an owning widget ceases to
/// be drawn. This hook covers target replacement while the widget stays drawn.
pub(super) fn interact(
    ui: &mut Ui,
    owner: egui::Id,
    editor: &Editor,
    geometry: Option<Geometry>,
) -> Interaction {
    let id = owner.with("matrix-result-bracket-drag");
    let mut drag = ui.data(|d| d.get_temp::<Drag>(id));
    if drag.is_none() {
        let navigation = navigator::interact(ui, owner, editor, geometry);
        if navigation.owns_pointer || navigation.gesture.is_some() {
            return navigation;
        }
    }
    let mut output = Interaction {
        owns_pointer: drag.is_some(),
        ..Default::default()
    };
    if let Some(active) = &drag {
        let escape = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape));
        let gone = ui.input(|i| {
            i.events
                .iter()
                .any(|e| matches!(e, egui::Event::PointerGone))
        });
        if escape || gone || !active.compatible(editor, geometry) {
            ui.data_mut(|d| d.remove::<Drag>(id));
            output.gesture = Some(MatrixViewGesture::Cancel);
            return output;
        }
    }
    let handle = geometry.map(|g| ui.interact(g.handle(), id.with("hit"), Sense::drag()));
    let pointer = ui.input(|i| i.pointer.interact_pos()).map(|p| {
        ui.ctx()
            .layer_transform_from_global(ui.layer_id())
            .unwrap_or_default()
            * p
    });
    if drag.is_none()
        && let (Some(g), Some(handle)) = (geometry, handle.as_ref())
        && handle.is_pointer_button_down_on()
        && ui.input(|i| {
            i.pointer.button_pressed(egui::PointerButton::Primary) && i.modifiers.is_none()
        })
        && let Some(start) = handle.interact_pointer_pos()
    {
        drag = Some(Drag {
            geometry: g,
            start,
            last: g.size,
            root: editor.root(),
            revision: editor.arena().revision(),
        });
        output.owns_pointer = true;
        output.gesture = Some(MatrixViewGesture::Begin {
            size: g.size,
            offset: g.offset,
        });
    } else if let Some(active) = &mut drag {
        let released = ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary));
        if released {
            if let Some(pointer) = pointer {
                active.last = active.requested(pointer);
            }
            output.gesture = Some(MatrixViewGesture::Commit {
                size: active.last,
                offset: active.geometry.offset,
            });
            ui.data_mut(|d| d.remove::<Drag>(id));
            return output;
        }
        if !ui.input(|i| i.pointer.primary_down()) {
            output.gesture = Some(MatrixViewGesture::Cancel);
            ui.data_mut(|d| d.remove::<Drag>(id));
            return output;
        }
        if let Some(pointer) = pointer {
            let requested = active.requested(pointer);
            if requested != active.last {
                active.last = requested;
                output.gesture = Some(MatrixViewGesture::Preview {
                    size: requested,
                    offset: active.geometry.offset,
                });
            }
        }
    }
    if let Some(g) = geometry
        && (output.owns_pointer || handle.as_ref().is_some_and(|h| h.hovered()))
    {
        ui.painter().rect_filled(
            Rect::from_center_size(g.bracket.right_bottom(), vec2(4., 4.)),
            0.,
            INK,
        );
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNwSe);
    }
    if let Some(active) = drag {
        ui.data_mut(|d| d.insert_temp(id, active));
    }
    output
}

#[cfg(test)]
mod tests;
