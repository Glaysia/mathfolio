//! Identifier label cycling follows the shipped label enum and HasNoLabel
//! distinction. Runtime typeover modes not represented by this editor are not
//! inferred here; only an atomic identifier or its single-node scope is handled.
use super::*;

fn next_explicit_label(label: &str) -> Option<&'static str> {
    Some(match label {
        "VARIABLE" => "UNIT",
        "UNIT" => "CONSTANT",
        "CONSTANT" => "FUNCTION",
        "FUNCTION" => "SYSTEM",
        "SYSTEM" => "KEYWORD",
        "KEYWORD" => "*",
        "*" | "NONE" => "VARIABLE",
        _ => return None,
    })
}

impl Editor {
    fn identifier_label_target(&self) -> Option<NodeId> {
        let target = match self.selection {
            Selection::None => self.caret.node,
            Selection::Subtree(node) => node,
            Selection::MatrixCells(_) => return None,
        };
        matches!(
            self.arena.get(target)?.kind,
            NodeKind::Identifier(_)
                | NodeKind::AnnotatedIdentifier { .. }
                | NodeKind::NativeIdentifier { .. }
        )
        .then_some(target)
    }

    /// Apply the next explicit identifier label as one undoable command.
    /// Unlabeled/contextual identifiers start at Variable, independently of
    /// their cached resolved label. Explicit None is `*`, never contextual.
    /// Unknown explicit labels and selections spanning nodes are unchanged.
    pub fn cycle_identifier_label(&mut self) -> bool {
        let Some(target) = self.identifier_label_target() else {
            return false;
        };
        let label = match &self.arena.get(target).unwrap().kind {
            NodeKind::Identifier(_) => "VARIABLE",
            NodeKind::AnnotatedIdentifier { labels, .. } => {
                let Some(label) = next_explicit_label(labels) else {
                    return false;
                };
                label
            }
            NodeKind::NativeIdentifier {
                contextual: Some(true),
                ..
            } => "VARIABLE",
            NodeKind::NativeIdentifier { labels, .. } => {
                let Some(label) = next_explicit_label(labels) else {
                    return false;
                };
                label
            }
            _ => return false,
        };
        self.set_identifier_label(label)
    }

    /// Set a known explicit label, preserving identifier text, literal suffix,
    /// caret and selection. `*` is explicit None with no contextual attribute.
    /// An already identical explicit choice leaves revision and redo untouched.
    pub fn set_identifier_label(&mut self, label: &str) -> bool {
        let label = if label == "NONE" { "*" } else { label };
        if next_explicit_label(label).is_none() {
            return false;
        }
        let Some(target) = self.identifier_label_target() else {
            return false;
        };
        let current = &self.arena.get(target).unwrap().kind;
        let next = match current {
            NodeKind::Identifier(name) => NodeKind::AnnotatedIdentifier {
                name: name.clone(),
                labels: label.into(),
            },
            NodeKind::AnnotatedIdentifier { name, .. } => NodeKind::AnnotatedIdentifier {
                name: name.clone(),
                labels: label.into(),
            },
            NodeKind::NativeIdentifier {
                name, subscript, ..
            } => NodeKind::NativeIdentifier {
                name: name.clone(),
                labels: label.into(),
                contextual: None,
                subscript: subscript.clone(),
            },
            _ => return false,
        };
        if *current == next {
            return false;
        }
        // Label metadata does not move the caret or invalidate a scope's node
        // coordinates. Preserve UTF-16 atom/suffix selections and scope history.
        let scope = self.scope.clone();
        self.checkpoint();
        self.scope = scope;
        self.arena.node_mut(target).kind = next;
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
    fn native(label: &str, contextual: Option<bool>) -> PortableTree {
        atom(NodeKind::NativeIdentifier {
            name: "m".into(),
            labels: label.into(),
            contextual,
            subscript: Some("dc".into()),
        })
    }

    #[test]
    fn explicit_label_setter_preserves_text_scope_and_utf16_suffix_selection_as_one_undo() {
        for label in [
            "VARIABLE", "UNIT", "CONSTANT", "FUNCTION", "SYSTEM", "KEYWORD", "*",
        ] {
            let source = atom(NodeKind::NativeIdentifier {
                name: "a𝑥b".into(),
                labels: "UNIT".into(),
                contextual: Some(true),
                subscript: Some("가😀β".into()),
            });
            let mut editor = Editor::from_portable(&source).unwrap();
            editor
                .place_subscript_selection(editor.root(), 4, 1)
                .unwrap();
            let before_scope = editor.caret();
            assert!(editor.expand_scope());
            let caret = editor.caret();
            let selection = editor.selection();
            assert!(editor.set_identifier_label(label));
            assert_eq!(editor.caret(), caret);
            assert_eq!(editor.selection(), selection);
            let expected = atom(NodeKind::NativeIdentifier {
                name: "a𝑥b".into(),
                labels: label.into(),
                contextual: None,
                subscript: Some("가😀β".into()),
            });
            assert_eq!(editor.to_portable().unwrap(), expected);
            assert!(editor.contract_scope());
            assert_eq!(editor.caret(), before_scope);
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), source);
            assert_eq!(editor.caret(), caret);
            assert_eq!(editor.selection(), selection);
            assert!(!editor.undo());
            assert!(editor.redo());
            assert_eq!(editor.to_portable().unwrap(), expected);
        }
    }

    #[test]
    fn same_explicit_choice_is_a_noop_but_false_contextual_attribute_is_canonicalized_undoably() {
        for source in [
            native("UNIT", None),
            atom(NodeKind::AnnotatedIdentifier {
                name: "x".into(),
                labels: "UNIT".into(),
            }),
        ] {
            let mut editor = Editor::from_portable(&source).unwrap();
            assert!(editor.set_identifier_label("CONSTANT"));
            let future = editor.to_portable().unwrap();
            assert!(editor.undo());
            let revision = editor.arena().revision();
            let caret = editor.caret();
            assert!(!editor.set_identifier_label("UNIT"));
            assert_eq!(editor.arena().revision(), revision);
            assert_eq!(editor.caret(), caret);
            assert_eq!(editor.to_portable().unwrap(), source);
            assert!(editor.redo());
            assert_eq!(editor.to_portable().unwrap(), future);
        }
        for label in ["UNIT", "*"] {
            let source = native(label, Some(false));
            let mut editor = Editor::from_portable(&source).unwrap();
            assert!(editor.set_identifier_label(label));
            assert_eq!(editor.to_portable().unwrap(), native(label, None));
            let revision = editor.arena().revision();
            assert!(!editor.set_identifier_label(label));
            assert_eq!(editor.arena().revision(), revision);
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), source);
            assert!(!editor.undo());
        }
    }

    #[test]
    fn explicit_label_setter_rejects_unknown_requested_labels_and_non_identifier_selections() {
        let source = native("UNIT", None);
        let mut editor = Editor::from_portable(&source).unwrap();
        for label in ["", "variable", "UNKNOWN", "VARIABLE UNIT"] {
            let revision = editor.arena().revision();
            assert!(!editor.set_identifier_label(label));
            assert_eq!(editor.arena().revision(), revision);
            assert_eq!(editor.to_portable().unwrap(), source);
            assert!(!editor.undo());
        }
        for source in [
            atom(NodeKind::Number("3".into())),
            PortableTree {
                kind: NodeKind::Binary(BinaryOp::Add),
                children: vec![native("UNIT", None), native("VARIABLE", None)],
            },
        ] {
            let mut editor = Editor::from_portable(&source).unwrap();
            editor.select_subtree(editor.root()).unwrap();
            let revision = editor.arena().revision();
            assert!(!editor.set_identifier_label("*"));
            assert_eq!(editor.arena().revision(), revision);
            assert_eq!(editor.to_portable().unwrap(), source);
            assert!(!editor.undo());
        }
        let source = PortableTree {
            kind: NodeKind::Matrix {
                rows: 1,
                cols: 2,
                display: None,
            },
            children: vec![native("UNIT", None), native("VARIABLE", None)],
        };
        let mut editor = Editor::from_portable(&source).unwrap();
        assert!(editor.select_matrix_cells(editor.root(), 0, 1));
        let revision = editor.arena().revision();
        let caret = editor.caret();
        let selection = editor.selection();
        assert!(!editor.set_identifier_label("*"));
        assert_eq!(editor.arena().revision(), revision);
        assert_eq!(editor.caret(), caret);
        assert_eq!(editor.selection(), selection);
        assert_eq!(editor.to_portable().unwrap(), source);
        assert!(!editor.undo());
    }

    #[test]
    fn native_none_spelling_is_preserved_until_a_label_command_then_canonicalized() {
        for source in [
            native("NONE", None),
            native("NONE", Some(false)),
            atom(NodeKind::AnnotatedIdentifier {
                name: "x".into(),
                labels: "NONE".into(),
            }),
        ] {
            let mut editor = Editor::from_portable(&source).unwrap();
            assert_eq!(editor.to_portable().unwrap(), source);
            assert!(editor.cycle_identifier_label());
            let expected = match &source.kind {
                NodeKind::NativeIdentifier { .. } => native("VARIABLE", None),
                _ => atom(NodeKind::AnnotatedIdentifier {
                    name: "x".into(),
                    labels: "VARIABLE".into(),
                }),
            };
            assert_eq!(editor.to_portable().unwrap(), expected);
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), source);
            assert!(!editor.undo());
            assert!(editor.set_identifier_label("NONE"));
            let expected = match &source.kind {
                NodeKind::NativeIdentifier { .. } => native("*", None),
                _ => atom(NodeKind::AnnotatedIdentifier {
                    name: "x".into(),
                    labels: "*".into(),
                }),
            };
            assert_eq!(editor.to_portable().unwrap(), expected);
            let revision = editor.arena().revision();
            assert!(!editor.set_identifier_label("NONE"));
            assert_eq!(editor.arena().revision(), revision);
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), source);
            assert!(!editor.undo());
        }
    }

    #[test]
    fn unlabeled_identifier_follows_the_entire_original_enum_cycle_with_one_undo_per_command() {
        let source = atom(NodeKind::Identifier("x".into()));
        let mut editor = Editor::from_portable(&source).unwrap();
        let id = editor.root();
        let mut history = vec![source];
        for label in [
            "VARIABLE", "UNIT", "CONSTANT", "FUNCTION", "SYSTEM", "KEYWORD", "*", "VARIABLE",
        ] {
            let revision = editor.arena().revision();
            assert!(editor.cycle_identifier_label());
            assert_ne!(editor.arena().revision(), revision);
            assert_eq!(editor.root(), id);
            let expected = atom(NodeKind::AnnotatedIdentifier {
                name: "x".into(),
                labels: label.into(),
            });
            assert_eq!(editor.to_portable().unwrap(), expected);
            history.push(expected);
        }
        for expected in history[..history.len() - 1].iter().rev() {
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), *expected);
        }
        assert!(!editor.undo());
        for expected in &history[1..] {
            assert!(editor.redo());
            assert_eq!(editor.to_portable().unwrap(), *expected);
        }
        assert!(!editor.redo());
    }

    #[test]
    fn contextual_labels_restart_at_variable_and_manual_labels_remove_contextual_metadata() {
        for (label, contextual, expected) in [
            ("UNIT", Some(true), "VARIABLE"),
            ("SYSTEM", Some(true), "VARIABLE"),
            ("UNKNOWN_RESOLVED_LABEL", Some(true), "VARIABLE"),
            ("VARIABLE", Some(false), "UNIT"),
            ("VARIABLE", None, "UNIT"),
            ("KEYWORD", Some(false), "*"),
            ("KEYWORD", None, "*"),
            ("*", None, "VARIABLE"),
            ("*", Some(false), "VARIABLE"),
            ("*", Some(true), "VARIABLE"),
        ] {
            let source = native(label, contextual);
            let mut editor = Editor::from_portable(&source).unwrap();
            assert!(editor.cycle_identifier_label());
            assert_eq!(editor.to_portable().unwrap(), native(expected, None));
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), source);
            assert!(!editor.undo());
        }
    }

    #[test]
    fn native_name_suffix_and_utf16_selections_survive_label_changes() {
        let source = atom(NodeKind::NativeIdentifier {
            name: "a𝑥b".into(),
            labels: "UNIT".into(),
            contextual: Some(false),
            subscript: Some("가😀β".into()),
        });
        for suffix in [false, true] {
            let mut editor = Editor::from_portable(&source).unwrap();
            let id = editor.root();
            if suffix {
                editor.place_subscript_selection(id, 4, 1).unwrap();
            } else {
                editor.place_text_selection(id, 3, 1).unwrap();
            }
            let caret = editor.caret();
            let selection = editor.selection();
            assert!(editor.cycle_identifier_label());
            assert_eq!(editor.caret(), caret);
            assert_eq!(editor.selection(), selection);
            let mut expected = source.clone();
            let NodeKind::NativeIdentifier {
                labels, contextual, ..
            } = &mut expected.kind
            else {
                unreachable!()
            };
            *labels = "CONSTANT".into();
            *contextual = None;
            assert_eq!(editor.to_portable().unwrap(), expected);
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), source);
            assert_eq!(editor.caret(), caret);
            assert_eq!(editor.selection(), selection);
        }
    }

    #[test]
    fn single_identifier_scope_keeps_its_caret_direction_and_contractible_history() {
        let source = PortableTree {
            kind: NodeKind::Binary(BinaryOp::Add),
            children: vec![atom(NodeKind::Identifier("a".into())), native("UNIT", None)],
        };
        let mut editor = Editor::from_portable(&source).unwrap();
        let right = editor.arena().get(editor.root()).unwrap().children()[1];
        editor.place_text_caret(right, 0).unwrap();
        let origin = editor.caret();
        assert!(editor.expand_scope());
        assert_eq!(editor.selection(), Selection::Subtree(right));
        let scoped_caret = editor.caret();
        let direction = editor.scope_direction();
        assert!(editor.cycle_identifier_label());
        assert_eq!(editor.selection(), Selection::Subtree(right));
        assert_eq!(editor.caret(), scoped_caret);
        assert_eq!(editor.scope_direction(), direction);
        assert_eq!(
            editor.to_portable().unwrap().children[0],
            source.children[0]
        );
        assert!(editor.contract_scope());
        assert_eq!(editor.selection(), Selection::None);
        assert_eq!(editor.caret(), origin);
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), source);
        assert_eq!(editor.selection(), Selection::Subtree(right));
        assert_eq!(editor.caret(), scoped_caret);
        assert!(editor.contract_scope());
        assert_eq!(editor.caret(), origin);
    }

    #[test]
    fn unknown_explicit_labels_do_not_guess_a_cycle_or_invalidate_history() {
        for label in ["UNKNOWN", "variable", "VARIABLE UNIT", "FUNCTION,VARIABLE"] {
            for source in [
                atom(NodeKind::AnnotatedIdentifier {
                    name: "x".into(),
                    labels: label.into(),
                }),
                native(label, None),
                native(label, Some(false)),
            ] {
                let mut editor = Editor::from_portable(&source).unwrap();
                let revision = editor.arena().revision();
                let caret = editor.caret();
                assert!(!editor.cycle_identifier_label());
                assert_eq!(editor.arena().revision(), revision);
                assert_eq!(editor.to_portable().unwrap(), source);
                assert_eq!(editor.caret(), caret);
                assert!(!editor.undo());
            }
        }
    }

    #[test]
    fn non_identifiers_multi_node_scopes_and_matrix_cell_ranges_are_noops_preserving_redo() {
        let source = PortableTree {
            kind: NodeKind::Binary(BinaryOp::Add),
            children: vec![
                atom(NodeKind::Identifier("x".into())),
                atom(NodeKind::Number("3".into())),
            ],
        };
        let mut editor = Editor::from_portable(&source).unwrap();
        assert!(editor.cycle_identifier_label());
        let changed = editor.to_portable().unwrap();
        assert!(editor.undo());
        editor.select_subtree(editor.root()).unwrap();
        let revision = editor.arena().revision();
        assert!(!editor.cycle_identifier_label());
        assert_eq!(editor.arena().revision(), revision);
        assert_eq!(editor.to_portable().unwrap(), source);
        assert!(editor.redo());
        assert_eq!(editor.to_portable().unwrap(), changed);
        let matrix = PortableTree {
            kind: NodeKind::Matrix {
                rows: 1,
                cols: 2,
                display: None,
            },
            children: vec![native("UNIT", None), atom(NodeKind::Identifier("x".into()))],
        };
        for focus in [0, 1] {
            let mut editor = Editor::from_portable(&matrix).unwrap();
            assert!(editor.select_matrix_cells(editor.root(), 0, focus));
            let revision = editor.arena().revision();
            let caret = editor.caret();
            let selection = editor.selection();
            assert!(!editor.cycle_identifier_label());
            assert_eq!(editor.arena().revision(), revision);
            assert_eq!(editor.caret(), caret);
            assert_eq!(editor.selection(), selection);
            assert_eq!(editor.to_portable().unwrap(), matrix);
            assert!(!editor.undo());
        }
        for kind in [
            NodeKind::Placeholder,
            NodeKind::Number("3".into()),
            NodeKind::Imaginary("2".into()),
        ] {
            let source = atom(kind);
            let mut editor = Editor::from_portable(&source).unwrap();
            let revision = editor.arena().revision();
            assert!(!editor.cycle_identifier_label());
            assert_eq!(editor.arena().revision(), revision);
            assert_eq!(editor.to_portable().unwrap(), source);
            assert!(!editor.undo());
        }
    }

    #[test]
    fn identifier_roles_and_expression_only_context_do_not_invent_override_restrictions() {
        let source = PortableTree {
            kind: NodeKind::Function { parameter_count: 1 },
            children: vec![native("FUNCTION", Some(false)), native("UNIT", None)],
        };
        let mut editor = Editor::from_portable(&source).unwrap();
        editor.set_expressions_only(true);
        let children = editor
            .arena()
            .get(editor.root())
            .unwrap()
            .children()
            .to_vec();
        editor.place_caret(children[0], Direction::Right).unwrap();
        assert!(editor.cycle_identifier_label());
        editor.place_caret(children[1], Direction::Right).unwrap();
        assert!(editor.cycle_identifier_label());
        assert_eq!(
            editor.to_portable().unwrap().children,
            vec![native("SYSTEM", None), native("CONSTANT", None)]
        );
        assert!(editor.expressions_only());
    }
}
