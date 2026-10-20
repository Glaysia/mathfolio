//! Default ParenthesizeOperands replacement at an ordinary infix center.
//! Native l0088 -> a2.m0533 / NearestNeighborSelector106a; see contract0129.
use super::*;

fn ordinary_infix(kind: &NodeKind) -> bool {
    matches!(kind, NodeKind::Fraction | NodeKind::Power)
        || matches!(kind, NodeKind::Binary(_)) && typed_kind_allowed(kind)
}

impl Editor {
    pub(super) fn typed_replace_operator_center(&mut self, mut kind: NodeKind) -> bool {
        let target = self.caret.node;
        let source = self.arena.get(target).unwrap().clone();
        if kind == NodeKind::UnaryNeg {
            kind = NodeKind::Binary(BinaryOp::Subtract);
        }
        if !ordinary_infix(&source.kind) || !ordinary_infix(&kind) || source.kind == kind {
            return false;
        }
        if self.typed_container_root(target).is_none() {
            return false;
        }
        // A former container's equal-binding loss needs the complete a3 slot
        // repair; keep that unrepresented branch atomic rather than rebind it.
        if source.children.iter().enumerate().any(|(slot, child)| {
            let old_container =
                source.kind == NodeKind::Fraction || source.kind == NodeKind::Power && slot == 1;
            let new_container = kind == NodeKind::Fraction || kind == NodeKind::Power && slot == 1;
            old_container
                && !new_container
                && self.arena.get(*child).unwrap().kind.precedence() == kind.precedence()
        }) {
            return false;
        }
        let right = source.children[1];
        let mut left = source.children[0];
        // c8c's last-child branch: container slots stop adjacency. Ordinary
        // represented infixes are left associative. Higher-precedence operands
        // remain intact; a same/lower-precedence chain contributes its last term.
        if !matches!(source.kind, NodeKind::Fraction | NodeKind::Power) {
            let old_left = self.arena.get(left).unwrap();
            if matches!(old_left.kind, NodeKind::Binary(_))
                && old_left.kind.precedence() <= source.kind.precedence()
            {
                if !ordinary_infix(&old_left.kind) {
                    return false;
                }
                left = old_left.children[1];
            }
        }
        self.arena.node_mut(target).kind = kind.clone();
        if self.arena.get(left).unwrap().parent != Some(target) {
            // 0533 IL0097..0102 relocates the retained operator between the
            // nearest operands, before grouping or whole-container reparsing.
            let old_parent = self.arena.get(left).unwrap().parent.unwrap();
            let grandparent = self.arena.get(old_parent).unwrap().parent;
            self.replace_child(grandparent, old_parent, left);
            let old_first = self.arena.get(target).unwrap().children[0];
            self.arena.node_mut(old_parent).children[1] = old_first;
            self.arena.node_mut(old_first).parent = Some(old_parent);
            self.arena.node_mut(target).children[0] = old_parent;
            self.arena.node_mut(old_parent).parent = Some(target);
            let join = self.arena.get(old_parent).unwrap().children[1];
            let parent = self.arena.get(target).unwrap().parent;
            self.replace_child(parent, target, old_parent);
            self.arena.node_mut(old_parent).parent = parent;
            self.replace_child(self.arena.get(join).unwrap().parent, join, right);
            self.replace_child(self.arena.get(right).unwrap().parent, right, target);
            self.arena.node_mut(target).children = vec![join, right];
            self.arena.node_mut(join).parent = Some(target);
            self.arena.node_mut(right).parent = Some(target);
        }
        for slot in 0..2 {
            let child = self.arena.get(target).unwrap().children[slot];
            if !self.grouping_container_slot(target, slot)
                && self.arena.get(child).unwrap().kind.precedence() < kind.precedence()
            {
                let group = self.arena.allocate(NodeKind::Group, vec![child]);
                self.replace_child(Some(target), child, group);
            }
        }
        if let Some(parent) = self.arena.get(target).unwrap().parent {
            let slot = self
                .arena
                .get(parent)
                .unwrap()
                .children
                .iter()
                .position(|id| *id == target)
                .unwrap();
            if !self.grouping_container_slot(parent, slot)
                && self.arena.get(parent).unwrap().kind.precedence() > kind.precedence()
            {
                let group = self.arena.allocate(NodeKind::Group, vec![target]);
                self.replace_child(Some(parent), target, group);
            }
        }
        let Some(container) = self.typed_container_root(target) else {
            return false;
        };
        let parent = self.arena.get(container).unwrap().parent;
        let mut handles = vec![];
        if !self.typed_emit(container, &mut handles, 0) {
            return false;
        }
        let handles = self.typed_normalize(handles);
        let Some(root) = self.typed_parse(&handles) else {
            return false;
        };
        self.replace_child(parent, container, root);
        self.arena.node_mut(root).parent = parent;
        self.typed_power_group_repair(root);
        self.caret = Caret::at(target, Direction::Indeterminate);
        self.selection = Selection::None;
        self.scope = None;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn n(value: &str) -> PortableTree {
        PortableTree {
            kind: NodeKind::Number(value.into()),
            children: vec![],
        }
    }
    fn op(kind: NodeKind, a: PortableTree, b: PortableTree) -> PortableTree {
        PortableTree {
            kind,
            children: vec![a, b],
        }
    }
    fn add(a: PortableTree, b: PortableTree) -> PortableTree {
        op(NodeKind::Binary(BinaryOp::Add), a, b)
    }
    #[test]
    fn replacement_reuses_operands_and_center_as_one_undo_command() {
        for kind in [
            NodeKind::Binary(BinaryOp::Multiply),
            NodeKind::Binary(BinaryOp::LessThan),
            NodeKind::Fraction,
        ] {
            let source = add(n("12"), n("34"));
            let mut e = Editor::from_portable(&source).unwrap();
            e.place_operator_caret(e.root(), Direction::Indeterminate)
                .unwrap();
            let root = e.root();
            let children = e.arena.get(root).unwrap().children.clone();
            let caret = e.caret();
            assert!(e.insert_typed_operator(kind.clone()));
            assert_eq!(e.to_portable().unwrap(), op(kind, n("12"), n("34")));
            assert_eq!(e.root(), root);
            assert_eq!(e.arena.get(root).unwrap().children, children);
            assert_eq!(e.caret().caret_side, Direction::Indeterminate);
            assert!(e.undo());
            assert_eq!(e.to_portable().unwrap(), source);
            assert_eq!(e.caret(), caret);
            assert!(e.redo());
            assert!(!e.redo());
        }
    }
    #[test]
    fn replacement_uses_adjacent_terms_in_a_chain_and_preserves_stronger_parent_binding() {
        let source = add(add(n("1"), n("2")), n("3"));
        let mut e = Editor::from_portable(&source).unwrap();
        e.place_operator_caret(e.root(), Direction::Indeterminate)
            .unwrap();
        assert!(e.insert_typed_operator(NodeKind::Binary(BinaryOp::Multiply)));
        assert_eq!(
            e.to_portable().unwrap(),
            add(
                n("1"),
                op(NodeKind::Binary(BinaryOp::Multiply), n("2"), n("3"))
            )
        );
        let source = op(NodeKind::Binary(BinaryOp::Multiply), n("1"), n("2"));
        let source = op(NodeKind::Power, source, n("3"));
        let mut e = Editor::from_portable(&source).unwrap();
        let target = e.arena.get(e.root()).unwrap().children[0];
        e.place_operator_caret(target, Direction::Indeterminate)
            .unwrap();
        assert!(e.insert_typed_operator(NodeKind::Binary(BinaryOp::Add)));
        assert_eq!(
            e.to_portable().unwrap(),
            op(
                NodeKind::Power,
                PortableTree {
                    kind: NodeKind::Group,
                    children: vec![add(n("1"), n("2"))]
                },
                n("3")
            )
        );
    }

    #[test]
    fn power_descriptor_replacement_preserves_its_two_operands_and_exponent_container() {
        for (old, new, left, right) in [
            (
                NodeKind::Binary(BinaryOp::Add),
                NodeKind::Power,
                n("2"),
                n("3"),
            ),
            (
                NodeKind::Power,
                NodeKind::Binary(BinaryOp::Add),
                n("2"),
                n("3"),
            ),
            (NodeKind::Power, NodeKind::Fraction, n("6"), n("3")),
            (NodeKind::Fraction, NodeKind::Power, n("2"), n("3")),
            (
                NodeKind::Binary(BinaryOp::Add),
                NodeKind::Power,
                n("2"),
                op(NodeKind::Binary(BinaryOp::Multiply), n("3"), n("4")),
            ),
        ] {
            let source = op(old, left.clone(), right.clone());
            let mut editor = Editor::from_portable(&source).unwrap();
            editor
                .place_operator_caret(editor.root(), Direction::Indeterminate)
                .unwrap();
            assert!(editor.insert_typed_operator(new.clone()));
            assert_eq!(editor.to_portable().unwrap(), op(new, left, right));
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), source);
        }
    }
    #[test]
    fn center_minus_is_binary_and_rejected_replacement_keeps_redo_and_caret() {
        let source = add(n("12"), n("34"));
        let mut e = Editor::from_portable(&source).unwrap();
        e.place_operator_caret(e.root(), Direction::Indeterminate)
            .unwrap();
        assert!(e.insert_typed_operator(NodeKind::UnaryNeg));
        assert_eq!(
            e.to_portable().unwrap(),
            op(NodeKind::Binary(BinaryOp::Subtract), n("12"), n("34"))
        );
        assert!(e.undo());
        let caret = e.caret();
        let revision = e.arena.revision();
        assert!(!e.insert_typed_operator(NodeKind::Binary(BinaryOp::Add)));
        assert!(!e.insert_typed_operator(NodeKind::LogicalNot));
        assert_eq!(e.to_portable().unwrap(), source);
        assert_eq!(e.caret(), caret);
        assert_eq!(e.arena.revision(), revision);
        assert!(e.redo());
    }
}
