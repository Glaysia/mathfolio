//! Optional bounded per-result indices, separate from matrix values and v1 wire.
//!
//! Prime Control 06000af9 generates an axis only when that axis has hidden
//! cells; 06000aee paints headings when ShowIndices || equation focus. The
//! checked integer/count admission, font/spacing and stable reserved gutters
//! are independent policies. Never use this token for an unexecuted/cold result
//! without separately proven ORIGIN. No original unchecked-overflow parity.
use super::*;

const MAX_HEADINGS: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatrixIndexLabels {
    origin: i32,
    show_indices: bool,
}

impl MatrixIndexLabels {
    /// Fallible O(1) preflight. `origin` must belong to this executed result.
    /// It is not worksheet final/current ORIGIN, nor the native ctor's default.
    pub fn for_projection(
        p: &MatrixProjection,
        origin: i32,
        show_indices: bool,
    ) -> Result<Self, String> {
        if !valid(p) {
            return Err("invalid matrix index projection".into());
        }
        let mut count = 0usize;
        for (hidden, n, full, before, after) in [
            (
                p.hidden_above || p.hidden_below,
                p.rows,
                p.full_rows,
                p.hidden_above,
                p.hidden_below,
            ),
            (
                p.hidden_left || p.hidden_right,
                p.cols,
                p.full_cols,
                p.hidden_left,
                p.hidden_right,
            ),
        ] {
            if hidden {
                // Every intermediate visible index lies between these endpoints.
                let last = i32::try_from(full - 1)
                    .ok()
                    .and_then(|last| origin.checked_add(last));
                if last.is_none() {
                    return Err("matrix index exceeds supported i32 range".into());
                }
                count = count
                    .checked_add(n + usize::from(before) + usize::from(after))
                    .ok_or("matrix index count overflow")?;
            }
        }
        if count > MAX_HEADINGS {
            return Err("matrix index heading budget exceeded".into());
        }
        Ok(Self {
            origin,
            show_indices,
        })
    }
    pub(super) fn origin(self) -> i32 {
        self.origin
    }
    pub(super) fn show_indices(self) -> bool {
        self.show_indices
    }
}

struct Axis {
    values: Vec<Arc<Galley>>,
    before: Option<Arc<Galley>>,
    after: Option<Arc<Galley>>,
}
impl Axis {
    #[allow(
        clippy::too_many_arguments,
        reason = "Explicit bounded axis geometry; no matrix/source ownership is copied"
    )]
    fn new(
        painter: &Painter,
        origin: i32,
        offset: usize,
        n: usize,
        full: usize,
        before: bool,
        after: bool,
        font: f32,
    ) -> Option<Self> {
        if !before && !after {
            return None;
        }
        // The private token was checked against this exact projection by caller.
        let label = |index: usize| origin.checked_add(i32::try_from(index).unwrap()).unwrap();
        Some(Self {
            values: (offset..offset + n)
                .map(|index| text_galley(painter, &label(index).to_string(), font))
                .collect(),
            before: before.then(|| text_galley(painter, &format!("{}\n⋮", origin), font)),
            after: after.then(|| text_galley(painter, &format!(" ⋮\n{}", label(full - 1)), font)),
        })
    }
    fn galleys(&self) -> impl Iterator<Item = &Arc<Galley>> {
        self.values
            .iter()
            .chain(self.before.iter())
            .chain(self.after.iter())
    }
}

pub(super) struct Prepared {
    rows: Option<Axis>,
    cols: Option<Axis>,
    persistent: bool,
}
impl Prepared {
    pub(super) fn new(
        painter: &Painter,
        p: &MatrixProjection,
        labels: MatrixIndexLabels,
        size: f32,
    ) -> Self {
        let font = size * 0.75;
        Self {
            rows: Axis::new(
                painter,
                labels.origin,
                p.row_offset,
                p.rows,
                p.full_rows,
                p.hidden_above,
                p.hidden_below,
                font,
            ),
            cols: Axis::new(
                painter,
                labels.origin,
                p.col_offset,
                p.cols,
                p.full_cols,
                p.hidden_left,
                p.hidden_right,
                font,
            ),
            persistent: labels.show_indices,
        }
    }

    /// Only indexed layouts reserve more space. The old variant is byte-for-byte
    /// unchanged in layout policy. Enlarge slots so long signed labels cannot
    /// overwrite adjacent cells or continuation markers.
    pub(super) fn reserve(
        &self,
        widths: &mut [f32],
        ascents: &mut [f32],
        descents: &mut [f32],
        before: &mut Vec2,
        after: &mut Vec2,
        gap: f32,
    ) {
        if let Some(cols) = &self.cols {
            for (width, label) in widths.iter_mut().zip(&cols.values) {
                *width = (*width).max(label.size().x);
            }
            if let Some(label) = &cols.before {
                before.x = before.x.max(label.size().x + gap);
            }
            if let Some(label) = &cols.after {
                after.x = after.x.max(label.size().x + gap);
            }
        }
        if let Some(rows) = &self.rows {
            for ((ascent, descent), label) in ascents.iter_mut().zip(descents).zip(&rows.values) {
                let missing = (label.size().y - *ascent - *descent).max(0.);
                *ascent += missing / 2.;
                *descent += missing / 2.;
            }
            if let Some(label) = &rows.before {
                before.y = before.y.max(label.size().y + gap);
            }
            if let Some(label) = &rows.after {
                after.y = after.y.max(label.size().y + gap);
            }
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "Borrowed measured axis metrics and exact existing chrome geometry"
    )]
    pub(super) fn attach(
        self,
        output: &mut FormulaBox,
        widths: &[f32],
        ascents: &[f32],
        descents: &[f32],
        before: Vec2,
        after: Vec2,
        gap: f32,
    ) {
        let gutter = vec2(
            self.rows.as_ref().map_or(0., |a| {
                a.galleys().map(|g| g.size().x).fold(0., f32::max) + gap
            }),
            self.cols.as_ref().map_or(0., |a| {
                a.galleys().map(|g| g.size().y).fold(0., f32::max) + gap
            }),
        );
        for primitive in &mut output.primitives {
            match primitive {
                Primitive::Text(by, _) | Primitive::FocusedText(by, _) => *by += gutter,
                Primitive::Line(a, b) | Primitive::MinorFence(a, b) => {
                    *a += gutter;
                    *b += gutter;
                }
                Primitive::Placeholder(rect) => *rect = rect.translate(gutter),
            }
        }
        // Data, brackets, navigator markers and resize handle share one move.
        if let Some(geometry) = &mut output.matrix_viewport {
            **geometry = geometry.translated(gutter);
        }
        output.size += gutter;
        output.baseline += gutter.y;
        let output_size = output.size;
        let mut add = |by: Vec2, galley: Arc<Galley>| {
            output.primitives.push(if self.persistent {
                Primitive::Text(by, galley)
            } else {
                Primitive::FocusedText(by, galley)
            })
        };
        if let Some(rows) = self.rows {
            let x = |g: &Arc<Galley>| gutter.x - gap - g.size().x;
            let mut y = gutter.y + before.y;
            for ((a, d), label) in ascents.iter().zip(descents).zip(rows.values) {
                add(vec2(x(&label), y + (a + d - label.size().y) / 2.), label);
                y += a + d + gap;
            }
            if let Some(label) = rows.before {
                add(
                    vec2(x(&label), gutter.y + (before.y - gap - label.size().y) / 2.),
                    label,
                );
            }
            if let Some(label) = rows.after {
                add(
                    vec2(
                        x(&label),
                        output_size.y - after.y + gap + (after.y - gap - label.size().y) / 2.,
                    ),
                    label,
                );
            }
        }
        if let Some(cols) = self.cols {
            let y = |g: &Arc<Galley>| gutter.y - gap - g.size().y;
            let mut x = gutter.x + 7. + before.x;
            for (width, label) in widths.iter().zip(cols.values) {
                add(vec2(x + (width - label.size().x) / 2., y(&label)), label);
                x += width + gap;
            }
            if let Some(label) = cols.before {
                add(
                    vec2(
                        gutter.x + 7. + (before.x - gap - label.size().x) / 2.,
                        y(&label),
                    ),
                    label,
                );
            }
            if let Some(label) = cols.after {
                add(
                    vec2(
                        output_size.x - 7. - after.x + gap + (after.x - gap - label.size().x) / 2.,
                        y(&label),
                    ),
                    label,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests;
