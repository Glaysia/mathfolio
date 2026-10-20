//! TRY header navigation, confirmed structural deletion and contract regressions.
//! Shared atomic insertion/conversion lives
//! in program_control so label, formatted-value, budget and history guards do
//! not diverge. Original source contract: docs/analysis/0103-try-control-contract.md
//! (TRY has two BODY children in try/on-error order, not an IF or onerror alias).
//! Both BODY slots are inherent. Program statements are not inherently empty
//! containers; generic outer-parent repair remains outside this bounded subset.
use super::*;

impl Editor {
    /// Select a TRY header word using its original side coordinate: Left is
    /// `try`, Right is `on error`. These are not IF-style operator-part indices.
    /// This is navigation only and preserves the document's Undo/Redo history.
    pub fn place_try_operator(&mut self, node: NodeId, side: Direction) -> Result<(), TreeError> {
        if !matches!(side, Direction::Left | Direction::Right) {
            return Err(TreeError::InvalidTree(
                "try header side must be left or right",
            ));
        }
        if self
            .arena
            .get(node)
            .is_none_or(|entry| entry.kind != NodeKind::Try)
        {
            return Err(TreeError::InvalidTree("header target is not a try"));
        }
        self.typeover = None;
        self.caret = Caret::at(node, Direction::Indeterminate);
        self.caret.node_side = side;
        self.caret.operator_part = None;
        self.selection = Selection::None;
        self.scope = None;
        Ok(())
    }

    // E ap.aa 0x06000440: return Some for the represented TRY/BODY states so
    // generic operand deletion cannot unwrap a populated TRY at an outer edge.
    // Explicit subtree deletion remains on its existing independent path.
    pub(super) fn delete_try_structure(&mut self, backwards: bool) -> Option<bool> {
        if self.selection != Selection::None {
            return None;
        }
        let current = self.caret.node;
        let node = self.arena.get(current).unwrap();
        if node.kind == NodeKind::Placeholder {
            return self.delete_fresh_try_placeholder(backwards);
        }
        let pair = if node.kind == NodeKind::Try {
            current
        } else if let Some(parent) = node.parent
            && self.arena.get(parent).unwrap().kind == NodeKind::Try
        {
            parent
        } else {
            return None;
        };
        let direction = if backwards {
            Direction::Left
        } else {
            Direction::Right
        };
        if self.caret.caret_side == direction {
            return Some(false); // outward paths belong to other original handlers
        }
        let bodies = self.arena.get(pair).unwrap().children.clone();
        // BODY children are not themselves inherent. Only direct placeholders
        // are empty; a nested Program([PH]) must not be recursively erased.
        let empty = bodies.iter().all(|body| {
            self.arena
                .get(*body)
                .unwrap()
                .children
                .iter()
                .all(|child| self.arena.get(*child).unwrap().kind == NodeKind::Placeholder)
        });
        if current != pair {
            if !empty
                || node.children.len() != 1
                || self.arena.get(node.children[0]).unwrap().kind != NodeKind::Placeholder
            {
                return Some(false);
            }
        } else if self.caret.caret_side != Direction::Indeterminate && !empty {
            return Some(false);
        }

        let mut candidate = self.clone_without_history();
        if empty && (current != pair || self.caret.caret_side != Direction::Indeterminate) {
            for body in &bodies {
                candidate.retire(*body, None);
            }
            let node = candidate.arena.node_mut(pair);
            node.kind = NodeKind::Placeholder;
            node.children.clear();
            candidate.caret = Caret::at(pair, Direction::Left);
        } else {
            // Both BODY slots are inherent. The original center deletion merges
            // them in order, including two empty bodies; it is not leaf deletion.
            let attempt = bodies[0];
            let handler = bodies[1];
            let attempt_children = candidate.arena.get(attempt).unwrap().children.clone();
            let handler_children = candidate.arena.get(handler).unwrap().children.clone();
            let parent = candidate.arena.get(pair).unwrap().parent;
            candidate
                .arena
                .node_mut(attempt)
                .children
                .extend(&handler_children);
            for child in &handler_children {
                candidate.arena.node_mut(*child).parent = Some(attempt);
            }
            let count = attempt_children.len() + handler_children.len();
            candidate.arena.node_mut(attempt).kind = NodeKind::Program {
                statement_count: count,
            };
            candidate.arena.node_mut(handler).children.clear();
            candidate.replace_child(parent, pair, attempt);
            candidate.arena.node_mut(attempt).parent = parent;
            candidate.retire(pair, Some(attempt));
            candidate.caret = if self.caret.node_side == Direction::Right {
                if backwards {
                    Caret::at(*attempt_children.last().unwrap(), Direction::Right)
                } else {
                    Caret::at(handler_children[0], Direction::Left)
                }
            } else {
                Caret::at(attempt, Direction::Left)
            };
            // Previous-Program merging precedes the header's own join focus.
            match candidate.merge_previous_try_program(attempt, backwards) {
                Ok(Some(caret)) => candidate.caret = caret,
                Ok(None) => {}
                Err(()) => return Some(false),
            }
        }
        Some(self.commit_try_deletion(candidate))
    }

    // E stage21 ap.t (0447) navigates a sole placeholder through bq's whole
    // equation list before stage29 ap.aa (0440) handles the resulting BODY or
    // TRY state. This is one delete command, not a required user arrow press.
    fn delete_fresh_try_placeholder(&mut self, backwards: bool) -> Option<bool> {
        if self.caret.caret_side != Direction::Indeterminate {
            return None;
        }
        let current = self.caret.node;
        let body = self.arena.get(current)?.parent?;
        let body_node = self.arena.get(body)?;
        if !matches!(body_node.kind, NodeKind::Program { .. })
            || body_node.children.as_slice() != [current]
        {
            return None;
        }
        let pair = body_node.parent?;
        let node = self.arena.get(pair)?;
        if node.kind != NodeKind::Try {
            return None;
        }
        let part = node.children.iter().position(|child| *child == body)?;
        if !node.children.iter().all(|body| {
            let children = &self.arena.get(*body).unwrap().children;
            children.len() == 1
                && self.arena.get(children[0]).unwrap().kind == NodeKind::Placeholder
        }) || (!backwards && part == 1)
        {
            return None;
        }
        let mut candidate = self.clone_without_history();
        if backwards {
            // Each BODY's sole PH is preceded by that BODY's operator center.
            candidate.caret = Caret::at(body, Direction::Indeterminate);
        } else {
            // The attempt's PH is followed by the last TRY part, the on-error
            // header. Native NavigatorState(1316) maps that part to NodeSideRight.
            candidate.caret = Caret::at(pair, Direction::Indeterminate);
            candidate.caret.node_side = Direction::Right;
            candidate.caret.operator_part = Some(1);
        }
        if candidate.delete_try_structure(backwards) != Some(true) {
            return Some(false);
        }
        Some(self.commit_try_deletion(candidate))
    }

    fn commit_try_deletion(&mut self, mut candidate: Editor) -> bool {
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

    fn merge_previous_try_program(
        &mut self,
        merged: NodeId,
        backwards: bool,
    ) -> Result<Option<Caret>, ()> {
        let Some(parent) = self.arena.get(merged).unwrap().parent else {
            return Ok(None);
        };
        if !matches!(
            self.arena.get(parent).unwrap().kind,
            NodeKind::Program { .. }
        ) {
            return Ok(None);
        }
        let siblings = &self.arena.get(parent).unwrap().children;
        let index = siblings.iter().position(|id| *id == merged).unwrap();
        if index == 0 {
            return Ok(None);
        }
        let previous = siblings[index - 1];
        let previous_node = self.arena.get(previous).unwrap();
        let destination = if matches!(previous_node.kind, NodeKind::Program { .. }) {
            previous
        } else if let Some(last) = previous_node.children.last()
            && matches!(
                self.arena.get(*last).unwrap().kind,
                NodeKind::Program { .. }
            )
        {
            // These last BODY slots have confirmed inherent positions. Other
            // descriptor families require their own mapping before enabling a
            // whole edit whose original previous-Program repair might differ.
            if matches!(previous_node.kind, NodeKind::Return) {
                return Ok(None); // Return has no inherent children.
            }
            if !matches!(
                previous_node.kind,
                NodeKind::For | NodeKind::While | NodeKind::If { .. } | NodeKind::Try
            ) {
                return Err(());
            }
            *last
        } else {
            return Ok(None);
        };
        let original = self.arena.get(destination).unwrap().children.clone();
        let appended = self.arena.get(merged).unwrap().children.clone();
        self.arena.node_mut(destination).children.extend(&appended);
        self.arena.node_mut(destination).kind = NodeKind::Program {
            statement_count: original.len() + appended.len(),
        };
        for child in &appended {
            self.arena.node_mut(*child).parent = Some(destination);
        }
        self.arena.node_mut(merged).children.clear();
        let parent_node = self.arena.node_mut(parent);
        parent_node.children.remove(index);
        parent_node.kind = NodeKind::Program {
            statement_count: parent_node.children.len(),
        };
        self.retire(merged, None);
        Ok(Some(if backwards {
            Caret::at(*original.last().unwrap(), Direction::Right)
        } else {
            Caret::at(appended[0], Direction::Left)
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;

    fn tree(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
        PortableTree { kind, children }
    }

    fn atom(kind: NodeKind) -> PortableTree {
        tree(kind, vec![])
    }

    fn body(contents: PortableTree) -> PortableTree {
        tree(NodeKind::Program { statement_count: 1 }, vec![contents])
    }

    fn empty_try() -> PortableTree {
        tree(
            NodeKind::Try,
            vec![
                body(atom(NodeKind::Placeholder)),
                body(atom(NodeKind::Placeholder)),
            ],
        )
    }

    #[test]
    fn try_identity_order_metadata_and_nested_body_roundtrip_are_preserved() {
        assert_eq!(NodeKind::Try.arity(), 2);
        assert_eq!(NodeKind::Try.precedence(), 290);
        assert_eq!(NodeKind::Try.descriptor_name(), "TRY");
        let source = tree(
            NodeKind::Try,
            vec![
                body(atom(NodeKind::NativeIdentifier {
                    name: "attempt".into(),
                    labels: "VARIABLE".into(),
                    contextual: None,
                    subscript: Some("normal".into()),
                })),
                body(body(atom(NodeKind::AnnotatedIdentifier {
                    name: "recovery".into(),
                    labels: "VARIABLE".into(),
                }))),
            ],
        );
        let encoded = serde_json::to_string(&source).unwrap();
        let recovered: PortableTree = serde_json::from_str(&encoded).unwrap();
        assert_eq!(recovered, source);
        let mut editor = Editor::from_portable(&source).unwrap();
        assert_eq!(editor.to_portable().unwrap(), source);
        let recovery_body = editor.arena().get(editor.root()).unwrap().children()[1];
        editor.place_caret(recovery_body, Direction::Left).unwrap();
        assert_eq!(
            editor.export_subtree(recovery_body).unwrap(),
            source.children[1]
        );
        assert!(
            matches!(
                source.children[1].children[0].kind,
                NodeKind::Program { .. }
            ),
            "nested programs must not be flattened"
        );
    }

    #[test]
    fn try_header_navigation_uses_sides_without_if_parts_and_preserves_history() {
        let mut editor = Editor::new();
        let original = editor.to_portable().unwrap();
        let root = editor.root();
        assert!(editor.insert_try());
        let attempt = editor.arena().get(root).unwrap().children()[0];
        for side in [Direction::Left, Direction::Right] {
            let origin = editor.caret();
            editor.selection = Selection::Subtree(root);
            editor.scope = Some(ScopeState {
                origin,
                direction: Direction::Right,
                history: vec![(origin, Selection::None)],
            });
            for (node, invalid_side) in [
                (root, Direction::Indeterminate),
                (attempt, side),
                (NodeId(usize::MAX), side),
            ] {
                assert!(editor.place_try_operator(node, invalid_side).is_err());
                assert_eq!(editor.caret(), origin);
                assert_eq!(editor.selection(), Selection::Subtree(root));
                assert!(editor.scope.is_some());
            }
            let revision = editor.arena().revision();
            editor.place_try_operator(root, side).unwrap();
            assert_eq!(editor.caret().node, root);
            assert_eq!(editor.caret().caret_side, Direction::Indeterminate);
            assert_eq!(editor.caret().node_side, side);
            assert_eq!(editor.caret().operator_part, None);
            assert_eq!(editor.selection(), Selection::None);
            assert!(editor.scope.is_none());
            assert_eq!(editor.arena().revision(), revision);
        }
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), original);
        assert!(
            !editor.undo(),
            "navigation must not introduce document history"
        );
        assert!(editor.redo());
        assert_eq!(editor.to_portable().unwrap(), empty_try());
        let attempt = editor.arena().get(root).unwrap().children()[0];
        let blank = editor.arena().get(attempt).unwrap().children()[0];
        editor.place_caret(blank, Direction::Right).unwrap();
        assert!(editor.insert_program_line());
        assert!(editor.undo());
        editor.place_try_operator(root, Direction::Right).unwrap();
        assert!(
            editor.redo(),
            "header navigation must retain a pending edit redo"
        );
        assert_eq!(editor.arena().get(attempt).unwrap().children().len(), 2);
        assert!(editor.undo());
        editor.place_try_operator(root, Direction::Left).unwrap();
        assert!(editor.undo());
        assert!(
            editor.redo(),
            "navigation must preserve existing redo across an undo"
        );
    }

    #[test]
    fn try_validation_requires_two_program_bodies_in_the_canonical_supported_subset() {
        let valid = empty_try();
        valid.validate().unwrap();
        for children in [
            vec![],
            vec![valid.children[0].clone()],
            vec![valid.children[0].clone(); 3],
        ] {
            assert!(matches!(
                tree(NodeKind::Try, children).validate(),
                Err(TreeError::Arity { .. })
            ));
        }
        for slot in [0, 1] {
            let mut invalid = valid.clone();
            invalid.children[slot] = atom(NodeKind::Placeholder);
            assert!(matches!(invalid.validate(), Err(TreeError::InvalidTree(_))));
        }
    }

    #[test]
    fn blank_try_insertion_has_no_program_ancestry_gate_and_is_one_undo_with_original_scope() {
        let mut editor = Editor::new();
        editor.set_expressions_only(true);
        let source = editor.to_portable().unwrap();
        let id = editor.root();
        let origin = editor.caret();
        editor.scope = Some(ScopeState {
            origin,
            direction: Direction::Right,
            history: vec![(origin, Selection::None)],
        });
        assert!(editor.insert_try());
        assert_eq!(editor.root(), id);
        assert_eq!(editor.to_portable().unwrap(), empty_try());
        let try_body = editor.arena().get(id).unwrap().children()[0];
        assert_eq!(
            editor.caret().node,
            editor.arena().get(try_body).unwrap().children()[0]
        );
        assert_eq!(editor.caret().caret_side, Direction::Indeterminate);
        assert!(editor.scope.is_none());
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), source);
        assert_eq!(editor.caret(), origin);
        assert_eq!(editor.scope.as_ref().unwrap().history.len(), 1);
        assert!(!editor.undo());
        assert!(editor.redo());
        editor.place_caret(id, Direction::Right).unwrap();
        assert!(
            !editor.insert_try(),
            "populated insertion is a separate unresolved policy"
        );
        assert_eq!(editor.to_portable().unwrap(), empty_try());
    }

    #[test]
    fn try_keyword_uses_shared_whole_identifier_label_suffix_and_scope_guards() {
        for keyword in [
            NodeKind::Identifier("try".into()),
            NodeKind::AnnotatedIdentifier {
                name: "try".into(),
                labels: "NONE".into(),
            },
            NodeKind::NativeIdentifier {
                name: "try".into(),
                labels: "UNKNOWN_RESOLVED".into(),
                contextual: Some(true),
                subscript: None,
            },
        ] {
            let source = atom(keyword);
            let mut editor = Editor::from_portable(&source).unwrap();
            editor.set_expressions_only(true);
            let id = editor.root();
            editor.place_text_selection(id, 1, 2).unwrap();
            let caret = editor.caret();
            assert!(editor.convert_program_keyword());
            assert_eq!(editor.to_portable().unwrap(), empty_try());
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), source);
            assert_eq!(editor.caret(), caret);
            assert!(!editor.undo());
            editor.select_subtree(id).unwrap();
            assert!(!editor.convert_program_keyword());
            assert!(
                editor.redo(),
                "scope rejection must keep the undone conversion"
            );
        }
        for keyword in [
            NodeKind::Identifier("Try".into()),
            NodeKind::Identifier("try ".into()),
            NodeKind::Identifier("onerror".into()),
            NodeKind::AnnotatedIdentifier {
                name: "try".into(),
                labels: "KEYWORD".into(),
            },
            NodeKind::NativeIdentifier {
                name: "try".into(),
                labels: "VARIABLE".into(),
                contextual: Some(false),
                subscript: None,
            },
            NodeKind::NativeIdentifier {
                name: "try".into(),
                labels: "*".into(),
                contextual: Some(true),
                subscript: Some(String::new()),
            },
        ] {
            let source = atom(keyword);
            let mut editor = Editor::from_portable(&source).unwrap();
            let revision = editor.arena().revision();
            let caret = editor.caret();
            assert!(!editor.convert_program_keyword());
            assert!(!editor.insert_try());
            assert_eq!(editor.to_portable().unwrap(), source);
            assert_eq!(editor.arena().revision(), revision);
            assert_eq!(editor.caret(), caret);
            assert!(!editor.undo());
        }
        let source = tree(
            NodeKind::Matrix {
                rows: 1,
                cols: 2,
                display: None,
            },
            vec![
                atom(NodeKind::Identifier("try".into())),
                atom(NodeKind::Placeholder),
            ],
        );
        let mut editor = Editor::from_portable(&source).unwrap();
        assert!(editor.select_matrix_cells(editor.root(), 0, 1));
        assert!(!editor.convert_program_keyword());
        assert!(!editor.insert_try());
        assert_eq!(editor.to_portable().unwrap(), source);
        assert!(!editor.undo());
    }

    #[test]
    fn try_insertion_preserves_program_siblings_and_targets_the_nested_blank_only() {
        let source = tree(
            NodeKind::Program { statement_count: 3 },
            vec![
                atom(NodeKind::Number("1".into())),
                atom(NodeKind::Placeholder),
                atom(NodeKind::Number("2".into())),
            ],
        );
        let mut editor = Editor::from_portable(&source).unwrap();
        let root = editor.root();
        let siblings = editor.arena().get(root).unwrap().children().to_vec();
        editor.place_caret(siblings[1], Direction::Left).unwrap();
        assert!(editor.insert_try());
        assert_eq!(editor.arena().get(root).unwrap().children(), siblings);
        let result = editor.to_portable().unwrap();
        assert_eq!(result.children[0], source.children[0]);
        assert_eq!(result.children[1], empty_try());
        assert_eq!(result.children[2], source.children[2]);
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), source);
        assert!(!editor.undo());
    }

    #[test]
    fn four_node_try_template_fits_exact_budget_and_rejects_structural_or_depth_failures_atomically()
     {
        for extra_groups in [0, 1] {
            let mut children = vec![];
            for i in 0..4096 {
                let mut value = atom(NodeKind::Identifier(
                    if i == 0 { "try" } else { "x" }.into(),
                ));
                let groups = if i == 0 {
                    extra_groups
                } else if i == 4095 {
                    1
                } else {
                    3
                };
                for _ in 0..groups {
                    value = tree(NodeKind::Group, vec![value]);
                }
                children.push(value);
            }
            let source = tree(
                NodeKind::Matrix {
                    rows: 64,
                    cols: 64,
                    display: None,
                },
                children,
            );
            let mut editor = Editor::from_portable(&source).unwrap();
            assert_eq!(
                editor.arena().nodes().count(),
                MAX_TREE_NODES - 4 + extra_groups
            );
            let revision = editor.arena().revision();
            assert_eq!(editor.convert_program_keyword(), extra_groups == 0);
            if extra_groups == 0 {
                assert_eq!(editor.arena().nodes().count(), MAX_TREE_NODES);
                assert!(editor.undo());
                assert_eq!(editor.to_portable().unwrap(), source);
                assert!(!editor.undo());
                assert!(editor.redo());
            } else {
                assert_eq!(editor.to_portable().unwrap(), source);
                assert_eq!(editor.arena().revision(), revision);
                assert!(!editor.undo());
            }
        }
        let mut deep = atom(NodeKind::Identifier("try".into()));
        for _ in 1..MAX_TREE_DEPTH {
            deep = tree(NodeKind::Group, vec![deep]);
        }
        let binder = tree(
            NodeKind::For,
            vec![
                atom(NodeKind::Identifier("try".into())),
                atom(NodeKind::Placeholder),
                body(atom(NodeKind::Placeholder)),
            ],
        );
        for source in [deep, binder] {
            let mut editor = Editor::from_portable(&source).unwrap();
            let revision = editor.arena().revision();
            assert!(!editor.convert_program_keyword());
            assert_eq!(editor.to_portable().unwrap(), source);
            assert_eq!(editor.arena().revision(), revision);
            assert!(!editor.undo());
        }
    }

    #[test]
    fn empty_try_inward_delete_collapses_only_its_slot_and_undo_restores_the_whole_template() {
        for source in [
            empty_try(),
            tree(
                NodeKind::Program { statement_count: 3 },
                vec![
                    atom(NodeKind::Number("1".into())),
                    empty_try(),
                    atom(NodeKind::Number("2".into())),
                ],
            ),
            tree(
                NodeKind::While,
                vec![
                    atom(NodeKind::Identifier("condition".into())),
                    body(empty_try()),
                ],
            ),
        ] {
            for (backwards, side) in [(true, Direction::Right), (false, Direction::Left)] {
                let mut editor = Editor::from_portable(&source).unwrap();
                let id = editor
                    .arena()
                    .nodes()
                    .find_map(|(id, node)| (node.kind() == &NodeKind::Try).then_some(id))
                    .unwrap();
                let parent = editor.arena().get(id).unwrap().parent();
                editor.place_caret(id, side).unwrap();
                let caret = editor.caret();
                assert!(editor.delete_structure(backwards));
                assert_eq!(
                    editor.arena().get(id).unwrap().kind(),
                    &NodeKind::Placeholder
                );
                assert_eq!(editor.arena().get(id).unwrap().parent(), parent);
                assert_eq!(editor.caret(), Caret::at(id, Direction::Left));
                let result = editor.to_portable().unwrap();
                assert!(editor.undo());
                assert_eq!(editor.to_portable().unwrap(), source);
                assert_eq!(editor.caret(), caret);
                assert!(!editor.undo());
                assert!(editor.redo());
                assert_eq!(editor.to_portable().unwrap(), result);
            }
        }
    }

    #[test]
    fn direct_empty_body_collapse_is_guarded_and_nested_empty_program_is_not_an_empty_statement() {
        for slot in [0, 1] {
            for (backwards, side) in [(true, Direction::Right), (false, Direction::Left)] {
                let mut editor = Editor::from_portable(&empty_try()).unwrap();
                let id = editor.root();
                let body = editor.arena().get(id).unwrap().children()[slot];
                editor.place_caret(body, side).unwrap();
                assert!(editor.delete_structure(backwards));
                assert_eq!(editor.to_portable().unwrap(), atom(NodeKind::Placeholder));
                assert_eq!(editor.caret(), Caret::at(id, Direction::Left));
                assert!(editor.undo());
                assert_eq!(editor.caret(), Caret::at(body, side));
            }
        }
        let nested = tree(
            NodeKind::Try,
            vec![
                body(body(atom(NodeKind::Placeholder))),
                body(atom(NodeKind::Placeholder)),
            ],
        );
        let multiple = tree(
            NodeKind::Try,
            vec![
                tree(
                    NodeKind::Program { statement_count: 2 },
                    vec![atom(NodeKind::Placeholder), atom(NodeKind::Placeholder)],
                ),
                body(atom(NodeKind::Placeholder)),
            ],
        );
        for source in [nested, multiple] {
            let mut editor = Editor::from_portable(&source).unwrap();
            let body = editor.arena().get(editor.root()).unwrap().children()[0];
            editor.place_caret(body, Direction::Right).unwrap();
            let revision = editor.arena().revision();
            assert!(!editor.delete_structure(true));
            assert_eq!(editor.to_portable().unwrap(), source);
            assert_eq!(editor.arena().revision(), revision);
            assert!(!editor.undo());
        }
    }

    #[test]
    fn header_delete_merges_bodies_in_order_and_uses_original_side_join_focus_even_when_empty() {
        for empty in [false, true] {
            let contents = if empty {
                vec![atom(NodeKind::Placeholder), atom(NodeKind::Placeholder)]
            } else {
                vec![
                    atom(NodeKind::Number("1".into())),
                    atom(NodeKind::Number("2".into())),
                ]
            };
            let source = tree(
                NodeKind::Try,
                vec![body(contents[0].clone()), body(contents[1].clone())],
            );
            for side in [Direction::Left, Direction::Right] {
                for backwards in [false, true] {
                    let mut editor = Editor::from_portable(&source).unwrap();
                    let id = editor.root();
                    let attempt = editor.arena().get(id).unwrap().children()[0];
                    let handler = editor.arena().get(id).unwrap().children()[1];
                    let first = editor.arena().get(attempt).unwrap().children()[0];
                    let second = editor.arena().get(handler).unwrap().children()[0];
                    editor.place_try_operator(id, side).unwrap();
                    let caret = editor.caret();
                    assert!(editor.delete_structure(backwards));
                    assert_eq!(editor.root(), attempt);
                    assert_eq!(
                        editor.to_portable().unwrap(),
                        tree(NodeKind::Program { statement_count: 2 }, contents.clone())
                    );
                    assert!(editor.arena().get(id).is_none());
                    assert!(editor.arena().get(handler).is_none());
                    let expected = if side == Direction::Left {
                        Caret::at(attempt, Direction::Left)
                    } else if backwards {
                        Caret::at(first, Direction::Right)
                    } else {
                        Caret::at(second, Direction::Left)
                    };
                    assert_eq!(editor.caret(), expected);
                    assert!(editor.undo());
                    assert_eq!(editor.to_portable().unwrap(), source);
                    assert_eq!(editor.caret(), caret);
                    assert!(!editor.undo());
                    assert!(editor.redo());
                    assert_eq!(editor.caret(), expected);
                }
            }
        }
    }

    #[test]
    fn populated_try_outer_boundaries_are_noops_and_do_not_discard_redo() {
        let source = tree(
            NodeKind::Try,
            vec![
                body(atom(NodeKind::Number("1".into()))),
                body(atom(NodeKind::Number("2".into()))),
            ],
        );
        for side in [Direction::Left, Direction::Right] {
            for backwards in [false, true] {
                let mut editor = Editor::from_portable(&source).unwrap();
                assert!(editor.insert_program_line());
                assert!(editor.undo());
                let id = editor.root();
                editor.place_caret(id, side).unwrap();
                let caret = editor.caret();
                let revision = editor.arena().revision();
                assert!(!editor.delete_structure(backwards));
                assert_eq!(editor.to_portable().unwrap(), source);
                assert_eq!(editor.caret(), caret);
                assert_eq!(editor.arena().revision(), revision);
                assert!(editor.redo());
            }
        }
    }

    #[test]
    fn header_merge_into_previous_program_or_inherent_body_precedes_its_own_join_focus() {
        let zero = || atom(NodeKind::Number("0".into()));
        for previous in [
            body(zero()),
            tree(
                NodeKind::While,
                vec![atom(NodeKind::Number("1".into())), body(zero())],
            ),
            tree(
                NodeKind::For,
                vec![
                    atom(NodeKind::Identifier("i".into())),
                    atom(NodeKind::Number("1".into())),
                    body(zero()),
                ],
            ),
            tree(
                NodeKind::If {
                    branch_count: 1,
                    has_else: true,
                    alternate: false,
                },
                vec![
                    atom(NodeKind::Number("1".into())),
                    body(atom(NodeKind::Number("9".into()))),
                    body(zero()),
                ],
            ),
            tree(
                NodeKind::Try,
                vec![body(atom(NodeKind::Number("9".into()))), body(zero())],
            ),
        ] {
            let source = tree(
                NodeKind::Program { statement_count: 3 },
                vec![
                    previous,
                    tree(
                        NodeKind::Try,
                        vec![
                            body(atom(NodeKind::Number("1".into()))),
                            body(atom(NodeKind::Number("2".into()))),
                        ],
                    ),
                    atom(NodeKind::Number("3".into())),
                ],
            );
            for backwards in [false, true] {
                let mut editor = Editor::from_portable(&source).unwrap();
                let root = editor.root();
                let siblings = editor.arena().get(root).unwrap().children().to_vec();
                let previous = siblings[0];
                let destination = if matches!(
                    editor.arena().get(previous).unwrap().kind,
                    NodeKind::Program { .. }
                ) {
                    previous
                } else {
                    *editor
                        .arena()
                        .get(previous)
                        .unwrap()
                        .children()
                        .last()
                        .unwrap()
                };
                let old_last = *editor
                    .arena()
                    .get(destination)
                    .unwrap()
                    .children()
                    .last()
                    .unwrap();
                let attempt = editor.arena().get(siblings[1]).unwrap().children()[0];
                let first_new = editor.arena().get(attempt).unwrap().children()[0];
                editor
                    .place_try_operator(siblings[1], Direction::Left)
                    .unwrap();
                assert!(editor.delete_structure(backwards));
                assert_eq!(
                    editor.arena().get(root).unwrap().children(),
                    &[siblings[0], siblings[2]]
                );
                assert_eq!(editor.arena().get(destination).unwrap().children().len(), 3);
                assert_eq!(
                    editor.caret(),
                    if backwards {
                        Caret::at(old_last, Direction::Right)
                    } else {
                        Caret::at(first_new, Direction::Left)
                    }
                );
                assert!(editor.arena().get(attempt).is_none());
                assert!(editor.undo());
                assert_eq!(editor.to_portable().unwrap(), source);
                assert!(!editor.undo());
            }
        }
    }

    #[test]
    fn try_merge_limits_and_unconfirmed_previous_container_reject_without_erasing_the_source() {
        let full = tree(
            NodeKind::Program {
                statement_count: 4096,
            },
            (0..4096)
                .map(|_| atom(NodeKind::Number("1".into())))
                .collect(),
        );
        let source = tree(
            NodeKind::Try,
            vec![full, body(atom(NodeKind::Number("2".into())))],
        );
        let previous = tree(
            NodeKind::Program {
                statement_count: 4095,
            },
            (0..4095)
                .map(|_| atom(NodeKind::Number("0".into())))
                .collect(),
        );
        let two = || {
            tree(
                NodeKind::Try,
                vec![
                    body(atom(NodeKind::Number("1".into()))),
                    body(atom(NodeKind::Number("2".into()))),
                ],
            )
        };
        let overflow_previous = tree(
            NodeKind::Program { statement_count: 2 },
            vec![previous, two()],
        );
        let unconfirmed_previous = tree(
            NodeKind::Program { statement_count: 2 },
            vec![
                tree(
                    NodeKind::Group,
                    vec![body(atom(NodeKind::Number("0".into())))],
                ),
                two(),
            ],
        );
        for source in [source, overflow_previous, unconfirmed_previous] {
            let mut editor = Editor::from_portable(&source).unwrap();
            let id = editor
                .arena()
                .nodes()
                .find_map(|(id, node)| (node.kind() == &NodeKind::Try).then_some(id))
                .unwrap();
            editor.place_try_operator(id, Direction::Right).unwrap();
            let caret = editor.caret();
            let revision = editor.arena().revision();
            assert!(!editor.delete_structure(true));
            assert_eq!(editor.to_portable().unwrap(), source);
            assert_eq!(editor.caret(), caret);
            assert_eq!(editor.arena().revision(), revision);
            assert!(!editor.undo());
        }
    }

    #[test]
    fn return_program_is_not_an_inherent_merge_target_and_explicit_selection_keeps_legacy_delete() {
        let previous = tree(
            NodeKind::Return,
            vec![body(atom(NodeKind::Number("0".into())))],
        );
        let source = tree(
            NodeKind::Program { statement_count: 3 },
            vec![
                previous.clone(),
                tree(
                    NodeKind::Try,
                    vec![
                        body(atom(NodeKind::Number("1".into()))),
                        body(atom(NodeKind::Number("2".into()))),
                    ],
                ),
                atom(NodeKind::Number("3".into())),
            ],
        );
        let mut editor = Editor::from_portable(&source).unwrap();
        let id = editor.arena().get(editor.root()).unwrap().children()[1];
        editor.place_try_operator(id, Direction::Left).unwrap();
        assert!(editor.delete_structure(true));
        let result = editor.to_portable().unwrap();
        assert_eq!(result.children.len(), 3);
        assert_eq!(result.children[0], previous);
        assert_eq!(
            result.children[1],
            tree(
                NodeKind::Program { statement_count: 2 },
                vec![
                    atom(NodeKind::Number("1".into())),
                    atom(NodeKind::Number("2".into()))
                ]
            )
        );
        assert!(editor.undo());
        editor.select_subtree(id).unwrap();
        assert!(editor.delete_structure(true));
        assert_eq!(
            editor.arena().get(id).unwrap().kind(),
            &NodeKind::Placeholder
        );
        assert_eq!(editor.caret().caret_side, Direction::Indeterminate);
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), source);
        assert_eq!(editor.selection(), Selection::Subtree(id));
    }

    #[test]
    fn fresh_ctrl_j_try_first_placeholder_deletes_without_parent_navigation_as_one_command() {
        for keyword in [
            NodeKind::Identifier("try".into()),
            NodeKind::NativeIdentifier {
                name: "try".into(),
                labels: "NONE".into(),
                contextual: Some(false),
                subscript: None,
            },
            NodeKind::NativeIdentifier {
                name: "try".into(),
                labels: "RESOLVED_FUNCTION".into(),
                contextual: Some(true),
                subscript: None,
            },
        ] {
            for backwards in [true, false] {
                let source = atom(keyword.clone());
                let mut editor = Editor::from_portable(&source).unwrap();
                let word = editor.root();
                editor.place_text_selection(word, 1, 2).unwrap();
                let word_caret = editor.caret();
                assert!(editor.convert_program_keyword());
                let fresh = editor.to_portable().unwrap();
                let initial = editor.caret();
                let bodies = editor.arena().get(word).unwrap().children().to_vec();
                let handler_ph = editor.arena().get(bodies[1]).unwrap().children()[0];
                assert_eq!(
                    initial.node,
                    editor.arena().get(bodies[0]).unwrap().children()[0]
                );
                assert!(editor.delete_structure(backwards));
                let deleted = editor.to_portable().unwrap();
                if backwards {
                    assert_eq!(deleted, atom(NodeKind::Placeholder));
                    assert_eq!(editor.caret(), Caret::at(word, Direction::Left));
                } else {
                    assert_eq!(
                        deleted,
                        tree(
                            NodeKind::Program { statement_count: 2 },
                            vec![atom(NodeKind::Placeholder), atom(NodeKind::Placeholder)]
                        )
                    );
                    assert_eq!(editor.caret(), Caret::at(handler_ph, Direction::Left));
                }
                assert!(
                    editor.undo(),
                    "delete is one command including native intermediate navigation"
                );
                assert_eq!(editor.to_portable().unwrap(), fresh);
                assert_eq!(editor.caret(), initial);
                assert!(
                    editor.undo(),
                    "next Undo restores the original Ctrl+J word and metadata"
                );
                assert_eq!(editor.to_portable().unwrap(), source);
                assert_eq!(editor.caret(), word_caret);
                assert!(!editor.undo());
                assert!(editor.redo());
                assert_eq!(editor.caret(), initial);
                assert!(editor.redo());
                assert_eq!(editor.to_portable().unwrap(), deleted);
            }
        }
    }

    #[test]
    fn fresh_ribbon_and_second_body_placeholder_backspace_preserve_outer_statements() {
        for handler in [false, true] {
            let source = tree(
                NodeKind::Program { statement_count: 3 },
                vec![
                    atom(NodeKind::Number("1".into())),
                    atom(NodeKind::Placeholder),
                    atom(NodeKind::Number("2".into())),
                ],
            );
            let mut editor = Editor::from_portable(&source).unwrap();
            let root = editor.root();
            let statements = editor.arena().get(root).unwrap().children().to_vec();
            editor
                .place_caret(statements[1], Direction::Indeterminate)
                .unwrap();
            assert!(editor.insert_try());
            if handler {
                let body = editor.arena().get(statements[1]).unwrap().children()[1];
                let ph = editor.arena().get(body).unwrap().children()[0];
                editor.place_caret(ph, Direction::Indeterminate).unwrap();
            }
            let fresh = editor.to_portable().unwrap();
            let caret = editor.caret();
            assert!(editor.delete_structure(true));
            assert_eq!(editor.to_portable().unwrap(), source);
            assert_eq!(editor.arena().get(root).unwrap().children(), statements);
            assert_eq!(editor.caret(), Caret::at(statements[1], Direction::Left));
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), fresh);
            assert_eq!(editor.caret(), caret);
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), source);
            assert!(!editor.undo());
        }
    }

    #[test]
    fn second_placeholder_delete_and_nonfresh_body_states_do_not_invent_try_collapse() {
        let source = empty_try();
        let mut editor = Editor::from_portable(&source).unwrap();
        let root = editor.root();
        let handler = editor.arena().get(root).unwrap().children()[1];
        let ph = editor.arena().get(handler).unwrap().children()[0];
        assert!(editor.insert_program_line());
        assert!(editor.undo());
        editor.place_caret(ph, Direction::Indeterminate).unwrap();
        let caret = editor.caret();
        let revision = editor.arena().revision();
        assert!(!editor.delete_structure(false));
        assert_eq!(editor.to_portable().unwrap(), source);
        assert_eq!(editor.caret(), caret);
        assert_eq!(editor.arena().revision(), revision);
        assert!(editor.redo());

        let populated_handler = tree(
            NodeKind::Try,
            vec![
                body(atom(NodeKind::Placeholder)),
                body(atom(NodeKind::Number("7".into()))),
            ],
        );
        let nested = tree(
            NodeKind::Try,
            vec![
                body(body(atom(NodeKind::Placeholder))),
                body(atom(NodeKind::Placeholder)),
            ],
        );
        for source in [populated_handler, nested] {
            for backwards in [true, false] {
                let mut editor = Editor::from_portable(&source).unwrap();
                let first = editor.caret().node;
                editor.place_caret(first, Direction::Indeterminate).unwrap();
                let revision = editor.arena().revision();
                assert!(!editor.delete_structure(backwards));
                assert_eq!(editor.to_portable().unwrap(), source);
                assert_eq!(editor.arena().revision(), revision);
                assert!(!editor.undo());
            }
        }
    }
}
