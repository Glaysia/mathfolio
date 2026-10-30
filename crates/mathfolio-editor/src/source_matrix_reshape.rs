//! Bounded source-shape commands, independent of evaluated-result viewports.
//!
//! Prime Equation 0239/023a transfer the old viewport only when IsResized;
//! 0256/0257 shrink by deleted COUNT, not intersection with the visible window.
//! Their inspected command chain does not resolve an out-of-window new caret.
//! Such cases remain atomic no-ops here; no new auto-pan policy is inferred.
use super::*;

#[derive(Clone, Copy)]
pub(super) enum Change {
    Insert { axis: usize, position: usize },
    Remove { axis: usize, count: usize },
}

impl Editor {
    pub(super) fn matrix_display_after_reshape(
        &self,
        matrix: NodeId,
        old_shape: [usize; 2],
        new_shape: [usize; 2],
        change: Change,
        target: [usize; 2],
    ) -> Option<Option<SourceMatrixDisplay>> {
        let NodeKind::Matrix { display, .. } = self.arena.get(matrix)?.kind else {
            return None;
        };
        transfer(display, old_shape, new_shape, change, target)
    }

    // Prime's bq.c(Left,retainedCell) selects the leftmost ALLOWED state, which
    // can be an outfix's outer Left rather than first_leaf. Preserve the old
    // editor rule only for atomic retained cells; compound entry stays closed.
    pub(super) fn matrix_reshape_target_supported(&self, cell: NodeId) -> bool {
        self.arena
            .get(cell)
            .is_some_and(|node| node.children.is_empty())
    }
}

fn transfer(
    display: Option<SourceMatrixDisplay>,
    old_shape: [usize; 2],
    new_shape: [usize; 2],
    change: Change,
    target: [usize; 2],
) -> Option<Option<SourceMatrixDisplay>> {
    // Existing None factories stay full at 13+ rows; min(12,full) belongs to
    // other original construction paths, not these shape command helpers.
    let Some(display) = display else {
        return Some(None);
    };
    let (mut offset, mut size) = display.window(old_shape).ok()?;
    if offset == [0, 0] && size == old_shape {
        return Some(None); // original IsResized=false, new full factory
    }
    match change {
        Change::Insert { axis, position } => {
            if axis > 1 || position > old_shape[axis] {
                return None;
            }
            if position >= offset[axis].checked_add(size[axis])? {
                offset[axis] = offset[axis].checked_add(1)?;
            }
        }
        Change::Remove { axis, count } => {
            if axis > 1 || count == 0 || count > old_shape[axis] {
                return None;
            }
            size[axis] = size[axis].saturating_sub(count).max(1);
            for a in 0..2 {
                size[a] = size[a].min(new_shape[a]);
                offset[a] = offset[a].min(new_shape[a].checked_sub(size[a])?);
            }
        }
    }
    if (0..2).any(|a| {
        target[a] < offset[a]
            || offset[a]
                .checked_add(size[a])
                .is_none_or(|end| target[a] >= end)
    }) {
        return None;
    }
    if offset == [0, 0] && size == new_shape {
        return Some(None);
    }
    let offset_pair = [
        u32::try_from(offset[0]).ok()?,
        u32::try_from(offset[1]).ok()?,
    ];
    let size_pair = [u32::try_from(size[0]).ok()?, u32::try_from(size[1]).ok()?];
    let offset = (offset != [0, 0]).then_some(offset_pair);
    let size = (size != new_shape).then_some(size_pair);
    let next = SourceMatrixDisplay::from_parts(offset, size).ok()?;
    next.window(new_shape).ok()?;
    Some(Some(next))
}

#[cfg(test)]
mod tests;
