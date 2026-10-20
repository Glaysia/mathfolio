//! MATRIX's native default is one empty cell. Its analysis handle is an operand,
//! unlike the opening GROUPING command's explicit prefix override (analysis0131).
use super::grouping_insertion::{grouping_fragment, grouping_text_slice};
use super::typed_operator_insertion::{
    Fixity, Handle, typed_completed_left, typed_completed_right,
};
use super::*;

impl Editor {
    /// Insert the installed MATRIX command (default 1 by 1), preserving the
    /// distinction between Scope wrapping, full text selection and adjacency.
    /// Unsupported parent repairs and formatted splits are atomic no-ops.
    pub fn insert_native_matrix(&mut self) -> bool {
        if self.matrix_selection().is_some()
            || self.typeover_target().is_some()
            || self.caret.insertion != InsertionMode::Normal
        {
            return false;
        }
        let mut candidate = self.clone_without_history();
        if !candidate.matrix_command_candidate() || candidate.to_portable().is_err() {
            return false;
        }
        self.checkpoint();
        self.arena = candidate.arena;
        self.root = candidate.root;
        self.caret = candidate.caret;
        self.selection = Selection::None;
        self.scope = None;
        true
    }

    fn matrix_command_candidate(&mut self) -> bool {
        let target = self.target();
        let source = self.arena.get(target).unwrap().clone();
        let Some(container) = self.typed_container_root(target) else {
            return false;
        };
        let scope = matches!(self.selection, Selection::Subtree(_));
        let text = source.kind.text();
        let formatted = matches!(
            source.kind,
            NodeKind::NativeIdentifier {
                subscript: Some(_),
                ..
            } | NodeKind::Imaginary(_)
                | NodeKind::ImaginarySymbol { .. }
        );
        let length = text.map(|s| s.encode_utf16().count());
        let range = if text.is_some() {
            match self.caret.caret_side {
                Direction::Left => Some(ValueRange {
                    anchor: 0,
                    focus: 0,
                }),
                Direction::Right => Some(ValueRange {
                    anchor: length.unwrap(),
                    focus: length.unwrap(),
                }),
                Direction::Indeterminate if !formatted => {
                    Some(self.caret.value_range.unwrap_or(ValueRange {
                        anchor: 0,
                        focus: 0,
                    }))
                }
                _ => None,
            }
        } else {
            None
        };
        let whole = range.is_some_and(|r| {
            r.anchor != r.focus
                && r.anchor.min(r.focus) == 0
                && Some(r.anchor.max(r.focus)) == length
        });
        if scope || whole {
            let side = if scope {
                self.scope
                    .as_ref()
                    .map_or(self.caret.caret_side, |s| s.direction)
            } else {
                Direction::Indeterminate
            };
            if scope && !matches!(side, Direction::Left | Direction::Right) {
                return false;
            }
            let Some(entry) = self.program_operand_left_entry(target) else {
                return false;
            };
            let matrix = self.arena.allocate(
                NodeKind::Matrix {
                    rows: 1,
                    cols: 1,
                    display: None,
                },
                vec![target],
            );
            self.replace_child(source.parent, target, matrix);
            self.caret = if scope {
                Caret::at(matrix, side)
            } else {
                entry
            };
            self.selection = Selection::None;
            self.scope = None;
            return true;
        }

        let parent = self.arena.get(container).unwrap().parent;
        let mut handles = vec![];
        if !self.typed_emit(container, &mut handles, 0) {
            return false;
        }
        let Some(position) = handles.iter().position(|h| h.node == target) else {
            return false;
        };
        let blank = source.kind == NodeKind::Placeholder;
        let cell = if blank {
            target
        } else {
            self.arena.allocate(NodeKind::Placeholder, vec![])
        };
        let matrix = self.arena.allocate(
            NodeKind::Matrix {
                rows: 1,
                cols: 1,
                display: None,
            },
            vec![cell],
        );
        let new = Handle {
            node: matrix,
            fixity: Fixity::Operand,
        };
        if blank {
            handles.splice(position..=position, [new]);
        } else if let Some(text) = text {
            let Some(range) = range else {
                return false;
            };
            let (start, end) = (range.anchor.min(range.focus), range.anchor.max(range.focus));
            let length = length.unwrap();
            if end > length {
                return false;
            }
            if start == 0 && end == 0 {
                handles.insert(position, new);
            } else if start == length && end == length {
                handles.insert(position + 1, new);
            } else {
                if formatted {
                    return false;
                }
                let (Some(prefix), Some(suffix)) = (
                    grouping_text_slice(text, 0, start),
                    grouping_text_slice(text, end, length),
                ) else {
                    return false;
                };
                let (Some(left), Some(right)) = (
                    grouping_fragment(&source.kind, prefix.clone()),
                    grouping_fragment(&source.kind, suffix.clone()),
                ) else {
                    return false;
                };
                let mut replacement = vec![];
                if !prefix.is_empty() {
                    self.arena.node_mut(target).kind = left;
                    replacement.push(Handle {
                        node: target,
                        fixity: Fixity::Operand,
                    });
                }
                replacement.push(new);
                if !suffix.is_empty() {
                    let right = if prefix.is_empty() {
                        self.arena.node_mut(target).kind = right;
                        target
                    } else {
                        self.arena.allocate(right, vec![])
                    };
                    replacement.push(Handle {
                        node: right,
                        fixity: Fixity::Operand,
                    });
                } else if prefix.is_empty() {
                    self.retire(target, None);
                }
                handles.splice(position..=position, replacement);
            }
        } else {
            match self.caret.caret_side {
                Direction::Right if typed_completed_right(&source.kind) => {
                    handles.insert(position + 1, new)
                }
                Direction::Left if typed_completed_left(&source.kind) => {
                    handles.insert(position, new)
                }
                _ => return false,
            }
        }
        let handles = self.typed_normalize(handles);
        let Some(root) = self.typed_parse(&handles) else {
            return false;
        };
        self.replace_child(parent, container, root);
        self.arena.node_mut(root).parent = parent;
        self.typed_power_group_repair(root);
        self.caret = Caret::at(cell, Direction::Indeterminate);
        self.selection = Selection::None;
        self.scope = None;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tree(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
        PortableTree { kind, children }
    }
    fn number(text: &str) -> PortableTree {
        tree(NodeKind::Number(text.into()), vec![])
    }
    fn blank_matrix() -> PortableTree {
        tree(
            NodeKind::Matrix {
                rows: 1,
                cols: 1,
                display: None,
            },
            vec![tree(NodeKind::Placeholder, vec![])],
        )
    }
    fn scale(left: PortableTree, right: PortableTree) -> PortableTree {
        tree(NodeKind::Binary(BinaryOp::Scale), vec![left, right])
    }

    #[test]
    fn default_matrix_has_one_cell_and_can_be_expanded_and_filled() {
        let mut e = Editor::new();
        let original = e.root();
        assert!(e.insert_native_matrix());
        assert_eq!(e.to_portable().unwrap(), blank_matrix());
        assert_eq!(e.caret().node, original);

        e.set_atom(original, NodeKind::Number("1".into())).unwrap();
        assert_eq!(e.matrix_tab(true), Some(true));
        let row_two = e.caret().node;
        e.set_atom(row_two, NodeKind::Number("3".into())).unwrap();
        assert!(e.insert_matrix_column(Direction::Right));
        let matrix = e.root();
        assert_eq!(
            e.arena.get(matrix).unwrap().kind,
            NodeKind::Matrix {
                rows: 2,
                cols: 2,
                display: None
            }
        );
        let cells = e.arena.get(matrix).unwrap().children.clone();
        e.set_atom(cells[1], NodeKind::Number("2".into())).unwrap();
        e.set_atom(cells[3], NodeKind::Number("4".into())).unwrap();
        assert_eq!(
            e.to_portable().unwrap(),
            tree(
                NodeKind::Matrix {
                    rows: 2,
                    cols: 2,
                    display: None
                },
                vec![number("1"), number("2"), number("3"), number("4")]
            )
        );
        assert!(e.validate().is_ok());
    }

    #[test]
    fn normal_boundaries_insert_adjacent_operand_and_focus_new_cell() {
        for (side, expected) in [
            (Direction::Left, scale(blank_matrix(), number("12"))),
            (Direction::Right, scale(number("12"), blank_matrix())),
        ] {
            let mut e = Editor::from_portable(&number("12")).unwrap();
            let before = e.root();
            e.place_caret(before, side).unwrap();
            let caret = e.caret();
            assert!(e.insert_native_matrix());
            assert_eq!(e.to_portable().unwrap(), expected);
            assert_eq!(
                e.arena.get(e.caret().node).unwrap().kind,
                NodeKind::Placeholder
            );
            assert_eq!(
                e.arena.get(before).unwrap().kind,
                NodeKind::Number("12".into())
            );
            let result = e.to_portable().unwrap();
            assert!(e.undo());
            assert_eq!(e.to_portable().unwrap(), number("12"));
            assert_eq!(e.caret(), caret);
            assert!(!e.undo());
            assert!(e.redo());
            assert_eq!(e.to_portable().unwrap(), result);
        }
    }

    #[test]
    fn full_value_selection_wraps_while_partial_selection_removes_selected_text() {
        for (anchor, focus) in [(0, 4), (4, 0)] {
            let mut e = Editor::from_portable(&number("1234")).unwrap();
            let old = e.root();
            e.place_text_selection(old, anchor, focus).unwrap();
            assert!(e.insert_native_matrix());
            assert_eq!(
                e.to_portable().unwrap(),
                tree(
                    NodeKind::Matrix {
                        rows: 1,
                        cols: 1,
                        display: None
                    },
                    vec![number("1234")]
                )
            );
            assert_eq!(e.caret().node, old);
            assert_eq!(
                e.caret().value_range,
                Some(ValueRange {
                    anchor: 0,
                    focus: 0
                })
            );
        }
        let mut e = Editor::from_portable(&number("1234")).unwrap();
        e.place_text_selection(e.root(), 1, 3).unwrap();
        assert!(e.insert_native_matrix());
        assert_eq!(
            e.to_portable().unwrap(),
            scale(scale(number("1"), blank_matrix()), number("4"))
        );
        assert_eq!(
            e.arena.get(e.caret().node).unwrap().kind,
            NodeKind::Placeholder
        );
    }

    #[test]
    fn container_boundary_retains_power_exponent_and_local_reparse_preserves_binding() {
        let source = tree(NodeKind::Power, vec![number("2"), number("34")]);
        let mut e = Editor::from_portable(&source).unwrap();
        let exponent = e.arena.get(e.root()).unwrap().children[1];
        e.place_text_caret(exponent, 1).unwrap();
        assert!(e.insert_native_matrix());
        assert_eq!(
            e.to_portable().unwrap(),
            tree(
                NodeKind::Power,
                vec![
                    number("2"),
                    scale(scale(number("3"), blank_matrix()), number("4"))
                ]
            )
        );
        let source = tree(
            NodeKind::Binary(BinaryOp::Scale),
            vec![number("2"), number("3")],
        );
        let mut e = Editor::from_portable(&source).unwrap();
        let right = e.arena.get(e.root()).unwrap().children[1];
        e.place_text_caret(right, 1).unwrap();
        assert!(e.insert_native_matrix());
        assert_eq!(e.to_portable().unwrap(), scale(source, blank_matrix()));
    }

    #[test]
    fn scope_preserves_selected_expression_and_native_outer_side() {
        for side in [Direction::Left, Direction::Right] {
            let source = tree(
                NodeKind::Binary(BinaryOp::Add),
                vec![number("1"), number("2")],
            );
            let mut e = Editor::from_portable(&source).unwrap();
            let node = e.root();
            e.select_subtree_from(node, side).unwrap();
            let caret = e.caret();
            assert!(e.insert_native_matrix());
            assert_eq!(
                e.to_portable().unwrap(),
                tree(
                    NodeKind::Matrix {
                        rows: 1,
                        cols: 1,
                        display: None
                    },
                    vec![source.clone()]
                )
            );
            assert_eq!(e.caret().caret_side, side);
            assert_eq!(e.caret().node, e.root());
            assert_eq!(e.selection(), Selection::None);
            assert!(e.undo());
            assert_eq!(e.to_portable().unwrap(), source);
            assert_eq!(e.caret(), caret);
            assert_eq!(e.selection(), Selection::Subtree(node));
        }
    }

    #[test]
    fn space_scope_history_and_formatted_operand_metadata_survive_one_undo() {
        for side in [Direction::Left, Direction::Right] {
            let source = tree(
                NodeKind::NativeIdentifier {
                    name: "I".into(),
                    labels: "UNIT".into(),
                    contextual: Some(true),
                    subscript: Some("dc".into()),
                },
                vec![],
            );
            let mut e = Editor::from_portable(&source).unwrap();
            e.place_caret(e.root(), side).unwrap();
            let origin = e.caret();
            assert!(e.expand_scope());
            assert!(e.scope.is_some());
            assert!(e.insert_native_matrix());
            assert_eq!(
                e.to_portable().unwrap(),
                tree(
                    NodeKind::Matrix {
                        rows: 1,
                        cols: 1,
                        display: None
                    },
                    vec![source.clone()]
                )
            );
            assert_eq!(e.caret().caret_side, side);
            assert!(e.scope.is_none());
            assert!(e.undo());
            assert!(e.scope.is_some());
            assert!(e.contract_scope());
            assert_eq!(e.caret(), origin);
            assert_eq!(e.to_portable().unwrap(), source);
            assert!(!e.undo());
        }
    }
    #[test]
    fn exact_live_node_budget_counts_matrix_wrapper_and_failed_growth_preserves_redo() {
        let mut source = number("1");
        for _ in 0..13 {
            source = tree(
                NodeKind::Binary(BinaryOp::Add),
                vec![source.clone(), source],
            );
        }
        let mut e = Editor::from_portable(&source).unwrap();
        assert_eq!(e.arena.nodes().count(), MAX_TREE_NODES - 1);
        e.select_subtree_from(e.root(), Direction::Right).unwrap();
        assert!(e.insert_native_matrix());
        assert!(e.validate().is_ok());
        assert!(e.undo());
        assert_eq!(e.to_portable().unwrap(), source);

        source = tree(NodeKind::Group, vec![source]);
        let mut e = Editor::from_portable(&source).unwrap();
        let first = first_leaf(&e.arena, e.root());
        e.set_atom(first, NodeKind::Number("2".into())).unwrap();
        assert!(e.undo());
        e.select_subtree_from(e.root(), Direction::Right).unwrap();
        let before = e.caret();
        let revision = e.arena.revision();
        assert!(!e.insert_native_matrix());
        assert_eq!(e.to_portable().unwrap(), source);
        assert_eq!(e.caret(), before);
        assert_eq!(e.arena.revision(), revision);
        assert!(e.redo());
        assert_eq!(
            e.arena.get(first).unwrap().kind,
            NodeKind::Number("2".into())
        );
    }
    #[test]
    fn unsupported_formatted_split_and_depth_limit_preserve_source_caret_and_redo() {
        let source = tree(
            NodeKind::NativeIdentifier {
                name: "I".into(),
                labels: "VARIABLE".into(),
                contextual: None,
                subscript: Some("dc".into()),
            },
            vec![],
        );
        let mut e = Editor::from_portable(&source).unwrap();
        e.set_atom(e.root(), NodeKind::Number("3".into())).unwrap();
        assert!(e.undo());
        e.place_subscript_selection(e.root(), 1, 1).unwrap();
        let caret = e.caret();
        let revision = e.arena.revision();
        assert!(!e.insert_native_matrix());
        assert_eq!(e.to_portable().unwrap(), source);
        assert_eq!(e.caret(), caret);
        assert_eq!(e.arena.revision(), revision);
        assert!(e.redo());
        let mut source = number("1");
        for _ in 0..MAX_TREE_DEPTH - 1 {
            source = tree(NodeKind::Group, vec![source]);
        }
        let mut e = Editor::from_portable(&source).unwrap();
        e.select_subtree_from(e.root(), Direction::Right).unwrap();
        let caret = e.caret();
        let revision = e.arena.revision();
        assert!(!e.insert_native_matrix());
        assert_eq!(e.to_portable().unwrap(), source);
        assert_eq!(e.caret(), caret);
        assert_eq!(e.arena.revision(), revision);
        assert!(!e.undo());
    }
}
