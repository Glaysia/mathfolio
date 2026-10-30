//! Parenthesis insertion keeps Normal adjacency distinct from Scope wrapping.
//! Original parser/repair facts: docs/analysis/0114-parenthesis-adjacency-contract.md.
//! SCALE and APPLY remain distinct from explicit MULTIPLICATION (0110).
use super::*;

impl Editor {
    /// The close command first tries native structural navigation. A successful
    /// caret move returns false because it does not alter the document.
    pub fn insert_close_parenthesis(&mut self) -> bool {
        if self.matrix_selection().is_some() || self.caret.insertion != InsertionMode::Normal {
            return false;
        }
        if matches!(self.selection, Selection::Subtree(_)) {
            return self.insert_open_parenthesis();
        }
        if let Some(boundary) = self.grouping_close_boundary() {
            self.caret = Caret::at(boundary, Direction::Right);
            self.selection = Selection::None;
            self.scope = None;
            return false;
        }
        let target = self.caret.node;
        let old = self.arena.get(target).unwrap();
        let end = if let Some(text) = old.kind.text() {
            self.caret.caret_side == Direction::Right
                || (self.caret.caret_side == Direction::Indeterminate
                    && !matches!(
                        old.kind,
                        NodeKind::NativeIdentifier {
                            subscript: Some(_),
                            ..
                        } | NodeKind::Imaginary(_)
                            | NodeKind::ImaginarySymbol { .. }
                    )
                    && self.caret.value_range.is_some_and(|range| {
                        range.anchor == range.focus && range.focus == text.encode_utf16().count()
                    }))
        } else {
            grouping_completed_right(&old.kind) && self.caret.caret_side == Direction::Right
        };
        if !end || !self.grouping_local_insertion_safe(target, 299) {
            return false;
        }
        let parent = old.parent;
        let mut candidate = self.clone_without_history();
        let group = candidate.arena.allocate(NodeKind::Group, vec![target]);
        candidate.replace_child(parent, target, group);
        candidate.caret = Caret::at(group, Direction::Right);
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
        true
    }

    // l.n 0079 uses one structural Right move, stripping ValueRange (060a).
    // Only a Family1 right state admits its second Right move. This represented
    // subset deliberately does not jump over Power/Index/Fraction right states.
    fn grouping_close_boundary(&self) -> Option<NodeId> {
        let mut target = self.caret.node;
        let current = self.arena.get(target)?;
        if current.kind.text().is_none()
            && current.kind != NodeKind::Placeholder
            && self.caret.caret_side != Direction::Right
        {
            return None;
        }
        while let Some(parent) = self.arena.get(target)?.parent {
            let node = self.arena.get(parent)?;
            let slot = node.children.iter().position(|child| *child == target)?;
            match node.kind {
                NodeKind::Group if slot == 0 => return Some(parent),
                NodeKind::Call { .. } if slot > 0 && slot + 1 == node.children.len() => {
                    return Some(parent);
                }
                NodeKind::Binary(
                    BinaryOp::Add
                    | BinaryOp::Subtract
                    | BinaryOp::Multiply
                    | BinaryOp::InlineDivide
                    | BinaryOp::Scale
                    | BinaryOp::Polar,
                ) if slot + 1 == node.children.len() => target = parent,
                NodeKind::UnaryNeg | NodeKind::LogicalNot | NodeKind::Return if slot == 0 => {
                    target = parent
                }
                _ => return None,
            }
        }
        None
    }

    /// Insert the opening parenthesis command using its represented native
    /// selection mode. Unsupported splits leave source and history intact.
    pub fn insert_open_parenthesis(&mut self) -> bool {
        if self.matrix_selection().is_some() || self.caret.insertion != InsertionMode::Normal {
            return false;
        }
        let target = self.target();
        let old = self.arena.get(target).unwrap();
        let parent = old.parent;
        let scope_side = match self.selection {
            Selection::Subtree(_) => match self.caret.caret_side {
                Direction::Left | Direction::Right => Some(self.caret.caret_side),
                _ => return false,
            },
            _ => None,
        };
        let blank = old.kind == NodeKind::Placeholder;
        let text = old.kind.text();
        let splittable = matches!(
            old.kind,
            NodeKind::Number(_)
                | NodeKind::Identifier(_)
                | NodeKind::AnnotatedIdentifier { .. }
                | NodeKind::NativeIdentifier {
                    subscript: None,
                    ..
                }
        );
        let range = if self.caret.caret_side == Direction::Indeterminate {
            self.caret.value_range
        } else {
            None
        };
        let full_text = splittable
            && range.is_some_and(|range| {
                range.anchor != range.focus
                    && range.anchor.min(range.focus) == 0
                    && range.anchor.max(range.focus) == text.unwrap().encode_utf16().count()
            });
        let text_start = text.is_some()
            && match self.caret.caret_side {
                Direction::Left => true,
                Direction::Indeterminate => range.is_none_or(|r| r.anchor == 0 && r.focus == 0),
                _ => false,
            };
        let text_end = text.is_some()
            && (self.caret.caret_side == Direction::Right
                || (splittable
                    && range.is_some_and(|r| {
                        r.anchor == r.focus && r.focus == text.unwrap().encode_utf16().count()
                    })));
        let node_end =
            self.caret.caret_side == Direction::Right && grouping_completed_right(&old.kind);
        let parts = if splittable && !full_text && !text_start && !text_end {
            range.and_then(|range| {
                let value = text.unwrap();
                Some((
                    grouping_text_slice(value, 0, range.anchor.min(range.focus))?,
                    grouping_text_slice(
                        value,
                        range.anchor.max(range.focus),
                        value.encode_utf16().count(),
                    )?,
                ))
            })
        } else {
            None
        };
        let fragments = if let Some((prefix, suffix)) = &parts {
            let (Some(prefix), Some(suffix)) = (
                grouping_fragment(&old.kind, prefix.clone()),
                grouping_fragment(&old.kind, suffix.clone()),
            ) else {
                return false;
            };
            Some((prefix, suffix))
        } else {
            None
        };
        let adjacency = text_end || node_end || parts.is_some();
        if scope_side.is_none() && !blank && !full_text && !text_start && !adjacency {
            return false;
        }
        let mut candidate = self.clone_without_history();
        let (wrapper, caret) = if scope_side.is_some() || blank || full_text || text_start {
            let wrapper = candidate.arena.allocate(NodeKind::Group, vec![target]);
            let caret = if let Some(side) = scope_side {
                Caret::at(wrapper, side)
            } else {
                grouping_value_left(target, !blank)
            };
            (wrapper, caret)
        } else if let Some((prefix, _)) = parts.as_ref()
            && prefix.is_empty()
        {
            candidate.arena.node_mut(target).kind = fragments.as_ref().unwrap().1.clone();
            let wrapper = candidate.arena.allocate(NodeKind::Group, vec![target]);
            (wrapper, grouping_value_left(target, true))
        } else {
            let (prefix, suffix) = if let Some((prefix, suffix)) = fragments {
                let right = candidate.arena.allocate(suffix, vec![]);
                candidate.arena.node_mut(target).kind = prefix;
                (target, right)
            } else {
                (
                    target,
                    candidate.arena.allocate(NodeKind::Placeholder, vec![]),
                )
            };
            let has_text = candidate.arena.get(suffix).unwrap().kind.text().is_some();
            let wrapper = if grouping_callable(&candidate.arena.get(prefix).unwrap().kind) {
                candidate
                    .arena
                    .allocate(NodeKind::Call { argument_count: 1 }, vec![prefix, suffix])
            } else {
                let group = candidate.arena.allocate(NodeKind::Group, vec![suffix]);
                candidate
                    .arena
                    .allocate(NodeKind::Binary(BinaryOp::Scale), vec![prefix, group])
            };
            (wrapper, grouping_value_left(suffix, has_text))
        };
        candidate.replace_child(parent, target, wrapper);
        if matches!(
            candidate.arena.get(wrapper).unwrap().kind,
            NodeKind::Binary(BinaryOp::Scale) | NodeKind::Call { .. }
        ) && !candidate.grouping_repair_adjacency(wrapper)
        {
            return false;
        }
        candidate.caret = caret;
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
        true
    }

    // Native container slots stop GetContainerRoot before linear analysis.
    // Call arguments represent the original ARGUMENT_LIST container's content.
    pub(super) fn grouping_container_slot(&self, parent: NodeId, slot: usize) -> bool {
        let node = self.arena.get(parent).unwrap();
        match node.kind {
            NodeKind::Group
            | NodeKind::Absolute
            | NodeKind::Determinant
            | NodeKind::Conjugate
            | NodeKind::Vectorize => slot == 0,
            NodeKind::Power | NodeKind::Row | NodeKind::Column => slot == 1,
            NodeKind::Index { .. } | NodeKind::Call { .. } => slot > 0,
            NodeKind::Fraction | NodeKind::Root | NodeKind::While | NodeKind::Try => slot < 2,
            NodeKind::For => slot < 3,
            NodeKind::Program { .. } | NodeKind::If { .. } | NodeKind::Matrix { .. } => true,
            _ => false,
        }
    }

    // Lower-binding ancestors retain the same linear parse after insertion.
    // Equal/higher-binding adjacency requires the full analysis/repair segment.
    fn grouping_local_insertion_safe(&self, mut target: NodeId, binding: i32) -> bool {
        while let Some(parent) = self.arena.get(target).unwrap().parent {
            let node = self.arena.get(parent).unwrap();
            let slot = node
                .children
                .iter()
                .position(|child| *child == target)
                .unwrap();
            if self.grouping_container_slot(parent, slot) {
                return true;
            }
            let linear = matches!(
                node.kind,
                NodeKind::Binary(_)
                    | NodeKind::LocalDefine
                    | NodeKind::Return
                    | NodeKind::Range { .. }
                    | NodeKind::UnaryNeg
                    | NodeKind::LogicalNot
                    | NodeKind::Power
                    | NodeKind::Index { .. }
                    | NodeKind::Row
                    | NodeKind::Column
                    | NodeKind::Factorial
                    | NodeKind::Percent
                    | NodeKind::Degree
                    | NodeKind::Transpose
            );
            if !linear || node.kind.precedence() >= binding {
                return false;
            }
            target = parent;
        }
        true
    }

    // OP_PLACEHOLDER 200 repairs to SCALE 142 and triggers a second parse.
    // These rotations reproduce that parse for represented linear handles;
    // native container operands are opaque, including Power's exponent.
    fn grouping_repair_adjacency(&mut self, wrapper: NodeId) -> bool {
        let binding = self.arena.get(wrapper).unwrap().kind.precedence();
        while let Some(parent) = self.arena.get(wrapper).unwrap().parent {
            let node = self.arena.get(parent).unwrap();
            let slot = node
                .children
                .iter()
                .position(|child| *child == wrapper)
                .unwrap();
            if self.grouping_container_slot(parent, slot) {
                return true;
            }
            let kind = node.kind.clone();
            let precedence = kind.precedence();
            let binary = matches!(
                kind,
                NodeKind::Binary(_) | NodeKind::LocalDefine | NodeKind::Range { .. }
            );
            let prefix = matches!(
                kind,
                NodeKind::UnaryNeg | NodeKind::LogicalNot | NodeKind::Return
            );
            let postfix = matches!(
                kind,
                NodeKind::Power
                    | NodeKind::Index { .. }
                    | NodeKind::Row
                    | NodeKind::Column
                    | NodeKind::Factorial
                    | NodeKind::Percent
                    | NodeKind::Degree
                    | NodeKind::Transpose
            );
            if (binary || prefix || postfix) && precedence < binding {
                return true;
            }
            if kind == NodeKind::Binary(BinaryOp::Scale) && precedence == binding && slot == 0 {
                return true;
            }
            let replace_slot = if (prefix && precedence > binding && slot == 0)
                || (kind == NodeKind::Binary(BinaryOp::Scale) && precedence == binding && slot == 1)
            {
                0
            } else if postfix && precedence > binding && slot == 0 {
                1
            } else {
                return false;
            };
            let grandparent = self.arena.get(parent).unwrap().parent;
            let moved = self.arena.get(wrapper).unwrap().children[replace_slot];
            self.arena.node_mut(parent).children[slot] = moved;
            self.arena.node_mut(moved).parent = Some(parent);
            self.arena.node_mut(wrapper).children[replace_slot] = parent;
            self.replace_child(grandparent, parent, wrapper);
            self.arena.node_mut(wrapper).parent = grandparent;
            self.arena.node_mut(parent).parent = Some(wrapper);
        }
        true
    }
}

fn grouping_callable(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Identifier(_)
            | NodeKind::AnnotatedIdentifier { .. }
            | NodeKind::NativeIdentifier { .. }
            | NodeKind::Index { .. }
    )
}

fn grouping_completed_right(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Group
            | NodeKind::Index { .. }
            | NodeKind::Power
            | NodeKind::Fraction
            | NodeKind::Degree
            | NodeKind::Factorial
            | NodeKind::Percent
            | NodeKind::Transpose
    )
}

fn grouping_value_left(node: NodeId, text: bool) -> Caret {
    let mut caret = Caret::at(node, Direction::Indeterminate);
    if text {
        caret.value_range = Some(ValueRange {
            anchor: 0,
            focus: 0,
        });
    }
    caret
}

pub(super) fn grouping_text_slice(value: &str, start: usize, end: usize) -> Option<String> {
    let boundary = |offset| {
        if offset == 0 {
            return Some(0);
        }
        let mut units = 0;
        value.char_indices().find_map(|(index, ch)| {
            units += ch.len_utf16();
            (units == offset).then_some(index + ch.len_utf8())
        })
    };
    Some(value.get(boundary(start)?..boundary(end)?)?.to_owned())
}

// Native factory selects a descriptor before applying cloned NodeInfo. Only
// represented decimal/scientific/imaginary and plain-name fragments are handled
// here; hexadecimal/radix and other formatted lexical cases are not guessed.
pub(super) fn grouping_fragment(kind: &NodeKind, text: String) -> Option<NodeKind> {
    if text.is_empty() {
        return Some(NodeKind::Placeholder);
    }
    if grouping_decimal(&text) {
        return Some(NodeKind::Number(text));
    }
    if let Some(value) = text.strip_suffix(['i', 'j'])
        && grouping_decimal(value)
    {
        return Some(NodeKind::ImaginarySymbol {
            value: value.into(),
            symbol: text.chars().last().unwrap().to_string(),
        });
    }
    let mut chars = text.chars();
    if !chars
        .next()
        .is_some_and(|ch| ch.is_alphabetic() || ch == '_')
        || !chars.all(|ch| ch.is_alphanumeric() || ch == '_')
    {
        return None;
    }
    Some(match kind {
        NodeKind::AnnotatedIdentifier { labels, .. } => NodeKind::AnnotatedIdentifier {
            name: text,
            labels: labels.clone(),
        },
        NodeKind::NativeIdentifier {
            labels, contextual, ..
        } => NodeKind::NativeIdentifier {
            name: text,
            labels: labels.clone(),
            contextual: *contextual,
            subscript: None,
        },
        _ => NodeKind::Identifier(text),
    })
}

fn grouping_decimal(text: &str) -> bool {
    let mut pieces = text.split('E');
    let mantissa = pieces.next().unwrap();
    if let Some(exponent) = pieces.next() {
        let Some(digits) = exponent.strip_prefix(['+', '-']) else {
            return false;
        };
        if digits.is_empty() || !digits.bytes().all(|ch| ch.is_ascii_digit()) {
            return false;
        }
    }
    if pieces.next().is_some() {
        return false;
    }
    let mut digits = 0;
    let mut dot = false;
    for ch in mantissa.bytes() {
        if ch.is_ascii_digit() {
            digits += 1;
        } else if ch == b'.' && !dot {
            dot = true;
        } else {
            return false;
        }
    }
    digits > 0
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
    fn number() -> PortableTree {
        atom(NodeKind::Number("2".into()))
    }

    #[test]
    fn blank_and_numeric_end_have_distinct_grouping_commands_and_one_undo() {
        let mut blank = Editor::new();
        let target = blank.root();
        assert!(blank.insert_open_parenthesis());
        assert_eq!(blank.arena.get(blank.root()).unwrap().kind, NodeKind::Group);
        assert_eq!(blank.caret, Caret::at(target, Direction::Indeterminate));
        assert!(blank.undo());
        assert_eq!(blank.root(), target);
        assert!(!blank.undo());

        let mut editor = Editor::from_portable(&number()).unwrap();
        let target = editor.root();
        editor.place_text_caret(target, 1).unwrap();
        let before = editor.caret;
        assert!(editor.insert_open_parenthesis());
        let result = editor.to_portable().unwrap();
        assert_eq!(result.kind, NodeKind::Binary(BinaryOp::Scale));
        assert_eq!(result.children[0], number());
        assert_eq!(
            result.children[1],
            tree(NodeKind::Group, vec![atom(NodeKind::Placeholder)])
        );
        assert_eq!(editor.caret.caret_side, Direction::Indeterminate);
        assert!(editor.undo());
        assert_eq!(editor.caret, before);
        assert_eq!(editor.root(), target);
        assert!(!editor.undo());
        assert!(editor.redo());
        assert_eq!(editor.to_portable().unwrap(), result);
    }

    #[test]
    fn identifier_labels_are_not_a_call_creation_gate_and_ids_survive() {
        for kind in [
            NodeKind::Identifier("f".into()),
            NodeKind::AnnotatedIdentifier {
                name: "m".into(),
                labels: "UNIT".into(),
            },
            NodeKind::NativeIdentifier {
                name: "f".into(),
                labels: "FUNCTION".into(),
                contextual: Some(true),
                subscript: None,
            },
        ] {
            let source = atom(kind);
            let mut editor = Editor::from_portable(&source).unwrap();
            let target = editor.root();
            editor.place_text_caret(target, 1).unwrap();
            assert!(editor.insert_open_parenthesis());
            let call = editor.arena.get(editor.root()).unwrap();
            assert_eq!(call.kind, NodeKind::Call { argument_count: 1 });
            assert_eq!(call.children[0], target);
            assert_eq!(editor.export_subtree(target).unwrap(), source);
            assert!(editor.undo());
            assert_eq!(editor.root(), target);
            assert_eq!(editor.to_portable().unwrap(), source);
        }
    }

    #[test]
    fn completed_group_gets_adjacent_group_and_preserves_its_whole_subtree() {
        let source = tree(
            NodeKind::Group,
            vec![tree(
                NodeKind::Binary(BinaryOp::Add),
                vec![number(), number()],
            )],
        );
        let mut editor = Editor::from_portable(&source).unwrap();
        let target = editor.root();
        editor.place_caret(target, Direction::Right).unwrap();
        assert!(editor.insert_open_parenthesis());
        assert_eq!(editor.arena.get(editor.root()).unwrap().children[0], target);
        assert_eq!(editor.export_subtree(target).unwrap(), source);
        assert_eq!(
            editor.to_portable().unwrap().kind,
            NodeKind::Binary(BinaryOp::Scale)
        );
    }

    #[test]
    fn explicit_container_slots_keep_exponent_and_fraction_inputs_inside() {
        for (kind, slot) in [
            (NodeKind::Power, 1),
            (NodeKind::Fraction, 0),
            (NodeKind::Fraction, 1),
        ] {
            let source = tree(kind.clone(), vec![number(), number()]);
            let mut editor = Editor::from_portable(&source).unwrap();
            let root = editor.root();
            let target = editor.arena.get(root).unwrap().children[slot];
            editor.place_text_caret(target, 1).unwrap();
            assert!(editor.insert_open_parenthesis());
            assert_eq!(editor.root(), root);
            assert_eq!(editor.arena.get(root).unwrap().kind, kind);
            let inserted = editor.arena.get(root).unwrap().children[slot];
            assert_eq!(
                editor.arena.get(inserted).unwrap().kind,
                NodeKind::Binary(BinaryOp::Scale)
            );
            assert_eq!(editor.arena.get(inserted).unwrap().children[0], target);
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), source);
        }
    }

    #[test]
    fn full_utf16_value_wraps_preserved_atom_and_enters_text_start() {
        let source = atom(NodeKind::Identifier("a𝛂z".into()));
        for (anchor, focus) in [(0, 4), (4, 0)] {
            let mut editor = Editor::from_portable(&source).unwrap();
            let target = editor.root();
            editor.place_text_selection(target, anchor, focus).unwrap();
            let before = editor.caret;
            assert!(editor.insert_open_parenthesis());
            assert_eq!(
                editor.to_portable().unwrap(),
                tree(NodeKind::Group, vec![source.clone()])
            );
            assert_eq!(editor.caret.node, target);
            assert_eq!(
                editor.caret.value_range,
                Some(ValueRange {
                    anchor: 0,
                    focus: 0
                })
            );
            assert!(editor.undo());
            assert_eq!(editor.caret, before);
            assert_eq!(editor.to_portable().unwrap(), source);
        }
    }

    #[test]
    fn actual_scope_wraps_and_consumes_scope_with_original_boundary_focus() {
        for side in [Direction::Left, Direction::Right] {
            let mut editor = Editor::from_portable(&number()).unwrap();
            let target = editor.root();
            editor
                .place_text_caret(target, usize::from(side == Direction::Right))
                .unwrap();
            let origin = editor.caret;
            assert!(editor.expand_scope());
            assert!(editor.scope.is_some());
            assert!(editor.insert_open_parenthesis());
            assert_eq!(editor.caret, Caret::at(editor.root(), side));
            assert_eq!(editor.selection, Selection::None);
            assert!(editor.scope.is_none());
            assert_eq!(
                editor.to_portable().unwrap(),
                tree(NodeKind::Group, vec![number()])
            );
            assert!(editor.undo());
            assert!(editor.scope.is_some());
            assert!(editor.contract_scope());
            assert_eq!(editor.caret, origin);
            assert!(!editor.undo());
        }
    }

    #[test]
    fn unsupported_suffix_fields_leave_redo_and_context_intact() {
        for kind in [
            NodeKind::NativeIdentifier {
                name: "abc".into(),
                labels: "VARIABLE".into(),
                contextual: None,
                subscript: Some("i".into()),
            },
            NodeKind::NativeIdentifier {
                name: "abc".into(),
                labels: "UNIT".into(),
                contextual: Some(true),
                subscript: Some("𝛂".into()),
            },
        ] {
            let mut editor = Editor::from_portable(&atom(kind)).unwrap();
            let target = editor.root();
            editor.insert_group();
            assert!(editor.undo());
            editor.place_text_selection(target, 1, 2).unwrap();
            let before = editor.to_portable().unwrap();
            let caret = editor.caret;
            let revision = editor.arena.revision();
            assert!(!editor.insert_open_parenthesis());
            assert_eq!(editor.to_portable().unwrap(), before);
            assert_eq!(editor.caret, caret);
            assert_eq!(editor.arena.revision(), revision);
            assert!(editor.redo());
        }
    }

    #[test]
    fn numeric_partial_selection_removes_selected_text_and_group_consumes_suffix() {
        for (anchor, focus, prefix, suffix) in [
            (2, 2, "12", "34"),
            (1, 3, "1", "4"),
            (3, 1, "1", "4"),
            (0, 2, "", "34"),
            (2, 4, "12", ""),
        ] {
            let source = atom(NodeKind::Number("1234".into()));
            let mut editor = Editor::from_portable(&source).unwrap();
            let target = editor.root();
            editor.place_text_selection(target, anchor, focus).unwrap();
            let before = editor.caret;
            assert!(editor.insert_open_parenthesis());
            let right = if suffix.is_empty() {
                atom(NodeKind::Placeholder)
            } else {
                atom(NodeKind::Number(suffix.into()))
            };
            let group = tree(NodeKind::Group, vec![right]);
            let expected = if prefix.is_empty() {
                group
            } else {
                tree(
                    NodeKind::Binary(BinaryOp::Scale),
                    vec![atom(NodeKind::Number(prefix.into())), group],
                )
            };
            assert_eq!(editor.to_portable().unwrap(), expected);
            assert_eq!(
                editor.caret.value_range,
                (!suffix.is_empty()).then_some(ValueRange {
                    anchor: 0,
                    focus: 0
                })
            );
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), source);
            assert_eq!(editor.root(), target);
            assert_eq!(editor.caret, before);
            assert!(!editor.undo());
            assert!(editor.redo());
            assert_eq!(editor.to_portable().unwrap(), expected);
        }
    }

    #[test]
    fn identifier_utf16_split_preserves_raw_labels_and_context_without_suffix_duplication() {
        let source = atom(NodeKind::NativeIdentifier {
            name: "a𝛂dz".into(),
            labels: "UNIT".into(),
            contextual: Some(true),
            subscript: None,
        });
        let mut editor = Editor::from_portable(&source).unwrap();
        let target = editor.root();
        editor.place_text_selection(target, 1, 4).unwrap();
        assert!(editor.insert_open_parenthesis());
        let call = editor.to_portable().unwrap();
        assert_eq!(call.kind, NodeKind::Call { argument_count: 1 });
        for (node, name) in call.children.iter().zip(["a", "z"]) {
            assert_eq!(
                node.kind,
                NodeKind::NativeIdentifier {
                    name: name.into(),
                    labels: "UNIT".into(),
                    contextual: Some(true),
                    subscript: None,
                }
            );
        }
        assert_eq!(editor.arena.get(editor.root()).unwrap().children[0], target);
        assert_eq!(
            editor.caret.value_range,
            Some(ValueRange {
                anchor: 0,
                focus: 0
            })
        );
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), source);
    }

    #[test]
    fn semantic_right_and_native_collapsed_end_agree_and_left_enters_preserved_atom() {
        for side in [Direction::Left, Direction::Indeterminate, Direction::Right] {
            let mut editor = Editor::from_portable(&number()).unwrap();
            let target = editor.root();
            editor.place_caret(target, side).unwrap();
            assert!(editor.insert_open_parenthesis());
            assert_eq!(
                editor.to_portable().unwrap().kind,
                if side == Direction::Right {
                    NodeKind::Binary(BinaryOp::Scale)
                } else {
                    NodeKind::Group
                }
            );
            if side != Direction::Right {
                assert_eq!(editor.caret.node, target);
                assert_eq!(
                    editor.caret.value_range,
                    Some(ValueRange {
                        anchor: 0,
                        focus: 0
                    })
                );
            }
        }
    }

    #[test]
    fn lower_binding_linear_ancestors_preserve_eval_arithmetic_and_argument_containers() {
        for kind in [
            NodeKind::Binary(BinaryOp::Evaluate),
            NodeKind::Binary(BinaryOp::Add),
            NodeKind::Binary(BinaryOp::Multiply),
            NodeKind::Call { argument_count: 1 },
        ] {
            let source = tree(kind.clone(), vec![number(), number()]);
            let mut editor = Editor::from_portable(&source).unwrap();
            let root = editor.root();
            let slot = usize::from(kind != NodeKind::Binary(BinaryOp::Evaluate));
            let target = editor.arena.get(root).unwrap().children[slot];
            editor.place_text_caret(target, 1).unwrap();
            assert!(editor.insert_open_parenthesis());
            assert_eq!(editor.root(), root);
            assert_eq!(editor.arena.get(root).unwrap().kind, kind);
            let inserted = editor.arena.get(root).unwrap().children[slot];
            assert_eq!(
                editor.arena.get(inserted).unwrap().kind,
                NodeKind::Binary(BinaryOp::Scale)
            );
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), source);
        }
    }

    #[test]
    fn ordinary_infix_stops_at_native_slots_but_explicit_operator_right_exits_them() {
        for (kind, slot) in [
            (NodeKind::Power, 1),
            (NodeKind::Fraction, 1),
            (NodeKind::Index { indices: 1 }, 1),
        ] {
            let source = tree(kind.clone(), vec![number(), number()]);
            let mut editor = Editor::from_portable(&source).unwrap();
            let root = editor.root();
            let target = editor.arena.get(root).unwrap().children[slot];
            editor.place_text_caret(target, 1).unwrap();
            editor.insert_infix(BinaryOp::Add);
            assert_eq!(editor.root(), root);
            let nested = editor.arena.get(root).unwrap().children[slot];
            assert_eq!(
                editor.arena.get(nested).unwrap().kind,
                NodeKind::Binary(BinaryOp::Add)
            );
            assert!(editor.undo());
            editor.place_caret(root, Direction::Right).unwrap();
            editor.insert_infix(BinaryOp::Add);
            assert_ne!(editor.root(), root);
            assert_eq!(editor.arena.get(editor.root()).unwrap().children[0], root);
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), source);
        }
    }

    #[test]
    fn invalid_numeric_fragments_do_not_checkpoint_or_destroy_redo() {
        let source = atom(NodeKind::Number("1e-3".into()));
        let mut editor = Editor::from_portable(&source).unwrap();
        let target = editor.root();
        editor.insert_group();
        assert!(editor.undo());
        editor.place_text_caret(target, 2).unwrap();
        let caret = editor.caret;
        let revision = editor.arena.revision();
        assert!(!editor.insert_open_parenthesis());
        assert_eq!(editor.to_portable().unwrap(), source);
        assert_eq!(editor.caret, caret);
        assert_eq!(editor.arena.revision(), revision);
        assert!(editor.redo());
    }

    #[test]
    fn matrix_cells_and_literal_subscript_mode_preserve_document_and_redo() {
        let source = tree(
            NodeKind::Matrix {
                rows: 1,
                cols: 2,
                display: None,
            },
            vec![number(), number()],
        );
        let mut editor = Editor::from_portable(&source).unwrap();
        let root = editor.root();
        let target = editor.arena.get(root).unwrap().children[0];
        editor
            .set_atom(target, NodeKind::Number("3".into()))
            .unwrap();
        assert!(editor.undo());
        assert!(editor.select_matrix_cells(root, 0, 1));
        let caret = editor.caret;
        let selection = editor.selection;
        let revision = editor.arena.revision();
        assert!(!editor.insert_open_parenthesis());
        assert_eq!(editor.caret, caret);
        assert_eq!(editor.selection, selection);
        assert_eq!(editor.arena.revision(), revision);
        assert_eq!(editor.to_portable().unwrap(), source);
        assert!(editor.redo());

        let mut editor = Editor::from_portable(&atom(NodeKind::Identifier("f".into()))).unwrap();
        let target = editor.root();
        editor.place_text_caret(target, 1).unwrap();
        editor.toggle_subscript();
        let before = editor.to_portable().unwrap();
        let caret = editor.caret;
        assert!(!editor.insert_open_parenthesis());
        assert_eq!(editor.to_portable().unwrap(), before);
        assert_eq!(editor.caret, caret);
    }

    #[test]
    fn exact_node_budget_accounts_for_call_vs_scale_and_failure_preserves_redo() {
        for (kind, added) in [
            (NodeKind::Number("2".into()), 3),
            (NodeKind::Identifier("f".into()), 2),
        ] {
            for spare in [added - 1, added] {
                let count = MAX_TREE_NODES - spare;
                let mut remaining = count - 5;
                let children = (0..4)
                    .map(|_| {
                        let statements = remaining.min(4096);
                        remaining -= statements;
                        tree(
                            NodeKind::Program {
                                statement_count: statements,
                            },
                            vec![atom(kind.clone()); statements],
                        )
                    })
                    .collect();
                assert_eq!(remaining, 0);
                let source = tree(NodeKind::Program { statement_count: 4 }, children);
                let mut editor = Editor::from_portable(&source).unwrap();
                let target = first_leaf(&editor.arena, editor.root());
                editor
                    .set_atom(target, NodeKind::Number("3".into()))
                    .unwrap();
                assert!(editor.undo());
                editor.place_text_caret(target, 1).unwrap();
                let caret = editor.caret;
                let revision = editor.arena.revision();
                assert_eq!(editor.insert_open_parenthesis(), spare == added);
                if spare == added {
                    assert_eq!(editor.arena.nodes().count(), MAX_TREE_NODES);
                    assert!(editor.undo());
                    assert_eq!(editor.to_portable().unwrap(), source);
                } else {
                    assert_eq!(editor.arena.revision(), revision);
                    assert_eq!(editor.caret, caret);
                    assert_eq!(editor.to_portable().unwrap(), source);
                    assert!(editor.redo());
                }
            }
        }
    }

    #[test]
    fn repaired_scale_reparses_left_association_and_prefix_binding_without_losing_ids() {
        for source in [
            tree(NodeKind::Binary(BinaryOp::Scale), vec![number(), number()]),
            tree(NodeKind::UnaryNeg, vec![number()]),
        ] {
            let mut editor = Editor::from_portable(&source).unwrap();
            let previous = editor.root();
            let target = last_leaf(&editor.arena, previous);
            editor.place_text_caret(target, 1).unwrap();
            let before = editor.caret;
            assert!(editor.insert_open_parenthesis());
            let result = editor.to_portable().unwrap();
            assert_eq!(result.kind, NodeKind::Binary(BinaryOp::Scale));
            assert_eq!(result.children[0], source);
            assert_eq!(
                editor.arena.get(editor.root()).unwrap().children[0],
                previous
            );
            assert_eq!(
                result.children[1],
                tree(NodeKind::Group, vec![atom(NodeKind::Placeholder)])
            );
            assert!(editor.arena.get(target).is_some());
            assert!(editor.undo());
            assert_eq!(editor.root(), previous);
            assert_eq!(editor.caret, before);
            assert_eq!(editor.to_portable().unwrap(), source);
            assert!(!editor.undo());
            assert!(editor.redo());
            assert_eq!(editor.to_portable().unwrap(), result);
        }
    }

    #[test]
    fn base_end_and_explicit_power_right_have_different_parser_ranges() {
        let source = tree(
            NodeKind::Power,
            vec![number(), atom(NodeKind::Number("3".into()))],
        );
        let mut editor = Editor::from_portable(&source).unwrap();
        let power = editor.root();
        let base = editor.arena.get(power).unwrap().children[0];
        let exponent = editor.arena.get(power).unwrap().children[1];
        editor.place_text_caret(base, 1).unwrap();
        assert!(editor.insert_open_parenthesis());
        let expected = tree(
            NodeKind::Binary(BinaryOp::Scale),
            vec![
                number(),
                tree(
                    NodeKind::Power,
                    vec![
                        tree(NodeKind::Group, vec![atom(NodeKind::Placeholder)]),
                        atom(NodeKind::Number("3".into())),
                    ],
                ),
            ],
        );
        assert_eq!(editor.to_portable().unwrap(), expected);
        let scale = editor.arena.get(editor.root()).unwrap();
        assert_eq!(scale.children[0], base);
        assert_eq!(scale.children[1], power);
        assert_eq!(editor.arena.get(power).unwrap().children[1], exponent);
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), source);
        editor.place_caret(power, Direction::Right).unwrap();
        assert!(editor.insert_open_parenthesis());
        assert_eq!(
            editor.to_portable().unwrap(),
            tree(
                NodeKind::Binary(BinaryOp::Scale),
                vec![
                    source.clone(),
                    tree(NodeKind::Group, vec![atom(NodeKind::Placeholder)])
                ]
            )
        );
        assert_eq!(editor.arena.get(editor.root()).unwrap().children[0], power);
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), source);
    }

    #[test]
    fn explicit_completed_postfix_and_fraction_right_keep_their_whole_operand() {
        for source in [
            tree(NodeKind::Degree, vec![number()]),
            tree(NodeKind::Fraction, vec![number(), number()]),
            tree(NodeKind::Factorial, vec![number()]),
            tree(NodeKind::Transpose, vec![number()]),
        ] {
            let mut editor = Editor::from_portable(&source).unwrap();
            let previous = editor.root();
            editor.place_caret(previous, Direction::Right).unwrap();
            assert!(editor.insert_open_parenthesis());
            assert_eq!(editor.to_portable().unwrap().children[0], source);
            assert_eq!(
                editor.arena.get(editor.root()).unwrap().children[0],
                previous
            );
            assert!(editor.undo());
            editor.place_caret(previous, Direction::Right).unwrap();
            assert!(editor.insert_close_parenthesis());
            assert_eq!(
                editor.to_portable().unwrap(),
                tree(NodeKind::Group, vec![source.clone()])
            );
            assert_eq!(editor.caret, Caret::at(editor.root(), Direction::Right));
        }
    }

    #[test]
    fn factory_reclassifies_numeric_name_fragments_before_applying_identifier_metadata() {
        for suffix in ["12", "1.5", "2E+3", "2i"] {
            for kind in [
                NodeKind::Identifier(format!("x{suffix}")),
                NodeKind::AnnotatedIdentifier {
                    name: format!("x{suffix}"),
                    labels: "UNIT".into(),
                },
                NodeKind::NativeIdentifier {
                    name: format!("x{suffix}"),
                    labels: "VARIABLE".into(),
                    contextual: Some(true),
                    subscript: None,
                },
            ] {
                let source = atom(kind.clone());
                let mut editor = Editor::from_portable(&source).unwrap();
                let target = editor.root();
                editor.place_text_caret(target, 1).unwrap();
                assert!(editor.insert_open_parenthesis());
                let call = editor.to_portable().unwrap();
                assert_eq!(call.kind, NodeKind::Call { argument_count: 1 });
                assert_eq!(
                    call.children[0].kind,
                    grouping_fragment(&kind, "x".into()).unwrap()
                );
                assert_eq!(
                    call.children[1].kind,
                    if suffix == "2i" {
                        NodeKind::ImaginarySymbol {
                            value: "2".into(),
                            symbol: "i".into(),
                        }
                    } else {
                        NodeKind::Number(suffix.into())
                    }
                );
                assert!(editor.undo());
                assert_eq!(editor.to_portable().unwrap(), source);
            }
        }
        for suffix in ["12h", "12b", "1E3", "1e3"] {
            let source = atom(NodeKind::Identifier(format!("x{suffix}")));
            let mut editor = Editor::from_portable(&source).unwrap();
            editor.place_text_caret(editor.root(), 1).unwrap();
            let revision = editor.arena.revision();
            let caret = editor.caret;
            assert!(!editor.insert_open_parenthesis());
            assert_eq!(editor.to_portable().unwrap(), source);
            assert_eq!(editor.arena.revision(), revision);
            assert_eq!(editor.caret, caret);
            assert!(!editor.undo());
        }
    }

    #[test]
    fn closing_group_navigates_from_atomic_midpoint_without_source_or_history_changes() {
        let source = tree(NodeKind::Group, vec![atom(NodeKind::Number("1234".into()))]);
        let mut editor = Editor::from_portable(&source).unwrap();
        let root = editor.root();
        let target = editor.arena.get(root).unwrap().children[0];
        editor
            .set_atom(target, NodeKind::Number("5".into()))
            .unwrap();
        assert!(editor.undo());
        editor.place_text_selection(target, 1, 3).unwrap();
        let revision = editor.arena.revision();
        assert!(!editor.insert_close_parenthesis());
        assert_eq!(editor.caret, Caret::at(root, Direction::Right));
        assert_eq!(editor.to_portable().unwrap(), source);
        assert_eq!(editor.arena.revision(), revision);
        assert!(editor.redo());
    }

    #[test]
    fn closing_walks_only_last_linear_operands_and_last_call_argument() {
        let expression = tree(
            NodeKind::Binary(BinaryOp::Add),
            vec![number(), tree(NodeKind::UnaryNeg, vec![number()])],
        );
        for source in [
            tree(NodeKind::Group, vec![expression.clone()]),
            tree(
                NodeKind::Call { argument_count: 2 },
                vec![atom(NodeKind::Identifier("f".into())), number(), expression],
            ),
        ] {
            let mut editor = Editor::from_portable(&source).unwrap();
            let root = editor.root();
            let last = last_leaf(&editor.arena, root);
            editor.place_text_caret(last, 1).unwrap();
            assert!(!editor.insert_close_parenthesis());
            assert_eq!(editor.caret, Caret::at(root, Direction::Right));
            assert_eq!(editor.to_portable().unwrap(), source);
            assert!(!editor.undo());
        }
    }

    #[test]
    fn closing_inside_power_fraction_or_index_inserts_local_group_instead_of_skipping_right_state()
    {
        for (kind, slot) in [
            (NodeKind::Power, 1),
            (NodeKind::Fraction, 1),
            (NodeKind::Index { indices: 1 }, 1),
        ] {
            let expression = tree(kind.clone(), vec![number(), number()]);
            let source = tree(NodeKind::Group, vec![expression]);
            let mut editor = Editor::from_portable(&source).unwrap();
            let root = editor.root();
            let operator = editor.arena.get(root).unwrap().children[0];
            let target = editor.arena.get(operator).unwrap().children[slot];
            editor.place_text_caret(target, 1).unwrap();
            assert!(editor.insert_close_parenthesis());
            assert_eq!(editor.root(), root);
            assert_eq!(editor.arena.get(operator).unwrap().kind, kind);
            let group = editor.arena.get(operator).unwrap().children[slot];
            assert_eq!(editor.arena.get(group).unwrap().kind, NodeKind::Group);
            assert_eq!(editor.arena.get(group).unwrap().children, [target]);
            assert_eq!(editor.caret, Caret::at(group, Direction::Right));
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), source);
            editor.place_caret(operator, Direction::Right).unwrap();
            assert!(!editor.insert_close_parenthesis());
            assert_eq!(editor.caret, Caret::at(root, Direction::Right));
        }
    }

    #[test]
    fn explicit_degree_right_closes_polar_group_but_value_inside_degree_does_not_skip_its_right_state()
     {
        let source = tree(
            NodeKind::Group,
            vec![tree(
                NodeKind::Binary(BinaryOp::Polar),
                vec![
                    number(),
                    tree(NodeKind::Degree, vec![atom(NodeKind::Number("90".into()))]),
                ],
            )],
        );
        let mut editor = Editor::from_portable(&source).unwrap();
        let root = editor.root();
        let polar = editor.arena.get(root).unwrap().children[0];
        let degree = editor.arena.get(polar).unwrap().children[1];
        let value = editor.arena.get(degree).unwrap().children[0];
        editor.place_caret(degree, Direction::Right).unwrap();
        let revision = editor.arena.revision();
        assert!(!editor.insert_close_parenthesis());
        assert_eq!(editor.caret, Caret::at(root, Direction::Right));
        assert_eq!(editor.arena.revision(), revision);
        assert_eq!(editor.to_portable().unwrap(), source);
        editor.place_text_caret(value, 2).unwrap();
        assert!(editor.insert_close_parenthesis());
        assert_eq!(editor.root(), root);
        let inserted = editor.arena.get(degree).unwrap().children[0];
        assert_eq!(editor.arena.get(inserted).unwrap().kind, NodeKind::Group);
        assert_eq!(editor.arena.get(inserted).unwrap().children, [value]);
        assert_eq!(editor.caret, Caret::at(inserted, Direction::Right));
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), source);
    }

    #[test]
    fn root_close_postfix_preserves_atom_metadata_and_has_one_undo() {
        let source = atom(NodeKind::NativeIdentifier {
            name: "v".into(),
            labels: "UNIT".into(),
            contextual: Some(true),
            subscript: Some("𝛂".into()),
        });
        let mut editor = Editor::from_portable(&source).unwrap();
        let target = editor.root();
        editor.place_caret(target, Direction::Right).unwrap();
        let before = editor.caret;
        assert!(editor.insert_close_parenthesis());
        assert_eq!(
            editor.to_portable().unwrap(),
            tree(NodeKind::Group, vec![source.clone()])
        );
        assert_eq!(editor.caret, Caret::at(editor.root(), Direction::Right));
        assert!(editor.undo());
        assert_eq!(editor.caret, before);
        assert_eq!(editor.root(), target);
        assert!(!editor.undo());
        assert!(editor.redo());
    }

    #[test]
    fn close_scope_keeps_original_direction_and_unconfirmed_states_do_not_invalidate_redo() {
        for side in [Direction::Left, Direction::Right] {
            let mut editor = Editor::from_portable(&number()).unwrap();
            let target = editor.root();
            editor.select_subtree_from(target, side).unwrap();
            assert!(editor.insert_close_parenthesis());
            assert_eq!(editor.caret, Caret::at(editor.root(), side));
            assert!(editor.undo());
            assert_eq!(editor.selection, Selection::Subtree(target));
        }
        let source = tree(
            NodeKind::Matrix {
                rows: 1,
                cols: 1,
                display: None,
            },
            vec![number()],
        );
        let mut editor = Editor::from_portable(&source).unwrap();
        let root = editor.root();
        editor.place_caret(root, Direction::Right).unwrap();
        let before = editor.caret;
        assert!(!editor.insert_close_parenthesis());
        assert_eq!(editor.caret, before);
        assert_eq!(editor.to_portable().unwrap(), source);
        assert!(!editor.undo());
    }

    #[test]
    fn depth_failure_does_not_change_source_caret_or_redo() {
        let mut source = number();
        for _ in 1..MAX_TREE_DEPTH {
            source = tree(NodeKind::Group, vec![source]);
        }
        let mut editor = Editor::from_portable(&source).unwrap();
        let target = editor.caret.node;
        editor.place_text_caret(target, 1).unwrap();
        let caret = editor.caret;
        let revision = editor.arena.revision();
        assert!(!editor.insert_open_parenthesis());
        assert_eq!(editor.to_portable().unwrap(), source);
        assert_eq!(editor.caret, caret);
        assert_eq!(editor.arena.revision(), revision);
        assert!(!editor.undo());
    }
}
