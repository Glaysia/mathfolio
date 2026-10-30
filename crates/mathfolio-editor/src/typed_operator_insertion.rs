//! Normal symbol entry is an analysis-list transaction, not subtree wrapping.
//! Native contracts: docs/analysis/0118-normal-infix-entry-contract.md and 0114.
//! Temporary handles below are never serialized as additional semantic nodes.
use super::grouping_insertion::{grouping_fragment, grouping_text_slice};
use super::*;

#[path = "operator_replacement.rs"]
mod operator_replacement;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Fixity {
    Operand,
    Prefix,
    Infix,
    Postfix,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Handle {
    pub(super) node: NodeId,
    pub(super) fixity: Fixity,
}

impl Editor {
    /// Insert a typed arithmetic/comparison/logical operator. Existing explicit
    /// wrapping APIs keep their separate policies. A rejected input preserves
    /// the expression, selection, caret, undo and redo as one atomic command.
    pub fn insert_typed_operator(&mut self, kind: NodeKind) -> bool {
        if !typed_kind_allowed(&kind)
            || self.is_inside_string_literal()
            || self.matrix_selection().is_some()
            || self.caret.insertion != InsertionMode::Normal
        {
            return false;
        }
        if self.typeover_target().is_some() {
            // Native Typeover inserts the new operator after removing its
            // selected segment. Reuse the ordinary empty-template factory and
            // transfer that exact cursor, rather than wrapping the old term.
            let mut template = Editor::new();
            if !template.insert_typed_operator(kind) {
                return false;
            }
            return self.replace_typeover_editor(&template);
        }
        let mut candidate = self.clone_without_history();
        let accepted = if matches!(self.selection, Selection::Subtree(_)) {
            candidate.typed_scope_operator(kind)
        } else {
            candidate.typed_normal_operator(kind)
        };
        if !accepted || candidate.to_portable().is_err() {
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

    fn typed_scope_operator(&mut self, mut kind: NodeKind) -> bool {
        let target = self.target();
        if self.arena.get(target).unwrap().kind == NodeKind::Placeholder {
            return false;
        }
        if self.typed_container_root(target).is_none() {
            // Replacing a native APPLY callee needs its RestrictChild repair;
            // preserving an arbitrary computed callee would invent that repair.
            return false;
        }
        let direction = self
            .scope
            .as_ref()
            .map_or(self.caret.caret_side, |scope| scope.direction);
        if !matches!(direction, Direction::Left | Direction::Right) {
            return false;
        }
        if kind == NodeKind::UnaryNeg && direction == Direction::Right {
            kind = NodeKind::Binary(BinaryOp::Subtract);
        } else if kind == NodeKind::LogicalNot && direction == Direction::Right {
            return false;
        }
        let prefix = matches!(kind, NodeKind::UnaryNeg | NodeKind::LogicalNot);
        let entry = if prefix {
            let Some(entry) = self.program_operand_left_entry(target) else {
                return false;
            };
            Some(entry)
        } else {
            None
        };
        let parent = self.arena.get(target).unwrap().parent;
        let children = if prefix {
            vec![target]
        } else {
            let placeholder = self.arena.allocate(NodeKind::Placeholder, vec![]);
            if direction == Direction::Left {
                vec![placeholder, target]
            } else {
                vec![target, placeholder]
            }
        };
        let focus = if prefix {
            target
        } else {
            children[usize::from(direction == Direction::Right)]
        };
        let operator = self.arena.allocate(kind, children);
        self.replace_child(parent, target, operator);
        self.caret = entry.unwrap_or_else(|| Caret::at(focus, Direction::Indeterminate));
        self.selection = Selection::None;
        self.scope = None;
        self.typed_power_group_repair(operator);
        true
    }

    fn typed_normal_operator(&mut self, kind: NodeKind) -> bool {
        if self.caret.caret_side == Direction::Indeterminate
            && self.caret.value_range.is_none()
            && self.caret.operator_part.is_none()
            && self.caret.node_side == Direction::Indeterminate
            && self.arena.get(self.caret.node).is_some_and(|node| {
                matches!(
                    node.kind,
                    NodeKind::Binary(_) | NodeKind::Fraction | NodeKind::Power
                )
            })
        {
            return self.typed_replace_operator_center(kind);
        }
        let target = self.caret.node;
        let source = self.arena.get(target).unwrap().clone();
        let Some(container) = self.typed_container_root(target) else {
            return false;
        };
        let parent = self.arena.get(container).unwrap().parent;
        let mut handles = vec![];
        if !self.typed_emit(container, &mut handles, 0) {
            return false;
        }
        let Some(position) = handles.iter().position(|handle| handle.node == target) else {
            return false;
        };
        let new = self.arena.allocate(kind.clone(), vec![]);
        let new_handle = Handle {
            node: new,
            fixity: typed_new_fixity(&kind),
        };
        let mut suffix = None;
        let mut replacement = false;
        let mut boundary = None;
        if source.kind == NodeKind::Placeholder {
            replacement = true;
            handles.splice(position..=position, [new_handle]);
            self.retire(target, None);
        } else if let Some(text) = source.kind.text() {
            let formatted = matches!(
                source.kind,
                NodeKind::NativeIdentifier {
                    subscript: Some(_),
                    ..
                } | NodeKind::Imaginary(_)
                    | NodeKind::ImaginarySymbol { .. }
            );
            let length = text.encode_utf16().count();
            let range = match self.caret.caret_side {
                Direction::Left => ValueRange {
                    anchor: 0,
                    focus: 0,
                },
                Direction::Right => ValueRange {
                    anchor: length,
                    focus: length,
                },
                Direction::Indeterminate => self.caret.value_range.unwrap_or(ValueRange {
                    anchor: 0,
                    focus: 0,
                }),
                _ => return false,
            };
            let start = range.anchor.min(range.focus);
            let end = range.anchor.max(range.focus);
            if end > length {
                return false;
            }
            if formatted
                && (self.caret.caret_side == Direction::Indeterminate
                    || (start != 0 && end != length))
            {
                return false;
            }
            if start == 0 && end == length && start != end {
                replacement = true;
                handles.splice(position..=position, [new_handle]);
                self.retire(target, None);
            } else if start == 0 && end == 0 {
                boundary = Some(Direction::Left);
                handles.insert(position, new_handle);
                suffix = Some(target);
            } else if start == length && end == length {
                boundary = Some(Direction::Right);
                handles.insert(position + 1, new_handle);
            } else {
                let Some(prefix) = grouping_text_slice(text, 0, start) else {
                    return false;
                };
                let Some(rest) = grouping_text_slice(text, end, length) else {
                    return false;
                };
                let (Some(prefix_kind), Some(suffix_kind)) = (
                    grouping_fragment(&source.kind, prefix.clone()),
                    grouping_fragment(&source.kind, rest.clone()),
                ) else {
                    return false;
                };
                let mut replacement = vec![];
                if !prefix.is_empty() {
                    self.arena.node_mut(target).kind = prefix_kind;
                    replacement.push(Handle {
                        node: target,
                        fixity: Fixity::Operand,
                    });
                }
                replacement.push(new_handle);
                if !rest.is_empty() {
                    let right = if prefix.is_empty() {
                        self.arena.node_mut(target).kind = suffix_kind;
                        target
                    } else {
                        self.arena.allocate(suffix_kind, vec![])
                    };
                    suffix = Some(right);
                    replacement.push(Handle {
                        node: right,
                        fixity: Fixity::Operand,
                    });
                } else if prefix.is_empty() {
                    self.retire(target, None);
                }
                handles.splice(position..=position, replacement);
            }
        } else {
            match self.caret.caret_side {
                Direction::Right if typed_completed_right(&source.kind) => {
                    handles.insert(position + 1, new_handle);
                    boundary = Some(Direction::Right);
                }
                Direction::Left if typed_completed_left(&source.kind) => {
                    handles.insert(position, new_handle);
                    suffix = Some(target);
                    boundary = Some(Direction::Left);
                }
                _ => return false,
            }
        }
        let handles = self.typed_normalize(handles);
        let Some(root) = self.typed_parse(&handles) else {
            return false;
        };
        self.replace_child(parent, container, root);
        self.arena.node_mut(root).parent = parent;
        self.typed_power_group_repair(root);
        let placeholder = self
            .arena
            .get(new)
            .unwrap()
            .children
            .iter()
            .enumerate()
            .find_map(|(slot, child)| {
                if self.arena.get(*child).unwrap().kind == NodeKind::Placeholder {
                    Some(*child)
                } else if self.grouping_container_slot(new, slot) {
                    self.typed_first_placeholder(*child)
                } else {
                    None
                }
            });
        self.caret = if replacement {
            let Some(caret) = self.typed_replacement_caret(new) else {
                return false;
            };
            caret
        } else if let Some(direction) = boundary {
            let Some(caret) = self.typed_boundary_caret(new, direction) else {
                return false;
            };
            caret
        } else if let Some(placeholder) = placeholder {
            Caret::at(placeholder, Direction::Indeterminate)
        } else if let Some(suffix) = suffix {
            let Some(caret) = self.program_operand_left_entry(suffix) else {
                return false;
            };
            caret
        } else {
            return false;
        };
        self.selection = Selection::None;
        self.scope = None;
        true
    }

    pub(super) fn typed_container_root(&self, mut node: NodeId) -> Option<NodeId> {
        while let Some(parent) = self.arena.get(node)?.parent {
            let item = self.arena.get(parent)?;
            let slot = item.children.iter().position(|child| *child == node)?;
            if self.grouping_container_slot(parent, slot) {
                return Some(node);
            }
            // Our Call omits the native APPLY/ARGUMENT_LIST intermediate nodes.
            // Do not invent callee splitting from its flattened argument AST.
            if matches!(item.kind, NodeKind::Call { .. } | NodeKind::Function { .. }) {
                return None;
            }
            node = parent;
        }
        Some(node)
    }

    pub(super) fn typed_emit(&self, node: NodeId, handles: &mut Vec<Handle>, depth: usize) -> bool {
        if depth > MAX_TREE_DEPTH {
            return false;
        }
        let item = self.arena.get(node).unwrap();
        let fixity = match item.kind {
            NodeKind::Binary(_) | NodeKind::LocalDefine | NodeKind::Range { has_step: false } => {
                Fixity::Infix
            }
            NodeKind::UnaryNeg | NodeKind::LogicalNot | NodeKind::Return => Fixity::Prefix,
            NodeKind::Power
            | NodeKind::Index { .. }
            | NodeKind::Row
            | NodeKind::Column
            | NodeKind::Factorial
            | NodeKind::Percent
            | NodeKind::Degree
            | NodeKind::Transpose => Fixity::Postfix,
            _ => Fixity::Operand,
        };
        if fixity == Fixity::Prefix {
            handles.push(Handle { node, fixity });
        }
        if matches!(fixity, Fixity::Infix | Fixity::Prefix | Fixity::Postfix)
            && !self.typed_emit(item.children[0], handles, depth + 1)
        {
            return false;
        }
        if fixity != Fixity::Prefix {
            handles.push(Handle { node, fixity });
        }
        if fixity == Fixity::Infix && !self.typed_emit(item.children[1], handles, depth + 1) {
            return false;
        }
        true
    }

    pub(super) fn typed_normalize(&mut self, handles: Vec<Handle>) -> Vec<Handle> {
        let mut normalized = vec![];
        let mut complete = false;
        for mut handle in handles {
            let kind = self.arena.get(handle.node).unwrap().kind.clone();
            if kind == NodeKind::UnaryNeg && handle.fixity == Fixity::Prefix && complete {
                self.arena.node_mut(handle.node).kind = NodeKind::Binary(BinaryOp::Subtract);
                handle.fixity = Fixity::Infix;
            } else if kind == NodeKind::Binary(BinaryOp::Subtract)
                && handle.fixity == Fixity::Infix
                && !complete
            {
                self.arena.node_mut(handle.node).kind = NodeKind::UnaryNeg;
                handle.fixity = Fixity::Prefix;
            }
            if complete && matches!(handle.fixity, Fixity::Operand | Fixity::Prefix) {
                let node = self
                    .arena
                    .allocate(NodeKind::Binary(BinaryOp::Scale), vec![]);
                normalized.push(Handle {
                    node,
                    fixity: Fixity::Infix,
                });
            }
            complete = matches!(handle.fixity, Fixity::Operand | Fixity::Postfix);
            normalized.push(handle);
        }
        normalized
    }

    pub(super) fn typed_parse(&mut self, handles: &[Handle]) -> Option<NodeId> {
        let mut parser = OperatorParser {
            editor: self,
            handles,
            position: 0,
        };
        let root = parser.expression(0, 0)?;
        (parser.position == handles.len()).then_some(root)
    }

    pub(super) fn typed_power_group_repair(&mut self, root: NodeId) {
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            let item = self.arena.get(node).unwrap().clone();
            if item.kind == NodeKind::Power {
                let base = item.children[0];
                if matches!(
                    self.arena.get(base).unwrap().kind,
                    NodeKind::Power | NodeKind::Fraction
                ) {
                    let group = self.arena.allocate(NodeKind::Group, vec![base]);
                    self.arena.node_mut(node).children[0] = group;
                    self.arena.node_mut(group).parent = Some(node);
                }
            }
            stack.extend(item.children.iter().rev().copied());
        }
    }

    fn typed_first_placeholder(&self, root: NodeId) -> Option<NodeId> {
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            let item = self.arena.get(node)?;
            if item.kind == NodeKind::Placeholder {
                return Some(node);
            }
            stack.extend(item.children.iter().rev().copied());
        }
        None
    }

    // 053e replacement enters one native state to the left of an infix center
    // only when that state is a placeholder; otherwise it enters on the right.
    // This differs from 05d4's child/container placeholder scan for text splits.
    fn typed_replacement_caret(&self, operator: NodeId) -> Option<Caret> {
        let node = self.arena.get(operator)?;
        if matches!(node.kind, NodeKind::UnaryNeg | NodeKind::LogicalNot) {
            return self.program_operand_left_entry(node.children[0]);
        }
        let left = self.typed_last_navigation_node(node.children[0])?;
        if self.arena.get(left)?.kind == NodeKind::Placeholder {
            Some(Caret::at(left, Direction::Indeterminate))
        } else {
            self.program_operand_left_entry(node.children[1])
        }
    }

    fn typed_boundary_caret(&self, operator: NodeId, direction: Direction) -> Option<Caret> {
        let node = self.arena.get(operator)?;
        if matches!(node.kind, NodeKind::UnaryNeg | NodeKind::LogicalNot) {
            return self.program_operand_left_entry(node.children[0]);
        }
        if direction == Direction::Right {
            return self.program_operand_left_entry(node.children[1]);
        }
        let last = self.typed_last_navigation_node(node.children[0])?;
        let item = self.arena.get(last)?;
        if item.kind == NodeKind::Placeholder {
            return Some(Caret::at(last, Direction::Indeterminate));
        }
        if let Some(text) = item.kind.text()
            && !matches!(
                item.kind,
                NodeKind::NativeIdentifier {
                    subscript: Some(_),
                    ..
                } | NodeKind::Imaginary(_)
                    | NodeKind::ImaginarySymbol { .. }
            )
        {
            let length = text.encode_utf16().count();
            let mut caret = Caret::at(last, Direction::Indeterminate);
            caret.value_range = Some(ValueRange {
                anchor: length,
                focus: length,
            });
            return Some(caret);
        }
        Some(Caret::at(last, Direction::Right))
    }

    pub(super) fn typed_last_navigation_node(&self, mut node: NodeId) -> Option<NodeId> {
        loop {
            let item = self.arena.get(node)?;
            if item.kind.text().is_some()
                || item.kind == NodeKind::Placeholder
                || typed_completed_right(&item.kind)
                || matches!(
                    item.kind,
                    NodeKind::Root | NodeKind::Conjugate | NodeKind::Vectorize
                )
            {
                return Some(node);
            }
            match item.kind {
                NodeKind::Binary(_)
                | NodeKind::LocalDefine
                | NodeKind::Range { has_step: false } => node = *item.children.last()?,
                NodeKind::UnaryNeg | NodeKind::LogicalNot | NodeKind::Return => {
                    node = item.children[0]
                }
                _ => return None,
            }
        }
    }
}

struct OperatorParser<'a> {
    editor: &'a mut Editor,
    handles: &'a [Handle],
    position: usize,
}

impl OperatorParser<'_> {
    fn expression(&mut self, minimum: i32, depth: usize) -> Option<NodeId> {
        if depth > MAX_TREE_DEPTH {
            return None;
        }
        let mut left = match self.handles.get(self.position).copied() {
            Some(Handle {
                node,
                fixity: Fixity::Operand,
            }) => {
                self.position += 1;
                node
            }
            Some(Handle {
                node,
                fixity: Fixity::Prefix,
            }) => {
                self.position += 1;
                let binding = self.editor.arena.get(node)?.kind.precedence();
                let right = self.expression(binding, depth + 1)?;
                self.assign(node, vec![right]);
                node
            }
            _ => self.editor.arena.allocate(NodeKind::Placeholder, vec![]),
        };
        while let Some(handle) = self.handles.get(self.position).copied() {
            let kind = self.editor.arena.get(handle.node)?.kind.clone();
            let binding = kind.precedence();
            if binding < minimum {
                break;
            }
            match handle.fixity {
                Fixity::Postfix => {
                    self.position += 1;
                    let mut children = self.editor.arena.get(handle.node)?.children.clone();
                    *children.first_mut()? = left;
                    self.assign(handle.node, children);
                    left = handle.node;
                }
                Fixity::Infix => {
                    self.position += 1;
                    let right_associative = matches!(
                        kind,
                        NodeKind::Power | NodeKind::Binary(BinaryOp::CrossProduct)
                    );
                    let right = self.expression(
                        if right_associative {
                            binding
                        } else {
                            binding.saturating_add(1)
                        },
                        depth + 1,
                    )?;
                    self.assign(handle.node, vec![left, right]);
                    left = handle.node;
                }
                _ => break,
            }
        }
        Some(left)
    }

    fn assign(&mut self, node: NodeId, children: Vec<NodeId>) {
        self.editor.arena.node_mut(node).children = children.clone();
        for child in children {
            self.editor.arena.node_mut(child).parent = Some(node);
        }
    }
}

fn typed_kind_allowed(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Binary(
            BinaryOp::Add
                | BinaryOp::Subtract
                | BinaryOp::Multiply
                | BinaryOp::Equal
                | BinaryOp::NotEqual
                | BinaryOp::LessThan
                | BinaryOp::LessOrEqual
                | BinaryOp::GreaterThan
                | BinaryOp::GreaterOrEqual
                | BinaryOp::And
                | BinaryOp::Or
                | BinaryOp::Xor
        ) | NodeKind::Fraction
            | NodeKind::Power
            | NodeKind::UnaryNeg
            | NodeKind::LogicalNot
    )
}

fn typed_new_fixity(kind: &NodeKind) -> Fixity {
    if matches!(kind, NodeKind::UnaryNeg | NodeKind::LogicalNot) {
        Fixity::Prefix
    } else {
        Fixity::Infix
    }
}

pub(super) fn typed_completed_right(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Group
            | NodeKind::Absolute
            | NodeKind::Determinant
            | NodeKind::Fraction
            | NodeKind::Power
            | NodeKind::Index { .. }
            | NodeKind::Row
            | NodeKind::Column
            | NodeKind::Factorial
            | NodeKind::Degree
            | NodeKind::Percent
            | NodeKind::Transpose
            | NodeKind::Call { .. }
            | NodeKind::Matrix { .. }
    )
}

pub(super) fn typed_completed_left(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Group
            | NodeKind::Absolute
            | NodeKind::Determinant
            | NodeKind::Fraction
            | NodeKind::UnaryNeg
            | NodeKind::LogicalNot
            | NodeKind::Return
            | NodeKind::Matrix { .. }
    )
}

impl Editor {
    // 045b exposes the removed operator's container children before parsing.
    // Value fusion0493 and PH normalization0495 precede implicit adjacency.
    // Plain lexical fusion follows the retained installed CIL. Shipped Help
    // summarizes nonintegers differently; the distinction is recorded in0122.
    pub(super) fn reparse_deleted_operator(
        &mut self,
        operator: NodeId,
        preferred: Option<Caret>,
        backwards: bool,
    ) -> Option<Caret> {
        let container = self.typed_container_root(operator)?;
        let parent = self.arena.get(container)?.parent;
        let removed = self.arena.get(operator)?.clone();
        let mut handles = vec![];
        if !self.typed_emit(container, &mut handles, 0) {
            return None;
        }
        let position = handles.iter().position(|h| h.node == operator)?;
        let handle = handles[position];
        let mut exposed = vec![];
        if handle.fixity == Fixity::Operand {
            for child in &removed.children {
                if !self.typed_emit(*child, &mut exposed, 0) {
                    return None;
                }
            }
        } else if handle.fixity == Fixity::Postfix {
            // Existing Power/Index retain opaque children after the base, which
            // was already emitted. Removing the handle exposes these as well.
            for child in removed.children.iter().skip(1) {
                if !self.typed_emit(*child, &mut exposed, 0) {
                    return None;
                }
            }
        }
        handles.splice(position..=position, exposed);
        if handles.is_empty() {
            return None;
        }
        let mut fusion_cursor = None;
        let mut i = 1;
        while i < handles.len() {
            let left = handles[i - 1];
            let right = handles[i];
            if left.fixity == Fixity::Operand && right.fixity == Fixity::Operand {
                let lkind = self.arena.get(left.node)?.kind.clone();
                let rkind = self.arena.get(right.node)?.kind.clone();
                let (drop, keep, caret) = match (&lkind, &rkind) {
                    (l, r)
                        if super::formatted_deletion::fusion_value(l)
                            && super::formatted_deletion::fusion_value(r) =>
                    {
                        match super::formatted_deletion::deletion_value_fusion(l, r)? {
                            super::formatted_deletion::DeletionValueFusion::Merged {
                                kind,
                                offset,
                                insertion,
                            } => {
                                let node = self.arena.allocate(kind, vec![]);
                                let caret =
                                    self.deletion_formatted_caret(node, offset, insertion)?;
                                self.retire(left.node, None);
                                self.retire(right.node, None);
                                handles[i - 1] = Handle {
                                    node,
                                    fixity: Fixity::Operand,
                                };
                                handles.remove(i);
                                fusion_cursor = Some(caret);
                                i = i.saturating_sub(1).max(1);
                                continue;
                            }
                            super::formatted_deletion::DeletionValueFusion::Separate => {
                                let scale = self
                                    .arena
                                    .allocate(NodeKind::Binary(BinaryOp::Scale), vec![]);
                                handles.insert(
                                    i,
                                    Handle {
                                        node: scale,
                                        fixity: Fixity::Infix,
                                    },
                                );
                                fusion_cursor = Some(if backwards {
                                    self.program_operand_left_entry(right.node)?
                                } else {
                                    self.deletion_right_entry(left.node)?
                                });
                                i += 2;
                                continue;
                            }
                        }
                    }
                    (NodeKind::Placeholder, _) if rkind != NodeKind::Placeholder => (
                        left.node,
                        right.node,
                        self.program_operand_left_entry(right.node)?,
                    ),
                    (_, NodeKind::Placeholder) if lkind != NodeKind::Placeholder => {
                        (right.node, left.node, self.deletion_right_entry(left.node)?)
                    }
                    _ => return None,
                };
                self.arena.node_mut(drop).children.clear();
                self.retire(drop, None);
                handles[i - 1] = Handle {
                    node: keep,
                    fixity: Fixity::Operand,
                };
                handles.remove(i);
                fusion_cursor = Some(caret);
                i = i.saturating_sub(1).max(1);
            } else {
                i += 1;
            }
        }
        self.arena.node_mut(operator).children.clear();
        self.retire(operator, None);
        let handles = self.typed_normalize(handles);
        let root = self.typed_parse(&handles)?;
        self.replace_child(parent, container, root);
        self.arena.node_mut(root).parent = parent;
        self.typed_power_group_repair(root);
        let caret = fusion_cursor.or(preferred)?;
        self.arena.get(caret.node)?;
        Some(caret)
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
    fn number(value: &str) -> PortableTree {
        atom(NodeKind::Number(value.into()))
    }
    fn missing() -> PortableTree {
        atom(NodeKind::Placeholder)
    }
    fn binary_kinds() -> Vec<NodeKind> {
        [
            BinaryOp::Add,
            BinaryOp::Subtract,
            BinaryOp::Multiply,
            BinaryOp::Equal,
            BinaryOp::NotEqual,
            BinaryOp::LessThan,
            BinaryOp::LessOrEqual,
            BinaryOp::GreaterThan,
            BinaryOp::GreaterOrEqual,
            BinaryOp::And,
            BinaryOp::Or,
            BinaryOp::Xor,
        ]
        .into_iter()
        .map(NodeKind::Binary)
        .chain([NodeKind::Fraction, NodeKind::Power])
        .collect()
    }

    #[test]
    fn numeric_middle_and_partial_selection_insert_operators_between_value_fragments() {
        for kind in binary_kinds() {
            for (anchor, focus, left, right) in
                [(2, 2, "12", "34"), (1, 3, "1", "4"), (3, 1, "1", "4")]
            {
                let source = number("1234");
                let mut editor = Editor::from_portable(&source).unwrap();
                let original = editor.root();
                editor
                    .place_text_selection(original, anchor, focus)
                    .unwrap();
                let before = editor.caret;
                assert!(editor.insert_typed_operator(kind.clone()), "{kind:?}");
                let expected = tree(kind.clone(), vec![number(left), number(right)]);
                assert_eq!(editor.to_portable().unwrap(), expected);
                let children = editor.arena.get(editor.root()).unwrap().children.clone();
                assert_eq!(children[0], original);
                assert_eq!(editor.caret.node, children[1]);
                assert_eq!(
                    editor.caret.value_range,
                    Some(ValueRange {
                        anchor: 0,
                        focus: 0
                    })
                );
                assert!(editor.undo());
                assert_eq!(editor.root(), original);
                assert_eq!(editor.to_portable().unwrap(), source);
                assert_eq!(editor.caret, before);
                assert!(!editor.undo());
                assert!(editor.redo());
                assert_eq!(editor.to_portable().unwrap(), expected);
            }
        }
    }

    #[test]
    fn start_end_and_full_value_selection_follow_distinct_normal_modes() {
        for kind in binary_kinds() {
            for offset in [0, 4] {
                let mut editor = Editor::from_portable(&number("1234")).unwrap();
                let original = editor.root();
                editor.place_text_caret(original, offset).unwrap();
                assert!(editor.insert_typed_operator(kind.clone()));
                let expected = if offset == 0 && kind == NodeKind::Binary(BinaryOp::Subtract) {
                    tree(NodeKind::UnaryNeg, vec![number("1234")])
                } else if offset == 0 {
                    tree(kind.clone(), vec![missing(), number("1234")])
                } else {
                    tree(kind.clone(), vec![number("1234"), missing()])
                };
                assert_eq!(editor.to_portable().unwrap(), expected);
                if offset == 0 && kind == NodeKind::Binary(BinaryOp::Subtract) {
                    assert_eq!(editor.caret.node, original);
                    assert_eq!(
                        editor.caret.value_range,
                        Some(ValueRange {
                            anchor: 0,
                            focus: 0
                        })
                    );
                } else {
                    assert_eq!(
                        editor.arena.get(editor.caret.node).unwrap().kind,
                        NodeKind::Placeholder
                    );
                }
            }
        }
        for kind in binary_kinds()
            .into_iter()
            .chain([NodeKind::UnaryNeg, NodeKind::LogicalNot])
        {
            for (anchor, focus) in [(0, 4), (4, 0)] {
                let source = atom(NodeKind::NativeIdentifier {
                    name: "abcd".into(),
                    labels: "UNIT".into(),
                    contextual: Some(true),
                    subscript: None,
                });
                let mut editor = Editor::from_portable(&source).unwrap();
                let original = editor.root();
                editor
                    .place_text_selection(original, anchor, focus)
                    .unwrap();
                let before = editor.caret;
                assert!(editor.insert_typed_operator(kind.clone()));
                let expected_kind = if kind == NodeKind::Binary(BinaryOp::Subtract) {
                    NodeKind::UnaryNeg
                } else {
                    kind.clone()
                };
                assert_eq!(
                    editor.to_portable().unwrap(),
                    tree(
                        expected_kind.clone(),
                        vec![missing(); expected_kind.arity()]
                    )
                );
                assert!(editor.arena.get(original).is_none());
                assert_eq!(
                    editor.caret.node,
                    editor.arena.get(editor.root()).unwrap().children[0]
                );
                assert!(editor.undo());
                assert_eq!(editor.to_portable().unwrap(), source);
                assert_eq!(editor.root(), original);
                assert_eq!(editor.caret, before);
                assert!(!editor.undo());
            }
        }
    }

    #[test]
    fn nested_full_replacement_and_partial_split_have_different_placeholder_cursor_rules() {
        let source = tree(
            NodeKind::Binary(BinaryOp::Multiply),
            vec![number("2"), number("1234")],
        );
        let mut editor = Editor::from_portable(&source).unwrap();
        let multiply = editor.root();
        let original = editor.arena.get(multiply).unwrap().children[1];
        editor.place_text_selection(original, 0, 4).unwrap();
        assert!(editor.insert_typed_operator(NodeKind::Binary(BinaryOp::Add)));
        assert_eq!(
            editor.to_portable().unwrap(),
            tree(
                NodeKind::Binary(BinaryOp::Add),
                vec![
                    tree(
                        NodeKind::Binary(BinaryOp::Multiply),
                        vec![number("2"), missing()]
                    ),
                    missing()
                ]
            )
        );
        let inner_placeholder = editor.arena.get(multiply).unwrap().children[1];
        let outer_placeholder = editor.arena.get(editor.root()).unwrap().children[1];
        assert_eq!(editor.caret.node, inner_placeholder);
        assert_ne!(editor.caret.node, outer_placeholder);
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), source);

        let source = tree(
            NodeKind::Binary(BinaryOp::Add),
            vec![missing(), number("1234")],
        );
        let mut editor = Editor::from_portable(&source).unwrap();
        let old_add = editor.root();
        let unrelated = editor.arena.get(old_add).unwrap().children[0];
        let original = editor.arena.get(old_add).unwrap().children[1];
        editor.place_text_caret(original, 2).unwrap();
        assert!(editor.insert_typed_operator(NodeKind::Binary(BinaryOp::Add)));
        assert_eq!(
            editor.to_portable().unwrap(),
            tree(
                NodeKind::Binary(BinaryOp::Add),
                vec![
                    tree(
                        NodeKind::Binary(BinaryOp::Add),
                        vec![missing(), number("12")]
                    ),
                    number("34")
                ]
            )
        );
        assert_ne!(editor.caret.node, unrelated);
        assert_eq!(
            editor.arena.get(editor.caret.node).unwrap().kind,
            NodeKind::Number("34".into())
        );
        assert_eq!(
            editor.caret.value_range,
            Some(ValueRange {
                anchor: 0,
                focus: 0
            })
        );
    }

    #[test]
    fn normal_minus_alternates_and_logical_not_uses_implicit_scaling_without_rewriting_multiplication()
     {
        for (request, expected) in [
            (
                NodeKind::UnaryNeg,
                tree(
                    NodeKind::Binary(BinaryOp::Subtract),
                    vec![number("12"), number("34")],
                ),
            ),
            (
                NodeKind::LogicalNot,
                tree(
                    NodeKind::Binary(BinaryOp::Scale),
                    vec![number("12"), tree(NodeKind::LogicalNot, vec![number("34")])],
                ),
            ),
        ] {
            let mut editor = Editor::from_portable(&number("1234")).unwrap();
            editor.place_text_caret(editor.root(), 2).unwrap();
            assert!(editor.insert_typed_operator(request));
            assert_eq!(editor.to_portable().unwrap(), expected);
            assert_eq!(
                editor.arena.get(editor.caret.node).unwrap().kind,
                NodeKind::Number("34".into())
            );
        }
        let source = tree(
            NodeKind::Binary(BinaryOp::Multiply),
            vec![number("10"), number("1234")],
        );
        let mut editor = Editor::from_portable(&source).unwrap();
        let multiplication = editor.root();
        let target = editor.arena.get(multiplication).unwrap().children[1];
        editor.place_text_caret(target, 2).unwrap();
        assert!(editor.insert_typed_operator(NodeKind::Binary(BinaryOp::Add)));
        assert_eq!(
            editor.to_portable().unwrap(),
            tree(
                NodeKind::Binary(BinaryOp::Add),
                vec![
                    tree(
                        NodeKind::Binary(BinaryOp::Multiply),
                        vec![number("10"), number("12")]
                    ),
                    number("34")
                ]
            )
        );
        assert_eq!(
            editor.arena.get(editor.root()).unwrap().children[0],
            multiplication
        );
        assert_eq!(
            editor.arena.get(multiplication).unwrap().kind,
            NodeKind::Binary(BinaryOp::Multiply)
        );
    }

    #[test]
    fn native_container_slots_and_explicit_completed_right_states_keep_different_ranges() {
        for (source, slot) in [
            (tree(NodeKind::Power, vec![number("2"), number("3")]), 1),
            (tree(NodeKind::Fraction, vec![number("2"), number("3")]), 1),
            (
                tree(
                    NodeKind::Index { indices: 1 },
                    vec![atom(NodeKind::Identifier("A".into())), number("3")],
                ),
                1,
            ),
        ] {
            let mut editor = Editor::from_portable(&source).unwrap();
            let root = editor.root();
            let target = editor.arena.get(root).unwrap().children[slot];
            editor.place_text_caret(target, 1).unwrap();
            assert!(editor.insert_typed_operator(NodeKind::Binary(BinaryOp::Add)));
            let mut expected = source.clone();
            expected.children[slot] = tree(
                NodeKind::Binary(BinaryOp::Add),
                vec![number("3"), missing()],
            );
            assert_eq!(editor.to_portable().unwrap(), expected);
            assert_eq!(editor.root(), root);
            assert!(editor.undo());
            editor.place_caret(root, Direction::Right).unwrap();
            assert!(editor.insert_typed_operator(NodeKind::Binary(BinaryOp::Add)));
            assert_eq!(
                editor.to_portable().unwrap(),
                tree(
                    NodeKind::Binary(BinaryOp::Add),
                    vec![source.clone(), missing()]
                )
            );
            assert_eq!(editor.arena.get(editor.root()).unwrap().children[0], root);
        }
    }

    #[test]
    fn scope_direction_preserves_expression_and_restores_space_history_as_one_undo() {
        for kind in binary_kinds() {
            for direction in [Direction::Left, Direction::Right] {
                let source = atom(NodeKind::AnnotatedIdentifier {
                    name: "x".into(),
                    labels: "UNIT".into(),
                });
                let mut editor = Editor::from_portable(&source).unwrap();
                let target = editor.root();
                editor
                    .place_text_caret(target, usize::from(direction == Direction::Right))
                    .unwrap();
                let origin = editor.caret;
                assert!(editor.expand_scope());
                assert!(editor.insert_typed_operator(kind.clone()));
                assert_eq!(
                    editor.to_portable().unwrap(),
                    tree(
                        kind.clone(),
                        if direction == Direction::Left {
                            vec![missing(), source.clone()]
                        } else {
                            vec![source.clone(), missing()]
                        }
                    )
                );
                assert!(editor.scope.is_none());
                assert_eq!(editor.selection, Selection::None);
                assert_eq!(
                    editor.arena.get(editor.caret.node).unwrap().kind,
                    NodeKind::Placeholder
                );
                assert!(editor.undo());
                assert!(editor.scope.is_some());
                assert!(editor.contract_scope());
                assert_eq!(editor.caret, origin);
                assert_eq!(editor.to_portable().unwrap(), source);
                assert!(!editor.undo());
            }
        }
    }

    #[test]
    fn prefix_scope_alternates_and_power_base_repair_keep_native_operand_entry() {
        let source = tree(NodeKind::Group, vec![number("2")]);
        for prefix in [NodeKind::UnaryNeg, NodeKind::LogicalNot] {
            let mut editor = Editor::from_portable(&source).unwrap();
            let target = editor.root();
            editor.select_subtree_from(target, Direction::Left).unwrap();
            assert!(editor.insert_typed_operator(prefix.clone()));
            assert_eq!(
                editor.to_portable().unwrap(),
                tree(prefix, vec![source.clone()])
            );
            assert_eq!(editor.caret, Caret::at(target, Direction::Left));
        }
        let mut editor = Editor::from_portable(&source).unwrap();
        editor
            .select_subtree_from(editor.root(), Direction::Right)
            .unwrap();
        assert!(!editor.insert_typed_operator(NodeKind::LogicalNot));
        assert!(editor.insert_typed_operator(NodeKind::UnaryNeg));
        assert_eq!(
            editor.to_portable().unwrap(),
            tree(
                NodeKind::Binary(BinaryOp::Subtract),
                vec![source, missing()]
            )
        );

        let source = tree(NodeKind::Fraction, vec![number("2"), number("3")]);
        let mut editor = Editor::from_portable(&source).unwrap();
        let target = editor.root();
        editor
            .select_subtree_from(target, Direction::Right)
            .unwrap();
        assert!(editor.insert_typed_operator(NodeKind::Power));
        assert_eq!(
            editor.to_portable().unwrap(),
            tree(
                NodeKind::Power,
                vec![tree(NodeKind::Group, vec![source.clone()]), missing()]
            )
        );
        assert!(editor.arena.get(target).is_some());
        assert!(editor.undo());
        assert_eq!(editor.root(), target);
        assert_eq!(editor.to_portable().unwrap(), source);
    }

    #[test]
    fn utf16_fragments_preserve_name_metadata_and_factory_reclassifies_numeric_suffixes() {
        for (name, anchor, focus, left, right) in [
            ("a𝛂dz", 1, 4, "a", "z"),
            ("a𝛂dz", 4, 1, "a", "z"),
            ("x12", 1, 1, "x", "12"),
        ] {
            let source = atom(NodeKind::NativeIdentifier {
                name: name.into(),
                labels: "UNIT".into(),
                contextual: Some(true),
                subscript: None,
            });
            let mut editor = Editor::from_portable(&source).unwrap();
            let target = editor.root();
            editor.place_text_selection(target, anchor, focus).unwrap();
            let before = editor.caret;
            assert!(editor.insert_typed_operator(NodeKind::Binary(BinaryOp::Add)));
            let result = editor.to_portable().unwrap();
            assert_eq!(
                result.children[0].kind,
                NodeKind::NativeIdentifier {
                    name: left.into(),
                    labels: "UNIT".into(),
                    contextual: Some(true),
                    subscript: None
                }
            );
            assert_eq!(
                result.children[1].kind,
                if right == "12" {
                    NodeKind::Number("12".into())
                } else {
                    NodeKind::NativeIdentifier {
                        name: right.into(),
                        labels: "UNIT".into(),
                        contextual: Some(true),
                        subscript: None,
                    }
                }
            );
            assert_eq!(editor.arena.get(editor.root()).unwrap().children[0], target);
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), source);
            assert_eq!(editor.caret, before);
        }
    }

    #[test]
    fn suffix_end_toggle_maps_to_whole_name_right_and_does_not_duplicate_literal_suffix() {
        let source = atom(NodeKind::NativeIdentifier {
            name: "I".into(),
            labels: "VARIABLE".into(),
            contextual: Some(true),
            subscript: Some("dc".into()),
        });
        let mut editor = Editor::from_portable(&source).unwrap();
        let target = editor.root();
        editor.place_subscript_selection(target, 2, 2).unwrap();
        assert!(!editor.toggle_literal_subscript());
        assert_eq!(editor.caret, Caret::at(target, Direction::Right));
        assert!(editor.insert_typed_operator(NodeKind::Binary(BinaryOp::Add)));
        assert_eq!(
            editor.to_portable().unwrap(),
            tree(
                NodeKind::Binary(BinaryOp::Add),
                vec![source.clone(), missing()]
            )
        );
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), source);
        assert_eq!(editor.caret, Caret::at(target, Direction::Right));
        assert!(!editor.undo());
        editor.place_subscript_selection(target, 1, 1).unwrap();
        assert!(!editor.toggle_literal_subscript());
        let caret = editor.caret;
        let revision = editor.arena.revision();
        assert!(!editor.insert_typed_operator(NodeKind::Binary(BinaryOp::Add)));
        assert_eq!(editor.caret, caret);
        assert_eq!(editor.arena.revision(), revision);
        assert_eq!(editor.to_portable().unwrap(), source);
    }

    #[test]
    fn call_argument_entry_is_supported_but_callee_and_unknown_lexical_repairs_preserve_redo() {
        let source = tree(
            NodeKind::Call { argument_count: 1 },
            vec![atom(NodeKind::Identifier("f12".into())), number("1234")],
        );
        let mut editor = Editor::from_portable(&source).unwrap();
        let root = editor.root();
        let name = editor.arena.get(root).unwrap().children[0];
        let argument = editor.arena.get(root).unwrap().children[1];
        editor.place_text_caret(argument, 2).unwrap();
        assert!(editor.insert_typed_operator(NodeKind::Binary(BinaryOp::Add)));
        assert_eq!(editor.root(), root);
        assert_eq!(
            editor.to_portable().unwrap().children[1],
            tree(
                NodeKind::Binary(BinaryOp::Add),
                vec![number("12"), number("34")]
            )
        );
        assert!(editor.undo());
        editor.place_text_caret(name, 1).unwrap();
        let revision = editor.arena.revision();
        assert!(!editor.insert_typed_operator(NodeKind::Binary(BinaryOp::Add)));
        editor.select_subtree_from(name, Direction::Right).unwrap();
        let caret = editor.caret;
        assert!(!editor.insert_typed_operator(NodeKind::Binary(BinaryOp::Add)));
        assert_eq!(editor.caret, caret);
        assert_eq!(editor.selection, Selection::Subtree(name));
        assert_eq!(editor.arena.revision(), revision);
        assert_eq!(editor.to_portable().unwrap(), source);
        assert!(editor.redo());
        for source in [atom(NodeKind::Identifier("x12h".into())), number("1e-3")] {
            let mut editor = Editor::from_portable(&source).unwrap();
            editor.place_text_caret(editor.root(), 2).unwrap();
            let revision = editor.arena.revision();
            assert!(!editor.insert_typed_operator(NodeKind::Binary(BinaryOp::Add)));
            assert_eq!(editor.arena.revision(), revision);
            assert_eq!(editor.to_portable().unwrap(), source);
        }
    }

    #[test]
    fn boundary_cursor_uses_insertion_direction_while_split_checks_only_its_operator_containers() {
        let source = tree(NodeKind::Group, vec![missing()]);
        let mut editor = Editor::from_portable(&source).unwrap();
        let group = editor.root();
        let old_placeholder = editor.arena.get(group).unwrap().children[0];
        editor.place_caret(group, Direction::Right).unwrap();
        assert!(editor.insert_typed_operator(NodeKind::Fraction));
        let denominator = editor.arena.get(editor.root()).unwrap().children[1];
        assert_eq!(editor.caret.node, denominator);
        assert_ne!(editor.caret.node, old_placeholder);
        assert_eq!(
            editor.to_portable().unwrap(),
            tree(NodeKind::Fraction, vec![source, missing()])
        );

        let source = tree(NodeKind::Power, vec![number("1234"), missing()]);
        let mut editor = Editor::from_portable(&source).unwrap();
        let power = editor.root();
        let base = editor.arena.get(power).unwrap().children[0];
        let exponent = editor.arena.get(power).unwrap().children[1];
        editor.place_text_caret(base, 2).unwrap();
        assert!(editor.insert_typed_operator(NodeKind::Fraction));
        assert_eq!(
            editor.to_portable().unwrap(),
            tree(
                NodeKind::Fraction,
                vec![
                    number("12"),
                    tree(NodeKind::Power, vec![number("34"), missing()])
                ]
            )
        );
        assert_eq!(editor.caret.node, exponent);
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), source);
    }

    #[test]
    fn mode_unknown_kind_and_nested_callee_scope_rejections_preserve_caret_selection_and_redo() {
        let source = tree(
            NodeKind::Call { argument_count: 1 },
            vec![
                tree(
                    NodeKind::Index { indices: 1 },
                    vec![atom(NodeKind::Identifier("A".into())), number("1")],
                ),
                number("3"),
            ],
        );
        let mut editor = Editor::from_portable(&source).unwrap();
        let call = editor.root();
        let index = editor.arena.get(call).unwrap().children[0];
        let name = editor.arena.get(index).unwrap().children[0];
        let index_value = editor.arena.get(index).unwrap().children[1];
        editor.place_text_caret(index_value, 1).unwrap();
        assert!(editor.insert_typed_operator(NodeKind::Binary(BinaryOp::Add)));
        assert!(editor.undo());
        editor.select_subtree_from(name, Direction::Right).unwrap();
        let caret = editor.caret;
        let revision = editor.arena.revision();
        assert!(!editor.insert_typed_operator(NodeKind::Binary(BinaryOp::Add)));
        assert_eq!(editor.caret, caret);
        assert_eq!(editor.selection, Selection::Subtree(name));
        assert_eq!(editor.arena.revision(), revision);
        assert_eq!(editor.to_portable().unwrap(), source);
        assert!(editor.redo());

        let source = tree(
            NodeKind::Matrix {
                rows: 1,
                cols: 2,
                display: None,
            },
            vec![number("1"), number("2")],
        );
        let mut editor = Editor::from_portable(&source).unwrap();
        let matrix = editor.root();
        assert!(editor.select_matrix_cells(matrix, 0, 1));
        let caret = editor.caret;
        let selection = editor.selection;
        for kind in binary_kinds() {
            assert!(!editor.insert_typed_operator(kind));
            assert_eq!(editor.caret, caret);
            assert_eq!(editor.selection, selection);
            assert_eq!(editor.to_portable().unwrap(), source);
        }
        let mut editor = Editor::from_portable(&number("1234")).unwrap();
        editor.place_text_caret(editor.root(), 2).unwrap();
        let caret = editor.caret;
        let revision = editor.arena.revision();
        for kind in [
            NodeKind::Group,
            NodeKind::Program { statement_count: 1 },
            NodeKind::Binary(BinaryOp::Define),
            NodeKind::Binary(BinaryOp::Evaluate),
            NodeKind::Binary(BinaryOp::Scale),
            NodeKind::Binary(BinaryOp::CrossProduct),
            NodeKind::Binary(BinaryOp::InlineDivide),
            NodeKind::Binary(BinaryOp::Polar),
        ] {
            assert!(!editor.insert_typed_operator(kind));
            assert_eq!(editor.caret, caret);
            assert_eq!(editor.arena.revision(), revision);
        }
        editor.toggle_subscript();
        let caret = editor.caret;
        assert!(!editor.insert_typed_operator(NodeKind::Binary(BinaryOp::Add)));
        assert_eq!(editor.caret, caret);
        assert!(!editor.undo());
    }

    fn node_budget_source(count: usize) -> PortableTree {
        let mut remaining = count - 5;
        let children = (0..4)
            .map(|_| {
                let size = remaining.min(4096);
                remaining -= size;
                tree(
                    NodeKind::Program {
                        statement_count: size,
                    },
                    vec![number("1234"); size],
                )
            })
            .collect();
        assert_eq!(remaining, 0);
        tree(NodeKind::Program { statement_count: 4 }, children)
    }

    #[test]
    fn exact_node_limits_count_split_prefix_implicit_scale_and_replacement_atom_reclamation() {
        for (kind, anchor, focus, added) in [
            (NodeKind::Binary(BinaryOp::Add), 2, 2, 2),
            (NodeKind::Binary(BinaryOp::Add), 0, 4, 2),
            (NodeKind::UnaryNeg, 0, 0, 1),
            (NodeKind::LogicalNot, 2, 2, 3),
        ] {
            for spare in [added - 1, added] {
                let source = node_budget_source(MAX_TREE_NODES - spare);
                let mut editor = Editor::from_portable(&source).unwrap();
                let target = first_leaf(&editor.arena, editor.root());
                editor
                    .set_atom(target, NodeKind::Number("3".into()))
                    .unwrap();
                assert!(editor.undo());
                editor.place_text_selection(target, anchor, focus).unwrap();
                let caret = editor.caret;
                let revision = editor.arena.revision();
                assert_eq!(editor.insert_typed_operator(kind.clone()), spare == added);
                if spare == added {
                    assert_eq!(editor.arena.nodes().count(), MAX_TREE_NODES);
                    assert!(editor.undo());
                    assert_eq!(editor.to_portable().unwrap(), source);
                    assert_eq!(editor.caret, caret);
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
    fn parser_depth_growth_and_oversized_reassociation_fail_without_partial_mutation() {
        let mut source = number("1234");
        for _ in 1..MAX_TREE_DEPTH {
            source = tree(NodeKind::Group, vec![source]);
        }
        let mut editor = Editor::from_portable(&source).unwrap();
        let target = first_leaf(&editor.arena, editor.root());
        editor.place_text_caret(target, 2).unwrap();
        let caret = editor.caret;
        let revision = editor.arena.revision();
        assert!(!editor.insert_typed_operator(NodeKind::Binary(BinaryOp::Add)));
        assert_eq!(editor.to_portable().unwrap(), source);
        assert_eq!(editor.caret, caret);
        assert_eq!(editor.arena.revision(), revision);
        assert!(!editor.undo());

        let mut source = number("2");
        for _ in 0..9 {
            source = tree(
                NodeKind::Binary(BinaryOp::CrossProduct),
                vec![source.clone(), source],
            );
        }
        let mut editor = Editor::from_portable(&source).unwrap();
        let target = last_leaf(&editor.arena, editor.root());
        editor.place_text_caret(target, 1).unwrap();
        let caret = editor.caret;
        let revision = editor.arena.revision();
        assert!(!editor.insert_typed_operator(NodeKind::Binary(BinaryOp::Add)));
        assert_eq!(editor.to_portable().unwrap(), source);
        assert_eq!(editor.caret, caret);
        assert_eq!(editor.arena.revision(), revision);
        assert!(!editor.undo());
    }

    #[test]
    fn independent_explicit_wrapping_apis_keep_their_existing_meaning() {
        let mut editor = Editor::from_portable(&number("1234")).unwrap();
        editor.place_text_caret(editor.root(), 2).unwrap();
        editor.insert_infix(BinaryOp::Add);
        assert_eq!(
            editor.to_portable().unwrap(),
            tree(
                NodeKind::Binary(BinaryOp::Add),
                vec![number("1234"), missing()]
            )
        );
        assert!(editor.undo());
        assert!(editor.insert_typed_operator(NodeKind::Binary(BinaryOp::Add)));
        assert_eq!(
            editor.to_portable().unwrap(),
            tree(
                NodeKind::Binary(BinaryOp::Add),
                vec![number("12"), number("34")]
            )
        );
    }
}
