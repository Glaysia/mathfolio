//! The confirmed blank/pair-boundary subset of original conditional insertion.
//! Original IF has flat test/body pairs and a single alternate-mode flag.
//! Generic populated-container splitting is separate from these explicit
//! descriptor APIs. The confirmed basic Ctrl+J IF template is in program_control.
//! See `docs/analysis/0099-conditional-branch-contract.md` for the original
//! descriptor/BODY persistence and pair-boundary CIL evidence. Program is this
//! editor's supported canonical BODY representation; nested programs stay nested.
use super::*;

fn empty_body(arena: &mut Arena) -> (NodeId, NodeId) {
    let value = arena.allocate(NodeKind::Placeholder, vec![]);
    let body = arena.allocate(NodeKind::Program { statement_count: 1 }, vec![value]);
    (body, value)
}

impl Editor {
    // Called only after program_control's shared original identifier guards.
    // The original conversion replaces the eligible word by a placeholder,
    // then performs ordinary pair insertion. Keep both stages transactional:
    // rejected adjacency/mode/limits must not first erase the identifier.
    pub(super) fn convert_branch_keyword(&mut self, mode: Option<bool>) -> bool {
        let mut candidate = self.clone_without_history();
        let target = candidate.caret.node;
        candidate.arena.node_mut(target).kind = NodeKind::Placeholder;
        candidate.caret = Caret::at(target, Direction::Indeterminate);
        let inserted = match mode {
            Some(false) => candidate.insert_else_if(),
            Some(true) => candidate.insert_also_if(),
            None => candidate.insert_else(),
        };
        if !inserted {
            return false;
        }
        self.commit_branch_candidate(candidate)
    }

    /// Navigate to one IF/ELSE_IF/ALSO_IF/ELSE operator word. Part zero is
    /// the first IF, subsequent pairs occupy one part each, and terminal ELSE
    /// (if present) occupies `branch_count`. Navigation does not alter history.
    pub fn place_if_operator(&mut self, node: NodeId, part: usize) -> Result<(), TreeError> {
        let Some(entry) = self.arena.get(node) else {
            return Err(TreeError::InvalidTree("missing conditional operator"));
        };
        let NodeKind::If {
            has_else,
            branch_count,
            ..
        } = entry.kind
        else {
            return Err(TreeError::InvalidTree("operator target is not an if"));
        };
        if part >= branch_count.saturating_add(usize::from(has_else)) {
            return Err(TreeError::InvalidTree("if operator part outside bounds"));
        }
        self.typeover = None;
        self.caret = Caret::at(node, Direction::Indeterminate);
        self.caret.operator_part = Some(part);
        self.selection = Selection::None;
        self.scope = None;
        Ok(())
    }

    /// Insert the original minimal IF descriptor through the confirmed blank,
    /// whole-value or Scope paths. It has one test/body pair; ELSE is separate.
    pub fn insert_if(&mut self) -> bool {
        self.insert_program_control(NodeKind::If {
            has_else: false,
            branch_count: 1,
            alternate: false,
        })
        .is_some()
    }

    /// Insert an ELSE_IF subelement at a confirmed incomplete-pair boundary.
    /// At an existing middle operator center, select the ordinary chain form.
    pub fn insert_else_if(&mut self) -> bool {
        self.insert_branch(Some(false))
    }

    /// Insert an ALSO_IF subelement or select the alternate form for the whole
    /// chain at an existing middle operator center. Mixed chain modes are not
    /// represented as independent per-branch flags in the original editor.
    pub fn insert_also_if(&mut self) -> bool {
        self.insert_branch(Some(true))
    }

    /// Insert the single terminal ELSE at an incomplete-pair boundary.
    pub fn insert_else(&mut self) -> bool {
        self.insert_branch(None)
    }

    fn commit_branch_candidate(&mut self, mut candidate: Editor) -> bool {
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
        true
    }

    fn insert_branch(&mut self, mode: Option<bool>) -> bool {
        if self.selection != Selection::None {
            return false;
        }
        let target = self.caret.node;
        let node = self.arena.get(target).unwrap();
        // a2.a operates on an actual pair operator center. The middle operator
        // changes the whole-chain form; it does not append another branch.
        if let NodeKind::If {
            has_else,
            branch_count,
            alternate,
        } = node.kind
            && self.caret.caret_side == Direction::Indeterminate
            && let (Some(part), Some(new_mode)) = (self.caret.operator_part, mode)
            && part > 0
            && part < branch_count
        {
            if alternate == new_mode {
                return false;
            }
            let mut candidate = self.clone_without_history();
            candidate.arena.node_mut(target).kind = NodeKind::If {
                has_else,
                branch_count,
                alternate: new_mode,
            };
            return self.commit_branch_candidate(candidate);
        }

        let (pair, consumed) = if matches!(node.kind, NodeKind::If { .. })
            && self.caret.caret_side == Direction::Right
        {
            (target, None)
        } else if node.kind == NodeKind::Placeholder {
            // bg.a explicitly recognizes a blank program statement following
            // an incomplete pair and consumes that statement after insertion.
            let Some(parent) = node.parent else {
                return false;
            };
            let parent_node = self.arena.get(parent).unwrap();
            if !matches!(parent_node.kind, NodeKind::Program { .. }) {
                return false;
            }
            let index = parent_node
                .children
                .iter()
                .position(|id| *id == target)
                .unwrap();
            let Some(previous) = index.checked_sub(1).map(|i| parent_node.children[i]) else {
                return false;
            };
            (previous, Some((parent, index)))
        } else {
            return false;
        };
        let NodeKind::If {
            has_else: false,
            branch_count,
            alternate,
        } = self.arena.get(pair).unwrap().kind
        else {
            return false;
        };
        if mode.is_some_and(|next| branch_count > 1 && next != alternate)
            || (mode.is_some() && branch_count >= MAX_IF_BRANCHES)
        {
            return false;
        }
        let mut candidate = self.clone_without_history();
        let (body, body_value) = empty_body(&mut candidate.arena);
        let mut added = vec![];
        let focus = if mode.is_some() {
            let test = candidate.arena.allocate(NodeKind::Placeholder, vec![]);
            added.push(test);
            test
        } else {
            body_value
        };
        added.push(body);
        for child in &added {
            candidate.arena.node_mut(*child).parent = Some(pair);
        }
        let pair_node = candidate.arena.node_mut(pair);
        pair_node.kind = NodeKind::If {
            has_else: mode.is_none(),
            branch_count: branch_count + usize::from(mode.is_some()),
            alternate: mode.unwrap_or(alternate),
        };
        pair_node.children.extend(added);
        if let Some((parent, index)) = consumed {
            let parent_node = candidate.arena.node_mut(parent);
            parent_node.children.remove(index);
            parent_node.kind = NodeKind::Program {
                statement_count: parent_node.children.len(),
            };
            candidate.retire(target, None);
        }
        candidate.caret = Caret::at(focus, Direction::Indeterminate);
        self.commit_branch_candidate(candidate)
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

    fn conditional(branches: usize, has_else: bool, alternate: bool) -> PortableTree {
        let mut children = vec![];
        for i in 0..branches {
            children.push(atom(NodeKind::Identifier(format!("test{i}"))));
            children.push(tree(
                NodeKind::Program { statement_count: 1 },
                vec![atom(NodeKind::Number(i.to_string()))],
            ));
        }
        if has_else {
            children.push(tree(
                NodeKind::Program { statement_count: 1 },
                vec![atom(NodeKind::Number("-1".into()))],
            ));
        }
        tree(
            NodeKind::If {
                has_else,
                branch_count: branches,
                alternate,
            },
            children,
        )
    }

    #[test]
    fn old_if_save_defaults_to_one_ordinary_branch_and_new_chain_roundtrips() {
        let old = r#"{"kind":{"If":{"has_else":true}},"children":[{"kind":"Placeholder","children":[]},{"kind":{"Program":{"statement_count":1}},"children":[{"kind":"Placeholder","children":[]}]},{"kind":{"Program":{"statement_count":1}},"children":[{"kind":"Placeholder","children":[]}]}]}"#;
        let parsed: PortableTree = serde_json::from_str(old).unwrap();
        assert_eq!(
            parsed.kind,
            NodeKind::If {
                has_else: true,
                branch_count: 1,
                alternate: false
            }
        );
        parsed.validate().unwrap();
        for count in [1, 2, 9] {
            for has_else in [false, true] {
                for alternate in [false, true] {
                    let source = conditional(count, has_else, alternate);
                    let encoded = serde_json::to_string(&source).unwrap();
                    let recovered: PortableTree = serde_json::from_str(&encoded).unwrap();
                    assert_eq!(recovered, source);
                    assert_eq!(
                        Editor::from_portable(&recovered)
                            .unwrap()
                            .to_portable()
                            .unwrap(),
                        source
                    );
                }
            }
        }
    }

    #[test]
    fn branch_keywords_consume_the_adjacent_identifier_with_shared_label_guards_and_one_undo() {
        for name in ["else", "elseif", "alsoif"] {
            for keyword in [
                NodeKind::Identifier(name.into()),
                NodeKind::AnnotatedIdentifier {
                    name: name.into(),
                    labels: "*".into(),
                },
                NodeKind::NativeIdentifier {
                    name: name.into(),
                    labels: "NONE".into(),
                    contextual: Some(false),
                    subscript: None,
                },
                NodeKind::NativeIdentifier {
                    name: name.into(),
                    labels: "UNRECOGNIZED_RESOLVED".into(),
                    contextual: Some(true),
                    subscript: None,
                },
            ] {
                let source = tree(
                    NodeKind::Program { statement_count: 4 },
                    vec![
                        atom(NodeKind::Number("17".into())),
                        conditional(1, false, false),
                        atom(keyword),
                        atom(NodeKind::Identifier("after".into())),
                    ],
                );
                let mut editor = Editor::from_portable(&source).unwrap();
                editor.set_expressions_only(true);
                let root = editor.root();
                let slots = editor.arena().get(root).unwrap().children().to_vec();
                editor.place_text_selection(slots[2], 1, 2).unwrap();
                let caret = editor.caret();
                assert!(editor.convert_program_keyword());
                assert_eq!(
                    editor.arena().get(root).unwrap().children(),
                    &[slots[0], slots[1], slots[3]]
                );
                assert!(editor.arena().get(slots[2]).is_none());
                let result = editor.to_portable().unwrap();
                assert_eq!(result.children[0], source.children[0]);
                assert_eq!(result.children[2], source.children[3]);
                assert_eq!(
                    result.children[1].kind,
                    NodeKind::If {
                        has_else: name == "else",
                        branch_count: if name == "else" { 1 } else { 2 },
                        alternate: name == "alsoif",
                    }
                );
                assert_eq!(
                    editor.arena().get(editor.caret().node).unwrap().kind(),
                    &NodeKind::Placeholder
                );
                assert!(editor.undo());
                assert_eq!(editor.to_portable().unwrap(), source);
                assert_eq!(editor.caret(), caret);
                assert!(
                    !editor.undo(),
                    "placeholder replacement and pair repair are one command"
                );
                assert!(editor.redo());
                assert_eq!(editor.to_portable().unwrap(), result);
            }
        }
    }

    #[test]
    fn rejected_branch_keyword_guards_context_and_modes_keep_original_word_and_redo() {
        for name in ["else", "elseif", "alsoif"] {
            for keyword in [
                NodeKind::Identifier(name.to_uppercase()),
                NodeKind::Identifier(format!("{name} ")),
                NodeKind::AnnotatedIdentifier {
                    name: name.into(),
                    labels: "KEYWORD".into(),
                },
                NodeKind::NativeIdentifier {
                    name: name.into(),
                    labels: "VARIABLE".into(),
                    contextual: None,
                    subscript: None,
                },
                NodeKind::NativeIdentifier {
                    name: name.into(),
                    labels: "*".into(),
                    contextual: Some(true),
                    subscript: Some(String::new()),
                },
            ] {
                let source = tree(
                    NodeKind::Program { statement_count: 2 },
                    vec![conditional(1, false, false), atom(keyword)],
                );
                let mut editor = Editor::from_portable(&source).unwrap();
                let word = editor.arena().get(editor.root()).unwrap().children()[1];
                editor.place_caret(word, Direction::Right).unwrap();
                let caret = editor.caret();
                let revision = editor.arena().revision();
                assert!(!editor.convert_program_keyword());
                assert_eq!(editor.to_portable().unwrap(), source);
                assert_eq!(editor.caret(), caret);
                assert_eq!(editor.arena().revision(), revision);
                assert!(!editor.undo());
            }
        }
        for (pair, name) in [
            (conditional(1, true, false), "else"),
            (conditional(2, false, false), "alsoif"),
            (conditional(2, false, true), "elseif"),
        ] {
            let source = tree(
                NodeKind::Program { statement_count: 2 },
                vec![pair, atom(NodeKind::Identifier(name.into()))],
            );
            let mut editor = Editor::from_portable(&source).unwrap();
            // Make and undo a valid edit to establish redo independent of the
            // incompatible branch command's candidate placeholder mutation.
            let word = editor.arena().get(editor.root()).unwrap().children()[1];
            editor.place_caret(word, Direction::Right).unwrap();
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
                "failed candidate must retain the previous redo"
            );
        }
        for source in [
            atom(NodeKind::Identifier("else".into())),
            tree(
                NodeKind::Program { statement_count: 3 },
                vec![
                    conditional(1, false, false),
                    atom(NodeKind::Number("3".into())),
                    atom(NodeKind::Identifier("else".into())),
                ],
            ),
        ] {
            let mut editor = Editor::from_portable(&source).unwrap();
            let word = editor
                .arena()
                .nodes()
                .find_map(|(id, node)| {
                    (node.kind() == &NodeKind::Identifier("else".into())).then_some(id)
                })
                .unwrap();
            editor.place_caret(word, Direction::Right).unwrap();
            assert!(!editor.convert_program_keyword());
            assert_eq!(editor.to_portable().unwrap(), source);
            assert!(!editor.undo());
        }
    }

    #[test]
    fn chain_validation_checks_every_body_but_accepts_expression_tests_and_untrusted_counts() {
        let mut valid = conditional(3, true, false);
        valid.children[2] = atom(NodeKind::Number("5".into()));
        valid.validate().unwrap();
        for slot in [1, 3, 5, 6] {
            let mut bad = valid.clone();
            bad.children[slot] = atom(NodeKind::Placeholder);
            assert!(bad.validate().is_err());
        }
        for branch_count in [0, MAX_IF_BRANCHES + 1, usize::MAX] {
            let bad = tree(
                NodeKind::If {
                    has_else: true,
                    branch_count,
                    alternate: false,
                },
                vec![],
            );
            assert!(bad.validate().is_err());
        }
        let limit = conditional(MAX_IF_BRANCHES, false, false);
        limit.validate().unwrap();
    }

    #[test]
    fn explicit_if_creates_no_fabricated_else_and_is_one_undo_with_stable_target_id() {
        let mut editor = Editor::new();
        editor.set_expressions_only(true);
        let before = editor.to_portable().unwrap();
        let original_caret = editor.caret();
        let root = editor.root();
        assert!(editor.insert_if());
        assert_eq!(editor.root(), root);
        assert_eq!(
            editor.arena().get(root).unwrap().kind(),
            &NodeKind::If {
                has_else: false,
                branch_count: 1,
                alternate: false
            }
        );
        assert_eq!(
            editor.caret().node,
            editor.arena().get(root).unwrap().children()[0]
        );
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), before);
        assert_eq!(editor.caret(), original_caret);
        assert!(!editor.undo());
        assert!(editor.redo());
        editor.place_caret(root, Direction::Right).unwrap();
        let written = editor.to_portable().unwrap();
        assert!(
            !editor.insert_if(),
            "populated generic repair is not implemented by this API"
        );
        assert_eq!(editor.to_portable().unwrap(), written);
    }

    #[test]
    fn append_uniform_branches_and_terminal_else_preserves_prior_slots_ids_and_undo() {
        for alternate in [false, true] {
            let source = conditional(1, false, false);
            let mut editor = Editor::from_portable(&source).unwrap();
            let pair = editor.root();
            let original_slots = editor.arena().get(pair).unwrap().children().to_vec();
            editor.place_caret(pair, Direction::Right).unwrap();
            let before_caret = editor.caret();
            let command = if alternate {
                Editor::insert_also_if
            } else {
                Editor::insert_else_if
            };
            assert!(command(&mut editor));
            let after = editor.to_portable().unwrap();
            assert_eq!(
                after.kind,
                NodeKind::If {
                    has_else: false,
                    branch_count: 2,
                    alternate
                }
            );
            assert_eq!(
                &editor.arena().get(pair).unwrap().children()[..2],
                original_slots
            );
            assert_eq!(
                editor.caret().node,
                editor.arena().get(pair).unwrap().children()[2]
            );
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), source);
            assert_eq!(editor.caret(), before_caret);
            assert!(!editor.undo());
            assert!(editor.redo());
            editor.place_caret(pair, Direction::Right).unwrap();
            let wrong = if alternate {
                Editor::insert_else_if
            } else {
                Editor::insert_also_if
            };
            assert!(!wrong(&mut editor));
            assert_eq!(editor.to_portable().unwrap(), after);
            assert!(command(&mut editor));
            editor.place_caret(pair, Direction::Right).unwrap();
            assert!(editor.insert_else());
            let finished = editor.to_portable().unwrap();
            assert_eq!(
                finished.kind,
                NodeKind::If {
                    has_else: true,
                    branch_count: 3,
                    alternate
                }
            );
            editor.place_caret(pair, Direction::Right).unwrap();
            assert!(!editor.insert_else());
            assert!(!editor.insert_else_if());
            assert!(!editor.insert_also_if());
            assert_eq!(editor.to_portable().unwrap(), finished);
        }
    }

    #[test]
    fn blank_statement_after_incomplete_pair_is_consumed_without_replacing_other_statements() {
        let source = tree(
            NodeKind::Program { statement_count: 4 },
            vec![
                atom(NodeKind::NativeIdentifier {
                    name: "before".into(),
                    labels: "VARIABLE".into(),
                    contextual: None,
                    subscript: Some("tag".into()),
                }),
                conditional(1, false, false),
                atom(NodeKind::Placeholder),
                atom(NodeKind::Number("99".into())),
            ],
        );
        let mut editor = Editor::from_portable(&source).unwrap();
        let root = editor.root();
        let old = editor.arena().get(root).unwrap().children().to_vec();
        editor.place_caret(old[2], Direction::Left).unwrap();
        assert!(editor.insert_also_if());
        assert_eq!(
            editor.arena().get(root).unwrap().children(),
            &[old[0], old[1], old[3]]
        );
        assert!(editor.arena().get(old[2]).is_none());
        assert_eq!(
            editor.to_portable().unwrap().children[0],
            source.children[0]
        );
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), source);
        assert_eq!(editor.arena().get(root).unwrap().children(), old);
        assert!(editor.redo());
        assert_eq!(
            editor.arena().get(root).unwrap().children(),
            &[old[0], old[1], old[3]]
        );
    }

    #[test]
    fn middle_operator_switches_entire_chain_mode_and_keeps_exact_caret() {
        let source = conditional(3, true, false);
        let mut editor = Editor::from_portable(&source).unwrap();
        let pair = editor.root();
        editor.caret = Caret::at(pair, Direction::Indeterminate);
        editor.caret.operator_part = Some(1);
        let caret = editor.caret();
        assert!(editor.insert_also_if());
        assert_eq!(editor.caret(), caret);
        let mut expected = source.clone();
        expected.kind = NodeKind::If {
            has_else: true,
            branch_count: 3,
            alternate: true,
        };
        assert_eq!(editor.to_portable().unwrap(), expected);
        assert!(!editor.insert_also_if());
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), source);
        assert!(editor.redo());
        assert!(editor.insert_else_if());
        assert_eq!(editor.to_portable().unwrap(), source);
        editor.caret.operator_part = Some(3); // ELSE is not a middle subelement.
        assert!(!editor.insert_also_if());
        editor.caret.operator_part = Some(0); // first IF is not a middle subelement.
        assert!(!editor.insert_also_if());
    }

    #[test]
    fn ambiguous_selection_non_boundaries_and_failed_commands_preserve_history() {
        let source = conditional(2, false, true);
        let mut editor = Editor::from_portable(&source).unwrap();
        let pair = editor.root();
        editor.place_caret(pair, Direction::Right).unwrap();
        assert!(editor.insert_also_if());
        assert!(editor.undo());
        assert!(!editor.insert_else_if()); // mismatched mode must not erase redo
        assert!(editor.redo());
        let unchanged = editor.to_portable().unwrap();
        editor.selection = Selection::Subtree(pair);
        let caret = editor.caret();
        assert!(!editor.insert_else());
        assert_eq!(editor.selection(), Selection::Subtree(pair));
        assert_eq!(editor.caret(), caret);
        editor.selection = Selection::None;
        let test = editor.arena().get(pair).unwrap().children()[0];
        editor.place_caret(test, Direction::Right).unwrap();
        assert!(!editor.insert_else());
        assert_eq!(editor.to_portable().unwrap(), unchanged);
    }

    #[test]
    fn nested_blank_boundary_binds_the_adjacent_inner_pair_and_restores_scope_on_undo() {
        let mut source = conditional(1, false, false);
        source.children[1] = tree(
            NodeKind::Program { statement_count: 3 },
            vec![
                conditional(1, false, false),
                atom(NodeKind::Placeholder),
                atom(NodeKind::Number("7".into())),
            ],
        );
        let mut editor = Editor::from_portable(&source).unwrap();
        let outer = editor.root();
        let body = editor.arena().get(outer).unwrap().children()[1];
        let inner = editor.arena().get(body).unwrap().children()[0];
        let blank = editor.arena().get(body).unwrap().children()[1];
        editor.place_caret(blank, Direction::Right).unwrap();
        let origin = editor.caret();
        // A previous scope cycle may leave history while returning to Normal.
        editor.scope = Some(ScopeState {
            origin,
            direction: Direction::Right,
            history: vec![(origin, Selection::None)],
        });
        assert!(editor.insert_else());
        assert!(editor.scope.is_none());
        assert_eq!(
            editor.arena().get(outer).unwrap().kind(),
            &NodeKind::If {
                has_else: false,
                branch_count: 1,
                alternate: false
            }
        );
        assert_eq!(
            editor.arena().get(inner).unwrap().kind(),
            &NodeKind::If {
                has_else: true,
                branch_count: 1,
                alternate: false
            }
        );
        assert_eq!(editor.arena().get(body).unwrap().children().len(), 2);
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), source);
        assert_eq!(editor.caret(), origin);
        assert_eq!(editor.scope.as_ref().unwrap().history.len(), 1);
        assert!(editor.redo());
        assert!(editor.scope.is_none());
    }

    #[test]
    fn conditional_resource_and_live_node_limits_fail_atomically() {
        let source = conditional(MAX_IF_BRANCHES, false, false);
        let mut editor = Editor::from_portable(&source).unwrap();
        editor.place_caret(editor.root(), Direction::Right).unwrap();
        let caret = editor.caret();
        assert!(!editor.insert_else_if());
        assert_eq!(editor.to_portable().unwrap(), source);
        assert_eq!(editor.caret(), caret);
        assert!(!editor.undo());

        // Maximal valid tree: one Program, the 12289-node IF and 4094 atoms.
        let mut children = vec![source];
        children.extend((0..4094).map(|_| atom(NodeKind::Placeholder)));
        let source = tree(
            NodeKind::Program {
                statement_count: children.len(),
            },
            children,
        );
        let mut editor = Editor::from_portable(&source).unwrap();
        assert_eq!(editor.arena().nodes().count(), MAX_TREE_NODES);
        let pair = editor.arena().get(editor.root()).unwrap().children()[0];
        editor.place_caret(pair, Direction::Right).unwrap();
        assert!(!editor.insert_else());
        assert_eq!(editor.to_portable().unwrap(), source);
        assert!(!editor.undo());
        let blank = *editor
            .arena()
            .get(editor.root())
            .unwrap()
            .children()
            .last()
            .unwrap();
        editor.place_caret(blank, Direction::Left).unwrap();
        assert!(!editor.insert_if());
        assert_eq!(editor.to_portable().unwrap(), source);
        assert!(!editor.undo());
    }

    #[test]
    fn operator_part_navigation_validates_targets_clears_scope_and_preserves_redo() {
        let source = conditional(2, false, false);
        let mut editor = Editor::from_portable(&source).unwrap();
        let pair = editor.root();
        editor.place_caret(pair, Direction::Right).unwrap();
        assert!(editor.insert_else());
        let with_else = editor.to_portable().unwrap();
        assert!(editor.undo());
        let origin = editor.caret();
        editor.selection = Selection::Subtree(pair);
        editor.scope = Some(ScopeState {
            origin,
            direction: Direction::Right,
            history: vec![(origin, Selection::None)],
        });
        let test = editor.arena().get(pair).unwrap().children()[0];
        for (node, part) in [(test, 0), (pair, 2), (NodeId(usize::MAX), 0)] {
            assert!(editor.place_if_operator(node, part).is_err());
            assert_eq!(editor.caret(), origin);
            assert_eq!(editor.selection(), Selection::Subtree(pair));
            assert!(editor.scope.is_some());
        }
        editor.place_if_operator(pair, 1).unwrap();
        assert_eq!(editor.caret().node, pair);
        assert_eq!(editor.caret().caret_side, Direction::Indeterminate);
        assert_eq!(editor.caret().operator_part, Some(1));
        assert_eq!(editor.selection(), Selection::None);
        assert!(editor.scope.is_none());
        assert_eq!(editor.to_portable().unwrap(), source);
        assert!(!editor.undo(), "navigation must not create an undo command");
        assert!(editor.redo(), "navigation must not discard the undone edit");
        assert_eq!(editor.to_portable().unwrap(), with_else);
        editor.place_if_operator(pair, 2).unwrap();
        assert!(
            !editor.insert_also_if(),
            "terminal else is not a middle part"
        );
        editor.place_if_operator(pair, 0).unwrap();
        assert!(!editor.insert_also_if(), "first if is not a middle part");
        editor.place_if_operator(pair, 1).unwrap();
        assert!(
            editor.insert_also_if(),
            "a middle part can select whole-chain alternate form"
        );
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), with_else);
        assert_eq!(editor.caret().operator_part, Some(1));
    }

    #[test]
    fn branch_keyword_node_budget_counts_the_consumed_word_and_rejects_overflow_atomically() {
        for extra_groups in [1, 2] {
            let mut trailing = atom(NodeKind::Identifier("unchanged".into()));
            for _ in 0..extra_groups {
                trailing = tree(NodeKind::Group, vec![trailing]);
            }
            let mut children = vec![
                conditional(MAX_IF_BRANCHES - 1, false, false),
                atom(NodeKind::Identifier("elseif".into())),
                trailing,
            ];
            children.extend((0..4092).map(|_| atom(NodeKind::Number("0".into()))));
            let source = tree(
                NodeKind::Program {
                    statement_count: children.len(),
                },
                children,
            );
            let mut editor = Editor::from_portable(&source).unwrap();
            let root = editor.root();
            let word = editor.arena().get(root).unwrap().children()[1];
            editor.place_caret(word, Direction::Right).unwrap();
            let count = editor.arena().nodes().count();
            assert_eq!(count, MAX_TREE_NODES - 3 + extra_groups);
            let caret = editor.caret();
            let revision = editor.arena().revision();
            assert_eq!(editor.convert_program_keyword(), extra_groups == 1);
            if extra_groups == 1 {
                assert_eq!(editor.arena().nodes().count(), MAX_TREE_NODES);
                assert!(editor.arena().get(word).is_none());
                assert!(editor.undo());
                assert_eq!(editor.to_portable().unwrap(), source);
                assert_eq!(editor.caret(), caret);
                assert!(!editor.undo());
                assert!(editor.redo());
                assert_eq!(editor.arena().nodes().count(), MAX_TREE_NODES);
            } else {
                assert_eq!(editor.to_portable().unwrap(), source);
                assert_eq!(editor.caret(), caret);
                assert_eq!(editor.arena().revision(), revision);
                assert!(!editor.undo());
            }
        }
    }
}
