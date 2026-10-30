//! Source display navigation policy. Full AST cell addresses remain unchanged.
//! A real source pan gets one editor checkpoint; exact Prime Undo granularity
//! is unknown. Visible entry/clamping changes only caret, never source bytes.
use super::*;

impl Editor {
    /// Source-display ancestors, nearest first, paired with their FULL AST cell.
    pub fn source_matrix_cells(&self) -> Vec<(NodeId, NodeId)> {
        let mut cells = Vec::new();
        let mut child = self.caret.node;
        while let Some(parent) = self.arena.get(child).and_then(Node::parent) {
            if matches!(
                self.arena.get(parent).map(Node::kind),
                Some(NodeKind::Matrix {
                    display: Some(_),
                    ..
                })
            ) {
                cells.push((parent, child));
            }
            child = parent;
        }
        cells
    }

    /// Enter a full source row, independently of clipped visual hit availability.
    /// Call only after in-cell vertical navigation has been considered.
    pub fn source_matrix_step_row(&mut self, matrix: NodeId, down: bool) -> bool {
        if self.selection != Selection::None || self.typeover.is_some() {
            return false;
        }
        let Some((index, _)) = source_cell(&self.arena, matrix, self.caret.node) else {
            return false;
        };
        let Some(node) = self.arena.get(matrix) else {
            return false;
        };
        let NodeKind::Matrix {
            rows,
            cols,
            display: Some(_),
        } = node.kind
        else {
            return false;
        };
        let row = index / cols;
        let next = if down {
            row.checked_add(1).filter(|r| *r < rows)
        } else {
            row.checked_sub(1)
        };
        let Some(next) = next else {
            return false;
        };
        let target = visible_first_leaf(&self.arena, node.children[next * cols + index % cols]);
        self.caret = Caret::at(target, Direction::Left);
        self.scope = None;
        true
    }

    /// Initial/import/paste/external caret entry: preserve source display and
    /// choose a real visible descendant. No Arena touch, source edit or Undo.
    pub fn clamp_source_matrix_caret(&mut self) -> bool {
        if self.selection != Selection::None {
            return false;
        }
        let before = self.caret;
        for _ in 0..MAX_TREE_DEPTH {
            let hidden = self
                .source_matrix_cells()
                .into_iter()
                .rev()
                .find_map(|(matrix, _)| {
                    clamped_source_cell(&self.arena, matrix, self.caret.node)
                        .map(|cell| (matrix, cell))
                });
            let Some((_, cell)) = hidden else {
                break;
            };
            self.caret = Caret::at(visible_first_leaf(&self.arena, cell), Direction::Left);
            self.scope = None;
        }
        before != self.caret
    }

    /// Reveal after an actual arrow/Tab transition. Shared ancestors pan; entry
    /// into a new nested matrix keeps that nested matrix's imported viewport.
    /// Effective size is materialized so offset-only sources never grow/shrink
    /// merely because their offset changed. No full Arena clone unless changed.
    pub fn reveal_source_matrix_caret_after_navigation(&mut self, before: Caret) -> bool {
        if self.selection != Selection::None
            || self.typeover.is_some()
            || self.arena.get(before.node).is_none()
        {
            return false;
        }
        for _ in 0..MAX_TREE_DEPTH {
            let hidden = self
                .source_matrix_cells()
                .into_iter()
                .rev()
                .find_map(|(matrix, _)| {
                    if source_cell(&self.arena, matrix, before.node).is_some() {
                        return None;
                    }
                    clamped_source_cell(&self.arena, matrix, self.caret.node)
                });
            let Some(cell) = hidden else {
                break;
            };
            self.caret = Caret::at(visible_first_leaf(&self.arena, cell), Direction::Left);
        }
        let mut changes = Vec::new();
        for (matrix, _) in self.source_matrix_cells() {
            if source_cell(&self.arena, matrix, before.node).is_none() {
                continue;
            }
            let node = self.arena.get(matrix).unwrap();
            let NodeKind::Matrix {
                rows,
                cols,
                display: Some(display),
            } = node.kind
            else {
                continue;
            };
            let Ok((offset, size)) = display.window([rows, cols]) else {
                continue;
            };
            let Some((index, _)) = source_cell(&self.arena, matrix, self.caret.node) else {
                continue;
            };
            let coordinate = [index / cols, index % cols];
            let mut next = offset;
            for axis in 0..2 {
                if coordinate[axis] < offset[axis] {
                    next[axis] = coordinate[axis];
                } else if coordinate[axis] >= offset[axis] + size[axis] {
                    next[axis] = coordinate[axis] + 1 - size[axis];
                }
            }
            if next != offset {
                let (Ok(row), Ok(col), Ok(nr), Ok(nc)) = (
                    u32::try_from(next[0]),
                    u32::try_from(next[1]),
                    u32::try_from(size[0]),
                    u32::try_from(size[1]),
                ) else {
                    return false;
                };
                let Ok(next) = SourceMatrixDisplay::from_parts(Some([row, col]), Some([nr, nc]))
                else {
                    return false;
                };
                changes.push((matrix, rows, cols, next));
            }
        }
        if changes.is_empty() {
            return false;
        }
        let after = self.caret;
        // The source checkpoint must restore the OLD visible caret and window,
        // not the post-navigation hidden caret paired with the old window.
        self.caret = before;
        self.checkpoint();
        self.caret = after;
        for (matrix, rows, cols, display) in changes {
            self.arena.node_mut(matrix).kind = NodeKind::Matrix {
                rows,
                cols,
                display: Some(display),
            };
        }
        debug_assert!(self.validate().is_ok());
        true
    }
}

fn source_cell(arena: &Arena, matrix: NodeId, mut child: NodeId) -> Option<(usize, NodeId)> {
    while let Some(parent) = arena.get(child)?.parent {
        if parent == matrix {
            return Some((
                arena
                    .get(matrix)?
                    .children
                    .iter()
                    .position(|id| *id == child)?,
                child,
            ));
        }
        child = parent;
    }
    None
}
fn clamped_source_cell(arena: &Arena, matrix: NodeId, node: NodeId) -> Option<NodeId> {
    let matrix_node = arena.get(matrix)?;
    let NodeKind::Matrix {
        rows,
        cols,
        display: Some(display),
    } = matrix_node.kind
    else {
        return None;
    };
    let (offset, size) = display.window([rows, cols]).ok()?;
    let (index, _) = source_cell(arena, matrix, node)?;
    let row = index / cols;
    let col = index % cols;
    let clamped = [
        row.clamp(offset[0], offset[0] + size[0] - 1),
        col.clamp(offset[1], offset[1] + size[1] - 1),
    ];
    (clamped != [row, col]).then(|| matrix_node.children[clamped[0] * cols + clamped[1]])
}
fn visible_first_leaf(arena: &Arena, mut node: NodeId) -> NodeId {
    loop {
        let current = arena.get(node).unwrap();
        if current.children.is_empty() {
            return node;
        }
        let first = match current.kind {
            NodeKind::Matrix {
                rows,
                cols,
                display: Some(display),
            } => display
                .window([rows, cols])
                .map(|(offset, _)| offset[0] * cols + offset[1])
                .unwrap_or(0),
            _ => 0,
        };
        node = current.children[first];
    }
}

#[cfg(test)]
mod tests;
