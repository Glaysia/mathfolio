//! Original Percent, Degree and Polar authoring operators. Degree is not the
//! separate affine DMS conversion function. See analysis/0077.
use super::*;

impl Editor {
    pub fn insert_percent(&mut self) -> NodeId {
        self.insert_postfix_operator(NodeKind::Percent)
    }

    pub fn insert_degree(&mut self) -> NodeId {
        self.insert_postfix_operator(NodeKind::Degree)
    }

    pub fn insert_polar(&mut self) -> NodeId {
        self.insert_infix(BinaryOp::Polar)
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
    fn binary(op: BinaryOp, lhs: PortableTree, rhs: PortableTree) -> PortableTree {
        PortableTree {
            kind: NodeKind::Binary(op),
            children: vec![lhs, rhs],
        }
    }

    #[test]
    fn descriptor_identity_arity_binding_power_are_original() {
        for (kind, name, count, precedence) in [
            (NodeKind::Percent, "PERCENT", 1, 180),
            (NodeKind::Degree, "DEGREE", 1, 180),
            (NodeKind::Binary(BinaryOp::Polar), "POLAR", 2, 120),
        ] {
            assert_eq!(kind.descriptor_name(), name);
            assert_eq!(kind.arity(), count);
            assert_eq!(kind.precedence(), precedence);
        }
    }

    #[test]
    fn postfix_commands_preserve_operand_identity_metadata_and_single_undo() {
        let source = PortableTree {
            kind: NodeKind::NativeIdentifier {
                name: "x".into(),
                labels: "VARIABLE".into(),
                contextual: Some(false),
                subscript: Some("rms".into()),
            },
            children: vec![],
        };
        for (command, kind) in [
            (
                Editor::insert_percent as fn(&mut Editor) -> NodeId,
                NodeKind::Percent,
            ),
            (Editor::insert_degree, NodeKind::Degree),
        ] {
            let mut editor = Editor::from_portable(&source).unwrap();
            let original = editor.root();
            let wrapper = command(&mut editor);
            assert_eq!(editor.arena().get(wrapper).unwrap().children(), &[original]);
            assert_eq!(
                editor.to_portable().unwrap(),
                PortableTree {
                    kind: kind.clone(),
                    children: vec![source.clone()]
                }
            );
            assert_eq!(editor.caret(), Caret::at(wrapper, Direction::Right));
            assert!(editor.delete_structure(true));
            assert_eq!(editor.to_portable().unwrap(), source);
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap().kind, kind);
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), source);
            assert!(!editor.undo());
        }
    }

    #[test]
    fn percent_groups_power_and_nth_root_but_degree_does_not_invent_grouping() {
        for source in [
            PortableTree {
                kind: NodeKind::Power,
                children: vec![atom("a"), atom("b")],
            },
            PortableTree {
                kind: NodeKind::Root,
                children: vec![atom("a"), atom("b")],
            },
        ] {
            let mut editor = Editor::from_portable(&source).unwrap();
            editor.select_subtree(editor.root()).unwrap();
            editor.insert_percent();
            let tree = editor.to_portable().unwrap();
            assert_eq!(tree.kind, NodeKind::Percent);
            assert_eq!(tree.children[0].kind, NodeKind::Group);
            assert_eq!(tree.children[0].children[0], source);
            assert!(editor.undo());
            editor.select_subtree(editor.root()).unwrap();
            editor.insert_degree();
            assert_eq!(editor.to_portable().unwrap().children[0], source);
        }
    }

    #[test]
    fn empty_postfix_keeps_operand_caret_and_polar_is_left_associative() {
        for command in [Editor::insert_percent, Editor::insert_degree] {
            let mut editor = Editor::new();
            let original = editor.root();
            command(&mut editor);
            assert_eq!(editor.caret().node, original);
            assert_eq!(
                editor.arena().get(original).unwrap().kind(),
                &NodeKind::Placeholder
            );
        }
        let mut editor = Editor::from_portable(&atom("a")).unwrap();
        editor.insert_polar();
        editor
            .set_atom(editor.caret().node, NodeKind::Identifier("b".into()))
            .unwrap();
        editor.insert_polar();
        editor
            .set_atom(editor.caret().node, NodeKind::Identifier("c".into()))
            .unwrap();
        assert_eq!(
            editor.to_portable().unwrap(),
            binary(
                BinaryOp::Polar,
                binary(BinaryOp::Polar, atom("a"), atom("b")),
                atom("c")
            )
        );
    }

    #[test]
    fn notation_operators_respect_matrix_ranges_and_expression_only_context() {
        let source = PortableTree {
            kind: NodeKind::Matrix {
                rows: 1,
                cols: 2,
                display: None,
            },
            children: vec![atom("a"), atom("b")],
        };
        let mut editor = Editor::from_portable(&source).unwrap();
        editor.select_matrix_cells(editor.root(), 0, 1);
        let revision = editor.arena().revision();
        for command in [
            Editor::insert_percent,
            Editor::insert_degree,
            Editor::insert_polar,
        ] {
            command(&mut editor);
            assert_eq!(editor.arena().revision(), revision);
            assert_eq!(editor.to_portable().unwrap(), source);
            assert!(!editor.undo());
        }
        let mut editor = Editor::from_portable(&atom("a")).unwrap();
        editor.set_expressions_only(true);
        editor.insert_degree();
        editor.insert_polar();
        assert_eq!(
            editor.to_portable().unwrap().kind,
            NodeKind::Binary(BinaryOp::Polar)
        );
        assert!(editor.expressions_only());
    }

    #[test]
    fn polar_preserves_original_negation_addition_and_angle_multiplication_binding() {
        // The documented -x∠y uses a negative magnitude; a following + z is
        // outside Polar, while a following multiplication belongs to its angle.
        let mut editor = Editor::from_portable(&PortableTree {
            kind: NodeKind::UnaryNeg,
            children: vec![atom("x")],
        })
        .unwrap();
        editor.insert_polar();
        editor
            .set_atom(editor.caret().node, NodeKind::Identifier("y".into()))
            .unwrap();
        assert_eq!(
            editor.to_portable().unwrap(),
            binary(
                BinaryOp::Polar,
                PortableTree {
                    kind: NodeKind::UnaryNeg,
                    children: vec![atom("x")]
                },
                atom("y")
            )
        );
        editor.insert_infix(BinaryOp::Multiply);
        editor
            .set_atom(editor.caret().node, NodeKind::Identifier("z".into()))
            .unwrap();
        let polar = binary(
            BinaryOp::Polar,
            PortableTree {
                kind: NodeKind::UnaryNeg,
                children: vec![atom("x")],
            },
            binary(BinaryOp::Multiply, atom("y"), atom("z")),
        );
        assert_eq!(editor.to_portable().unwrap(), polar);
        editor.insert_infix(BinaryOp::Add);
        editor
            .set_atom(editor.caret().node, NodeKind::Identifier("w".into()))
            .unwrap();
        assert_eq!(
            editor.to_portable().unwrap(),
            binary(BinaryOp::Add, polar, atom("w"))
        );
    }

    #[test]
    fn mixed_equal_precedence_uses_the_incoming_operator_associativity() {
        for (first, second, expected) in [
            (
                BinaryOp::Polar,
                BinaryOp::CrossProduct,
                binary(
                    BinaryOp::Polar,
                    atom("a"),
                    binary(BinaryOp::CrossProduct, atom("b"), atom("c")),
                ),
            ),
            (
                BinaryOp::CrossProduct,
                BinaryOp::Polar,
                binary(
                    BinaryOp::Polar,
                    binary(BinaryOp::CrossProduct, atom("a"), atom("b")),
                    atom("c"),
                ),
            ),
        ] {
            let mut editor = Editor::from_portable(&atom("a")).unwrap();
            editor.insert_infix(first);
            editor
                .set_atom(editor.caret().node, NodeKind::Identifier("b".into()))
                .unwrap();
            let before = editor.to_portable().unwrap();
            editor.insert_infix(second);
            editor
                .set_atom(editor.caret().node, NodeKind::Identifier("c".into()))
                .unwrap();
            assert_eq!(editor.to_portable().unwrap(), expected);
            assert!(editor.undo());
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), before);
        }
    }
}
