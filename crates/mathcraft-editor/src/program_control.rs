//! Program control tokens and the confirmed subset of normal-mode Ctrl+J.
//! Program/loop validity belongs to calculation, not an invented authoring gate.
use super::*;

fn placeholder() -> PortableTree {
    PortableTree {
        kind: NodeKind::Placeholder,
        children: vec![],
    }
}

pub(super) fn control_template(kind: NodeKind) -> PortableTree {
    let body = || PortableTree {
        kind: NodeKind::Program { statement_count: 1 },
        children: vec![placeholder()],
    };
    let children = match kind {
        NodeKind::Break | NodeKind::Continue => vec![],
        NodeKind::Return => vec![placeholder()],
        NodeKind::While
        | NodeKind::If {
            has_else: false,
            branch_count: 1,
            alternate: false,
        } => vec![placeholder(), body()],
        NodeKind::For => vec![placeholder(), placeholder(), body()],
        NodeKind::Try => vec![body(), body()],
        _ => unreachable!("private control template kind"),
    };
    PortableTree { kind, children }
}

impl Editor {
    /// Insert Break into an empty operand or replace a confirmed whole atomic
    /// value selection. This remains separate from keyword conversion.
    pub fn insert_break(&mut self) -> bool {
        self.insert_empty_control(NodeKind::Break)
    }

    /// Insert a Continue token into an empty operand, without a loop/program
    /// ancestry restriction. Calculation can still reject the resulting use.
    pub fn insert_continue(&mut self) -> bool {
        self.insert_empty_control(NodeKind::Continue)
    }

    /// Insert TRY through the confirmed blank, whole-value or Scope paths.
    /// Scope preserves its expression in child zero, the try BODY; child one
    /// is the on-error BODY. Partial text parsing remains separate.
    pub fn insert_try(&mut self) -> bool {
        self.insert_empty_control(NodeKind::Try)
    }

    fn insert_empty_control(&mut self, kind: NodeKind) -> bool {
        self.insert_program_control(kind).is_some()
    }

    /// Convert the whole unlabelled identifier in the represented normal mode.
    /// Partial UTF-16 text selection is still an identifier value; structural
    /// scope and matrix typeover are different modes and are not converted.
    /// Remaining original keyword descriptors await their insertion contracts.
    pub fn convert_program_keyword(&mut self) -> bool {
        let Some(name) = self.eligible_program_keyword_name() else {
            return false;
        };
        let kind = match name {
            "else" => return self.convert_branch_keyword(None),
            "elseif" => return self.convert_branch_keyword(Some(false)),
            "alsoif" => return self.convert_branch_keyword(Some(true)),
            "while" => NodeKind::While,
            "for" => NodeKind::For,
            "return" => NodeKind::Return,
            "break" => NodeKind::Break,
            "continue" => NodeKind::Continue,
            "try" => NodeKind::Try,
            "if" => NodeKind::If {
                has_else: false,
                branch_count: 1,
                alternate: false,
            },
            _ => return false,
        };
        if let Some(changed) = self.convert_scaled_program_keyword(kind.clone()) {
            return changed;
        }
        self.replace_atomic_control(self.caret.node, control_template(kind))
    }

    // Every keyword uses the same original mode/HasNoLabel/formatted-name
    // guards, including the context-dependent pair-subelement conversions.
    fn eligible_program_keyword_name(&self) -> Option<&str> {
        if self.selection != Selection::None {
            return None;
        }
        let target = self.caret.node;
        match &self.arena.get(target).unwrap().kind {
            NodeKind::Identifier(name) => Some(name),
            NodeKind::AnnotatedIdentifier { name, labels }
                if matches!(labels.as_str(), "*" | "NONE") =>
            {
                Some(name)
            }
            NodeKind::NativeIdentifier {
                name,
                labels,
                contextual,
                subscript: None,
            } if *contextual == Some(true) || matches!(labels.as_str(), "*" | "NONE") => Some(name),
            _ => None,
        }
    }

    // Both callers restrict the target to an atomic node. Build a candidate
    // before checkpointing so existing structural/node/depth limits reject
    // impossible replacements atomically, without changing the original redo.
    pub(super) fn replace_atomic_control(
        &mut self,
        target: NodeId,
        template: PortableTree,
    ) -> bool {
        let added = match template.kind {
            NodeKind::Break | NodeKind::Continue => 0,
            NodeKind::Return => 1,
            NodeKind::While | NodeKind::If { .. } => 3,
            NodeKind::For | NodeKind::Try => 4,
            _ => unreachable!("private control template kind"),
        };
        if self.arena.nodes().count() + added > MAX_TREE_NODES {
            return false;
        }
        let mut candidate = self.clone_without_history();
        let children: Vec<_> = template
            .children
            .iter()
            .map(|child| import_node(&mut candidate.arena, child))
            .collect();
        for child in &children {
            candidate.arena.node_mut(*child).parent = Some(target);
        }
        let node = candidate.arena.node_mut(target);
        node.kind = template.kind;
        node.children = children;
        let leaf = first_leaf(&candidate.arena, target);
        candidate.caret = Caret::at(
            leaf,
            if leaf == target {
                Direction::Right
            } else {
                Direction::Indeterminate
            },
        );
        candidate.selection = Selection::None;
        candidate.scope = None;
        if candidate.to_portable().is_err() {
            return false;
        }
        self.checkpoint();
        self.arena = candidate.arena;
        self.caret = candidate.caret;
        self.selection = Selection::None;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atom(kind: NodeKind) -> PortableTree {
        PortableTree {
            kind,
            children: vec![],
        }
    }
    fn word(name: &str) -> PortableTree {
        atom(NodeKind::Identifier(name.into()))
    }

    #[test]
    fn if_keyword_obeys_original_label_formatted_value_guards_and_restores_metadata_in_one_undo() {
        for source in [
            word("if"),
            atom(NodeKind::AnnotatedIdentifier {
                name: "if".into(),
                labels: "*".into(),
            }),
            atom(NodeKind::NativeIdentifier {
                name: "if".into(),
                labels: "UNKNOWN_RESOLVED".into(),
                contextual: Some(true),
                subscript: None,
            }),
            atom(NodeKind::NativeIdentifier {
                name: "if".into(),
                labels: "NONE".into(),
                contextual: Some(false),
                subscript: None,
            }),
        ] {
            let mut editor = Editor::from_portable(&source).unwrap();
            editor.set_expressions_only(true);
            let id = editor.root();
            editor.place_text_selection(id, 0, 1).unwrap();
            let caret = editor.caret();
            assert!(editor.convert_program_keyword());
            let converted = editor.to_portable().unwrap();
            assert_eq!(
                converted.kind,
                NodeKind::If {
                    has_else: false,
                    branch_count: 1,
                    alternate: false
                }
            );
            assert_eq!(converted.children.len(), 2);
            assert_eq!(converted.children[0], placeholder());
            assert_eq!(
                converted.children[1].kind,
                NodeKind::Program { statement_count: 1 }
            );
            assert_eq!(editor.root(), id);
            assert_eq!(
                editor.caret().node,
                editor.arena().get(id).unwrap().children()[0]
            );
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), source);
            assert_eq!(editor.caret(), caret);
            assert!(!editor.undo());
            editor.select_subtree(id).unwrap();
            assert!(!editor.convert_program_keyword());
            assert!(editor.redo());
            assert_eq!(editor.to_portable().unwrap(), converted);
        }
        for source in [
            word("IF"),
            word("if "),
            word("elseif"),
            word("alsoif"),
            word("else"),
            atom(NodeKind::AnnotatedIdentifier {
                name: "if".into(),
                labels: "VARIABLE".into(),
            }),
            atom(NodeKind::AnnotatedIdentifier {
                name: "if".into(),
                labels: "KEYWORD".into(),
            }),
            atom(NodeKind::NativeIdentifier {
                name: "if".into(),
                labels: "*".into(),
                contextual: Some(true),
                subscript: Some(String::new()),
            }),
        ] {
            let mut editor = Editor::from_portable(&source).unwrap();
            let caret = editor.caret();
            let revision = editor.arena().revision();
            assert!(!editor.convert_program_keyword());
            assert_eq!(editor.to_portable().unwrap(), source);
            assert_eq!(editor.caret(), caret);
            assert_eq!(editor.arena().revision(), revision);
            assert!(!editor.undo());
        }
    }

    #[test]
    fn if_keyword_three_added_nodes_fit_the_exact_budget_and_overflow_preserves_redo() {
        for first_groups in [0, 1] {
            let mut cells = Vec::new();
            for i in 0..4096 {
                let mut cell = word(if i == 0 { "if" } else { "x" });
                let groups = if i == 0 {
                    first_groups
                } else if i == 4095 {
                    2
                } else {
                    3
                };
                for _ in 0..groups {
                    cell = PortableTree {
                        kind: NodeKind::Group,
                        children: vec![cell],
                    };
                }
                cells.push(cell);
            }
            let source = PortableTree {
                kind: NodeKind::Matrix {
                    rows: 64,
                    cols: 64,
                    display: None,
                },
                children: cells,
            };
            let mut editor = Editor::from_portable(&source).unwrap();
            let before_nodes = editor.arena().nodes().count();
            assert_eq!(before_nodes, MAX_TREE_NODES - 3 + first_groups);
            let caret = editor.caret();
            let revision = editor.arena().revision();
            assert_eq!(editor.convert_program_keyword(), first_groups == 0);
            if first_groups == 0 {
                assert_eq!(editor.arena().nodes().count(), MAX_TREE_NODES);
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

    #[test]
    fn break_and_continue_are_max_precedence_zero_arity_tokens_without_an_authoring_ancestry_gate()
    {
        for (kind, descriptor, insert) in [
            (
                NodeKind::Break,
                "BREAK",
                Editor::insert_break as fn(&mut Editor) -> bool,
            ),
            (NodeKind::Continue, "CONTINUE", Editor::insert_continue),
        ] {
            assert_eq!(kind.precedence(), i32::MAX);
            assert_eq!(kind.arity(), 0);
            assert_eq!(kind.descriptor_name(), descriptor);
            assert_eq!(kind.text(), None);
            let mut editor = Editor::new();
            editor.set_expressions_only(true);
            let original = editor.to_portable().unwrap();
            let id = editor.root();
            assert!(insert(&mut editor));
            let result = atom(kind);
            assert_eq!(editor.root(), id);
            assert_eq!(editor.to_portable().unwrap(), result);
            assert_eq!(editor.caret(), Caret::at(id, Direction::Right));
            let json = serde_json::to_string(&result).unwrap();
            assert_eq!(serde_json::from_str::<PortableTree>(&json).unwrap(), result);
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), original);
            assert!(!editor.undo());
            assert!(editor.redo());
            assert_eq!(editor.to_portable().unwrap(), result);
        }
    }

    #[test]
    fn confirmed_keyword_templates_replace_the_word_and_focus_the_first_operand_as_one_undo() {
        for (name, kind, added) in [
            (
                "if",
                NodeKind::If {
                    has_else: false,
                    branch_count: 1,
                    alternate: false,
                },
                3,
            ),
            ("while", NodeKind::While, 3),
            ("for", NodeKind::For, 4),
            ("return", NodeKind::Return, 1),
            ("break", NodeKind::Break, 0),
            ("continue", NodeKind::Continue, 0),
            ("try", NodeKind::Try, 4),
        ] {
            let source = word(name);
            let mut editor = Editor::from_portable(&source).unwrap();
            let id = editor.root();
            editor.place_text_selection(id, 1, 2).unwrap();
            let caret = editor.caret();
            assert!(editor.convert_program_keyword());
            let result = editor.to_portable().unwrap();
            assert_eq!(result, control_template(kind));
            assert_eq!(editor.arena().nodes().count(), 1 + added);
            assert_eq!(editor.root(), id);
            assert_eq!(editor.selection(), Selection::None);
            assert_eq!(editor.caret().node, first_leaf(editor.arena(), id));
            if added == 0 {
                assert_eq!(editor.caret().caret_side, Direction::Right)
            } else {
                assert_eq!(
                    editor.arena().get(editor.caret().node).unwrap().kind(),
                    &NodeKind::Placeholder
                )
            }
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), source);
            assert_eq!(editor.caret(), caret);
            assert!(!editor.undo());
            assert!(editor.redo());
            assert_eq!(editor.to_portable().unwrap(), result);
        }
    }

    #[test]
    fn keyword_conversion_uses_has_no_label_and_whole_formatted_case_sensitive_value() {
        for source in [
            word("break"),
            atom(NodeKind::AnnotatedIdentifier {
                name: "break".into(),
                labels: "*".into(),
            }),
            atom(NodeKind::AnnotatedIdentifier {
                name: "break".into(),
                labels: "NONE".into(),
            }),
            atom(NodeKind::NativeIdentifier {
                name: "break".into(),
                labels: "UNIT".into(),
                contextual: Some(true),
                subscript: None,
            }),
            atom(NodeKind::NativeIdentifier {
                name: "break".into(),
                labels: "*".into(),
                contextual: Some(false),
                subscript: None,
            }),
        ] {
            let mut editor = Editor::from_portable(&source).unwrap();
            editor.set_expressions_only(true);
            assert!(editor.convert_program_keyword());
            assert_eq!(editor.to_portable().unwrap(), atom(NodeKind::Break));
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), source);
        }
        for source in [
            word("Break"),
            word("break "),
            word("whilex"),
            word("else"),
            atom(NodeKind::AnnotatedIdentifier {
                name: "break".into(),
                labels: "VARIABLE".into(),
            }),
            atom(NodeKind::AnnotatedIdentifier {
                name: "break".into(),
                labels: "KEYWORD".into(),
            }),
            atom(NodeKind::NativeIdentifier {
                name: "break".into(),
                labels: "VARIABLE".into(),
                contextual: Some(false),
                subscript: None,
            }),
            atom(NodeKind::NativeIdentifier {
                name: "break".into(),
                labels: "*".into(),
                contextual: Some(true),
                subscript: Some(String::new()),
            }),
            atom(NodeKind::NativeIdentifier {
                name: "break".into(),
                labels: "*".into(),
                contextual: None,
                subscript: Some("index".into()),
            }),
            atom(NodeKind::Number("3".into())),
        ] {
            let mut editor = Editor::from_portable(&source).unwrap();
            let revision = editor.arena().revision();
            let caret = editor.caret();
            assert!(!editor.convert_program_keyword());
            assert_eq!(editor.to_portable().unwrap(), source);
            assert_eq!(editor.arena().revision(), revision);
            assert_eq!(editor.caret(), caret);
            assert!(!editor.undo());
        }
    }

    #[test]
    fn structural_scope_and_matrix_typeover_are_not_normal_keyword_mode_and_preserve_redo() {
        let source = word("break");
        let mut editor = Editor::from_portable(&source).unwrap();
        assert!(editor.convert_program_keyword());
        assert!(editor.undo());
        editor.select_subtree(editor.root()).unwrap();
        let revision = editor.arena().revision();
        let caret = editor.caret();
        let selection = editor.selection();
        assert!(!editor.convert_program_keyword());
        assert_eq!(editor.arena().revision(), revision);
        assert_eq!(editor.caret(), caret);
        assert_eq!(editor.selection(), selection);
        assert!(editor.redo());
        assert_eq!(editor.to_portable().unwrap(), atom(NodeKind::Break));
        let source = PortableTree {
            kind: NodeKind::Matrix {
                rows: 1,
                cols: 2,
                display: None,
            },
            children: vec![word("break"), word("continue")],
        };
        let mut editor = Editor::from_portable(&source).unwrap();
        assert!(editor.select_matrix_cells(editor.root(), 0, 1));
        let revision = editor.arena().revision();
        assert!(!editor.convert_program_keyword());
        assert!(!editor.insert_break());
        assert!(!editor.insert_continue());
        assert_eq!(editor.arena().revision(), revision);
        assert_eq!(editor.to_portable().unwrap(), source);
        assert!(!editor.undo());
    }

    #[test]
    fn structural_constraints_and_tree_depth_are_checked_before_mutating_keyword_history() {
        let source = PortableTree {
            kind: NodeKind::For,
            children: vec![
                word("while"),
                placeholder(),
                PortableTree {
                    kind: NodeKind::Program { statement_count: 1 },
                    children: vec![placeholder()],
                },
            ],
        };
        let mut editor = Editor::from_portable(&source).unwrap();
        let revision = editor.arena().revision();
        assert!(!editor.convert_program_keyword());
        assert_eq!(editor.arena().revision(), revision);
        assert_eq!(editor.to_portable().unwrap(), source);
        assert!(!editor.undo());
        let mut source = word("while");
        for _ in 1..MAX_TREE_DEPTH {
            source = PortableTree {
                kind: NodeKind::Group,
                children: vec![source],
            };
        }
        let mut editor = Editor::from_portable(&source).unwrap();
        let revision = editor.arena().revision();
        assert!(!editor.convert_program_keyword());
        assert_eq!(editor.arena().revision(), revision);
        assert_eq!(editor.to_portable().unwrap(), source);
        assert!(!editor.undo());
    }

    #[test]
    fn zero_arity_conversion_fits_at_the_live_node_limit_but_growing_templates_do_not() {
        let mut cells = Vec::new();
        for i in 0..4096 {
            let mut cell = word(if i == 0 { "while" } else { "break" });
            for _ in 0..if i == 4095 { 2 } else { 3 } {
                cell = PortableTree {
                    kind: NodeKind::Group,
                    children: vec![cell],
                };
            }
            cells.push(cell);
        }
        let source = PortableTree {
            kind: NodeKind::Matrix {
                rows: 64,
                cols: 64,
                display: None,
            },
            children: cells,
        };
        let mut editor = Editor::from_portable(&source).unwrap();
        assert_eq!(editor.arena().nodes().count(), MAX_TREE_NODES);
        let revision = editor.arena().revision();
        assert!(!editor.convert_program_keyword());
        assert_eq!(editor.arena().revision(), revision);
        assert!(!editor.undo());
        let keyword = editor
            .arena()
            .nodes()
            .find_map(|(id, node)| {
                (node.kind() == &NodeKind::Identifier("break".into())).then_some(id)
            })
            .unwrap();
        editor.place_caret(keyword, Direction::Right).unwrap();
        assert!(editor.convert_program_keyword());
        assert_eq!(editor.arena().nodes().count(), MAX_TREE_NODES);
        assert_eq!(
            editor.arena().get(keyword).unwrap().kind(),
            &NodeKind::Break
        );
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), source);
    }

    #[test]
    fn confirmed_program_precedences_do_not_guess_values_for_unrelated_structures() {
        for kind in [
            NodeKind::Program { statement_count: 1 },
            NodeKind::If {
                has_else: false,
                branch_count: 1,
                alternate: false,
            },
            NodeKind::If {
                has_else: true,
                branch_count: 1,
                alternate: false,
            },
            NodeKind::While,
            NodeKind::For,
        ] {
            assert_eq!(kind.precedence(), 290);
        }
        assert_eq!(NodeKind::Return.precedence(), 40);
        assert_eq!(NodeKind::For.descriptor_name(), "FOR_LOOP");
        assert_eq!(NodeKind::While.descriptor_name(), "WHILE_LOOP");
        assert_eq!(NodeKind::Break.precedence(), i32::MAX);
        assert_eq!(NodeKind::Continue.precedence(), i32::MAX);
        assert_eq!(NodeKind::LocalDefine.precedence(), 41);
        assert!(NodeKind::Return.precedence() < NodeKind::LocalDefine.precedence());
        assert_eq!(NodeKind::Integral.precedence(), 0);
    }

    #[test]
    fn typed_arithmetic_stays_inside_program_and_control_operands_after_precedence_corrections() {
        let value = || atom(NodeKind::Number("7".into()));
        let body = || PortableTree {
            kind: NodeKind::Program { statement_count: 1 },
            children: vec![value()],
        };
        let condition = || atom(NodeKind::Number("1".into()));
        let sources = [
            body(),
            PortableTree {
                kind: NodeKind::While,
                children: vec![condition(), body()],
            },
            PortableTree {
                kind: NodeKind::For,
                children: vec![word("i"), condition(), body()],
            },
            PortableTree {
                kind: NodeKind::Return,
                children: vec![value()],
            },
            PortableTree {
                kind: NodeKind::LocalDefine,
                children: vec![word("local"), value()],
            },
            PortableTree {
                kind: NodeKind::Return,
                children: vec![PortableTree {
                    kind: NodeKind::LocalDefine,
                    children: vec![word("local"), value()],
                }],
            },
            PortableTree {
                kind: NodeKind::If {
                    has_else: false,
                    branch_count: 1,
                    alternate: false,
                },
                children: vec![condition(), body()],
            },
        ];
        fn add_to_value(tree: &mut PortableTree) {
            if tree.kind == NodeKind::Number("7".into()) {
                let old = tree.clone();
                *tree = PortableTree {
                    kind: NodeKind::Binary(BinaryOp::Add),
                    children: vec![old, placeholder()],
                };
            } else {
                for child in &mut tree.children {
                    add_to_value(child);
                }
            }
        }
        for source in sources {
            let mut editor = Editor::from_portable(&source).unwrap();
            let target = editor
                .arena()
                .nodes()
                .find_map(|(id, node)| (node.kind() == &NodeKind::Number("7".into())).then_some(id))
                .unwrap();
            editor.place_caret(target, Direction::Right).unwrap();
            editor.insert_infix(BinaryOp::Add);
            let mut expected = source.clone();
            add_to_value(&mut expected);
            assert_eq!(editor.to_portable().unwrap(), expected);
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), source);
            assert!(!editor.undo());
        }
    }

    #[test]
    fn inward_atomic_deletion_finishes_as_one_command_with_placeholder_left_caret() {
        for kind in [NodeKind::Break, NodeKind::Continue] {
            for (side, backwards) in [(Direction::Right, true), (Direction::Left, false)] {
                let source = atom(kind.clone());
                let mut editor = Editor::from_portable(&source).unwrap();
                let id = editor.root();
                editor.place_caret(id, side).unwrap();
                let before_caret = editor.caret();
                // Return from a structural scope before the normal-mode
                // deletion; the delete must finish without scope/selection.
                assert!(editor.expand_scope());
                assert!(editor.contract_scope());
                assert!(editor.scope.is_none());
                assert_eq!(editor.selection(), Selection::None);
                assert!(editor.delete_structure(backwards));
                assert_eq!(editor.root(), id);
                assert_eq!(editor.to_portable().unwrap(), placeholder());
                assert_eq!(editor.caret(), Caret::at(id, Direction::Left));
                assert_eq!(editor.selection(), Selection::None);
                assert!(editor.scope.is_none());
                assert_eq!(editor.arena().nodes().count(), 1);
                assert!(editor.undo());
                assert_eq!(editor.to_portable().unwrap(), source);
                assert_eq!(editor.caret(), before_caret);
                assert!(!editor.undo());
                assert!(editor.redo());
                assert_eq!(editor.to_portable().unwrap(), placeholder());
                assert_eq!(editor.caret(), Caret::at(id, Direction::Left));
            }
        }
    }

    #[test]
    fn inward_control_deletion_keeps_nested_final_program_slots_and_populated_siblings() {
        fn replace_token(tree: &mut PortableTree) {
            if matches!(tree.kind, NodeKind::Break | NodeKind::Continue) {
                *tree = placeholder();
            } else {
                for child in &mut tree.children {
                    replace_token(child);
                }
            }
        }
        for kind in [NodeKind::Break, NodeKind::Continue] {
            let token = || atom(kind.clone());
            let sources = [
                PortableTree {
                    kind: NodeKind::Program { statement_count: 1 },
                    children: vec![token()],
                },
                PortableTree {
                    kind: NodeKind::Program { statement_count: 3 },
                    children: vec![word("before"), token(), word("after")],
                },
                PortableTree {
                    kind: NodeKind::While,
                    children: vec![
                        word("condition"),
                        PortableTree {
                            kind: NodeKind::Program { statement_count: 1 },
                            children: vec![token()],
                        },
                    ],
                },
            ];
            for source in sources {
                for (side, backwards) in [(Direction::Right, true), (Direction::Left, false)] {
                    let mut editor = Editor::from_portable(&source).unwrap();
                    let id = editor
                        .arena()
                        .nodes()
                        .find_map(|(id, node)| (node.kind() == &kind).then_some(id))
                        .unwrap();
                    let parent = editor.arena().get(id).unwrap().parent();
                    let live_nodes = editor.arena().nodes().count();
                    editor.place_caret(id, side).unwrap();
                    assert!(editor.delete_structure(backwards));
                    let mut expected = source.clone();
                    replace_token(&mut expected);
                    assert_eq!(editor.to_portable().unwrap(), expected);
                    assert_eq!(editor.arena().get(id).unwrap().parent(), parent);
                    assert_eq!(editor.arena().nodes().count(), live_nodes);
                    assert_eq!(editor.caret(), Caret::at(id, Direction::Left));
                    assert!(editor.undo());
                    assert_eq!(editor.to_portable().unwrap(), source);
                    assert!(!editor.undo());
                    assert!(editor.redo());
                    assert_eq!(editor.to_portable().unwrap(), expected);
                }
            }
        }
    }

    #[test]
    fn outward_control_deletion_preserves_redo_and_explicit_selection_uses_its_existing_path() {
        for kind in [NodeKind::Break, NodeKind::Continue] {
            for (side, backwards) in [(Direction::Left, true), (Direction::Right, false)] {
                let source = atom(kind.clone());
                let mut editor = Editor::from_portable(&source).unwrap();
                editor.place_caret(editor.root(), Direction::Right).unwrap();
                assert!(editor.delete_structure(true));
                assert!(editor.undo());
                editor.place_caret(editor.root(), side).unwrap();
                let caret = editor.caret();
                let revision = editor.arena().revision();
                assert!(!editor.delete_structure(backwards));
                assert_eq!(editor.to_portable().unwrap(), source);
                assert_eq!(editor.arena().revision(), revision);
                assert_eq!(editor.caret(), caret);
                assert!(editor.redo());
                assert_eq!(editor.to_portable().unwrap(), placeholder());
                let mut editor = Editor::from_portable(&source).unwrap();
                let root = editor.root();
                editor.select_subtree(root).unwrap();
                assert!(editor.delete_structure(backwards));
                assert_eq!(editor.to_portable().unwrap(), placeholder());
                assert_eq!(editor.caret(), Caret::at(root, Direction::Indeterminate));
                assert!(editor.undo());
                assert_eq!(editor.to_portable().unwrap(), source);
                assert_eq!(editor.selection(), Selection::Subtree(root));
                assert!(!editor.undo());
                assert!(editor.redo());
                assert_eq!(editor.to_portable().unwrap(), placeholder());
                assert_eq!(editor.caret(), Caret::at(root, Direction::Indeterminate));
            }
        }
    }
}
