//! Structural editing primitives informed by the Prime 12 static observations.
//!
//! Descriptor numbers and fraction layout formulas are observed facts. Wrapping,
//! scope order, and initial caret placement are independent implementation
//! policies awaiting recorded comparison with Prime; this crate claims no parity.
//! See `docs/spec/mathcad12-editor-contract.json` and the editor analysis report.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Append-only arena coordinate. Removed nodes never transfer their ID to a new node.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NodeId(usize);

impl NodeId {
    pub fn index(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    InlineDivide,
    Define,
    Evaluate,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum NodeKind {
    Placeholder,
    Identifier(String),
    AnnotatedIdentifier { name: String, labels: String },
    Number(String),
    Binary(BinaryOp),
    Fraction,
    Power,
    Group,
}

impl NodeKind {
    /// Values explicitly assigned by DefaultDescriptorTable, method 0x06000bbb.
    pub fn precedence(&self) -> i32 {
        match self {
            Self::Placeholder
            | Self::Identifier(_)
            | Self::AnnotatedIdentifier { .. }
            | Self::Number(_) => i32::MAX,
            Self::Binary(BinaryOp::Define) => 10,
            Self::Binary(BinaryOp::Evaluate) => 20,
            Self::Binary(BinaryOp::Add | BinaryOp::Subtract) => 100,
            Self::Binary(BinaryOp::Multiply | BinaryOp::InlineDivide) => 140,
            Self::Fraction => 143,
            Self::Power => 160,
            Self::Group => 299,
        }
    }

    pub fn descriptor_name(&self) -> &'static str {
        match self {
            Self::Placeholder => "PLACEHOLDER",
            Self::Identifier(_) | Self::AnnotatedIdentifier { .. } => "IDENTIFIER",
            Self::Number(_) => "NUMERIC",
            Self::Binary(BinaryOp::Add) => "ADDITION",
            Self::Binary(BinaryOp::Subtract) => "SUBTRACTION",
            Self::Binary(BinaryOp::Multiply) => "MULTIPLICATION",
            Self::Binary(BinaryOp::InlineDivide) => "INLINE_DIVISION",
            Self::Binary(BinaryOp::Define) => "DEFINITION",
            Self::Binary(BinaryOp::Evaluate) => "EVALUATION",
            Self::Fraction => "DIVISION",
            Self::Power => "EXPONENTIATION",
            Self::Group => "GROUPING",
        }
    }

    fn arity(&self) -> usize {
        match self {
            Self::Placeholder
            | Self::Identifier(_)
            | Self::AnnotatedIdentifier { .. }
            | Self::Number(_) => 0,
            Self::Group => 1,
            Self::Binary(_) | Self::Fraction | Self::Power => 2,
        }
    }

    fn text(&self) -> Option<&str> {
        match self {
            Self::Identifier(text) | Self::Number(text) => Some(text),
            Self::AnnotatedIdentifier { name, .. } => Some(name),
            _ => None,
        }
    }
}

/// Portable semantic shape only: never includes arena IDs, caret, undo, or redo.
/// Deserialization alone does not validate; use `validate` or Editor import APIs.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableTree {
    pub kind: NodeKind,
    pub children: Vec<PortableTree>,
}

pub const MAX_TREE_DEPTH: usize = 128;
pub const MAX_TREE_NODES: usize = 16_384;
pub const MAX_ATOM_BYTES: usize = 16_384;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TreeError {
    TooDeep,
    TooManyNodes,
    TextTooLong,
    InvalidNumber,
    InvalidIdentifier,
    InvalidLabels,
    Arity {
        descriptor: &'static str,
        expected: usize,
        actual: usize,
    },
    InvalidTree(&'static str),
}

impl std::fmt::Display for TreeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for TreeError {}

impl PortableTree {
    /// Bounded, iterative validation of the independently implemented subset.
    /// Limits and numeric grammar are project policies, not recovered Prime limits.
    pub fn validate(&self) -> Result<(), TreeError> {
        let mut stack = vec![(self, 1)];
        let mut count = 0;
        while let Some((node, depth)) = stack.pop() {
            count += 1;
            if count > MAX_TREE_NODES {
                return Err(TreeError::TooManyNodes);
            }
            if depth > MAX_TREE_DEPTH {
                return Err(TreeError::TooDeep);
            }
            let expected = node.kind.arity();
            if node.children.len() != expected {
                return Err(TreeError::Arity {
                    descriptor: node.kind.descriptor_name(),
                    expected,
                    actual: node.children.len(),
                });
            }
            if let Some(text) = node.kind.text() {
                if text.len() > MAX_ATOM_BYTES {
                    return Err(TreeError::TextTooLong);
                }
                match &node.kind {
                    NodeKind::Number(_)
                        if text
                            .parse::<f64>()
                            .ok()
                            .is_none_or(|value| !value.is_finite()) =>
                    {
                        return Err(TreeError::InvalidNumber);
                    }
                    NodeKind::Identifier(_) | NodeKind::AnnotatedIdentifier { .. }
                        if text.trim().is_empty() || text.chars().any(char::is_control) =>
                    {
                        return Err(TreeError::InvalidIdentifier);
                    }
                    _ => {}
                }
            }
            if let NodeKind::AnnotatedIdentifier { labels, .. } = &node.kind {
                if labels.len() > MAX_ATOM_BYTES {
                    return Err(TreeError::TextTooLong);
                }
                if labels.trim().is_empty() || labels.chars().any(char::is_control) {
                    return Err(TreeError::InvalidLabels);
                }
            }
            stack.extend(node.children.iter().map(|child| (child, depth + 1)));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Node {
    kind: NodeKind,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
}

impl Node {
    pub fn kind(&self) -> &NodeKind {
        &self.kind
    }
    pub fn parent(&self) -> Option<NodeId> {
        self.parent
    }
    pub fn children(&self) -> &[NodeId] {
        &self.children
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Arena {
    slots: Vec<Option<Node>>,
}

impl Arena {
    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.slots.get(id.0)?.as_ref()
    }

    pub fn nodes(&self) -> impl Iterator<Item = (NodeId, &Node)> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(id, node)| node.as_ref().map(|node| (NodeId(id), node)))
    }

    fn allocate(&mut self, kind: NodeKind, children: Vec<NodeId>) -> NodeId {
        let id = NodeId(self.slots.len());
        self.slots.push(Some(Node {
            kind,
            parent: None,
            children: children.clone(),
        }));
        for child in children {
            self.node_mut(child).parent = Some(id);
        }
        id
    }

    fn node_mut(&mut self, id: NodeId) -> &mut Node {
        self.slots[id.0].as_mut().expect("internally valid node")
    }
}

/// Values correspond to the observed Ptc.Core.Direction constants.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Direction {
    Left = 0,
    Right = 1,
    Up = 2,
    Down = 3,
    Indeterminate = 4,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InsertionMode {
    Normal,
    Subscript,
}

/// Offsets count UTF-16 code units, matching .NET string length coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValueRange {
    pub anchor: usize,
    pub focus: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Caret {
    pub node: NodeId,
    pub caret_side: Direction,
    pub node_side: Direction,
    pub operator_part: Option<usize>,
    pub enter_side: Direction,
    pub value_range: Option<ValueRange>,
    pub insertion: InsertionMode,
}

impl Caret {
    fn at(node: NodeId, side: Direction) -> Self {
        Self {
            node,
            caret_side: side,
            node_side: side,
            operator_part: None,
            enter_side: side,
            value_range: None,
            insertion: InsertionMode::Normal,
        }
    }
}

/// Whole-subtree selection only. Matrix-cell and partial operand sets are deferred.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Selection {
    None,
    Subtree(NodeId),
}

#[derive(Clone, Debug)]
struct Snapshot {
    arena: Arena,
    root: NodeId,
    caret: Caret,
    selection: Selection,
}

#[derive(Clone, Debug)]
pub struct Editor {
    arena: Arena,
    root: NodeId,
    caret: Caret,
    selection: Selection,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditError {
    MissingNode,
    NotAnAtom,
    InvalidTextBoundary,
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

impl Editor {
    pub fn from_portable(tree: &PortableTree) -> Result<Self, TreeError> {
        tree.validate()?;
        let mut editor = Self::new();
        editor.arena.slots.clear();
        editor.root = import_node(&mut editor.arena, tree);
        editor.caret = Caret::at(first_leaf(&editor.arena, editor.root), Direction::Right);
        Ok(editor)
    }

    pub fn to_portable(&self) -> Result<PortableTree, TreeError> {
        self.validate().map_err(TreeError::InvalidTree)?;
        let mut count = 0;
        let tree = export_node(&self.arena, self.root, 1, &mut count)?;
        tree.validate()?;
        Ok(tree)
    }

    /// Validate before mutating. A successful import is one undoable operation.
    pub fn load_portable(&mut self, tree: &PortableTree) -> Result<(), TreeError> {
        tree.validate()?;
        let mut arena = self.arena.clone();
        arena.slots.iter_mut().for_each(|slot| *slot = None);
        let root = import_node(&mut arena, tree);
        self.checkpoint();
        self.arena = arena;
        self.root = root;
        self.caret = Caret::at(first_leaf(&self.arena, root), Direction::Right);
        self.selection = Selection::None;
        Ok(())
    }

    pub fn new() -> Self {
        let mut arena = Arena::default();
        let root = arena.allocate(NodeKind::Placeholder, vec![]);
        Self {
            arena,
            root,
            caret: Caret::at(root, Direction::Indeterminate),
            selection: Selection::None,
            undo: vec![],
            redo: vec![],
        }
    }

    pub fn arena(&self) -> &Arena {
        &self.arena
    }
    pub fn root(&self) -> NodeId {
        self.root
    }
    pub fn caret(&self) -> Caret {
        self.caret
    }
    pub fn selection(&self) -> Selection {
        self.selection
    }

    pub fn place_caret(&mut self, node: NodeId, side: Direction) -> Result<(), EditError> {
        self.arena.get(node).ok_or(EditError::MissingNode)?;
        self.caret = Caret::at(node, side);
        self.selection = Selection::None;
        Ok(())
    }

    pub fn place_text_caret(&mut self, node: NodeId, offset: usize) -> Result<(), EditError> {
        let text = self
            .arena
            .get(node)
            .ok_or(EditError::MissingNode)?
            .kind
            .text()
            .ok_or(EditError::NotAnAtom)?;
        let mut length = 0;
        let mut valid = offset == 0;
        for ch in text.chars() {
            length += ch.len_utf16();
            valid |= offset == length;
        }
        if !valid {
            return Err(EditError::InvalidTextBoundary);
        }
        self.place_caret(node, Direction::Indeterminate)?;
        self.caret.value_range = Some(ValueRange {
            anchor: offset,
            focus: offset,
        });
        Ok(())
    }

    pub fn select_subtree(&mut self, node: NodeId) -> Result<(), EditError> {
        self.arena.get(node).ok_or(EditError::MissingNode)?;
        self.selection = Selection::Subtree(node);
        Ok(())
    }

    /// Replace an atomic node while retaining its ID. No text grammar is implied.
    pub fn set_atom(&mut self, node: NodeId, kind: NodeKind) -> Result<(), EditError> {
        let old = self.arena.get(node).ok_or(EditError::MissingNode)?;
        if old.kind.arity() != 0 || kind.arity() != 0 {
            return Err(EditError::NotAnAtom);
        }
        self.checkpoint();
        self.arena.node_mut(node).kind = kind;
        self.caret = Caret::at(node, Direction::Right);
        self.selection = Selection::None;
        Ok(())
    }

    pub fn toggle_subscript(&mut self) {
        self.caret.insertion = match self.caret.insertion {
            InsertionMode::Normal => InsertionMode::Subscript,
            InsertionMode::Subscript => InsertionMode::Normal,
        };
    }

    /// Independent wrapping policy: selection (or current node) becomes numerator.
    pub fn insert_fraction(&mut self) -> NodeId {
        self.wrap_binary(NodeKind::Fraction)
    }

    /// Independent wrapping policy. A fraction or power base gets explicit grouping,
    /// matching the observed prohibited-child descriptor registration.
    pub fn insert_power(&mut self) -> NodeId {
        self.wrap_binary(NodeKind::Power)
    }

    pub fn insert_binary(&mut self, op: BinaryOp) -> NodeId {
        self.wrap_binary(NodeKind::Binary(op))
    }

    pub fn insert_group(&mut self) -> NodeId {
        self.checkpoint();
        let target = self.target();
        let parent = self.arena.get(target).unwrap().parent;
        let group = self.arena.allocate(NodeKind::Group, vec![target]);
        self.replace_child(parent, target, group);
        self.caret = Caret::at(target, Direction::Left);
        self.selection = Selection::None;
        group
    }

    /// Independent minimal policy: select the current subtree, then its parents.
    /// PLACEHOLDER has observed IsScopable=false, so it is skipped.
    pub fn expand_scope(&mut self) -> bool {
        let mut next = match self.selection {
            Selection::None => Some(self.caret.node),
            Selection::Subtree(node) => self.arena.get(node).and_then(Node::parent),
        };
        while let Some(id) = next {
            let node = self.arena.get(id).unwrap();
            if node.kind != NodeKind::Placeholder {
                self.selection = Selection::Subtree(id);
                return true;
            }
            next = node.parent;
        }
        false
    }

    /// Independent nearest-group policy for the observed ')' group-exit command.
    pub fn exit_group_right(&mut self) -> bool {
        let on_right_of_group = self.caret.caret_side == Direction::Right
            && self.arena.get(self.caret.node).unwrap().kind == NodeKind::Group;
        let mut current = if on_right_of_group {
            self.arena.get(self.caret.node).unwrap().parent
        } else {
            Some(self.caret.node)
        };
        while let Some(id) = current {
            let node = self.arena.get(id).unwrap();
            if node.kind == NodeKind::Group {
                self.caret = Caret::at(id, Direction::Right);
                self.selection = Selection::None;
                return true;
            }
            current = node.parent;
        }
        false
    }

    pub fn undo(&mut self) -> bool {
        let Some(snapshot) = self.undo.pop() else {
            return false;
        };
        self.redo.push(self.snapshot());
        self.restore(snapshot);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(snapshot) = self.redo.pop() else {
            return false;
        };
        self.undo.push(self.snapshot());
        self.restore(snapshot);
        true
    }

    /// Validate arity, reachability, unique parents, acyclicity, and UI references.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self
            .arena
            .get(self.root)
            .ok_or("root missing")?
            .parent
            .is_some()
        {
            return Err("root has parent");
        }
        let mut visited = HashSet::new();
        let mut stack = vec![self.root];
        while let Some(id) = stack.pop() {
            if !visited.insert(id) {
                return Err("cycle or shared child");
            }
            let node = self.arena.get(id).ok_or("child missing")?;
            if node.children.len() != node.kind.arity() {
                return Err("incorrect arity");
            }
            for child in &node.children {
                if self.arena.get(*child).ok_or("child missing")?.parent != Some(id) {
                    return Err("parent mismatch");
                }
                stack.push(*child);
            }
        }
        if visited.len() != self.arena.nodes().count() {
            return Err("unreachable live node");
        }
        if !visited.contains(&self.caret.node) {
            return Err("caret is detached");
        }
        if let Selection::Subtree(id) = self.selection
            && !visited.contains(&id)
        {
            return Err("selection is detached");
        }
        Ok(())
    }

    fn target(&self) -> NodeId {
        match self.selection {
            Selection::None => self.caret.node,
            Selection::Subtree(id) => id,
        }
    }

    fn wrap_binary(&mut self, kind: NodeKind) -> NodeId {
        self.checkpoint();
        let target = self.target();
        let old = self.arena.get(target).unwrap();
        let parent = old.parent;
        let empty = old.kind == NodeKind::Placeholder;
        let needs_group =
            kind == NodeKind::Power && matches!(old.kind, NodeKind::Power | NodeKind::Fraction);
        let first = if needs_group {
            self.arena.allocate(NodeKind::Group, vec![target])
        } else {
            target
        };
        let second = self.arena.allocate(NodeKind::Placeholder, vec![]);
        let wrapper = self.arena.allocate(kind, vec![first, second]);
        self.replace_child(parent, target, wrapper);
        self.caret = Caret::at(
            if empty { target } else { second },
            Direction::Indeterminate,
        );
        self.selection = Selection::None;
        debug_assert!(self.validate().is_ok());
        wrapper
    }

    fn replace_child(&mut self, parent: Option<NodeId>, old: NodeId, new: NodeId) {
        if let Some(parent) = parent {
            let children = &mut self.arena.node_mut(parent).children;
            let slot = children.iter().position(|id| *id == old).unwrap();
            children[slot] = new;
            self.arena.node_mut(new).parent = Some(parent);
        } else {
            self.root = new;
        }
    }

    fn checkpoint(&mut self) {
        self.undo.push(self.snapshot());
        self.redo.clear();
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            arena: self.arena.clone(),
            root: self.root,
            caret: self.caret,
            selection: self.selection,
        }
    }

    fn restore(&mut self, mut snapshot: Snapshot) {
        // Tombstone abandoned future nodes so branch edits cannot reuse their IDs.
        snapshot
            .arena
            .slots
            .resize(self.arena.slots.len().max(snapshot.arena.slots.len()), None);
        self.arena = snapshot.arena;
        self.root = snapshot.root;
        self.caret = snapshot.caret;
        self.selection = snapshot.selection;
    }
}

fn first_leaf(arena: &Arena, mut node: NodeId) -> NodeId {
    while let Some(child) = arena.get(node).unwrap().children.first() {
        node = *child;
    }
    node
}

fn import_node(arena: &mut Arena, tree: &PortableTree) -> NodeId {
    let children = tree
        .children
        .iter()
        .map(|child| import_node(arena, child))
        .collect();
    arena.allocate(tree.kind.clone(), children)
}

fn export_node(
    arena: &Arena,
    node: NodeId,
    depth: usize,
    count: &mut usize,
) -> Result<PortableTree, TreeError> {
    *count += 1;
    if depth > MAX_TREE_DEPTH {
        return Err(TreeError::TooDeep);
    }
    if *count > MAX_TREE_NODES {
        return Err(TreeError::TooManyNodes);
    }
    let node = arena
        .get(node)
        .ok_or(TreeError::InvalidTree("missing node"))?;
    let children = node
        .children
        .iter()
        .map(|child| export_node(arena, *child, depth + 1, count))
        .collect::<Result<_, _>>()?;
    Ok(PortableTree {
        kind: node.kind.clone(),
        children,
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FractionMetrics {
    pub numerator_width: f64,
    pub numerator_height: f64,
    pub denominator_width: f64,
    pub denominator_height: f64,
    pub overhang: f64,
    pub vertical_pad: f64,
    pub font_baseline_adjustment: f64,
    pub bar_height: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FractionLayout {
    pub width: f64,
    pub height: f64,
    pub text_baseline: f64,
    pub numerator_origin: [f64; 2],
    pub denominator_origin: [f64; 2],
    pub bar_origin: [f64; 2],
}

/// Formulas observed in control methods 0x060006f6 and 0x060006f7.
/// Font, padding, overhang, and bar metrics must come from the caller.
pub fn layout_fraction(m: FractionMetrics) -> FractionLayout {
    let width = m.numerator_width.max(m.denominator_width) + 2.0 * m.overhang;
    FractionLayout {
        width,
        height: m.numerator_height + m.denominator_height + 2.0 * m.vertical_pad,
        text_baseline: m.numerator_height + m.vertical_pad - m.font_baseline_adjustment,
        numerator_origin: [(width - m.numerator_width) / 2.0, 0.0],
        denominator_origin: [
            (width - m.denominator_width) / 2.0,
            m.numerator_height + 2.0 * m.vertical_pad,
        ],
        bar_origin: [
            0.0,
            m.numerator_height + m.vertical_pad - m.bar_height / 2.0,
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn number(e: &mut Editor, text: &str) -> NodeId {
        let id = e.caret().node;
        e.set_atom(id, NodeKind::Number(text.into())).unwrap();
        id
    }

    #[test]
    fn incomplete_fraction_preserves_placeholders_and_stable_numerator() {
        let mut e = Editor::new();
        let empty = e.root();
        let fraction = e.insert_fraction();
        assert_eq!(e.caret().node, empty);
        assert_eq!(e.arena().get(fraction).unwrap().children()[0], empty);
        number(&mut e, "1");
        let denominator = e.arena().get(fraction).unwrap().children()[1];
        assert_eq!(
            e.arena().get(denominator).unwrap().kind(),
            &NodeKind::Placeholder
        );
        e.place_caret(denominator, Direction::Indeterminate)
            .unwrap();
        number(&mut e, "2");
        assert_eq!(e.validate(), Ok(()));
    }

    #[test]
    fn selected_fraction_becomes_grouped_power_base_as_one_undo_command() {
        let mut e = Editor::new();
        let one = number(&mut e, "1");
        let fraction = e.insert_fraction();
        number(&mut e, "2");
        e.select_subtree(fraction).unwrap();
        let selected = e.selection();
        let caret = e.caret();
        let power = e.insert_power();
        let group = e.arena().get(power).unwrap().children()[0];
        assert_eq!(e.arena().get(group).unwrap().kind(), &NodeKind::Group);
        assert_eq!(e.arena().get(group).unwrap().children(), &[fraction]);
        assert_eq!(e.arena().get(fraction).unwrap().children()[0], one);
        assert!(e.undo());
        assert_eq!(e.root(), fraction);
        assert_eq!(e.selection(), selected);
        assert_eq!(e.caret(), caret);
        assert!(e.arena().get(group).is_none());
        assert!(e.redo());
        assert_eq!(e.root(), power);
        assert_eq!(e.validate(), Ok(()));
    }

    #[test]
    fn scope_skips_placeholder_and_replaces_only_selected_branch() {
        let mut e = Editor::new();
        let left = number(&mut e, "1");
        let add = e.insert_binary(BinaryOp::Add);
        let empty_right = e.caret().node;
        assert!(e.expand_scope());
        assert_eq!(e.selection(), Selection::Subtree(add));
        assert!(!e.expand_scope());
        e.place_caret(empty_right, Direction::Indeterminate)
            .unwrap();
        number(&mut e, "2");
        let fraction = e.insert_fraction();
        assert_eq!(e.arena().get(add).unwrap().children(), &[left, fraction]);
        assert_eq!(e.root(), add);
        assert_eq!(e.validate(), Ok(()));
    }

    #[test]
    fn group_exit_uses_nearest_group_without_destroying_structure() {
        let mut e = Editor::new();
        let atom = number(&mut e, "4");
        let outer = e.insert_group();
        let inner = e.insert_group();
        assert!(e.exit_group_right());
        assert_eq!(e.caret().node, inner);
        assert_eq!(e.caret().caret_side, Direction::Right);
        assert_eq!(e.root(), outer);
        assert_eq!(e.arena().get(inner).unwrap().children(), &[atom]);
        assert!(e.exit_group_right());
        assert_eq!(e.caret().node, outer);
        assert!(!e.exit_group_right());
        assert_eq!(e.validate(), Ok(()));
    }

    #[test]
    fn abandoned_future_ids_are_not_reused_after_undo_branch() {
        let mut e = Editor::new();
        number(&mut e, "9");
        let abandoned = e.insert_fraction();
        assert!(e.undo());
        let power = e.insert_power();
        assert!(power.index() > abandoned.index());
        assert!(e.arena().get(abandoned).is_none());
        assert!(!e.redo());
        assert_eq!(e.validate(), Ok(()));
    }

    #[test]
    fn utf16_caret_rejects_surrogate_split_and_preserves_other_state_on_toggle() {
        let mut e = Editor::new();
        let atom = e.root();
        e.set_atom(atom, NodeKind::Identifier("a𝛼b".into()))
            .unwrap();
        assert_eq!(
            e.place_text_caret(atom, 2),
            Err(EditError::InvalidTextBoundary)
        );
        e.place_text_caret(atom, 3).unwrap();
        let before = e.caret();
        e.toggle_subscript();
        assert_eq!(e.caret().insertion, InsertionMode::Subscript);
        assert_eq!(e.caret().value_range, before.value_range);
        assert_eq!(e.caret().node, before.node);
        assert_eq!(e.validate(), Ok(()));
    }

    #[test]
    fn observed_fraction_measurement_preserves_overhang_centering_and_baseline() {
        let layout = layout_fraction(FractionMetrics {
            numerator_width: 30.0,
            numerator_height: 12.0,
            denominator_width: 10.0,
            denominator_height: 8.0,
            overhang: 2.0,
            vertical_pad: 3.0,
            font_baseline_adjustment: 1.0,
            bar_height: 2.0,
        });
        assert_eq!(layout.width, 34.0);
        assert_eq!(layout.height, 26.0);
        assert_eq!(layout.text_baseline, 14.0);
        assert_eq!(layout.numerator_origin, [2.0, 0.0]);
        assert_eq!(layout.denominator_origin, [12.0, 18.0]);
        assert_eq!(layout.bar_origin, [0.0, 14.0]);
    }

    fn portable(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
        PortableTree { kind, children }
    }

    #[test]
    fn portable_roundtrip_preserves_labels_and_excludes_runtime_state() {
        let tree = portable(
            NodeKind::Fraction,
            vec![
                portable(
                    NodeKind::AnnotatedIdentifier {
                        name: "length".into(),
                        labels: "VARIABLE".into(),
                    },
                    vec![],
                ),
                portable(NodeKind::Number("2.5e-3".into()), vec![]),
            ],
        );
        let mut editor = Editor::from_portable(&tree).unwrap();
        assert!(!editor.undo());
        editor.expand_scope();
        let exported = editor.to_portable().unwrap();
        assert_eq!(exported, tree);
        let json = serde_json::to_value(&exported).unwrap();
        assert_eq!(json.as_object().unwrap().len(), 2);
        assert!(json.get("arena").is_none());
        assert!(json.get("undo").is_none());
        assert!(json.get("caret").is_none());
        let decoded: PortableTree = serde_json::from_value(json).unwrap();
        assert_eq!(
            Editor::from_portable(&decoded)
                .unwrap()
                .to_portable()
                .unwrap(),
            tree
        );
    }

    #[test]
    fn rejected_portable_load_is_atomic_and_does_not_push_undo() {
        let mut editor = Editor::new();
        let old = number(&mut editor, "7");
        let before = editor.to_portable().unwrap();
        let malformed = portable(
            NodeKind::Fraction,
            vec![portable(NodeKind::Number("2".into()), vec![])],
        );
        assert!(matches!(
            editor.load_portable(&malformed),
            Err(TreeError::Arity { .. })
        ));
        assert_eq!(editor.to_portable().unwrap(), before);
        assert_eq!(editor.root(), old);
        assert!(editor.undo());
        assert_eq!(
            editor.arena().get(old).unwrap().kind(),
            &NodeKind::Placeholder
        );
        assert!(!editor.undo());
    }

    #[test]
    fn portable_validation_rejects_invalid_literals_depth_and_metadata() {
        for text in ["", "NaN", "inf", "1..2", "1e999", " 1"] {
            assert_eq!(
                portable(NodeKind::Number(text.into()), vec![]).validate(),
                Err(TreeError::InvalidNumber)
            );
        }
        assert_eq!(
            portable(NodeKind::Identifier("\n".into()), vec![]).validate(),
            Err(TreeError::InvalidIdentifier)
        );
        assert_eq!(
            portable(
                NodeKind::AnnotatedIdentifier {
                    name: "x".into(),
                    labels: "".into()
                },
                vec![]
            )
            .validate(),
            Err(TreeError::InvalidLabels)
        );
        let mut deep = portable(NodeKind::Placeholder, vec![]);
        for _ in 0..MAX_TREE_DEPTH {
            deep = portable(NodeKind::Group, vec![deep]);
        }
        assert_eq!(deep.validate(), Err(TreeError::TooDeep));
        assert_eq!(
            portable(NodeKind::Identifier("x".repeat(MAX_ATOM_BYTES + 1)), vec![]).validate(),
            Err(TreeError::TextTooLong)
        );
        assert!(
            serde_json::from_str::<PortableTree>(
                r#"{"kind":"Placeholder","children":[],"undo":[]}"#
            )
            .is_err()
        );
    }

    #[test]
    fn imported_tree_keeps_old_ids_tombstoned_and_is_undoable() {
        let mut editor = Editor::new();
        let old = number(&mut editor, "7");
        let imported = portable(NodeKind::Identifier("x".into()), vec![]);
        editor.load_portable(&imported).unwrap();
        let new = editor.root();
        assert!(new.index() > old.index());
        assert!(editor.arena().get(old).is_none());
        assert!(editor.undo());
        assert_eq!(editor.root(), old);
        assert!(editor.arena().get(new).is_none());
        assert!(editor.redo());
        assert_eq!(editor.to_portable().unwrap(), imported);
        assert_eq!(editor.validate(), Ok(()));
    }
}
