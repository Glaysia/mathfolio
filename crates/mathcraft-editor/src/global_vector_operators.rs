//! Global definitions, vectorization and cross products are original structural
//! operators, not function names. See docs/analysis/0073-global-vector-operator-contract.md.
use super::*;

impl Editor {
    /// Global definitions belong at the worksheet region root. Existing
    /// definitions/evaluations and unsupported expression LHSs are unchanged.
    pub fn insert_global_definition(&mut self) -> NodeId {
        if self.expressions_only {
            return self.root;
        }
        if let Some(range) = self.matrix_selection() {
            return range.matrix;
        }
        let root = self.root;
        let kind = &self.arena.get(root).unwrap().kind;
        if !matches!(
            kind,
            NodeKind::Placeholder
                | NodeKind::Identifier(_)
                | NodeKind::AnnotatedIdentifier { .. }
                | NodeKind::NativeIdentifier { .. }
                | NodeKind::Function { .. }
                | NodeKind::Call { .. }
                | NodeKind::Index { .. }
                | NodeKind::Row
                | NodeKind::Column
        ) {
            return root;
        }
        if matches!(
            kind,
            NodeKind::Index { .. }
                | NodeKind::Row
                | NodeKind::Column
                | NodeKind::Call { .. }
                | NodeKind::Function { .. }
        ) && !matches!(
            self.arena
                .get(self.arena.get(root).unwrap().children[0])
                .unwrap()
                .kind,
            NodeKind::Placeholder
                | NodeKind::Identifier(_)
                | NodeKind::AnnotatedIdentifier { .. }
                | NodeKind::NativeIdentifier { .. }
        ) {
            return root;
        }
        self.checkpoint();
        if let NodeKind::Call { argument_count } = self.arena.get(root).unwrap().kind {
            self.arena.node_mut(root).kind = NodeKind::Function {
                parameter_count: argument_count,
            };
        }
        self.selection = Selection::Subtree(root);
        self.wrap_binary_inner(NodeKind::Binary(BinaryOp::GlobalDefine))
    }

    pub fn insert_vectorize(&mut self) -> NodeId {
        self.wrap_unary(NodeKind::Vectorize)
    }

    pub fn insert_cross_product(&mut self) -> NodeId {
        self.insert_infix(BinaryOp::CrossProduct)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn atom(name: &str) -> PortableTree {
        PortableTree {
            kind: NodeKind::Identifier(name.into()),
            children: vec![],
        }
    }
    fn binary(op: BinaryOp, a: PortableTree, b: PortableTree) -> PortableTree {
        PortableTree {
            kind: NodeKind::Binary(op),
            children: vec![a, b],
        }
    }

    #[test]
    fn named_row_and_column_targets_support_root_definitions_and_single_undo() {
        for kind in [NodeKind::Row, NodeKind::Column] {
            let source = PortableTree {
                kind,
                children: vec![
                    PortableTree {
                        kind: NodeKind::NativeIdentifier {
                            name: "M".into(),
                            labels: "VARIABLE".into(),
                            contextual: Some(false),
                            subscript: Some("source".into()),
                        },
                        children: vec![],
                    },
                    atom("ORIGIN"),
                ],
            };
            for global in [false, true] {
                let mut editor = Editor::from_portable(&source).unwrap();
                let lhs = editor.root();
                let root = if global {
                    editor.insert_global_definition()
                } else {
                    editor.insert_definition()
                };
                assert_eq!(editor.arena().get(root).unwrap().children()[0], lhs);
                assert_eq!(editor.to_portable().unwrap().children[0], source);
                assert_eq!(
                    editor.to_portable().unwrap().kind,
                    NodeKind::Binary(if global {
                        BinaryOp::GlobalDefine
                    } else {
                        BinaryOp::Define
                    })
                );
                assert!(editor.undo());
                assert_eq!(editor.to_portable().unwrap(), source);
                assert!(!editor.undo());
                assert!(editor.redo());
                assert_eq!(editor.to_portable().unwrap().children[0], source);
            }
        }
    }

    #[test]
    fn expression_only_slice_targets_and_non_named_global_slices_do_not_mutate() {
        for kind in [NodeKind::Row, NodeKind::Column] {
            let source = PortableTree {
                kind,
                children: vec![atom("M"), atom("i")],
            };
            let mut editor = Editor::from_portable(&source).unwrap();
            editor.set_expressions_only(true);
            let revision = editor.arena().revision();
            let root = editor.root();
            assert_eq!(editor.insert_definition(), root);
            assert_eq!(editor.insert_global_definition(), root);
            assert_eq!(editor.to_portable().unwrap(), source);
            assert_eq!(editor.arena().revision(), revision);
            assert!(!editor.undo());
            let mut non_named = source;
            non_named.children[0] = binary(BinaryOp::Add, atom("M"), atom("N"));
            let mut editor = Editor::from_portable(&non_named).unwrap();
            let revision = editor.arena().revision();
            assert_eq!(editor.insert_definition(), editor.root());
            assert_eq!(editor.insert_global_definition(), editor.root());
            assert_eq!(editor.to_portable().unwrap(), non_named);
            assert_eq!(editor.arena().revision(), revision);
            assert!(!editor.undo());
        }
    }

    #[test]
    fn program_assignment_accepts_named_slices_without_widening_loop_targets() {
        for kind in [
            NodeKind::Row,
            NodeKind::Column,
            NodeKind::Index { indices: 1 },
        ] {
            let source = PortableTree {
                kind: NodeKind::Program { statement_count: 1 },
                children: vec![PortableTree {
                    kind: NodeKind::LocalDefine,
                    children: vec![
                        PortableTree {
                            kind: NodeKind::Placeholder,
                            children: vec![],
                        },
                        atom("rhs"),
                    ],
                }],
            };
            let mut editor = Editor::from_portable(&source).unwrap();
            let statement = editor.arena().get(editor.root()).unwrap().children()[0];
            let target = editor.arena().get(statement).unwrap().children()[0];
            editor.select_subtree(target).unwrap();
            let slice = PortableTree {
                kind,
                children: vec![atom("M"), atom("ORIGIN")],
            };
            editor.replace_selection(&slice).unwrap();
            assert_eq!(editor.to_portable().unwrap().children[0].children[0], slice);
            assert!(editor.validate().is_ok());
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), source);
            assert!(!editor.undo());
            let mut invalid = source;
            invalid.children[0].kind = NodeKind::For;
            invalid.children[0].children[0] = slice;
            invalid.children[0].children.push(atom("body"));
            assert!(invalid.validate().is_err());
        }
    }

    #[test]
    fn expression_only_context_rejects_definition_commands_and_clipboard_without_history() {
        let source = atom("x");
        let mut editor = Editor::from_portable(&source).unwrap();
        assert!(!editor.expressions_only());
        let revision = editor.arena().revision();
        editor.set_expressions_only(true);
        let root = editor.root();
        assert_eq!(editor.insert_definition(), root);
        assert_eq!(editor.insert_global_definition(), root);
        for op in [BinaryOp::Define, BinaryOp::GlobalDefine] {
            assert_eq!(editor.insert_binary(op), root);
            assert_eq!(editor.insert_infix(op), root);
            assert!(!editor.replace_infix_operator(root, op));
            assert!(
                editor
                    .replace_selection(&binary(op, atom("a"), atom("b")))
                    .is_err()
            );
        }
        assert_eq!(editor.to_portable().unwrap(), source);
        assert_eq!(editor.arena().revision(), revision);
        assert!(!editor.undo());
        assert!(editor.clone_without_history().expressions_only());
        // Expression operators remain available in this context.
        editor.insert_vectorize();
        assert_eq!(editor.to_portable().unwrap().kind, NodeKind::Vectorize);
        assert!(editor.undo());
        assert!(editor.expressions_only());
        assert_eq!(editor.to_portable().unwrap(), source);
        assert!(!editor.undo());
    }

    #[test]
    fn expression_context_is_retained_by_document_load_undo_and_redo() {
        let mut editor = Editor::from_portable(&atom("a")).unwrap();
        editor.insert_global_definition();
        editor.set_expressions_only(true);
        assert!(editor.undo());
        assert!(editor.expressions_only());
        assert!(editor.redo());
        assert!(editor.expressions_only());
        editor.load_portable(&atom("b")).unwrap();
        assert!(editor.expressions_only());
        assert!(editor.undo());
        assert!(editor.expressions_only());
        editor.set_expressions_only(false);
        editor.load_portable(&atom("c")).unwrap();
        editor.insert_definition();
        assert_eq!(
            editor.to_portable().unwrap().kind,
            NodeKind::Binary(BinaryOp::Define)
        );
    }

    #[test]
    fn global_and_vector_descriptor_identity_arity_and_precedence_match_the_original() {
        for (kind, name, arity, precedence) in [
            (
                NodeKind::Binary(BinaryOp::GlobalDefine),
                "GLOBAL_DEFINITION",
                2,
                10,
            ),
            (
                NodeKind::Binary(BinaryOp::CrossProduct),
                "CROSSPRODUCT",
                2,
                120,
            ),
            (NodeKind::Vectorize, "VECTORIZATION", 1, 130),
        ] {
            assert_eq!(kind.descriptor_name(), name);
            assert_eq!(kind.arity(), arity);
            assert_eq!(kind.precedence(), precedence);
        }
    }

    #[test]
    fn global_definition_preserves_name_metadata_and_evaluates_only_its_rhs_slot() {
        let source = PortableTree {
            kind: NodeKind::NativeIdentifier {
                name: "I".into(),
                labels: "VARIABLE".into(),
                contextual: Some(false),
                subscript: Some("dc".into()),
            },
            children: vec![],
        };
        let mut editor = Editor::from_portable(&source).unwrap();
        let original = editor.root();
        let definition = editor.insert_global_definition();
        assert_eq!(
            editor.arena().get(definition).unwrap().kind(),
            &NodeKind::Binary(BinaryOp::GlobalDefine)
        );
        assert_eq!(
            editor.arena().get(definition).unwrap().children()[0],
            original
        );
        editor
            .set_atom(editor.caret().node, NodeKind::Number("7".into()))
            .unwrap();
        let before = editor.to_portable().unwrap();
        let evaluation = editor.insert_evaluation();
        assert_eq!(
            editor.arena().get(definition).unwrap().children()[1],
            evaluation
        );
        assert_eq!(
            editor.arena().get(evaluation).unwrap().kind(),
            &NodeKind::Binary(BinaryOp::Evaluate)
        );
        let revision = editor.arena().revision();
        assert_eq!(editor.insert_evaluation(), evaluation);
        assert_eq!(editor.arena().revision(), revision);
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), before);
        assert!(editor.undo());
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), source);
        assert!(!editor.undo());
    }

    #[test]
    fn a_function_call_becomes_a_global_function_declaration_as_one_command() {
        let source = PortableTree {
            kind: NodeKind::Call { argument_count: 2 },
            children: vec![atom("f"), atom("x"), atom("y")],
        };
        let mut editor = Editor::from_portable(&source).unwrap();
        editor.insert_global_definition();
        let tree = editor.to_portable().unwrap();
        assert_eq!(tree.kind, NodeKind::Binary(BinaryOp::GlobalDefine));
        assert_eq!(
            tree.children[0].kind,
            NodeKind::Function { parameter_count: 2 }
        );
        assert_eq!(tree.children[0].children, source.children);
        assert_eq!(tree.children[1].kind, NodeKind::Placeholder);
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), source);
        assert!(!editor.undo());
    }

    #[test]
    fn existing_definitions_evaluations_or_expression_targets_do_not_gain_a_nested_global() {
        for source in [
            binary(BinaryOp::Define, atom("x"), atom("y")),
            binary(BinaryOp::GlobalDefine, atom("x"), atom("y")),
            binary(
                BinaryOp::Evaluate,
                atom("x"),
                PortableTree {
                    kind: NodeKind::Placeholder,
                    children: vec![],
                },
            ),
            binary(BinaryOp::Add, atom("x"), atom("y")),
            PortableTree {
                kind: NodeKind::Index { indices: 1 },
                children: vec![binary(BinaryOp::Add, atom("x"), atom("y")), atom("i")],
            },
            PortableTree {
                kind: NodeKind::Program { statement_count: 1 },
                children: vec![atom("x")],
            },
        ] {
            let mut editor = Editor::from_portable(&source).unwrap();
            let root = editor.root();
            let revision = editor.arena().revision();
            assert_eq!(editor.insert_global_definition(), root);
            assert_eq!(editor.arena().revision(), revision);
            assert_eq!(editor.to_portable().unwrap(), source);
            assert!(!editor.undo());
        }
    }

    #[test]
    fn cross_product_uses_its_original_binding_power_and_right_associativity() {
        let mut editor = Editor::from_portable(&atom("a")).unwrap();
        editor.insert_cross_product();
        editor
            .set_atom(editor.caret().node, NodeKind::Identifier("b".into()))
            .unwrap();
        editor.insert_cross_product();
        editor
            .set_atom(editor.caret().node, NodeKind::Identifier("c".into()))
            .unwrap();
        assert_eq!(
            editor.to_portable().unwrap(),
            binary(
                BinaryOp::CrossProduct,
                atom("a"),
                binary(BinaryOp::CrossProduct, atom("b"), atom("c"))
            )
        );
        let mut editor =
            Editor::from_portable(&binary(BinaryOp::Multiply, atom("a"), atom("b"))).unwrap();
        let rhs = editor.arena().get(editor.root()).unwrap().children()[1];
        editor.place_caret(rhs, Direction::Right).unwrap();
        editor.insert_cross_product();
        editor
            .set_atom(editor.caret().node, NodeKind::Identifier("c".into()))
            .unwrap();
        assert_eq!(
            editor.to_portable().unwrap(),
            binary(
                BinaryOp::CrossProduct,
                binary(BinaryOp::Multiply, atom("a"), atom("b")),
                atom("c")
            )
        );
    }

    #[test]
    fn vectorize_is_a_container_with_boundary_exit_deletion_and_single_undo() {
        let source = atom("A");
        let mut editor = Editor::from_portable(&source).unwrap();
        let vectorize = editor.insert_vectorize();
        assert_eq!(editor.to_portable().unwrap().kind, NodeKind::Vectorize);
        assert!(editor.exit_group_right());
        assert_eq!(editor.caret().node, vectorize);
        assert!(!editor.delete_structure(true));
        editor
            .place_operator_caret(vectorize, Direction::Indeterminate)
            .unwrap();
        assert!(editor.delete_structure(true));
        assert_eq!(editor.to_portable().unwrap(), source);
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap().kind, NodeKind::Vectorize);
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), source);
    }

    #[test]
    fn global_cross_and_vectorize_do_not_mutate_matrix_cell_ranges() {
        let source = PortableTree {
            kind: NodeKind::Matrix {
                rows: 2,
                cols: 2,
                display: None,
            },
            children: vec![atom("A"); 4],
        };
        let mut editor = Editor::from_portable(&source).unwrap();
        let root = editor.root();
        editor.select_matrix_cells(root, 0, 3);
        let selection = editor.selection();
        for command in [
            Editor::insert_global_definition,
            Editor::insert_vectorize,
            Editor::insert_cross_product,
        ] {
            assert_eq!(command(&mut editor), root);
            assert_eq!(editor.selection(), selection);
            assert_eq!(editor.to_portable().unwrap(), source);
            assert!(!editor.undo());
        }
    }
}
