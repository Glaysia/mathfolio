//! Rectangular array selection contracts recovered from the shipped editor's
//! TypeoverState mode. See docs/analysis/0065-matrix-selection-contract.md.
use super::*;

/// Inclusive corner cell offsets in a matrix's row-major child list. Retaining
/// anchor and focus separately lets a pointer drag contract its rectangle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MatrixCellSelection {
    pub matrix: NodeId,
    pub anchor: usize,
    pub focus: usize,
}

impl MatrixCellSelection {
    /// Inclusive (top, left, bottom, right) bounds. A valid selection always
    /// has a positive column count; callers obtain it from its Matrix node.
    pub fn bounds(self, cols: usize) -> (usize, usize, usize, usize) {
        let (ar, ac) = (self.anchor / cols, self.anchor % cols);
        let (fr, fc) = (self.focus / cols, self.focus % cols);
        (ar.min(fr), ac.min(fc), ar.max(fr), ac.max(fc))
    }
}

impl Editor {
    pub(super) fn valid_matrix_selection(&self, range: MatrixCellSelection) -> bool {
        self.arena.get(range.matrix).is_some_and(|matrix| {
            matches!(matrix.kind, NodeKind::Matrix { .. })
                && range.anchor < matrix.children.len()
                && range.focus < matrix.children.len()
        })
    }

    pub fn matrix_selection(&self) -> Option<MatrixCellSelection> {
        match self.selection {
            Selection::MatrixCells(range) => Some(range),
            _ => None,
        }
    }

    /// Selection changes are navigation only and do not invalidate redo.
    pub fn select_matrix_cells(&mut self, matrix: NodeId, anchor: usize, focus: usize) -> bool {
        let range = MatrixCellSelection {
            matrix,
            anchor,
            focus,
        };
        if !self.valid_matrix_selection(range) {
            return false;
        }
        self.typeover = None;
        let cell = self.arena.get(matrix).unwrap().children[focus];
        self.caret = Caret::at(first_leaf(&self.arena, cell), Direction::Left);
        self.selection = Selection::MatrixCells(range);
        self.scope = None;
        true
    }

    pub(super) fn collapse_matrix_selection(&mut self) {
        if let Some(range) = self.matrix_selection() {
            let cell = self.arena.get(range.matrix).unwrap().children[range.anchor];
            self.caret = Caret::at(first_leaf(&self.arena, cell), Direction::Left);
            self.selection = Selection::None;
            self.scope = None;
        }
    }

    /// Clone only the selected rectangle, retaining complete cell expression
    /// trees and label metadata. Clipboard representation remains a matrix.
    pub fn copy_matrix_selection(&self) -> Option<PortableTree> {
        let range = self.matrix_selection()?;
        let matrix = self.arena.get(range.matrix)?;
        let NodeKind::Matrix { cols, .. } = matrix.kind else {
            return None;
        };
        let (top, left, bottom, right) = range.bounds(cols);
        let mut children = Vec::new();
        for row in top..=bottom {
            for col in left..=right {
                children.push(
                    self.export_subtree(matrix.children[row * cols + col])
                        .ok()?,
                );
            }
        }
        Some(PortableTree {
            kind: NodeKind::Matrix {
                rows: bottom - top + 1,
                cols: right - left + 1,
                display: None,
            },
            children,
        })
    }

    /// The original matrix Typeover mode's Paste method inherits a false
    /// implementation. Do not invent spreadsheet filling, tiling or expansion.
    pub fn paste_matrix_selection(&mut self, tree: &PortableTree) -> Result<bool, TreeError> {
        tree.validate()?;
        Ok(false)
    }

    /// Clear Cells replaces selected cell contents with placeholders without
    /// changing the surrounding array's shape. With no rectangle, clear the
    /// active matrix cell. Whole-matrix subtree selection is kept separate.
    pub fn clear_matrix_cells(&mut self) -> bool {
        let (matrix, cols, top, left, bottom, right) = if let Some(range) = self.matrix_selection()
        {
            let NodeKind::Matrix { cols, .. } = self.arena.get(range.matrix).unwrap().kind else {
                return false;
            };
            let (top, left, bottom, right) = range.bounds(cols);
            (range.matrix, cols, top, left, bottom, right)
        } else if let Some((matrix, _, cols, index, _)) = self.matrix_cell() {
            (
                matrix,
                cols,
                index / cols,
                index % cols,
                index / cols,
                index % cols,
            )
        } else {
            return false;
        };
        let indices: Vec<_> = (top..=bottom)
            .flat_map(|r| (left..=right).map(move |c| r * cols + c))
            .collect();
        let cells = self.arena.get(matrix).unwrap().children.clone();
        if indices
            .iter()
            .all(|i| self.arena.get(cells[*i]).unwrap().kind == NodeKind::Placeholder)
        {
            return false;
        }
        self.checkpoint();
        for index in indices {
            let cell = cells[index];
            let children = self.arena.get(cell).unwrap().children.clone();
            for child in children {
                self.retire(child, None);
            }
            let node = self.arena.node_mut(cell);
            node.kind = NodeKind::Placeholder;
            node.children.clear();
        }
        self.caret = Caret::at(cells[top * cols + left], Direction::Indeterminate);
        self.selection = Selection::None;
        self.scope = None;
        debug_assert!(self.validate().is_ok());
        true
    }

    /// Delete and Backspace share the original matrix block operation. Entire
    /// rows/columns are removed only when the rectangle spans that whole axis;
    /// an interior block is cleared and a full matrix becomes blank 1x1.
    pub fn delete_matrix_selection(&mut self, _backwards: bool) -> bool {
        let Some(range) = self.matrix_selection() else {
            return false;
        };
        let NodeKind::Matrix { rows, cols, .. } = self.arena.get(range.matrix).unwrap().kind else {
            return false;
        };
        let (top, left, bottom, right) = range.bounds(cols);
        if left == 0 && right + 1 == cols {
            self.remove_matrix_axis_range(range.matrix, true, top, bottom, left)
        } else if top == 0 && bottom + 1 == rows {
            self.remove_matrix_axis_range(range.matrix, false, left, right, top)
        } else {
            self.clear_matrix_cells()
        }
    }

    pub(super) fn delete_selected_matrix_axis(&mut self, row_axis: bool) -> bool {
        let Some(range) = self.matrix_selection() else {
            return false;
        };
        let NodeKind::Matrix { cols, .. } = self.arena.get(range.matrix).unwrap().kind else {
            return false;
        };
        let (top, left, bottom, right) = range.bounds(cols);
        if row_axis {
            self.remove_matrix_axis_range(range.matrix, true, top, bottom, 0)
        } else {
            self.remove_matrix_axis_range(range.matrix, false, left, right, 0)
        }
    }

    fn remove_matrix_axis_range(
        &mut self,
        matrix: NodeId,
        row_axis: bool,
        first: usize,
        last: usize,
        retained: usize,
    ) -> bool {
        let node = self.arena.get(matrix).unwrap();
        let NodeKind::Matrix { rows, cols, .. } = node.kind else {
            return false;
        };
        let old = node.children.clone();
        let removed = last - first + 1;
        let reset = removed == if row_axis { rows } else { cols };
        if reset
            && rows == 1
            && cols == 1
            && self.arena.get(old[0]).unwrap().kind == NodeKind::Placeholder
        {
            return false;
        }
        let next_shape = if reset {
            [1, 1]
        } else {
            [
                rows - if row_axis { removed } else { 0 },
                cols - if row_axis { 0 } else { removed },
            ]
        };
        let target = if row_axis {
            [
                first.min(next_shape[0] - 1),
                retained.min(next_shape[1] - 1),
            ]
        } else {
            [
                retained.min(next_shape[0] - 1),
                first.min(next_shape[1] - 1),
            ]
        };
        let Some(display) = self.matrix_display_after_reshape(
            matrix,
            [rows, cols],
            next_shape,
            source_matrix_reshape::Change::Remove {
                axis: usize::from(!row_axis),
                count: removed,
            },
            target,
        ) else {
            return false;
        };
        if !reset && self.matrix_has_source_display(matrix) {
            let old_target = if row_axis {
                (target[0] + if target[0] >= first { removed } else { 0 }) * cols + target[1]
            } else {
                target[0] * cols + target[1] + if target[1] >= first { removed } else { 0 }
            };
            if !self.matrix_reshape_target_supported(old[old_target]) {
                return false;
            }
        }
        self.checkpoint();
        let (new_rows, new_cols, children, focus) = if reset {
            for cell in old {
                self.retire(cell, None);
            }
            let cell = self.arena.allocate(NodeKind::Placeholder, vec![]);
            self.arena.node_mut(cell).parent = Some(matrix);
            (1, 1, vec![cell], cell)
        } else {
            let new_rows = rows - if row_axis { removed } else { 0 };
            let new_cols = cols - if row_axis { 0 } else { removed };
            let mut children = Vec::with_capacity(new_rows * new_cols);
            for (i, cell) in old.into_iter().enumerate() {
                let position = if row_axis { i / cols } else { i % cols };
                if (first..=last).contains(&position) {
                    self.retire(cell, None);
                } else {
                    children.push(cell);
                }
            }
            let (row, col) = if row_axis {
                (first.min(new_rows - 1), retained.min(new_cols - 1))
            } else {
                (retained.min(new_rows - 1), first.min(new_cols - 1))
            };
            let focus = first_leaf(&self.arena, children[row * new_cols + col]);
            (new_rows, new_cols, children, focus)
        };
        let node = self.arena.node_mut(matrix);
        node.kind = NodeKind::Matrix {
            rows: new_rows,
            cols: new_cols,
            display,
        };
        node.children = children;
        self.caret = Caret::at(focus, Direction::Left);
        self.selection = Selection::None;
        self.scope = None;
        debug_assert!(self.validate().is_ok());
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn number(value: usize) -> PortableTree {
        PortableTree {
            kind: NodeKind::Number(value.to_string()),
            children: vec![],
        }
    }
    fn matrix(rows: usize, cols: usize) -> PortableTree {
        PortableTree {
            kind: NodeKind::Matrix {
                rows,
                cols,
                display: None,
            },
            children: (0..rows * cols).map(number).collect(),
        }
    }
    fn values(editor: &Editor) -> Vec<String> {
        editor
            .to_portable()
            .unwrap()
            .children
            .iter()
            .map(|c| match &c.kind {
                NodeKind::Number(value) => value.clone(),
                NodeKind::Placeholder => "_".into(),
                _ => panic!("unexpected {c:?}"),
            })
            .collect()
    }

    #[test]
    fn selection_and_copy_normalize_corners_without_mutating_or_losing_cell_metadata() {
        let mut source = matrix(3, 4);
        source.children[6] = PortableTree {
            kind: NodeKind::NativeIdentifier {
                name: "V".into(),
                labels: "UNIT".into(),
                contextual: Some(false),
                subscript: Some("rms".into()),
            },
            children: vec![],
        };
        let mut editor = Editor::from_portable(&source).unwrap();
        let matrix = editor.root();
        let revision = editor.arena().revision();
        assert!(editor.select_matrix_cells(matrix, 10, 1));
        assert_eq!(editor.matrix_selection().unwrap().bounds(4), (0, 1, 2, 2));
        let copied = editor.copy_matrix_selection().unwrap();
        assert_eq!(
            copied.kind,
            NodeKind::Matrix {
                rows: 3,
                cols: 2,
                display: None
            }
        );
        assert_eq!(
            copied.children,
            [1, 2, 5, 6, 9, 10].map(|i| source.children[i].clone())
        );
        assert_eq!(editor.arena().revision(), revision);
        assert!(!editor.undo());
        assert_eq!(editor.to_portable().unwrap(), source);
        assert!(editor.validate().is_ok());
    }

    #[test]
    fn clear_cells_preserves_shape_and_cell_ids_and_is_one_undoable_command() {
        let source = matrix(3, 4);
        let mut editor = Editor::from_portable(&source).unwrap();
        let root = editor.root();
        let cells = editor.arena().get(root).unwrap().children().to_vec();
        editor.select_matrix_cells(root, 5, 10);
        let selected = editor.selection();
        assert!(editor.clear_matrix_cells());
        assert_eq!(editor.arena().get(root).unwrap().children(), cells);
        assert_eq!(
            values(&editor),
            ["0", "1", "2", "3", "4", "_", "_", "7", "8", "_", "_", "11"]
        );
        assert_eq!(editor.selection(), Selection::None);
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), source);
        assert_eq!(editor.selection(), selected);
        assert!(!editor.undo());
        assert!(editor.redo());
        assert_eq!(values(&editor)[5], "_");
        assert!(editor.validate().is_ok());
    }

    #[test]
    fn delete_and_cut_shape_rule_distinguishes_rows_columns_interior_and_whole_matrix() {
        for backwards in [false, true] {
            for (anchor, focus, shape, expected) in [
                (
                    4,
                    11,
                    (2, 4),
                    vec!["0", "1", "2", "3", "12", "13", "14", "15"],
                ),
                (
                    1,
                    14,
                    (4, 2),
                    vec!["0", "3", "4", "7", "8", "11", "12", "15"],
                ),
                (
                    5,
                    10,
                    (4, 4),
                    vec![
                        "0", "1", "2", "3", "4", "_", "_", "7", "8", "_", "_", "11", "12", "13",
                        "14", "15",
                    ],
                ),
                (15, 0, (1, 1), vec!["_"]),
            ] {
                let source = matrix(4, 4);
                let mut editor = Editor::from_portable(&source).unwrap();
                let root = editor.root();
                editor.select_matrix_cells(root, anchor, focus);
                let copied = editor.copy_matrix_selection().unwrap();
                assert!(matches!(copied.kind, NodeKind::Matrix { .. }));
                assert!(editor.delete_matrix_selection(backwards));
                assert_eq!(
                    editor.to_portable().unwrap().kind,
                    NodeKind::Matrix {
                        rows: shape.0,
                        cols: shape.1,
                        display: None
                    }
                );
                assert_eq!(values(&editor), expected);
                assert!(editor.validate().is_ok());
                assert!(editor.undo());
                assert_eq!(editor.to_portable().unwrap(), source);
                assert!(!editor.undo());
            }
        }
    }

    #[test]
    fn row_and_column_ribbon_delete_remove_all_axes_intersecting_a_partial_rectangle() {
        for row_axis in [true, false] {
            let source = matrix(4, 4);
            let mut editor = Editor::from_portable(&source).unwrap();
            let root = editor.root();
            let cells = editor.arena().get(root).unwrap().children().to_vec();
            editor.select_matrix_cells(root, 5, 10);
            assert!(if row_axis {
                editor.delete_matrix_row()
            } else {
                editor.delete_matrix_column()
            });
            let retained: Vec<_> = if row_axis {
                [0, 1, 2, 3, 12, 13, 14, 15].to_vec()
            } else {
                [0, 3, 4, 7, 8, 11, 12, 15].to_vec()
            };
            assert_eq!(
                editor.arena().get(root).unwrap().children(),
                retained.iter().map(|i| cells[*i]).collect::<Vec<_>>()
            );
            assert!(editor.validate().is_ok());
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), source);
        }
    }

    #[test]
    fn insert_in_rectangle_exits_typeover_without_changing_document_or_redo() {
        for row_axis in [true, false] {
            let source = matrix(2, 2);
            let mut editor = Editor::from_portable(&source).unwrap();
            let root = editor.root();
            editor.select_matrix_cells(root, 0, 3);
            assert!(editor.clear_matrix_cells());
            assert!(editor.undo());
            assert!(!if row_axis {
                editor.insert_matrix_row(Direction::Down)
            } else {
                editor.insert_matrix_column(Direction::Right)
            });
            assert_eq!(editor.selection(), Selection::None);
            assert_eq!(editor.to_portable().unwrap(), source);
            assert!(editor.redo());
            assert_eq!(values(&editor), ["_", "_", "_", "_"]);
        }
    }

    #[test]
    fn rectangle_paste_expression_and_operator_guards_are_noops_with_no_history_entry() {
        let source = matrix(2, 2);
        let mut editor = Editor::from_portable(&source).unwrap();
        let root = editor.root();
        editor.select_matrix_cells(root, 0, 3);
        let selection = editor.selection();
        assert!(!editor.paste_matrix_selection(&matrix(1, 2)).unwrap());
        assert!(!editor.paste_matrix_selection(&number(4)).unwrap());
        assert!(editor.replace_selection(&number(4)).is_err());
        assert!(
            editor
                .set_atom(editor.caret().node, NodeKind::Number("7".into()))
                .is_err()
        );
        assert!(!editor.toggle_literal_subscript());
        let operations: [fn(&mut Editor) -> NodeId; 19] = [
            Editor::insert_fraction,
            Editor::insert_fraction_infix,
            Editor::insert_power,
            Editor::insert_power_infix,
            Editor::insert_unary_neg,
            Editor::insert_logical_not,
            Editor::insert_root,
            Editor::insert_call,
            Editor::insert_index,
            Editor::insert_range,
            Editor::insert_step_range,
            Editor::insert_program,
            Editor::insert_if_else,
            Editor::insert_for_loop,
            Editor::insert_while_loop,
            Editor::insert_definition,
            Editor::insert_evaluation,
            Editor::insert_symbolic_evaluation,
            Editor::insert_group,
        ];
        for operation in operations {
            assert_eq!(operation(&mut editor), root);
        }
        assert_eq!(editor.insert_infix(BinaryOp::Add), root);
        assert!(editor.insert_matrix(1, 1).is_none());
        assert!(!editor.append_argument());
        assert_eq!(editor.selection(), selection);
        assert_eq!(editor.to_portable().unwrap(), source);
        assert!(!editor.undo());
    }

    #[test]
    fn clearing_nested_cells_does_not_replace_the_outer_matrix_or_its_siblings() {
        let mut source = matrix(1, 2);
        source.children[0] = matrix(2, 2);
        let mut editor = Editor::from_portable(&source).unwrap();
        let root = editor.root();
        let outer = editor.arena().get(root).unwrap().children().to_vec();
        assert!(editor.select_matrix_cells(outer[0], 0, 3));
        assert!(editor.clear_matrix_cells());
        let after = editor.to_portable().unwrap();
        assert_eq!(after.kind, source.kind);
        assert_eq!(after.children[1], source.children[1]);
        assert_eq!(
            after.children[0].kind,
            NodeKind::Matrix {
                rows: 2,
                cols: 2,
                display: None
            }
        );
        assert_eq!(editor.arena().get(root).unwrap().children(), outer);
        assert!(editor.validate().is_ok());
    }

    #[test]
    fn invalid_selection_and_already_blank_clear_do_not_mutate_or_record_history() {
        let mut editor = Editor::from_portable(&matrix(2, 2)).unwrap();
        let root = editor.root();
        let cell = editor.arena().get(root).unwrap().children()[0];
        assert!(!editor.select_matrix_cells(root, 0, 4));
        assert!(!editor.select_matrix_cells(cell, 0, 0));
        assert_eq!(editor.selection(), Selection::None);
        assert!(!editor.undo());
        editor.select_matrix_cells(root, 0, 3);
        assert!(editor.clear_matrix_cells());
        editor.select_matrix_cells(root, 0, 3);
        let revision = editor.arena().revision();
        assert!(!editor.clear_matrix_cells());
        assert_eq!(editor.arena().revision(), revision);
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), matrix(2, 2));
        assert!(!editor.undo());
    }

    #[test]
    fn a_matrix_range_inside_a_program_cannot_turn_into_a_program_line_insertion() {
        let source = PortableTree {
            kind: NodeKind::Program { statement_count: 2 },
            children: vec![matrix(2, 2), number(7)],
        };
        let mut editor = Editor::from_portable(&source).unwrap();
        let matrix = editor.arena().get(editor.root()).unwrap().children()[0];
        editor.select_matrix_cells(matrix, 0, 3);
        assert!(!editor.insert_program_line());
        assert!(editor.matrix_selection().is_some());
        assert_eq!(editor.to_portable().unwrap(), source);
        assert!(!editor.undo());
    }
}
