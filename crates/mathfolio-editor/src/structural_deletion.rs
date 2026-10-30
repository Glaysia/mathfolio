//! Structural deletion follows native caret/analysis stages rather than an
//! arbitrary list of wrappers whose first child may survive. See 0122.
use super::*;

impl Editor {
    /// Place the normal operator cursor without selecting/replacing a subtree.
    /// Fences use NodeSide; IF and TRY retain their separate navigation APIs.
    pub fn place_operator_caret(
        &mut self,
        node: NodeId,
        node_side: Direction,
    ) -> Result<(), EditError> {
        let item = self.arena.get(node).ok_or(EditError::MissingNode)?;
        if !matches!(
            node_side,
            Direction::Left | Direction::Right | Direction::Indeterminate
        ) || !matches!(
            item.kind,
            NodeKind::Binary(_)
                | NodeKind::Group
                | NodeKind::Fraction
                | NodeKind::Power
                | NodeKind::UnaryNeg
                | NodeKind::LogicalNot
                | NodeKind::Conjugate
                | NodeKind::Vectorize
                | NodeKind::Determinant
                | NodeKind::Factorial
                | NodeKind::Percent
                | NodeKind::Degree
                | NodeKind::Transpose
        ) {
            return Err(EditError::InvalidOperatorCaret);
        }
        self.typeover = None;
        self.caret = Caret::at(node, Direction::Indeterminate);
        self.caret.node_side = node_side;
        self.selection = Selection::None;
        self.scope = None;
        Ok(())
    }
}

impl Editor {
    pub(super) fn delete_ordinary_structure(&mut self, backwards: bool) -> bool {
        if self.selection != Selection::None {
            return false;
        }
        if self.caret.insertion != InsertionMode::Normal
            && self.arena.get(self.caret.node).is_none_or(|node| {
                self.deletion_formatted_boundary(&node.kind, backwards) != Some(true)
            })
        {
            return false;
        }
        let Some((operator, side)) = self.deletion_operator_target(backwards) else {
            return false;
        };
        let item = self.arena.get(operator).unwrap();
        let preferred = if matches!(item.kind, NodeKind::Group | NodeKind::Determinant) {
            let child = item.children[0];
            if side == Direction::Left {
                self.program_operand_left_entry(child)
            } else {
                self.deletion_right_entry(child)
            }
        } else if matches!(
            item.kind,
            NodeKind::Factorial | NodeKind::Percent | NodeKind::Degree | NodeKind::Transpose
        ) {
            self.deletion_right_entry(item.children[0])
        } else if deletion_prefix(&item.kind) {
            self.program_operand_left_entry(item.children[0])
        } else {
            None
        };
        let mut candidate = self.clone_without_history();
        let result = if matches!(item.kind, NodeKind::Conjugate | NodeKind::Vectorize) {
            candidate.delete_preserved_container_unary(operator)
        } else {
            candidate.reparse_deleted_operator(operator, preferred, backwards)
        };
        let Some(caret) = result else {
            return false;
        };
        candidate.caret = caret;
        candidate.selection = Selection::None;
        candidate.scope = None;
        if candidate.to_portable().is_err() {
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

    // Only represented one-step native states are used. Working navigation is
    // never committed on a failed rewrite (bh05c8/rollback05c9).
    fn deletion_operator_target(&self, backwards: bool) -> Option<(NodeId, Direction)> {
        let current = self.caret.node;
        let item = self.arena.get(current)?;
        if item.kind == NodeKind::Group
            || (item.kind == NodeKind::Determinant
                && item.parent.is_none()
                && self.arena.get(item.children[0])?.kind.precedence() > 0)
        {
            let side = if self.caret.caret_side == Direction::Indeterminate {
                self.caret.node_side
            } else {
                self.caret.caret_side
            };
            return matches!(side, Direction::Left | Direction::Right).then_some((current, side));
        }
        if (deletion_binary(&item.kind)
            || deletion_prefix(&item.kind)
            || matches!(
                item.kind,
                NodeKind::Factorial | NodeKind::Percent | NodeKind::Degree | NodeKind::Transpose
            ))
            && self.caret.caret_side == Direction::Indeterminate
        {
            return Some((current, Direction::Indeterminate));
        }
        if matches!(
            item.kind,
            NodeKind::Factorial | NodeKind::Percent | NodeKind::Degree | NodeKind::Transpose
        ) && backwards
            && self.caret.caret_side == Direction::Right
        {
            return Some((current, Direction::Indeterminate));
        }
        let boundary = if item.kind == NodeKind::Placeholder {
            true
        } else if matches!(
            item.kind,
            NodeKind::NativeIdentifier {
                subscript: Some(_),
                ..
            } | NodeKind::Imaginary(_)
                | NodeKind::ImaginarySymbol { .. }
        ) || (matches!(item.kind, NodeKind::NativeIdentifier { .. })
            && self.caret.insertion == InsertionMode::Subscript)
        {
            self.deletion_formatted_boundary(&item.kind, backwards)?
        } else if let Some(text) = item.kind.text() {
            let length = text.encode_utf16().count();
            if let Some(range) = self.caret.value_range {
                range.anchor == range.focus && range.focus == if backwards { 0 } else { length }
            } else {
                self.caret.caret_side
                    == if backwards {
                        Direction::Left
                    } else {
                        Direction::Right
                    }
            }
        } else {
            false
        };
        if !boundary {
            return None;
        }
        let mut child = current;
        loop {
            let parent = self.arena.get(child)?.parent?;
            let owner = self.arena.get(parent)?;
            let slot = owner
                .children
                .iter()
                .position(|candidate| *candidate == child)?;
            if owner.kind == NodeKind::Group {
                return Some((
                    parent,
                    if backwards {
                        Direction::Left
                    } else {
                        Direction::Right
                    },
                ));
            }
            if deletion_binary(&owner.kind)
                && ((backwards && slot == 1) || (!backwards && slot == 0))
            {
                return Some((parent, Direction::Indeterminate));
            }
            if deletion_prefix(&owner.kind) && backwards && slot == 0 {
                return Some((parent, Direction::Indeterminate));
            }
            if matches!(
                owner.kind,
                NodeKind::Factorial | NodeKind::Percent | NodeKind::Degree | NodeKind::Transpose
            ) && !backwards
                && slot == 0
            {
                return Some((parent, Direction::Indeterminate));
            }
            // 060d/060e: ordinary infix/prefix tails have no own edge state.
            // Container slots and postfix own states interpose and stop this
            // bounded one-navigation-step ascent.
            if (matches!(
                owner.kind,
                NodeKind::Binary(
                    BinaryOp::Add | BinaryOp::Subtract | BinaryOp::Multiply | BinaryOp::Scale
                )
            ) && ((backwards && slot == 0) || (!backwards && slot == 1)))
                || (!backwards
                    && matches!(owner.kind, NodeKind::UnaryNeg | NodeKind::LogicalNot)
                    && slot == 0)
            {
                child = parent;
            } else {
                return None;
            }
        }
    }

    // 043e replaces a full-container unary by its complete operand, then055e
    // repairs the parent's binding before reparse. This is not045b's operator
    // handle removal. Only represented parent/container repair is admitted.
    fn delete_preserved_container_unary(&mut self, operator: NodeId) -> Option<Caret> {
        let item = self.arena.get(operator)?.clone();
        let child = item.children[0];
        let parent = item.parent;
        let caret = self.program_operand_left_entry(child)?;
        let mut survivor = child;
        if let Some(parent) = parent {
            let owner = self.arena.get(parent)?;
            let slot = owner.children.iter().position(|node| *node == operator)?;
            if !self.grouping_container_slot(parent, slot) {
                if !matches!(owner.kind, NodeKind::Binary(_)) {
                    return None;
                }
                let child_prec = self.arena.get(child)?.kind.precedence();
                let parent_prec = owner.kind.precedence();
                if child_prec == parent_prec {
                    return None;
                }
                if child_prec < parent_prec {
                    survivor = self.arena.allocate(NodeKind::Group, vec![child]);
                }
            }
        }
        self.replace_child(parent, operator, survivor);
        self.arena.node_mut(survivor).parent = parent;
        self.retire(operator, Some(child));
        Some(caret)
    }

    pub(super) fn deletion_right_entry(&self, node: NodeId) -> Option<Caret> {
        let last = self.typed_last_navigation_node(node)?;
        let item = self.arena.get(last)?;
        if item.kind == NodeKind::Placeholder {
            return Some(Caret::at(last, Direction::Indeterminate));
        }
        if let Some(text) = item.kind.text() {
            if matches!(
                item.kind,
                NodeKind::StringLiteral(_)
                    | NodeKind::NativeIdentifier {
                        subscript: Some(_),
                        ..
                    }
                    | NodeKind::Imaginary(_)
                    | NodeKind::ImaginarySymbol { .. }
            ) {
                return Some(Caret::at(last, Direction::Right));
            }
            let length = text.encode_utf16().count();
            let mut caret = Caret::at(last, Direction::Indeterminate);
            caret.value_range = Some(ValueRange {
                anchor: length,
                focus: length,
            });
            return Some(caret);
        }
        Some(Caret::at(last, Direction::Right))
    }
}

fn deletion_binary(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Binary(BinaryOp::Add | BinaryOp::Subtract | BinaryOp::Multiply)
            | NodeKind::Fraction
            | NodeKind::Power
    )
}

fn deletion_prefix(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::UnaryNeg | NodeKind::LogicalNot | NodeKind::Conjugate | NodeKind::Vectorize
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn n(s: &str) -> PortableTree {
        PortableTree {
            kind: NodeKind::Number(s.into()),
            children: vec![],
        }
    }
    fn ph() -> PortableTree {
        PortableTree {
            kind: NodeKind::Placeholder,
            children: vec![],
        }
    }
    fn tree(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
        PortableTree { kind, children }
    }
    fn assert_join(
        mut e: Editor,
        backwards: bool,
        before: &PortableTree,
        expected: &str,
        offset: usize,
    ) {
        let oldcaret = e.caret;
        assert!(e.delete_structure(backwards));
        assert_eq!(e.to_portable().unwrap(), n(expected));
        assert_eq!(
            e.caret.value_range,
            Some(ValueRange {
                anchor: offset,
                focus: offset
            })
        );
        assert_eq!(e.selection, Selection::None);
        assert!(e.scope.is_none());
        let result = e.caret;
        assert!(e.undo());
        assert_eq!(e.to_portable().unwrap(), *before);
        assert_eq!(e.caret, oldcaret);
        assert!(e.redo());
        assert_eq!(e.caret, result);
        assert_eq!(e.to_portable().unwrap(), n(expected));
        assert!(e.validate().is_ok());
    }
    #[test]
    fn integer_operators_join_at_center_or_adjacent_text_edge_in_one_command() {
        for (kind, left, right, expected) in [
            (NodeKind::Binary(BinaryOp::Add), "12", "34", "1234"),
            (NodeKind::Fraction, "12", "34", "1234"),
            (NodeKind::Power, "5", "2", "52"),
        ] {
            let source = tree(kind, vec![n(left), n(right)]);
            for backwards in [false, true] {
                let mut e = Editor::from_portable(&source).unwrap();
                e.place_operator_caret(e.root(), Direction::Indeterminate)
                    .unwrap();
                assert_join(e, backwards, &source, expected, left.len());
                let mut e = Editor::from_portable(&source).unwrap();
                let child = e.arena.get(e.root).unwrap().children[usize::from(backwards)];
                e.place_text_caret(child, if backwards { 0 } else { left.len() })
                    .unwrap();
                assert_join(e, backwards, &source, expected, left.len());
            }
        }
    }
    #[test]
    fn grouping_fences_delete_as_pair_in_both_directions_with_fence_cursor() {
        for side in [Direction::Left, Direction::Right] {
            for backwards in [false, true] {
                let source = tree(NodeKind::Group, vec![n("12")]);
                let mut e = Editor::from_portable(&source).unwrap();
                let id = e.arena.get(e.root).unwrap().children[0];
                e.place_operator_caret(e.root, side).unwrap();
                assert!(e.delete_structure(backwards));
                assert_eq!(e.root, id);
                assert_eq!(e.to_portable().unwrap(), n("12"));
                let position = if side == Direction::Left { 0 } else { 2 };
                assert_eq!(
                    e.caret.value_range,
                    Some(ValueRange {
                        anchor: position,
                        focus: position
                    })
                );
                assert!(e.undo());
                assert_eq!(e.to_portable().unwrap(), source);
                assert!(e.redo());
                assert_eq!(e.root, id);
            }
        }
    }
    #[test]
    fn fraction_and_power_placeholder_deletion_is_directional() {
        for (kind, children, slot, backwards, expected, offset) in [
            (NodeKind::Fraction, vec![ph(), n("34")], 0, false, "34", 0),
            (NodeKind::Fraction, vec![n("12"), ph()], 1, true, "12", 2),
            (NodeKind::Power, vec![n("12"), ph()], 1, true, "12", 2),
        ] {
            let source = tree(kind, children);
            let mut e = Editor::from_portable(&source).unwrap();
            let placeholder = e.arena.get(e.root).unwrap().children[slot];
            e.place_caret(placeholder, Direction::Indeterminate)
                .unwrap();
            let state = e.clone_without_history();
            assert!(!e.delete_structure(!backwards));
            assert_eq!(e.to_portable().unwrap(), source);
            assert_eq!(e.caret, state.caret);
            assert_join(e, backwards, &source, expected, offset);
        }
    }
    #[test]
    fn ordinary_deletion_never_discards_filled_operands_at_outer_boundaries() {
        for kind in [
            NodeKind::Power,
            NodeKind::Fraction,
            NodeKind::Binary(BinaryOp::Add),
        ] {
            let source = tree(kind, vec![n("12"), n("34")]);
            let mut e = Editor::from_portable(&source).unwrap();
            e.set_atom(
                e.arena.get(e.root).unwrap().children[1],
                NodeKind::Number("35".into()),
            )
            .unwrap();
            assert!(e.undo());
            e.place_caret(e.root, Direction::Right).unwrap();
            let caret = e.caret;
            assert!(!e.delete_structure(true));
            assert_eq!(e.to_portable().unwrap(), source);
            assert_eq!(e.caret, caret);
            assert!(e.redo());
        }
    }
    #[test]
    fn operator_navigation_preserves_redo_and_rejects_wrong_target_atomically() {
        let source = tree(NodeKind::Binary(BinaryOp::Add), vec![n("12"), n("34")]);
        let mut e = Editor::from_portable(&source).unwrap();
        let child = e.arena.get(e.root).unwrap().children[0];
        e.set_atom(child, NodeKind::Number("13".into())).unwrap();
        assert!(e.undo());
        e.place_operator_caret(e.root, Direction::Indeterminate)
            .unwrap();
        let caret = e.caret;
        assert_eq!(
            e.place_operator_caret(child, Direction::Left),
            Err(EditError::InvalidOperatorCaret)
        );
        assert_eq!(e.caret, caret);
        assert_eq!(e.to_portable().unwrap(), source);
        assert!(e.redo());
    }
    #[test]
    fn installed_lexical_fusion_is_not_the_help_integer_only_summary() {
        for (left, right, expected, offset) in [
            (n("1.2"), n("3"), n("1.23"), 3),
            (
                tree(NodeKind::Identifier("x".into()), vec![]),
                tree(NodeKind::Identifier("y".into()), vec![]),
                tree(NodeKind::Identifier("xy".into()), vec![]),
                1,
            ),
        ] {
            for backwards in [false, true] {
                let source = tree(
                    NodeKind::Binary(BinaryOp::Add),
                    vec![left.clone(), right.clone()],
                );
                let mut e = Editor::from_portable(&source).unwrap();
                e.place_operator_caret(e.root, Direction::Indeterminate)
                    .unwrap();
                assert!(e.delete_structure(backwards));
                assert_eq!(e.to_portable().unwrap(), expected);
                assert_eq!(
                    e.caret.value_range,
                    Some(ValueRange {
                        anchor: offset,
                        focus: offset
                    })
                );
                assert!(e.undo());
                assert_eq!(e.to_portable().unwrap(), source);
            }
        }
    }
    #[test]
    fn invalid_decimal_fusion_becomes_scale_and_enters_opposite_delete_side() {
        for backwards in [false, true] {
            let source = tree(NodeKind::Binary(BinaryOp::Add), vec![n("1.2"), n("3.4")]);
            let mut e = Editor::from_portable(&source).unwrap();
            let children = e.arena.get(e.root).unwrap().children.clone();
            e.place_operator_caret(e.root, Direction::Indeterminate)
                .unwrap();
            assert!(e.delete_structure(backwards));
            assert_eq!(
                e.to_portable().unwrap(),
                tree(NodeKind::Binary(BinaryOp::Scale), vec![n("1.2"), n("3.4")])
            );
            assert_eq!(e.arena.get(e.root).unwrap().children, children);
            let (node, pos) = if backwards {
                (children[1], 0)
            } else {
                (children[0], 3)
            };
            assert_eq!(e.caret.node, node);
            assert_eq!(
                e.caret.value_range,
                Some(ValueRange {
                    anchor: pos,
                    focus: pos
                })
            );
            assert!(e.undo());
            assert_eq!(e.to_portable().unwrap(), source);
            assert!(e.redo());
            assert_eq!(e.caret.node, node);
        }
    }
    #[test]
    fn group_removal_reparses_neighbors_without_discarding_or_grouping_child() {
        let source = tree(
            NodeKind::Binary(BinaryOp::Multiply),
            vec![
                tree(
                    NodeKind::Group,
                    vec![tree(NodeKind::Binary(BinaryOp::Add), vec![n("1"), n("2")])],
                ),
                n("3"),
            ],
        );
        let mut e = Editor::from_portable(&source).unwrap();
        let group = e.arena.get(e.root).unwrap().children[0];
        e.place_operator_caret(group, Direction::Left).unwrap();
        assert!(e.delete_structure(false));
        assert_eq!(
            e.to_portable().unwrap(),
            tree(
                NodeKind::Binary(BinaryOp::Add),
                vec![
                    n("1"),
                    tree(NodeKind::Binary(BinaryOp::Multiply), vec![n("2"), n("3")])
                ]
            )
        );
        assert_eq!(
            e.arena.get(e.caret.node).unwrap().kind,
            NodeKind::Number("1".into())
        );
        assert!(e.undo());
        assert_eq!(e.to_portable().unwrap(), source);
    }
    #[test]
    fn formatted_unary_operand_metadata_survives_center_deletion_and_history() {
        let atom = tree(
            NodeKind::NativeIdentifier {
                name: "I".into(),
                labels: "VARIABLE".into(),
                contextual: Some(false),
                subscript: Some("dc".into()),
            },
            vec![],
        );
        for kind in [
            NodeKind::Conjugate,
            NodeKind::Vectorize,
            NodeKind::UnaryNeg,
            NodeKind::LogicalNot,
        ] {
            let source = tree(kind, vec![atom.clone()]);
            let mut e = Editor::from_portable(&source).unwrap();
            let child = e.arena.get(e.root).unwrap().children[0];
            e.place_operator_caret(e.root, Direction::Indeterminate)
                .unwrap();
            let before = e.caret;
            assert!(e.delete_structure(true));
            assert_eq!(e.to_portable().unwrap(), atom);
            assert_eq!(e.root, child);
            assert!(e.undo());
            assert_eq!(e.to_portable().unwrap(), source);
            assert_eq!(e.caret, before);
            assert!(e.redo());
            assert_eq!(e.root, child);
        }
    }

    #[test]
    fn full_container_unary_preserves_binding_before_global_reparse() {
        for kind in [NodeKind::Conjugate, NodeKind::Vectorize] {
            let operand = tree(NodeKind::Binary(BinaryOp::Add), vec![n("1"), n("2")]);
            let source = tree(
                NodeKind::Binary(BinaryOp::Multiply),
                vec![tree(kind, vec![operand.clone()]), n("3")],
            );
            let mut e = Editor::from_portable(&source).unwrap();
            let wrapper = e.arena.get(e.root).unwrap().children[0];
            let child = e.arena.get(wrapper).unwrap().children[0];
            e.place_operator_caret(wrapper, Direction::Indeterminate)
                .unwrap();
            assert!(e.delete_structure(true));
            assert_eq!(
                e.to_portable().unwrap(),
                tree(
                    NodeKind::Binary(BinaryOp::Multiply),
                    vec![tree(NodeKind::Group, vec![operand]), n("3")]
                )
            );
            assert!(e.arena.get(child).is_some());
            assert!(e.undo());
            assert_eq!(e.to_portable().unwrap(), source);
        }
    }
    #[test]
    fn deletion_inside_program_and_matrix_containers_preserves_siblings_and_ids() {
        for kind in [
            NodeKind::Program { statement_count: 2 },
            NodeKind::Matrix {
                rows: 1,
                cols: 2,
                display: None,
            },
        ] {
            let source = tree(
                kind,
                vec![
                    tree(NodeKind::Binary(BinaryOp::Add), vec![n("12"), n("34")]),
                    n("99"),
                ],
            );
            let mut e = Editor::from_portable(&source).unwrap();
            let root = e.root;
            let children = e.arena.get(root).unwrap().children.clone();
            e.place_operator_caret(children[0], Direction::Indeterminate)
                .unwrap();
            assert!(e.delete_structure(true));
            assert_eq!(e.root, root);
            assert_eq!(e.arena.get(root).unwrap().children[1], children[1]);
            assert_eq!(e.to_portable().unwrap().children, vec![n("1234"), n("99")]);
            assert!(e.undo());
            assert_eq!(e.to_portable().unwrap(), source);
        }
    }
    #[test]
    fn unrepresented_formatted_fusion_preserves_source_cursor_revision_and_redo() {
        let atom = tree(
            NodeKind::NativeIdentifier {
                name: "x".into(),
                labels: "VARIABLE".into(),
                contextual: Some(false),
                subscript: Some("rms".into()),
            },
            vec![],
        );
        let source = tree(NodeKind::Binary(BinaryOp::Add), vec![atom, n("2")]);
        let mut e = Editor::from_portable(&source).unwrap();
        let child = e.arena.get(e.root).unwrap().children[1];
        e.set_atom(child, NodeKind::Number("3".into())).unwrap();
        assert!(e.undo());
        e.place_operator_caret(e.root, Direction::Indeterminate)
            .unwrap();
        let caret = e.caret;
        let revision = e.arena.revision();
        assert!(!e.delete_structure(true));
        assert_eq!(e.to_portable().unwrap(), source);
        assert_eq!(e.caret, caret);
        assert_eq!(e.arena.revision(), revision);
        assert!(e.redo());
    }
    #[test]
    fn depth_budget_after_reparse_is_atomic_and_preserves_redo() {
        fn balanced(count: usize) -> PortableTree {
            if count == 1 {
                n("1")
            } else {
                tree(
                    NodeKind::Binary(BinaryOp::Add),
                    vec![balanced(count / 2), balanced(count - count / 2)],
                )
            }
        }
        let source = tree(NodeKind::Group, vec![balanced(MAX_TREE_DEPTH + 2)]);
        let mut e = Editor::from_portable(&source).unwrap();
        let first = first_leaf(&e.arena, e.root);
        e.set_atom(first, NodeKind::Number("2".into())).unwrap();
        assert!(e.undo());
        e.place_operator_caret(e.root, Direction::Left).unwrap();
        let caret = e.caret;
        let revision = e.arena.revision();
        assert!(!e.delete_structure(false));
        assert_eq!(e.to_portable().unwrap(), source);
        assert_eq!(e.caret, caret);
        assert_eq!(e.arena.revision(), revision);
        assert!(e.redo());
    }
    #[test]
    fn postfix_center_or_operand_end_delete_retains_operand_right_entry() {
        for kind in [
            NodeKind::Factorial,
            NodeKind::Percent,
            NodeKind::Degree,
            NodeKind::Transpose,
        ] {
            let source = tree(kind, vec![n("12")]);
            for backwards in [false, true] {
                let mut e = Editor::from_portable(&source).unwrap();
                let child = e.arena.get(e.root).unwrap().children[0];
                e.place_operator_caret(e.root, Direction::Indeterminate)
                    .unwrap();
                assert!(e.delete_structure(backwards));
                assert_eq!(e.to_portable().unwrap(), n("12"));
                assert_eq!(e.caret.node, child);
                assert_eq!(
                    e.caret.value_range,
                    Some(ValueRange {
                        anchor: 2,
                        focus: 2
                    })
                );
                assert!(e.undo());
                assert_eq!(e.to_portable().unwrap(), source);
                e.place_text_caret(child, 2).unwrap();
                assert!(e.delete_structure(false));
                assert_eq!(e.to_portable().unwrap(), n("12"));
            }
        }
    }
}
