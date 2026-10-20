//! Bounded scaling-cell default labels from the shipped nullable-label walk.
//! Calls/binders and imported NativeIdentifier metadata remain untouched.
use super::*;
impl Editor {
    /// Apply scaling-cell defaults only inside the proven arithmetic subset.
    /// Group with the actual input operation to retain one local Undo step.
    pub fn auto_label_scaling_identifiers(&mut self) -> bool {
        fn visit(
            arena: &Arena,
            id: NodeId,
            prohibited: bool,
            changes: &mut Vec<(NodeId, NodeKind)>,
        ) {
            let Some(node) = arena.get(id) else { return };
            match &node.kind {
                NodeKind::Identifier(name) => changes.push((
                    id,
                    NodeKind::AnnotatedIdentifier {
                        name: name.clone(),
                        labels: if prohibited { "*" } else { "UNIT" }.into(),
                    },
                )),
                NodeKind::Group
                | NodeKind::UnaryNeg
                | NodeKind::Fraction
                | NodeKind::Power
                | NodeKind::Root
                | NodeKind::Absolute
                | NodeKind::Percent
                | NodeKind::Degree
                | NodeKind::Factorial
                | NodeKind::Binary(
                    BinaryOp::Add
                    | BinaryOp::Subtract
                    | BinaryOp::Multiply
                    | BinaryOp::Scale
                    | BinaryOp::InlineDivide,
                ) => {
                    for child in &node.children {
                        visit(arena, *child, prohibited, changes);
                    }
                }
                NodeKind::Index { .. } | NodeKind::Row | NodeKind::Column => {
                    for (i, child) in node.children.iter().enumerate() {
                        visit(arena, *child, prohibited || i == 1, changes);
                    }
                }
                _ => (),
            }
        }
        let mut changes = Vec::new();
        visit(&self.arena, self.root, false, &mut changes);
        if changes.is_empty() {
            return false;
        }
        let scope = self.scope.clone();
        self.checkpoint();
        self.scope = scope;
        for (id, kind) in changes {
            self.arena.node_mut(id).kind = kind;
        }
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn t(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
        PortableTree { kind, children }
    }
    #[test]
    fn defaults_group_with_input_keep_caret_and_leave_imported_labels_untouched() {
        let mut e = Editor::new();
        let before = e.to_portable().unwrap();
        e.edit_group(|e| {
            e.set_atom(e.root(), NodeKind::Identifier("m".into()))
                .unwrap();
            let caret = e.caret();
            e.auto_label_scaling_identifiers();
            assert_eq!(e.caret(), caret);
        });
        assert!(
            matches!(e.to_portable().unwrap().kind,NodeKind::AnnotatedIdentifier{ref labels,..} if labels=="UNIT")
        );
        assert!(e.undo());
        assert_eq!(e.to_portable().unwrap(), before);
        assert!(e.redo());
        let explicit = t(
            NodeKind::NativeIdentifier {
                name: "m".into(),
                labels: "VARIABLE".into(),
                contextual: Some(true),
                subscript: Some("𝛼".into()),
            },
            vec![],
        );
        let mut e = Editor::from_portable(&explicit).unwrap();
        let revision = e.arena().revision();
        assert!(!e.auto_label_scaling_identifiers());
        assert_eq!(e.arena().revision(), revision);
        assert_eq!(e.to_portable().unwrap(), explicit);
    }
    #[test]
    fn index_label_defaults_are_nullable_only_and_call_repair_remains_outside_subset() {
        let original = t(
            NodeKind::Index { indices: 1 },
            vec![
                t(NodeKind::Identifier("v".into()), vec![]),
                t(NodeKind::Identifier("k".into()), vec![]),
            ],
        );
        let mut e = Editor::from_portable(&original).unwrap();
        assert!(e.auto_label_scaling_identifiers());
        let tree = e.to_portable().unwrap();
        assert!(
            matches!(&tree.children[0].kind,NodeKind::AnnotatedIdentifier{labels,..} if labels=="UNIT")
        );
        assert!(
            matches!(&tree.children[1].kind,NodeKind::AnnotatedIdentifier{labels,..} if labels=="*")
        );
        assert!(!e.auto_label_scaling_identifiers());
        let call = t(
            NodeKind::Call { argument_count: 1 },
            vec![
                t(NodeKind::Identifier("sin".into()), vec![]),
                t(NodeKind::Identifier("x".into()), vec![]),
            ],
        );
        let mut e = Editor::from_portable(&call).unwrap();
        assert!(!e.auto_label_scaling_identifiers());
        assert_eq!(e.to_portable().unwrap(), call);
    }
}
