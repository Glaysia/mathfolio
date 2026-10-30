//! Bounded native failed-deletion mode. REA contract 0126: this is distinct
//! from Scope, and its first key is navigation rather than a document command.
use super::*;

#[derive(Clone, Debug)]
pub(super) struct TypeoverState {
    target: NodeId,
    origin: Caret,
    power_suffix: bool,
}

impl Editor {
    /// The operator whose native analysis segment is in Typeover.
    /// General native LinearSegment/APPLY selections remain unsupported.
    pub fn typeover_target(&self) -> Option<NodeId> {
        self.typeover.as_ref().map(|state| state.target)
    }

    /// Power Typeover selects the postfix script handle, excluding its base.
    /// Fraction selects its entire opaque handle and returns None here.
    pub fn typeover_excluded_operand(&self) -> Option<NodeId> {
        let state = self.typeover.as_ref()?;
        state
            .power_suffix
            .then(|| self.arena.get(state.target).unwrap().children[0])
    }

    fn typeover_parent_allowed(&self, target: NodeId) -> bool {
        let Some(item) = self.arena.get(target) else {
            return false;
        };
        let Some(parent) = item.parent else {
            return target == self.root;
        };
        if parent != self.root {
            return false;
        }
        let owner = self.arena.get(parent).unwrap();
        match owner.kind {
            NodeKind::Binary(BinaryOp::Evaluate) => owner.children[0] == target,
            NodeKind::Binary(BinaryOp::Define | BinaryOp::GlobalDefine) => {
                owner.children[1] == target
            }
            _ => false,
        }
    }

    /// Cancel the temporary selection and restore its original caret. Native
    /// linear arrow movement consumes its first key by taking this path only.
    pub fn cancel_typeover(&mut self) -> bool {
        let Some(state) = self.typeover.take() else {
            return false;
        };
        self.caret = state.origin;
        self.selection = Selection::None;
        self.scope = None;
        true
    }

    pub(super) fn begin_root_typeover(&mut self, backwards: bool) -> bool {
        if self.selection != Selection::None
            || self.scope.is_some()
            || !self.typeover_parent_allowed(self.caret.node)
            || self.caret.value_range.is_some()
            || self.caret.insertion != InsertionMode::Normal
            || self.caret.operator_part.is_some()
        {
            return false;
        }
        let target = self.caret.node;
        let item = self.arena.get(target).unwrap();
        let side = self.caret.caret_side;
        let inward = match item.kind {
            NodeKind::Power => backwards && side == Direction::Right,
            NodeKind::Fraction => {
                (backwards && side == Direction::Right) || (!backwards && side == Direction::Left)
            }
            _ => false,
        };
        // Empty/incomplete templates can be rewritten by earlier native stages;
        // do not reinterpret those unmodeled paths as an unconditional selection.
        if !inward
            || self.subtree_has_placeholder(target)
            || (item.kind == NodeKind::Power
                && self
                    .arena
                    .get(item.children[0])
                    .unwrap()
                    .kind
                    .text()
                    .is_none())
        {
            return false;
        }
        self.typeover = Some(TypeoverState {
            target,
            origin: self.caret,
            power_suffix: item.kind == NodeKind::Power,
        });
        true
    }

    fn subtree_has_placeholder(&self, node: NodeId) -> bool {
        let item = self.arena.get(node).unwrap();
        item.kind == NodeKind::Placeholder
            || item
                .children
                .iter()
                .any(|child| self.subtree_has_placeholder(*child))
    }

    pub(super) fn delete_typeover(&mut self) -> bool {
        if let Some(state) = &self.typeover
            && state.power_suffix
        {
            let target = state.target;
            if !self.typeover_parent_allowed(target) {
                return false;
            }
            let mut candidate = self.clone_without_history();
            let item = candidate.arena.get(target).unwrap().clone();
            let base = item.children[0];
            let Some(caret) = candidate.deletion_right_entry(base) else {
                return false;
            };
            candidate.replace_child(item.parent, target, base);
            candidate.arena.node_mut(base).parent = item.parent;
            candidate.retire(target, Some(base));
            candidate.caret = caret;
            if candidate.to_portable().is_err() {
                return false;
            }
            self.checkpoint();
            self.arena = candidate.arena;
            self.root = candidate.root;
            self.caret = caret;
            self.selection = Selection::None;
            self.scope = None;
            self.typeover = None;
            return true;
        }
        self.replace_typeover_tree(
            &PortableTree {
                kind: NodeKind::Placeholder,
                children: vec![],
            },
            Direction::Left,
        )
    }

    /// Replace a pending whole-term selection atomically. `focus` is the native
    /// entry side selected by the caller's insertion contract: value insertion
    /// uses Right; deletion uses Left. This does not infer operator templates.
    pub fn replace_typeover_tree(&mut self, tree: &PortableTree, focus: Direction) -> bool {
        let Some(state) = self.typeover.as_ref() else {
            return false;
        };
        if state.power_suffix
            || !self.typeover_parent_allowed(state.target)
            || !matches!(focus, Direction::Left | Direction::Right)
            || tree.validate().is_err()
        {
            return false;
        }
        let mut candidate = self.clone_without_history();
        let old = state.target;
        let parent = candidate.arena.get(old).unwrap().parent;
        let new = import_node(&mut candidate.arena, tree);
        candidate.replace_child(parent, old, new);
        candidate.arena.node_mut(new).parent = parent;
        candidate.retire(old, None);
        let caret = if tree.kind == NodeKind::Placeholder {
            Some(Caret::at(new, focus))
        } else if focus == Direction::Left {
            candidate.program_operand_left_entry(new)
        } else {
            candidate.deletion_right_entry(new)
        };
        let Some(caret) = caret else {
            return false;
        };
        candidate.caret = caret;
        candidate.typeover = None;
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
        self.typeover = None;
        true
    }

    /// Transplant a caller-built expression and its exact relative caret. This
    /// keeps verified insertion templates in their existing command path instead
    /// of inventing a universal first-placeholder cursor for Typeover.
    pub fn replace_typeover_editor(&mut self, template: &Editor) -> bool {
        let Some(state) = self.typeover.as_ref() else {
            return false;
        };
        if state.power_suffix
            || !self.typeover_parent_allowed(state.target)
            || template.typeover.is_some()
            || template.selection != Selection::None
        {
            return false;
        }
        let Ok(tree) = template.to_portable() else {
            return false;
        };
        let mut path = vec![];
        let mut node = template.caret.node;
        while node != template.root {
            let Some(parent) = template.arena.get(node).and_then(Node::parent) else {
                return false;
            };
            let Some(slot) = template
                .arena
                .get(parent)
                .unwrap()
                .children
                .iter()
                .position(|child| *child == node)
            else {
                return false;
            };
            path.push(slot);
            node = parent;
        }
        let mut candidate = self.clone_without_history();
        let old = state.target;
        let parent = candidate.arena.get(old).unwrap().parent;
        let new = import_node(&mut candidate.arena, &tree);
        candidate.replace_child(parent, old, new);
        candidate.arena.node_mut(new).parent = parent;
        candidate.retire(old, None);
        let mut caret_node = new;
        for slot in path.into_iter().rev() {
            caret_node = candidate.arena.get(caret_node).unwrap().children[slot];
        }
        let mut caret = template.caret;
        caret.node = caret_node;
        candidate.caret = caret;
        candidate.typeover = None;
        candidate.selection = Selection::None;
        candidate.scope = None;
        if candidate.to_portable().is_err() {
            return false;
        }
        self.checkpoint();
        self.arena = candidate.arena;
        self.root = candidate.root;
        self.caret = caret;
        self.selection = Selection::None;
        self.scope = None;
        self.typeover = None;
        true
    }

    /// Native formatted-value Typeover replacement enters the new value at its
    /// right boundary (bc05b8). Factory/grammar classification belongs to caller.
    pub fn replace_typeover_value(&mut self, kind: NodeKind) -> bool {
        if kind.arity() != 0 || kind.text().is_none() {
            return false;
        }
        self.replace_typeover_tree(
            &PortableTree {
                kind,
                children: vec![],
            },
            Direction::Right,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source(kind: NodeKind) -> PortableTree {
        PortableTree {
            kind,
            children: vec![
                PortableTree {
                    kind: NodeKind::Number("12".into()),
                    children: vec![],
                },
                PortableTree {
                    kind: NodeKind::Number("34".into()),
                    children: vec![],
                },
            ],
        }
    }
    fn pending(kind: NodeKind, side: Direction, backwards: bool) -> Editor {
        let mut e = Editor::from_portable(&source(kind)).unwrap();
        e.place_caret(e.root, side).unwrap();
        let before = e.to_portable().unwrap();
        let revision = e.arena.revision();
        let caret = e.caret;
        assert!(!e.delete_structure(backwards));
        assert_eq!(e.typeover_target(), Some(e.root));
        assert_eq!(e.caret, caret);
        assert_eq!(e.to_portable().unwrap(), before);
        assert_eq!(e.arena.revision(), revision);
        assert!(e.undo.is_empty() && e.redo.is_empty());
        assert_eq!(e.selection, Selection::None);
        e
    }
    #[test]
    fn first_key_selects_second_either_direction_deletes_and_undo_restores_red() {
        for (kind, side, backwards) in [
            (NodeKind::Power, Direction::Right, true),
            (NodeKind::Fraction, Direction::Right, true),
            (NodeKind::Fraction, Direction::Left, false),
        ] {
            for second in [false, true] {
                let mut e = pending(kind.clone(), side, backwards);
                let old = e.to_portable().unwrap();
                let caret = e.caret;
                assert!(e.delete_structure(second));
                assert_eq!(
                    e.arena.get(e.root).unwrap().kind,
                    if kind == NodeKind::Power {
                        NodeKind::Number("12".into())
                    } else {
                        NodeKind::Placeholder
                    }
                );
                if kind == NodeKind::Power {
                    assert_eq!(
                        e.caret.value_range,
                        Some(ValueRange {
                            anchor: 2,
                            focus: 2
                        })
                    );
                } else {
                    assert_eq!(e.caret.caret_side, Direction::Left);
                }
                assert_eq!(e.typeover_target(), None);
                assert!(e.undo());
                assert_eq!(e.to_portable().unwrap(), old);
                assert_eq!(e.typeover_target(), Some(e.root));
                assert_eq!(e.caret, caret);
                assert!(e.redo());
                assert_eq!(e.typeover_target(), None);
                assert_eq!(
                    e.arena.get(e.root).unwrap().kind,
                    if kind == NodeKind::Power {
                        NodeKind::Number("12".into())
                    } else {
                        NodeKind::Placeholder
                    }
                );
            }
        }
    }
    #[test]
    fn cancellation_and_click_are_navigation_only_and_preserve_redo() {
        let mut e = pending(NodeKind::Power, Direction::Right, true);
        let origin = e.caret;
        assert!(e.cancel_typeover());
        assert_eq!(e.caret, origin);
        assert!(!e.cancel_typeover());
        assert!(e.undo.is_empty());
        assert!(!e.delete_structure(true));
        let base = e.arena.get(e.root).unwrap().children[0];
        e.place_text_caret(base, 1).unwrap();
        assert_eq!(e.typeover_target(), None);
        assert!(e.undo.is_empty());
        let base = e.arena.get(e.root).unwrap().children[0];
        e.set_atom(base, NodeKind::Number("99".into())).unwrap();
        assert!(e.undo());
        e.place_caret(e.root, Direction::Right).unwrap();
        assert!(!e.delete_structure(true));
        assert_eq!(e.redo.len(), 1);
        assert!(e.cancel_typeover());
        assert_eq!(e.redo.len(), 1);
    }
    #[test]
    fn value_replacement_is_atomic_and_undo_restores_temporary_mode() {
        let mut e = pending(NodeKind::Fraction, Direction::Left, false);
        let old = e.to_portable().unwrap();
        assert!(!e.replace_typeover_value(NodeKind::Number("x".repeat(MAX_ATOM_BYTES + 1))));
        assert_eq!(e.to_portable().unwrap(), old);
        assert_eq!(e.typeover_target(), Some(e.root));
        assert!(e.undo.is_empty());
        assert!(e.replace_typeover_value(NodeKind::Number("7".into())));
        assert_eq!(
            e.caret.value_range,
            Some(ValueRange {
                anchor: 1,
                focus: 1
            })
        );
        assert_eq!(e.typeover_target(), None);
        assert!(e.undo());
        assert_eq!(e.to_portable().unwrap(), old);
        assert_eq!(e.typeover_target(), Some(e.root));
    }
    #[test]
    fn operator_template_transplant_retains_relative_placeholder_focus_and_one_undo() {
        for kind in [
            NodeKind::Binary(BinaryOp::Add),
            NodeKind::Fraction,
            NodeKind::Power,
            NodeKind::UnaryNeg,
            NodeKind::LogicalNot,
        ] {
            let mut template = Editor::new();
            assert!(template.insert_typed_operator(kind.clone()));
            let expected = template.to_portable().unwrap();
            let mut e = pending(NodeKind::Fraction, Direction::Right, true);
            let old = e.to_portable().unwrap();
            assert!(e.replace_typeover_editor(&template));
            assert_eq!(e.to_portable().unwrap(), expected);
            assert_eq!(
                e.arena.get(e.caret.node).unwrap().kind,
                NodeKind::Placeholder
            );
            assert_eq!(e.caret.caret_side, template.caret.caret_side);
            let parent = e.arena.get(e.caret.node).unwrap().parent.unwrap();
            assert_eq!(e.arena.get(parent).unwrap().children[0], e.caret.node);
            assert!(e.undo());
            assert_eq!(e.to_portable().unwrap(), old);
            assert_eq!(e.typeover_target(), Some(e.root));
            assert!(!e.undo());
        }
    }

    #[test]
    fn worksheet_shell_fraction_replacement_preserves_owner_and_sibling_ids() {
        for op in [BinaryOp::Evaluate, BinaryOp::Define, BinaryOp::GlobalDefine] {
            let slot = usize::from(op != BinaryOp::Evaluate);
            let sibling = PortableTree {
                kind: NodeKind::Identifier("kept".into()),
                children: vec![],
            };
            let mut children = vec![sibling.clone(), sibling];
            children[slot] = source(NodeKind::Fraction);
            let tree = PortableTree {
                kind: NodeKind::Binary(op),
                children,
            };
            let mut e = Editor::from_portable(&tree).unwrap();
            let owner = e.root;
            let target = e.arena.get(owner).unwrap().children[slot];
            let kept = e.arena.get(owner).unwrap().children[1 - slot];
            e.place_caret(target, Direction::Right).unwrap();
            assert!(!e.delete_structure(true));
            assert_eq!(e.typeover_target(), Some(target));
            assert_eq!(e.typeover_excluded_operand(), None);
            assert!(e.replace_typeover_value(NodeKind::Number("7".into())));
            assert_eq!(e.root, owner);
            assert_eq!(e.arena.get(owner).unwrap().children[1 - slot], kept);
            assert_eq!(
                e.arena
                    .get(e.arena.get(owner).unwrap().children[slot])
                    .unwrap()
                    .kind,
                NodeKind::Number("7".into())
            );
            assert!(e.undo());
            assert_eq!(e.to_portable().unwrap(), tree);
            assert_eq!(e.typeover_target(), Some(target));
            assert!(e.delete_structure(false));
            assert_eq!(e.root, owner);
            assert_eq!(e.arena.get(owner).unwrap().children[1 - slot], kept);
            assert_eq!(
                e.arena.get(e.caret.node).unwrap().kind,
                NodeKind::Placeholder
            );
            assert_eq!(e.caret.caret_side, Direction::Left);
        }
    }
    #[test]
    fn power_suffix_deletion_preserves_atomic_base_metadata_and_rejects_whole_replacement() {
        let base = PortableTree {
            kind: NodeKind::NativeIdentifier {
                name: "v".into(),
                labels: "UNIT".into(),
                contextual: Some(true),
                subscript: Some("dc".into()),
            },
            children: vec![],
        };
        for shell in [
            None,
            Some(BinaryOp::Evaluate),
            Some(BinaryOp::Define),
            Some(BinaryOp::GlobalDefine),
        ] {
            let mut power = source(NodeKind::Power);
            power.children[0] = base.clone();
            let (tree, slot) = if let Some(op) = shell {
                let slot = usize::from(op != BinaryOp::Evaluate);
                let mut children = vec![
                    PortableTree {
                        kind: NodeKind::Identifier("kept".into()),
                        children: vec![]
                    };
                    2
                ];
                children[slot] = power;
                (
                    PortableTree {
                        kind: NodeKind::Binary(op),
                        children,
                    },
                    Some(slot),
                )
            } else {
                (power, None)
            };
            let mut e = Editor::from_portable(&tree).unwrap();
            let owner = e.root;
            let target = slot.map_or(e.root, |slot| e.arena.get(e.root).unwrap().children[slot]);
            let base_id = e.arena.get(target).unwrap().children[0];
            e.place_caret(target, Direction::Right).unwrap();
            assert!(!e.delete_structure(true));
            assert_eq!(e.typeover_excluded_operand(), Some(base_id));
            assert!(!e.replace_typeover_value(NodeKind::Number("7".into())));
            assert!(!e.replace_typeover_editor(&Editor::new()));
            assert_eq!(e.to_portable().unwrap(), tree);
            assert!(e.undo.is_empty());
            assert!(e.delete_structure(true));
            assert_eq!(e.caret.node, base_id);
            assert_eq!(e.caret.caret_side, Direction::Right);
            assert_eq!(e.arena.get(base_id).unwrap().kind, base.kind);
            if let Some(slot) = slot {
                assert_eq!(e.root, owner);
                assert_eq!(e.arena.get(owner).unwrap().children[slot], base_id);
            } else {
                assert_eq!(e.root, base_id);
            }
            assert!(e.undo());
            assert_eq!(e.to_portable().unwrap(), tree);
            assert_eq!(e.typeover_excluded_operand(), Some(base_id));
        }
    }

    #[test]
    fn unsupported_outer_positions_and_incomplete_templates_remain_unchanged() {
        for (kind, side, backwards) in [
            (NodeKind::Power, Direction::Right, false),
            (NodeKind::Power, Direction::Left, false),
            (NodeKind::Fraction, Direction::Left, true),
            (NodeKind::Fraction, Direction::Right, false),
        ] {
            let mut e = Editor::from_portable(&source(kind)).unwrap();
            e.place_caret(e.root, side).unwrap();
            let before = e.to_portable().unwrap();
            assert!(!e.delete_structure(backwards));
            assert_eq!(e.typeover_target(), None);
            assert_eq!(e.to_portable().unwrap(), before);
        }
        let mut tree = source(NodeKind::Power);
        tree.children[1].kind = NodeKind::Placeholder;
        let mut e = Editor::from_portable(&tree).unwrap();
        e.place_caret(e.root, Direction::Right).unwrap();
        assert!(!e.begin_root_typeover(true));
    }
}
