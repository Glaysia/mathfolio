//! Confirmed program-operator insertion modes. Native Scope preserves its
//! parsed expression in the primal BODY; normal whole-value replacement erases
//! an isolated atomic value. Partial text/implicit adjacency needs a parser and
//! is not silently treated as another whole-subtree wrapper.
//! See docs/analysis/0106-populated-program-insertion-contract.md.
//! Direct SCALE-parent keyword preservation is documented separately in
//! docs/analysis/0110-implicit-adjacency-contract.md. Imported MULTIPLICATION
//! does not supply SCALE's IsImplicit provenance.
use super::*;

impl Editor {
    // E a8.bq 0x0600059e: only an implicit parent, keyword at child zero,
    // and a Prefix/primal operator preserve the parent's second operand.
    // SCALE has IsImplicit=true. MULTIPLICATION does not establish that fact.
    pub(super) fn convert_scaled_program_keyword(&mut self, kind: NodeKind) -> Option<bool> {
        let primal = match kind {
            NodeKind::While | NodeKind::If { .. } => 1,
            NodeKind::For => 2,
            NodeKind::Try | NodeKind::Return => 0,
            _ => return None,
        };
        let keyword = self.caret.node;
        let scale = self.arena.get(keyword)?.parent?;
        let source = self.arena.get(scale)?;
        if source.kind != NodeKind::Binary(BinaryOp::Scale) || source.children[0] != keyword {
            return None;
        }
        let operand = source.children[1];
        let parent = source.parent;
        let mut candidate = self.clone_without_history();
        let wrapper = import_node(
            &mut candidate.arena,
            &program_control::control_template(kind.clone()),
        );
        let slot = candidate.arena.get(wrapper).unwrap().children[primal];
        if kind == NodeKind::Return
            || matches!(
                candidate.arena.get(operand).unwrap().kind,
                NodeKind::Program { .. }
            )
        {
            candidate.arena.node_mut(wrapper).children[primal] = operand;
            candidate.arena.node_mut(operand).parent = Some(wrapper);
            candidate.retire(slot, None);
        } else {
            let placeholder = candidate.arena.get(slot).unwrap().children[0];
            candidate.arena.node_mut(slot).children[0] = operand;
            candidate.arena.node_mut(operand).parent = Some(slot);
            candidate.retire(placeholder, None);
        }
        candidate.replace_child(parent, scale, wrapper);
        candidate.arena.node_mut(wrapper).parent = parent;
        candidate.retire(scale, Some(operand));
        candidate.caret = match kind {
            NodeKind::Return => match candidate.program_operand_left_entry(operand) {
                Some(caret) => caret,
                None => return Some(false),
            },
            // The initial attempt PH was removed by primal replacement. The
            // original converter's fallback enters the repaired BODY itself.
            NodeKind::Try => Caret::at(
                candidate.arena.get(wrapper).unwrap().children[0],
                Direction::Left,
            ),
            _ => Caret::at(
                candidate.arena.get(wrapper).unwrap().children[0],
                Direction::Indeterminate,
            ),
        };
        candidate.selection = Selection::None;
        candidate.scope = None;
        if candidate.to_portable().is_err() {
            return Some(false);
        }
        self.checkpoint();
        self.arena = candidate.arena;
        self.root = candidate.root;
        self.caret = candidate.caret;
        self.selection = Selection::None;
        self.scope = None;
        Some(true)
    }

    pub(super) fn insert_program_control(&mut self, kind: NodeKind) -> Option<NodeId> {
        if self.matrix_selection().is_some() {
            return None;
        }
        let target = self.target();
        match self.selection {
            Selection::None => {
                let node = self.arena.get(target).unwrap();
                let blank = node.kind == NodeKind::Placeholder;
                let isolated = node.parent.is_none_or(|parent| {
                    let parent = self.arena.get(parent).unwrap();
                    matches!(parent.kind, NodeKind::Program { .. })
                        || (parent.kind == NodeKind::Binary(BinaryOp::Evaluate)
                            && parent.children.first() == Some(&target))
                });
                let whole_value = node.kind.text().is_some_and(|text| {
                    isolated
                        && self.caret.caret_side == Direction::Indeterminate
                        && self.caret.insertion == InsertionMode::Normal
                        && self.caret.value_range.is_some_and(|range| {
                            !matches!(
                                node.kind,
                                NodeKind::NativeIdentifier {
                                    subscript: Some(_),
                                    ..
                                }
                            ) && range.anchor.min(range.focus) == 0
                                && range.anchor.max(range.focus) == text.encode_utf16().count()
                        })
                });
                if !blank && !whole_value {
                    return None;
                }
                let is_return = kind == NodeKind::Return;
                if !self.replace_atomic_control(target, program_control::control_template(kind)) {
                    return None;
                }
                if is_return {
                    self.caret = Caret::at(self.caret.node, Direction::Left);
                }
                Some(target)
            }
            Selection::Subtree(selected) => self.insert_scoped_program_control(selected, kind),
            Selection::MatrixCells(_) => None,
        }
    }

    fn insert_scoped_program_control(&mut self, target: NodeId, kind: NodeKind) -> Option<NodeId> {
        let side = self.caret.caret_side;
        if !matches!(side, Direction::Left | Direction::Right)
            || self.arena.get(target).unwrap().kind == NodeKind::Placeholder
        {
            return None;
        }
        let primal = match kind {
            NodeKind::While | NodeKind::If { .. } => 1,
            NodeKind::For => 2,
            NodeKind::Try => 0,
            NodeKind::Return if side == Direction::Left => 0,
            _ => return None,
        };
        let return_entry = if kind == NodeKind::Return {
            Some(self.program_operand_left_entry(target)?)
        } else {
            None
        };
        let parent = self.arena.get(target).unwrap().parent;
        let mut candidate = self.clone_without_history();
        let template = program_control::control_template(kind.clone());
        let wrapper = import_node(&mut candidate.arena, &template);
        let placeholder_slot = candidate.arena.get(wrapper).unwrap().children[primal];
        if kind == NodeKind::Return {
            candidate.arena.node_mut(wrapper).children[primal] = target;
            candidate.arena.node_mut(target).parent = Some(wrapper);
            candidate.retire(placeholder_slot, None);
        } else if matches!(
            candidate.arena.get(target).unwrap().kind,
            NodeKind::Program { .. }
        ) {
            // Original PROGRAM -> BODY repair changes the descriptor without
            // nesting another program or replacing the selected node's ID.
            candidate.arena.node_mut(wrapper).children[primal] = target;
            candidate.arena.node_mut(target).parent = Some(wrapper);
            candidate.retire(placeholder_slot, None);
        } else {
            let old = candidate.arena.get(placeholder_slot).unwrap().children[0];
            candidate.arena.node_mut(placeholder_slot).children[0] = target;
            candidate.arena.node_mut(target).parent = Some(placeholder_slot);
            candidate.retire(old, None);
        }
        candidate.replace_child(parent, target, wrapper);
        candidate.arena.node_mut(wrapper).parent = parent;
        candidate.caret = if let Some(entry) = return_entry {
            entry
        } else {
            let mut stack = vec![wrapper];
            let mut placeholders = vec![];
            while let Some(node) = stack.pop() {
                let entry = candidate.arena.get(node).unwrap();
                if entry.kind == NodeKind::Placeholder {
                    placeholders.push(node);
                }
                stack.extend(entry.children.iter().rev().copied());
            }
            let reverse = side == Direction::Right && primal + 1 != template.children.len();
            let placeholder = if reverse {
                placeholders.last()
            } else {
                placeholders.first()
            }?;
            Caret::at(*placeholder, Direction::Indeterminate)
        };
        candidate.selection = Selection::None;
        candidate.scope = None;
        if candidate.to_portable().is_err() {
            return None;
        }
        self.checkpoint();
        self.arena = candidate.arena;
        self.root = candidate.root;
        self.caret = candidate.caret;
        self.selection = Selection::None;
        self.scope = None;
        Some(wrapper)
    }

    // The confirmed Return Scope-left entry subset of native bq's first
    // navigable state. Do not equate every container's entry with first_leaf.
    pub(super) fn program_operand_left_entry(&self, target: NodeId) -> Option<Caret> {
        let node = self.arena.get(target)?;
        if matches!(node.kind, NodeKind::StringLiteral(_)) {
            return Some(Caret::at(target, Direction::Left));
        }
        if node.kind.text().is_some() {
            let mut caret = Caret::at(target, Direction::Indeterminate);
            caret.value_range = Some(ValueRange {
                anchor: 0,
                focus: 0,
            });
            return Some(caret);
        }
        if node.kind == NodeKind::Placeholder {
            return Some(Caret::at(target, Direction::Indeterminate));
        }
        if matches!(
            node.kind,
            NodeKind::Group
                | NodeKind::Absolute
                | NodeKind::Determinant
                | NodeKind::Matrix { .. }
                | NodeKind::Fraction
                | NodeKind::Root
                | NodeKind::UnaryNeg
                | NodeKind::LogicalNot
                | NodeKind::Conjugate
                | NodeKind::Vectorize
                | NodeKind::Integral
                | NodeKind::Derivative { .. }
                | NodeKind::Summation
                | NodeKind::Product
                | NodeKind::Program { .. }
                | NodeKind::If { .. }
                | NodeKind::For
                | NodeKind::While
                | NodeKind::Try
                | NodeKind::Return
                | NodeKind::Break
                | NodeKind::Continue
        ) {
            return Some(Caret::at(target, Direction::Left));
        }
        // The supported infix/postfix descriptors below have neither a left
        // edge nor a container at slot zero. Stepped range is Special, but its
        // first part is at slot one, so it also starts in child zero. Group,
        // matrix and fences above have PreFenceNavigatable=true by default.
        if matches!(
            node.kind,
            NodeKind::Binary(_)
                | NodeKind::Power
                | NodeKind::Index { .. }
                | NodeKind::Call { .. }
                | NodeKind::Function { .. }
                | NodeKind::Range { .. }
                | NodeKind::LocalDefine
                | NodeKind::Row
                | NodeKind::Column
                | NodeKind::Transpose
                | NodeKind::Factorial
                | NodeKind::Percent
                | NodeKind::Degree
        ) {
            return self.program_operand_left_entry(node.children[0]);
        }
        // Symbolic descriptor navigation has a separate, unrepresented stack.
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
        PortableTree { kind, children }
    }
    fn atom(kind: NodeKind) -> PortableTree {
        tree(kind, vec![])
    }
    fn identifier() -> PortableTree {
        atom(NodeKind::NativeIdentifier {
            name: "v😀".into(),
            labels: "VARIABLE".into(),
            contextual: None,
            subscript: Some("literal".into()),
        })
    }
    fn kinds() -> [(NodeKind, usize); 4] {
        [
            (NodeKind::While, 1),
            (NodeKind::For, 2),
            (
                NodeKind::If {
                    branch_count: 1,
                    has_else: false,
                    alternate: false,
                },
                1,
            ),
            (NodeKind::Try, 0),
        ]
    }
    fn insert(editor: &mut Editor, kind: &NodeKind) {
        match kind {
            NodeKind::While => {
                editor.insert_while_loop();
            }
            NodeKind::For => {
                editor.insert_for_loop();
            }
            NodeKind::If { .. } => {
                editor.insert_if();
            }
            NodeKind::Try => {
                editor.insert_try();
            }
            NodeKind::Return => {
                editor.insert_return();
            }
            NodeKind::Break => {
                editor.insert_break();
            }
            NodeKind::Continue => {
                editor.insert_continue();
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn scoped_program_operators_preserve_the_selected_expression_in_the_original_primal_body() {
        for (kind, primal) in kinds() {
            for side in [Direction::Left, Direction::Right] {
                let source = tree(
                    NodeKind::Binary(BinaryOp::Evaluate),
                    vec![identifier(), atom(NodeKind::Placeholder)],
                );
                let mut editor = Editor::from_portable(&source).unwrap();
                let root = editor.root();
                let target = editor.arena().get(root).unwrap().children()[0];
                let unit = editor.arena().get(root).unwrap().children()[1];
                editor.select_subtree_from(target, side).unwrap();
                let caret = editor.caret();
                insert(&mut editor, &kind);
                let wrapper = editor.arena().get(root).unwrap().children()[0];
                assert_eq!(editor.arena().get(wrapper).unwrap().kind(), &kind);
                let body = editor.arena().get(wrapper).unwrap().children()[primal];
                assert_eq!(
                    editor.arena().get(body).unwrap().kind(),
                    &NodeKind::Program { statement_count: 1 }
                );
                assert_eq!(editor.arena().get(body).unwrap().children(), &[target]);
                assert_eq!(editor.export_subtree(target).unwrap(), source.children[0]);
                assert_eq!(editor.arena().get(root).unwrap().children()[1], unit);
                assert_eq!(editor.selection(), Selection::None);
                let result = editor.to_portable().unwrap();
                assert!(editor.undo());
                assert_eq!(editor.to_portable().unwrap(), source);
                assert_eq!(editor.selection(), Selection::Subtree(target));
                assert_eq!(editor.caret(), caret);
                assert!(!editor.undo());
                assert!(editor.redo());
                assert_eq!(editor.to_portable().unwrap(), result);
            }
        }
    }

    #[test]
    fn selected_program_reuses_its_node_and_statements_without_double_body_nesting() {
        let source = tree(
            NodeKind::Program { statement_count: 2 },
            vec![identifier(), atom(NodeKind::Number("3".into()))],
        );
        for (kind, primal) in kinds() {
            let mut editor = Editor::from_portable(&source).unwrap();
            let target = editor.root();
            let statements = editor.arena().get(target).unwrap().children().to_vec();
            editor
                .select_subtree_from(target, Direction::Right)
                .unwrap();
            insert(&mut editor, &kind);
            assert_eq!(
                editor.arena().get(editor.root()).unwrap().children()[primal],
                target
            );
            assert_eq!(editor.arena().get(target).unwrap().children(), statements);
            assert_eq!(editor.export_subtree(target).unwrap(), source);
            assert!(editor.undo());
            assert_eq!(editor.root(), target);
            assert_eq!(editor.to_portable().unwrap(), source);
        }
    }

    #[test]
    fn space_scope_is_consumed_and_undo_restores_its_contractible_origin_history() {
        for side in [Direction::Left, Direction::Right] {
            for (kind, _) in kinds() {
                let source = identifier();
                let mut editor = Editor::from_portable(&source).unwrap();
                let target = editor.root();
                editor
                    .place_text_caret(target, if side == Direction::Left { 0 } else { 3 })
                    .unwrap();
                let origin = editor.caret();
                assert!(editor.expand_scope());
                assert_eq!(editor.caret().caret_side, side);
                assert!(editor.scope.is_some());
                insert(&mut editor, &kind);
                assert_eq!(editor.selection(), Selection::None);
                assert!(editor.scope.is_none());
                assert!(editor.undo());
                assert_eq!(editor.selection(), Selection::Subtree(target));
                assert!(editor.scope.is_some());
                assert!(editor.contract_scope());
                assert_eq!(editor.caret(), origin);
                assert_eq!(editor.selection(), Selection::None);
                assert!(
                    editor.redo(),
                    "contracting restored scope is navigation, not a new edit"
                );
            }
        }
    }

    #[test]
    fn try_scoped_direction_selects_first_or_reversed_last_placeholder() {
        let source = tree(
            NodeKind::Binary(BinaryOp::Add),
            vec![identifier(), atom(NodeKind::Placeholder)],
        );
        for side in [Direction::Left, Direction::Right] {
            let mut editor = Editor::from_portable(&source).unwrap();
            let selected = editor.root();
            let existing = editor.arena().get(selected).unwrap().children()[1];
            editor.select_subtree_from(selected, side).unwrap();
            assert!(editor.insert_try());
            let handler = editor.arena().get(editor.root()).unwrap().children()[1];
            let new = editor.arena().get(handler).unwrap().children()[0];
            assert_eq!(
                editor.caret().node,
                if side == Direction::Left {
                    existing
                } else {
                    new
                }
            );
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), source);
        }
    }

    #[test]
    fn return_scope_left_enters_the_first_native_state_and_right_is_a_redo_preserving_noop() {
        let values = [
            identifier(),
            tree(
                NodeKind::Binary(BinaryOp::Multiply),
                vec![
                    atom(NodeKind::Number("73".into())),
                    atom(NodeKind::AnnotatedIdentifier {
                        name: "V".into(),
                        labels: "UNIT".into(),
                    }),
                ],
            ),
            tree(NodeKind::Group, vec![identifier()]),
            tree(NodeKind::Program { statement_count: 1 }, vec![identifier()]),
            tree(
                NodeKind::Matrix {
                    rows: 1,
                    cols: 1,
                    display: None,
                },
                vec![identifier()],
            ),
        ];
        for source in values {
            let mut editor = Editor::from_portable(&source).unwrap();
            let selected = editor.root();
            editor
                .select_subtree_from(selected, Direction::Left)
                .unwrap();
            let wrapper = editor.insert_return();
            assert_eq!(editor.root(), wrapper);
            assert_eq!(
                editor.arena().get(wrapper).unwrap().kind(),
                &NodeKind::Return
            );
            assert_eq!(editor.arena().get(wrapper).unwrap().children(), &[selected]);
            if matches!(
                source.kind,
                NodeKind::Identifier(_) | NodeKind::NativeIdentifier { .. } | NodeKind::Binary(_)
            ) {
                let expected = if matches!(source.kind, NodeKind::Binary(_)) {
                    editor.arena().get(selected).unwrap().children()[0]
                } else {
                    selected
                };
                assert_eq!(editor.caret().node, expected);
                assert_eq!(
                    editor.caret().value_range,
                    Some(ValueRange {
                        anchor: 0,
                        focus: 0
                    })
                );
            } else {
                assert_eq!(editor.caret(), Caret::at(selected, Direction::Left));
            }
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), source);
            editor
                .select_subtree_from(selected, Direction::Right)
                .unwrap();
            let revision = editor.arena().revision();
            assert_eq!(editor.insert_return(), selected);
            assert_eq!(editor.arena().revision(), revision);
            assert_eq!(editor.to_portable().unwrap(), source);
            assert!(editor.redo());
        }
    }

    #[test]
    fn normal_full_atomic_value_replaces_instead_of_wrapping_and_undo_restores_its_metadata() {
        let source = atom(NodeKind::NativeIdentifier {
            name: "v😀".into(),
            labels: "UNIT".into(),
            contextual: Some(false),
            subscript: None,
        });
        let kinds = [
            NodeKind::While,
            NodeKind::For,
            NodeKind::If {
                branch_count: 1,
                has_else: false,
                alternate: false,
            },
            NodeKind::Try,
            NodeKind::Return,
            NodeKind::Break,
            NodeKind::Continue,
        ];
        for kind in kinds {
            for (anchor, focus) in [(0, 3), (3, 0)] {
                let mut editor = Editor::from_portable(&source).unwrap();
                let id = editor.root();
                editor.place_text_selection(id, anchor, focus).unwrap();
                let caret = editor.caret();
                insert(&mut editor, &kind);
                assert_eq!(editor.root(), id);
                assert_eq!(
                    editor.to_portable().unwrap(),
                    program_control::control_template(kind.clone())
                );
                if kind == NodeKind::Return {
                    assert_eq!(editor.caret().caret_side, Direction::Left);
                }
                if matches!(kind, NodeKind::Break | NodeKind::Continue) {
                    assert_eq!(editor.caret(), Caret::at(id, Direction::Right));
                }
                assert!(editor.undo());
                assert_eq!(editor.to_portable().unwrap(), source);
                assert_eq!(editor.caret(), caret);
                assert!(!editor.undo());
            }
        }
    }

    #[test]
    fn partial_text_naked_center_suffix_field_and_adjacent_atoms_do_not_use_whole_replacement() {
        for source in [
            atom(NodeKind::Identifier("atom".into())),
            identifier(),
            tree(
                NodeKind::Binary(BinaryOp::Add),
                vec![
                    atom(NodeKind::Identifier("atom".into())),
                    atom(NodeKind::Number("1".into())),
                ],
            ),
        ] {
            for (kind, _) in kinds() {
                let mut editor = Editor::from_portable(&source).unwrap();
                let target = editor.caret().node;
                for range in [None, Some((0, 0)), Some((1, 3))] {
                    editor
                        .place_caret(target, Direction::Indeterminate)
                        .unwrap();
                    if let Some((start, end)) = range {
                        editor.place_text_selection(target, start, end).unwrap();
                    }
                    let caret = editor.caret();
                    let revision = editor.arena().revision();
                    insert(&mut editor, &kind);
                    assert_eq!(editor.to_portable().unwrap(), source);
                    assert_eq!(editor.caret(), caret);
                    assert_eq!(editor.arena().revision(), revision);
                    assert!(!editor.undo());
                }
                if matches!(source.kind, NodeKind::Binary(_)) {
                    editor.place_text_selection(target, 0, 4).unwrap();
                    insert(&mut editor, &kind);
                    assert_eq!(editor.to_portable().unwrap(), source);
                }
                if matches!(
                    source.kind,
                    NodeKind::NativeIdentifier {
                        subscript: Some(_),
                        ..
                    }
                ) {
                    editor.place_subscript_selection(target, 0, 7).unwrap();
                    insert(&mut editor, &kind);
                    assert_eq!(editor.to_portable().unwrap(), source);
                }
            }
        }
    }

    #[test]
    fn scope_budget_accounts_for_reused_programs_and_failed_growth_preserves_redo() {
        let mut cells = vec![];
        for i in 0..4096 {
            let mut cell = atom(NodeKind::Identifier("x".into()));
            let groups = if i == 0 {
                0
            } else if i == 4095 {
                2
            } else {
                3
            };
            for _ in 0..groups {
                cell = tree(NodeKind::Group, vec![cell]);
            }
            cells.push(cell);
        }
        let matrix = tree(
            NodeKind::Matrix {
                rows: 64,
                cols: 64,
                display: None,
            },
            cells,
        );
        for source in [
            matrix.clone(),
            tree(NodeKind::Program { statement_count: 1 }, vec![matrix]),
        ] {
            let mut editor = Editor::from_portable(&source).unwrap();
            let target = editor.root();
            editor
                .select_subtree_from(target, Direction::Right)
                .unwrap();
            editor.insert_while_loop();
            assert_eq!(editor.arena().nodes().count(), MAX_TREE_NODES);
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), source);
            let revision = editor.arena().revision();
            assert!(
                !editor.insert_try(),
                "Try requires one more node than the matching While path"
            );
            assert_eq!(editor.to_portable().unwrap(), source);
            assert_eq!(editor.arena().revision(), revision);
            assert!(editor.redo());
            assert_eq!(editor.arena().nodes().count(), MAX_TREE_NODES);
        }
    }

    #[test]
    fn scoped_depth_binder_constraints_and_normal_partial_text_fail_before_history_or_source_changes()
     {
        let mut deep = identifier();
        for _ in 1..MAX_TREE_DEPTH {
            deep = tree(NodeKind::Group, vec![deep]);
        }
        let binder = tree(
            NodeKind::For,
            vec![
                atom(NodeKind::Identifier("i".into())),
                atom(NodeKind::Number("1".into())),
                tree(
                    NodeKind::Program { statement_count: 1 },
                    vec![atom(NodeKind::Placeholder)],
                ),
            ],
        );
        for source in [deep, binder] {
            let mut editor = Editor::from_portable(&source).unwrap();
            let target = editor.caret().node;
            editor
                .select_subtree_from(target, Direction::Right)
                .unwrap();
            let caret = editor.caret();
            let revision = editor.arena().revision();
            assert_eq!(editor.insert_while_loop(), target);
            assert_eq!(editor.to_portable().unwrap(), source);
            assert_eq!(editor.arena().revision(), revision);
            assert_eq!(editor.caret(), caret);
            assert_eq!(editor.selection(), Selection::Subtree(target));
            assert!(!editor.undo());
        }
        let source = atom(NodeKind::Identifier("atom".into()));
        let mut editor = Editor::from_portable(&source).unwrap();
        let target = editor.root();
        editor.place_text_selection(target, 0, 4).unwrap();
        editor.insert_while_loop();
        assert!(editor.undo());
        editor.place_text_selection(target, 1, 3).unwrap();
        let revision = editor.arena().revision();
        editor.insert_for_loop();
        assert_eq!(editor.to_portable().unwrap(), source);
        assert_eq!(editor.arena().revision(), revision);
        assert!(editor.redo());
    }

    #[test]
    fn normal_full_statement_replacement_is_isolated_and_keeps_other_program_line_ids() {
        let source = tree(
            NodeKind::Program { statement_count: 3 },
            vec![
                atom(NodeKind::Number("1".into())),
                atom(NodeKind::NativeIdentifier {
                    name: "value".into(),
                    labels: "VARIABLE".into(),
                    contextual: Some(false),
                    subscript: None,
                }),
                atom(NodeKind::Number("2".into())),
            ],
        );
        let mut editor = Editor::from_portable(&source).unwrap();
        let root = editor.root();
        let lines = editor.arena().get(root).unwrap().children().to_vec();
        editor.place_text_selection(lines[1], 0, 5).unwrap();
        editor.insert_for_loop();
        assert_eq!(editor.arena().get(root).unwrap().children(), lines);
        let result = editor.to_portable().unwrap();
        assert_eq!(result.children[0], source.children[0]);
        assert_eq!(
            result.children[1],
            program_control::control_template(NodeKind::For)
        );
        assert_eq!(result.children[2], source.children[2]);
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), source);
        assert!(!editor.undo());
        assert!(editor.redo());
        assert_eq!(editor.to_portable().unwrap(), result);
    }

    #[test]
    fn ctrl_j_scale_keyword_preserves_the_right_operand_and_all_metadata_as_one_command() {
        let mut operators = kinds().to_vec();
        operators.push((NodeKind::Return, 0));
        for (kind, primal) in operators {
            let name = match kind {
                NodeKind::While => "while",
                NodeKind::For => "for",
                NodeKind::If { .. } => "if",
                NodeKind::Try => "try",
                NodeKind::Return => "return",
                _ => unreachable!(),
            };
            for operand in [
                identifier(),
                tree(
                    NodeKind::Program { statement_count: 2 },
                    vec![identifier(), atom(NodeKind::Placeholder)],
                ),
            ] {
                let source = tree(
                    NodeKind::Binary(BinaryOp::Scale),
                    vec![
                        atom(NodeKind::NativeIdentifier {
                            name: name.into(),
                            labels: "NONE".into(),
                            contextual: Some(false),
                            subscript: None,
                        }),
                        operand.clone(),
                    ],
                );
                let mut editor = Editor::from_portable(&source).unwrap();
                let scale = editor.root();
                let keyword = editor.arena().get(scale).unwrap().children()[0];
                let right = editor.arena().get(scale).unwrap().children()[1];
                editor.place_text_selection(keyword, 0, 1).unwrap();
                let caret = editor.caret();
                assert!(editor.convert_program_keyword());
                assert_eq!(editor.arena().get(editor.root()).unwrap().kind(), &kind);
                assert_eq!(editor.export_subtree(right).unwrap(), operand);
                assert!(editor.arena().get(scale).is_none());
                assert!(editor.arena().get(keyword).is_none());
                let slot = editor.arena().get(editor.root()).unwrap().children()[primal];
                if kind == NodeKind::Return || matches!(operand.kind, NodeKind::Program { .. }) {
                    assert_eq!(slot, right);
                } else {
                    assert_eq!(
                        editor.arena().get(slot).unwrap().kind(),
                        &NodeKind::Program { statement_count: 1 }
                    );
                    assert_eq!(editor.arena().get(slot).unwrap().children(), &[right]);
                }
                match kind {
                    NodeKind::Try => assert_eq!(editor.caret(), Caret::at(slot, Direction::Left)),
                    NodeKind::Return => assert_eq!(
                        editor.caret(),
                        editor.program_operand_left_entry(right).unwrap()
                    ),
                    _ => assert_eq!(
                        editor.caret().node,
                        editor.arena().get(editor.root()).unwrap().children()[0]
                    ),
                }
                let result = editor.to_portable().unwrap();
                assert!(editor.undo());
                assert_eq!(editor.to_portable().unwrap(), source);
                assert_eq!(editor.root(), scale);
                assert_eq!(editor.caret(), caret);
                assert!(
                    !editor.undo(),
                    "keyword removal and primal preservation share one checkpoint"
                );
                assert!(editor.redo());
                assert_eq!(editor.to_portable().unwrap(), result);
                assert_eq!(editor.export_subtree(right).unwrap(), operand);
            }
        }
    }

    #[test]
    fn multiplication_right_keyword_zero_arity_and_branch_words_do_not_gain_scale_primal_preservation()
     {
        let right = identifier();
        for source in [
            tree(
                NodeKind::Binary(BinaryOp::Multiply),
                vec![atom(NodeKind::Identifier("while".into())), right.clone()],
            ),
            tree(
                NodeKind::Binary(BinaryOp::Scale),
                vec![right.clone(), atom(NodeKind::Identifier("while".into()))],
            ),
        ] {
            let mut editor = Editor::from_portable(&source).unwrap();
            let root = editor.root();
            let keyword = editor
                .arena()
                .nodes()
                .find_map(|(id, node)| {
                    (node.kind() == &NodeKind::Identifier("while".into())).then_some(id)
                })
                .unwrap();
            editor.place_caret(keyword, Direction::Right).unwrap();
            assert!(editor.convert_program_keyword());
            assert_eq!(editor.root(), root);
            assert_eq!(editor.arena().get(root).unwrap().kind(), &source.kind);
            assert_eq!(
                editor.export_subtree(keyword).unwrap(),
                program_control::control_template(NodeKind::While)
            );
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), source);
        }
        for (name, kind) in [("break", NodeKind::Break), ("continue", NodeKind::Continue)] {
            let source = tree(
                NodeKind::Binary(BinaryOp::Scale),
                vec![atom(NodeKind::Identifier(name.into())), right.clone()],
            );
            let mut editor = Editor::from_portable(&source).unwrap();
            let root = editor.root();
            assert!(editor.convert_program_keyword());
            assert_eq!(editor.root(), root);
            assert_eq!(
                editor.to_portable().unwrap(),
                tree(
                    NodeKind::Binary(BinaryOp::Scale),
                    vec![atom(kind), right.clone()]
                )
            );
        }
        for name in ["else", "elseif", "alsoif"] {
            let source = tree(
                NodeKind::Binary(BinaryOp::Scale),
                vec![atom(NodeKind::Identifier(name.into())), right.clone()],
            );
            let mut editor = Editor::from_portable(&source).unwrap();
            let revision = editor.arena().revision();
            assert!(!editor.convert_program_keyword());
            assert_eq!(editor.to_portable().unwrap(), source);
            assert_eq!(editor.arena().revision(), revision);
            assert!(!editor.undo());
        }
    }

    #[test]
    fn scale_conversion_keeps_shared_explicit_label_literal_suffix_and_scope_guards() {
        for keyword in [
            NodeKind::AnnotatedIdentifier {
                name: "while".into(),
                labels: "VARIABLE".into(),
            },
            NodeKind::NativeIdentifier {
                name: "while".into(),
                labels: "*".into(),
                contextual: Some(true),
                subscript: Some("literal".into()),
            },
        ] {
            let source = tree(
                NodeKind::Binary(BinaryOp::Scale),
                vec![atom(keyword), identifier()],
            );
            let mut editor = Editor::from_portable(&source).unwrap();
            let revision = editor.arena().revision();
            assert!(!editor.convert_program_keyword());
            assert_eq!(editor.to_portable().unwrap(), source);
            assert_eq!(editor.arena().revision(), revision);
            assert!(!editor.undo());
        }
        let source = tree(
            NodeKind::Binary(BinaryOp::Scale),
            vec![
                atom(NodeKind::NativeIdentifier {
                    name: "while".into(),
                    labels: "UNKNOWN_RESOLVED".into(),
                    contextual: Some(true),
                    subscript: None,
                }),
                identifier(),
            ],
        );
        let mut editor = Editor::from_portable(&source).unwrap();
        let keyword = editor.caret().node;
        editor.select_subtree(keyword).unwrap();
        assert!(!editor.convert_program_keyword());
        editor.place_text_caret(keyword, 0).unwrap();
        assert!(
            editor.convert_program_keyword(),
            "resolved contextual labels are not explicit labels"
        );
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), source);
    }

    #[test]
    fn scale_conversion_depth_and_node_failures_preserve_source_history_and_do_not_fall_back() {
        let mut deep = identifier();
        for _ in 2..MAX_TREE_DEPTH {
            deep = tree(NodeKind::Group, vec![deep]);
        }
        let source = tree(
            NodeKind::Binary(BinaryOp::Scale),
            vec![atom(NodeKind::Identifier("while".into())), deep],
        );
        let mut editor = Editor::from_portable(&source).unwrap();
        editor.set_identifier_label("VARIABLE");
        assert!(editor.undo());
        let caret = editor.caret();
        let revision = editor.arena().revision();
        assert!(!editor.convert_program_keyword());
        assert_eq!(editor.to_portable().unwrap(), source);
        assert_eq!(editor.caret(), caret);
        assert_eq!(editor.arena().revision(), revision);
        assert!(
            editor.redo(),
            "a failed special path cannot erase redo or replace only the keyword"
        );

        let mut cells = vec![];
        for i in 0..4096 {
            let mut value = atom(NodeKind::Identifier("x".into()));
            for _ in 0..if i == 0 { 0 } else { 3 } {
                value = tree(NodeKind::Group, vec![value]);
            }
            cells.push(value);
        }
        let operand = tree(
            NodeKind::Matrix {
                rows: 64,
                cols: 64,
                display: None,
            },
            cells,
        );
        for name in ["for", "return"] {
            let source = tree(
                NodeKind::Binary(BinaryOp::Scale),
                vec![atom(NodeKind::Identifier(name.into())), operand.clone()],
            );
            let mut editor = Editor::from_portable(&source).unwrap();
            assert_eq!(editor.arena().nodes().count(), MAX_TREE_NODES);
            let revision = editor.arena().revision();
            assert_eq!(editor.convert_program_keyword(), name == "return");
            if name == "return" {
                assert_eq!(editor.arena().nodes().count(), MAX_TREE_NODES - 1);
                assert!(editor.undo());
                assert_eq!(editor.to_portable().unwrap(), source);
                assert!(!editor.undo());
            } else {
                assert_eq!(editor.to_portable().unwrap(), source);
                assert_eq!(editor.arena().revision(), revision);
                assert!(!editor.undo());
            }
        }
    }
}
