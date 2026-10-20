//! Original operator identities and bounded structural insertion policies.
//! Norm's original command uses DETERMINANT/determinant; the ordinary | shortcut
//! uses ABSOLUTEVALUE/absval. See docs/analysis/0069-structural-array-operators-contract.md.
use super::*;

impl Editor {
    /// Postfix factorial on the current operand; leave the caret after its mark.
    pub fn insert_factorial(&mut self) -> NodeId {
        self.insert_postfix_operator(NodeKind::Factorial)
    }

    /// Plain transpose, distinct from the conjugate operator.
    pub fn insert_transpose(&mut self) -> NodeId {
        self.insert_postfix_operator(NodeKind::Transpose)
    }

    /// Complex conjugate is an overline container, not a function call name.
    pub fn insert_conjugate(&mut self) -> NodeId {
        self.wrap_unary(NodeKind::Conjugate)
    }

    /// The original "absolute value/determinant" command and | key use absval.
    pub fn insert_determinant(&mut self) -> NodeId {
        self.insert_absolute()
    }

    /// The original Norm command uses descriptor DETERMINANT and native
    /// determinant. There is no separate inferred <norm/> XML operator here.
    pub fn insert_norm(&mut self) -> NodeId {
        self.wrap_unary(NodeKind::Determinant)
    }

    pub fn insert_row_operator(&mut self) -> NodeId {
        self.wrap_binary(NodeKind::Row)
    }

    pub fn insert_column_operator(&mut self) -> NodeId {
        self.wrap_binary(NodeKind::Column)
    }

    pub(super) fn insert_postfix_operator(&mut self, kind: NodeKind) -> NodeId {
        if let Some(range) = self.matrix_selection() {
            return range.matrix;
        }
        self.checkpoint();
        let target = self.target();
        let parent = self.arena.get(target).unwrap().parent;
        let empty = self.arena.get(target).unwrap().kind == NodeKind::Placeholder;
        let grouped = (kind == NodeKind::Transpose
            && matches!(
                self.arena.get(target).unwrap().kind,
                NodeKind::Row | NodeKind::Column
            ))
            || (kind == NodeKind::Percent
                && matches!(
                    self.arena.get(target).unwrap().kind,
                    NodeKind::Power | NodeKind::Root
                ));
        let operand = if grouped {
            self.arena.allocate(NodeKind::Group, vec![target])
        } else {
            target
        };
        let wrapper = self.arena.allocate(kind, vec![operand]);
        self.replace_child(parent, target, wrapper);
        self.caret = if empty {
            Caret::at(target, Direction::Indeterminate)
        } else {
            Caret::at(wrapper, Direction::Right)
        };
        self.selection = Selection::None;
        self.scope = None;
        debug_assert!(self.validate().is_ok());
        wrapper
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn atom(value: &str) -> PortableTree {
        PortableTree {
            kind: NodeKind::Identifier(value.into()),
            children: vec![],
        }
    }

    #[test]
    fn descriptor_precedence_arity_and_native_command_aliases_are_explicit() {
        for (kind, name, precedence, arity) in [
            (NodeKind::Factorial, "FACTORIAL", 180, 1),
            (NodeKind::Transpose, "MATRIX_TRANSPOSE", 160, 1),
            (NodeKind::Conjugate, "CONJUGATE", 130, 1),
            (NodeKind::Determinant, "DETERMINANT", 190, 1),
            (NodeKind::Row, "MATRIX_ROW", 160, 2),
            (NodeKind::Column, "MATRIX_COLUMN", 160, 2),
        ] {
            assert_eq!(kind.descriptor_name(), name);
            assert_eq!(kind.precedence(), precedence);
            assert_eq!(kind.arity(), arity);
            assert!(
                PortableTree {
                    kind: kind.clone(),
                    children: vec![atom("A"); arity]
                }
                .validate()
                .is_ok()
            );
            assert!(
                PortableTree {
                    kind,
                    children: vec![atom("A"); arity + 1]
                }
                .validate()
                .is_err()
            );
        }
        let mut editor = Editor::from_portable(&atom("A")).unwrap();
        editor.insert_determinant();
        assert_eq!(editor.to_portable().unwrap().kind, NodeKind::Absolute);
        editor.undo();
        editor.insert_norm();
        assert_eq!(editor.to_portable().unwrap().kind, NodeKind::Determinant);
    }

    #[test]
    fn postfix_and_row_column_insertion_preserve_operand_and_undo_as_one_command() {
        for operation in [Editor::insert_factorial, Editor::insert_transpose] {
            let mut editor = Editor::from_portable(&atom("A")).unwrap();
            let old = editor.root();
            let wrapper = operation(&mut editor);
            assert_eq!(editor.arena().get(wrapper).unwrap().children(), [old]);
            assert_eq!(editor.caret().node, wrapper);
            assert_eq!(editor.caret().caret_side, Direction::Right);
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), atom("A"));
            assert!(!editor.undo());
            assert!(editor.redo());
            assert_eq!(editor.arena().get(wrapper).unwrap().children(), [old]);
        }
        for (operation, kind) in [
            (
                Editor::insert_row_operator as fn(&mut Editor) -> NodeId,
                NodeKind::Row,
            ),
            (Editor::insert_column_operator, NodeKind::Column),
        ] {
            let mut editor = Editor::from_portable(&atom("A")).unwrap();
            let old = editor.root();
            let wrapper = operation(&mut editor);
            assert_eq!(editor.arena().get(wrapper).unwrap().kind(), &kind);
            assert_eq!(editor.arena().get(wrapper).unwrap().children()[0], old);
            assert_eq!(
                editor.arena().get(editor.caret().node).unwrap().kind(),
                &NodeKind::Placeholder
            );
            editor
                .set_atom(editor.caret().node, NodeKind::Number("2".into()))
                .unwrap();
            assert!(editor.exit_group_right());
            assert_eq!(editor.caret().node, wrapper);
            assert!(editor.validate().is_ok());
            assert!(editor.undo());
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), atom("A"));
        }
    }

    #[test]
    fn an_empty_postfix_stays_in_its_operand_and_nested_power_scope_is_not_reassociated() {
        let mut editor = Editor::new();
        let original = editor.root();
        editor.insert_factorial();
        assert_eq!(editor.caret().node, original);
        editor
            .set_atom(original, NodeKind::Number("3".into()))
            .unwrap();
        assert_eq!(
            editor.to_portable().unwrap().children[0].kind,
            NodeKind::Number("3".into())
        );
        let power = PortableTree {
            kind: NodeKind::Power,
            children: vec![atom("A"), atom("B")],
        };
        let mut editor = Editor::from_portable(&power).unwrap();
        let exponent = editor.arena().get(editor.root()).unwrap().children()[1];
        editor.place_caret(exponent, Direction::Right).unwrap();
        editor.insert_transpose();
        let after = editor.to_portable().unwrap();
        assert_eq!(after.kind, NodeKind::Power);
        assert_eq!(after.children[0], atom("A"));
        assert_eq!(after.children[1].kind, NodeKind::Transpose);
        assert_eq!(after.children[1].children[0], atom("B"));
    }

    #[test]
    fn unary_operator_boundary_deletion_unwraps_without_losing_metadata_or_creating_alias_nodes() {
        let original = PortableTree {
            kind: NodeKind::NativeIdentifier {
                name: "I".into(),
                labels: "VARIABLE".into(),
                contextual: Some(false),
                subscript: Some("dc".into()),
            },
            children: vec![],
        };
        for operation in [
            Editor::insert_factorial,
            Editor::insert_transpose,
            Editor::insert_conjugate,
            Editor::insert_norm,
        ] {
            let mut editor = Editor::from_portable(&original).unwrap();
            let wrapper = operation(&mut editor);
            editor.place_caret(wrapper, Direction::Right).unwrap();
            if editor.arena().get(wrapper).unwrap().kind == NodeKind::Conjugate {
                // A full-container prefix's outer edge enters its operand;
                // deleting the selected operator center is the confirmed path.
                assert!(!editor.delete_structure(true));
                editor
                    .place_operator_caret(wrapper, Direction::Indeterminate)
                    .unwrap();
            }
            assert!(editor.delete_structure(true));
            assert_eq!(editor.to_portable().unwrap(), original);
            assert!(editor.validate().is_ok());
            assert!(editor.undo());
            assert_ne!(editor.to_portable().unwrap(), original);
        }
    }

    #[test]
    fn all_operator_commands_leave_matrix_cell_ranges_and_history_unchanged() {
        let tree = PortableTree {
            kind: NodeKind::Matrix {
                rows: 2,
                cols: 2,
                display: None,
            },
            children: vec![atom("A"); 4],
        };
        let mut editor = Editor::from_portable(&tree).unwrap();
        let matrix = editor.root();
        editor.select_matrix_cells(matrix, 0, 3);
        let selection = editor.selection();
        for operation in [
            Editor::insert_factorial,
            Editor::insert_transpose,
            Editor::insert_conjugate,
            Editor::insert_determinant,
            Editor::insert_norm,
            Editor::insert_row_operator,
            Editor::insert_column_operator,
        ] {
            assert_eq!(operation(&mut editor), matrix);
            assert_eq!(editor.selection(), selection);
            assert_eq!(editor.to_portable().unwrap(), tree);
            assert!(!editor.undo());
        }
    }

    #[test]
    fn overlapping_row_column_and_transpose_scripts_insert_the_original_required_grouping() {
        for base in [NodeKind::Row, NodeKind::Column, NodeKind::Transpose] {
            let children = if base == NodeKind::Transpose {
                vec![atom("A")]
            } else {
                vec![atom("A"), atom("i")]
            };
            let source = PortableTree {
                kind: base.clone(),
                children,
            };
            for (operation, kind) in [
                (
                    Editor::insert_row_operator as fn(&mut Editor) -> NodeId,
                    NodeKind::Row,
                ),
                (Editor::insert_column_operator, NodeKind::Column),
                (Editor::insert_transpose, NodeKind::Transpose),
            ] {
                let mut editor = Editor::from_portable(&source).unwrap();
                let root = editor.root();
                editor.select_subtree(root).unwrap();
                operation(&mut editor);
                let after = editor.to_portable().unwrap();
                assert_eq!(after.kind, kind);
                if base == NodeKind::Transpose && kind == NodeKind::Transpose {
                    assert_eq!(after.children[0], source);
                } else {
                    assert_eq!(after.children[0].kind, NodeKind::Group);
                    assert_eq!(after.children[0].children[0], source);
                }
                assert!(editor.validate().is_ok());
                assert!(editor.undo());
                assert_eq!(editor.to_portable().unwrap(), source);
                assert!(!editor.undo());
            }
        }
    }
}
