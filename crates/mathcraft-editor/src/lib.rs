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
    Scale,
    InlineDivide,
    Define,
    Evaluate,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum NodeKind {
    Placeholder,
    Identifier(String),
    AnnotatedIdentifier {
        name: String,
        labels: String,
    },
    NativeIdentifier {
        name: String,
        labels: String,
        contextual: Option<bool>,
        subscript: Option<String>,
    },
    Number(String),
    Imaginary(String),
    ImaginarySymbol {
        value: String,
        symbol: String,
    },
    UnaryNeg,
    Root,
    Absolute,
    Call {
        argument_count: usize,
    },
    Function {
        parameter_count: usize,
    },
    Matrix {
        rows: usize,
        cols: usize,
    },
    Index {
        indices: usize,
    },
    Range {
        has_step: bool,
    },
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
            | Self::NativeIdentifier { .. }
            | Self::Number(_)
            | Self::Imaginary(_)
            | Self::ImaginarySymbol { .. } => i32::MAX,
            Self::Binary(BinaryOp::Define) => 10,
            Self::Binary(BinaryOp::Evaluate) => 20,
            Self::Binary(BinaryOp::Add | BinaryOp::Subtract) => 100,
            Self::Binary(BinaryOp::Multiply | BinaryOp::InlineDivide) => 140,
            Self::Binary(BinaryOp::Scale) => 142,
            Self::Fraction => 143,
            Self::UnaryNeg => 150,
            Self::Power => 160,
            Self::Index { .. } => 170,
            Self::Root => 180,
            Self::Absolute => 190,
            Self::Range { has_step: false } => 50,
            Self::Range { has_step: true } => 51,
            Self::Call { .. } | Self::Function { .. } => 300,
            Self::Matrix { .. } => 200,
            Self::Group => 299,
        }
    }

    pub fn descriptor_name(&self) -> &'static str {
        match self {
            Self::Placeholder => "PLACEHOLDER",
            Self::Identifier(_)
            | Self::AnnotatedIdentifier { .. }
            | Self::NativeIdentifier { .. } => "IDENTIFIER",
            Self::Number(_) => "NUMERIC",
            Self::Imaginary(_) | Self::ImaginarySymbol { .. } => "NUMERIC",
            Self::UnaryNeg => "NEGATION",
            Self::Root => "NTHROOT",
            Self::Absolute => "ABSOLUTEVALUE",
            Self::Call { .. } => "APPLY",
            Self::Function { .. } => "FUNCTION_DEFINITION",
            Self::Matrix { .. } => "MATRIX",
            Self::Index { .. } => "MATRIX_INDEX",
            Self::Range { has_step: false } => "RANGE",
            Self::Range { has_step: true } => "STEPRANGE",
            Self::Binary(BinaryOp::Add) => "ADDITION",
            Self::Binary(BinaryOp::Subtract) => "SUBTRACTION",
            Self::Binary(BinaryOp::Multiply) => "MULTIPLICATION",
            Self::Binary(BinaryOp::Scale) => "SCALE",
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
            | Self::NativeIdentifier { .. }
            | Self::Number(_)
            | Self::Imaginary(_)
            | Self::ImaginarySymbol { .. } => 0,
            Self::Group | Self::UnaryNeg | Self::Absolute => 1,
            Self::Binary(_) | Self::Fraction | Self::Power | Self::Root => 2,
            Self::Call { argument_count } => argument_count.saturating_add(1),
            Self::Function { parameter_count } => parameter_count.saturating_add(1),
            Self::Matrix { rows, cols } => rows.checked_mul(*cols).unwrap_or(usize::MAX),
            Self::Index { indices } => indices.saturating_add(1),
            Self::Range { has_step } => {
                if *has_step {
                    3
                } else {
                    2
                }
            }
        }
    }

    fn text(&self) -> Option<&str> {
        match self {
            Self::Identifier(text) | Self::Number(text) | Self::Imaginary(text) => Some(text),
            Self::AnnotatedIdentifier { name, .. } | Self::NativeIdentifier { name, .. } => {
                Some(name)
            }
            Self::ImaginarySymbol { value, .. } => Some(value),
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
                    | NodeKind::Imaginary(_)
                    | NodeKind::ImaginarySymbol { .. }
                        if text
                            .parse::<f64>()
                            .ok()
                            .is_none_or(|value| !value.is_finite()) =>
                    {
                        return Err(TreeError::InvalidNumber);
                    }
                    NodeKind::Identifier(_)
                    | NodeKind::AnnotatedIdentifier { .. }
                    | NodeKind::NativeIdentifier { .. }
                        if text.trim().is_empty() || text.chars().any(char::is_control) =>
                    {
                        return Err(TreeError::InvalidIdentifier);
                    }
                    _ => {}
                }
            }
            if matches!(
                node.kind,
                NodeKind::Matrix { rows: 0, .. }
                    | NodeKind::Matrix { cols: 0, .. }
                    | NodeKind::Index { indices: 0 }
            ) {
                return Err(TreeError::InvalidTree(
                    "empty matrix dimensions or zero index count",
                ));
            }
            if let NodeKind::ImaginarySymbol { symbol, .. } = &node.kind
                && !matches!(symbol.as_str(), "i" | "j")
            {
                return Err(TreeError::InvalidNumber);
            }
            if let NodeKind::NativeIdentifier {
                subscript: Some(text),
                ..
            } = &node.kind
                && (text.len() > MAX_ATOM_BYTES || text.chars().any(char::is_control))
            {
                return Err(TreeError::InvalidIdentifier);
            }
            if let NodeKind::AnnotatedIdentifier { labels, .. }
            | NodeKind::NativeIdentifier { labels, .. } = &node.kind
            {
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
    scope: Option<ScopeState>,
}

#[derive(Clone, Debug)]
struct ScopeState {
    origin: Caret,
    direction: Direction,
    history: Vec<(Caret, Selection)>,
}

#[derive(Clone, Debug)]
pub struct Editor {
    arena: Arena,
    root: NodeId,
    caret: Caret,
    selection: Selection,
    scope: Option<ScopeState>,
    edit_group_depth: usize,
    edit_group_checkpointed: bool,
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

    pub fn export_subtree(&self, node: NodeId) -> Result<PortableTree, TreeError> {
        let mut count = 0;
        let tree = export_node(&self.arena, node, 1, &mut count)?;
        tree.validate()?;
        Ok(tree)
    }

    pub fn replace_selection(&mut self, tree: &PortableTree) -> Result<(), TreeError> {
        tree.validate()?;
        let target = self.target();
        let parent = self
            .arena
            .get(target)
            .ok_or(TreeError::InvalidTree("missing target"))?
            .parent;
        self.checkpoint();
        let imported = import_node(&mut self.arena, tree);
        self.replace_child(parent, target, imported);
        self.retire(target, None);
        self.caret = Caret::at(first_leaf(&self.arena, imported), Direction::Right);
        self.selection = Selection::None;
        Ok(())
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
            scope: None,
            edit_group_depth: 0,
            edit_group_checkpointed: false,
            undo: vec![],
            redo: vec![],
        }
    }

    pub fn arena(&self) -> &Arena {
        &self.arena
    }

    /// One user command may perform many primitive mutations (for example
    /// plain-text paste). The original paste path uses a CompoundCommand;
    /// this nested Rust grouping API is our independent implementation of that
    /// atomic undo contract. Navigation-only groups do not touch undo/redo.
    /// Undo/redo is unavailable inside a group. A panic unwinds the group state
    /// and leaves any partial mutation available as one undoable command.
    pub fn edit_group<R>(&mut self, edit: impl FnOnce(&mut Self) -> R) -> R {
        let outer = self.edit_group_depth == 0;
        if outer {
            self.edit_group_checkpointed = false;
        }
        self.edit_group_depth += 1;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| edit(self)));
        self.edit_group_depth -= 1;
        if outer {
            self.edit_group_checkpointed = false;
        }
        match result {
            Ok(value) => value,
            Err(payload) => std::panic::resume_unwind(payload),
        }
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
        self.scope = None;
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

    /// Select UTF-16 text inside one atom. Invalid boundaries leave state intact.
    pub fn place_text_selection(
        &mut self,
        node: NodeId,
        anchor: usize,
        focus: usize,
    ) -> Result<(), EditError> {
        let text = self
            .arena
            .get(node)
            .ok_or(EditError::MissingNode)?
            .kind
            .text()
            .ok_or(EditError::NotAnAtom)?;
        let boundary = |offset| {
            offset == 0
                || text
                    .chars()
                    .scan(0, |units, ch| {
                        *units += ch.len_utf16();
                        Some(*units)
                    })
                    .any(|units| units == offset)
        };
        if !boundary(anchor) || !boundary(focus) {
            return Err(EditError::InvalidTextBoundary);
        }
        self.place_text_caret(node, focus)?;
        self.caret.value_range = Some(ValueRange { anchor, focus });
        Ok(())
    }

    /// Keep the live expression and editing context without recursively copying
    /// its undo history. The worksheet owns its document-wide command history.
    pub fn clone_without_history(&self) -> Self {
        Self {
            arena: self.arena.clone(),
            root: self.root,
            caret: self.caret,
            selection: self.selection,
            scope: self.scope.clone(),
            edit_group_depth: 0,
            edit_group_checkpointed: false,
            undo: vec![],
            redo: vec![],
        }
    }

    pub fn select_subtree(&mut self, node: NodeId) -> Result<(), EditError> {
        self.arena.get(node).ok_or(EditError::MissingNode)?;
        self.selection = Selection::Subtree(node);
        self.scope = None;
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

    /// Independent whole-subtree wrapping policy: preserve the operand on the
    /// scoped side (numerator for Right, denominator for Left).
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

    /// Independent linear-entry policy using recovered binding powers. Explicit
    /// selections and container boundaries take priority over automatic climbing.
    pub fn insert_infix(&mut self, op: BinaryOp) -> NodeId {
        self.checkpoint();
        self.prepare_infix(&NodeKind::Binary(op));
        self.wrap_binary_inner(NodeKind::Binary(op))
    }

    pub fn insert_fraction_infix(&mut self) -> NodeId {
        self.checkpoint();
        self.prepare_infix(&NodeKind::Fraction);
        self.wrap_binary_inner(NodeKind::Fraction)
    }

    pub fn insert_power_infix(&mut self) -> NodeId {
        self.checkpoint();
        self.prepare_infix(&NodeKind::Power);
        self.wrap_binary_inner(NodeKind::Power)
    }

    fn prepare_infix(&mut self, incoming: &NodeKind) {
        if self.selection != Selection::None {
            return;
        }
        let mut target = self.caret.node;
        while let Some(parent) = self.arena.get(target).unwrap().parent {
            let node = self.arena.get(parent).unwrap();
            let infix = matches!(node.kind, NodeKind::Binary(op) if !matches!(op, BinaryOp::Define | BinaryOp::Evaluate))
                || matches!(
                    node.kind,
                    NodeKind::Fraction
                        | NodeKind::Power
                        | NodeKind::UnaryNeg
                        | NodeKind::Range { .. }
                );
            let stronger = node.kind.precedence() > incoming.precedence()
                || (node.kind.precedence() == incoming.precedence()
                    && !matches!(incoming, NodeKind::Power));
            if !infix || node.children.last() != Some(&target) || !stronger {
                break;
            }
            target = parent;
        }
        self.selection = Selection::Subtree(target);
    }

    pub fn insert_unary_neg(&mut self) -> NodeId {
        self.wrap_unary(NodeKind::UnaryNeg)
    }
    pub fn insert_absolute(&mut self) -> NodeId {
        self.wrap_unary(NodeKind::Absolute)
    }

    fn wrap_unary(&mut self, kind: NodeKind) -> NodeId {
        self.checkpoint();
        let target = self.target();
        let parent = self.arena.get(target).unwrap().parent;
        let node = self.arena.allocate(kind, vec![target]);
        self.replace_child(parent, target, node);
        self.caret = Caret::at(target, Direction::Right);
        self.selection = Selection::None;
        node
    }

    pub fn insert_root(&mut self) -> NodeId {
        self.checkpoint();
        let target = self.target();
        let parent = self.arena.get(target).unwrap().parent;
        let degree = self.arena.allocate(NodeKind::Number("2".into()), vec![]);
        let root = self.arena.allocate(NodeKind::Root, vec![degree, target]);
        self.replace_child(parent, target, root);
        self.caret = Caret::at(target, Direction::Right);
        self.selection = Selection::None;
        root
    }

    pub fn insert_call(&mut self) -> NodeId {
        self.wrap_binary(NodeKind::Call { argument_count: 1 })
    }

    pub fn insert_index(&mut self) -> NodeId {
        self.wrap_binary(NodeKind::Index { indices: 1 })
    }

    pub fn insert_range(&mut self) -> NodeId {
        self.checkpoint();
        self.prepare_infix(&NodeKind::Range { has_step: false });
        self.wrap_binary_inner(NodeKind::Range { has_step: false })
    }

    pub fn insert_step_range(&mut self) -> NodeId {
        self.checkpoint();
        let target = self.target();
        let parent = self.arena.get(target).unwrap().parent;
        let second = self.arena.allocate(NodeKind::Placeholder, vec![]);
        let end = self.arena.allocate(NodeKind::Placeholder, vec![]);
        let range = self.arena.allocate(
            NodeKind::Range { has_step: true },
            vec![target, second, end],
        );
        self.replace_child(parent, target, range);
        self.caret = Caret::at(second, Direction::Indeterminate);
        self.selection = Selection::None;
        range
    }

    /// Append a call/index argument, or move through an existing stepped range.
    pub fn append_argument(&mut self) -> bool {
        let mut here = self.caret.node;
        while let Some(parent) = self.arena.get(here).unwrap().parent {
            let kind = self.arena.get(parent).unwrap().kind.clone();
            let next = match kind {
                NodeKind::Call { argument_count } => NodeKind::Call {
                    argument_count: argument_count + 1,
                },
                NodeKind::Function { parameter_count } => NodeKind::Function {
                    parameter_count: parameter_count + 1,
                },
                NodeKind::Index { indices } => NodeKind::Index {
                    indices: indices + 1,
                },
                NodeKind::Matrix { cols, .. } => {
                    let children = &self.arena.get(parent).unwrap().children;
                    let index = children.iter().position(|id| *id == here).unwrap();
                    if index % cols + 1 < cols
                        && self.arena.get(children[index + 1]).unwrap().kind
                            == NodeKind::Placeholder
                    {
                        self.caret = Caret::at(children[index + 1], Direction::Indeterminate);
                        self.selection = Selection::None;
                        return true;
                    }
                    return self.append_matrix_column(parent);
                }
                _ => {
                    here = parent;
                    continue;
                }
            };
            self.checkpoint();
            let arg = self.arena.allocate(NodeKind::Placeholder, vec![]);
            self.arena.node_mut(arg).parent = Some(parent);
            self.arena.node_mut(parent).children.push(arg);
            self.arena.node_mut(parent).kind = next;
            self.caret = Caret::at(arg, Direction::Indeterminate);
            self.selection = Selection::None;
            return true;
        }
        false
    }

    pub fn insert_matrix(&mut self, rows: usize, cols: usize) -> Option<NodeId> {
        if rows == 0 || cols == 0 || rows.checked_mul(cols)? > 4096 {
            return None;
        }
        self.checkpoint();
        let target = self.target();
        let parent = self.arena.get(target)?.parent;
        let mut cells = vec![target];
        for _ in 1..rows * cols {
            cells.push(self.arena.allocate(NodeKind::Placeholder, vec![]));
        }
        let node = self.arena.allocate(NodeKind::Matrix { rows, cols }, cells);
        self.replace_child(parent, target, node);
        self.caret = Caret::at(target, Direction::Indeterminate);
        self.selection = Selection::None;
        Some(node)
    }

    fn append_matrix_column(&mut self, matrix: NodeId) -> bool {
        let NodeKind::Matrix { rows, cols } = self.arena.get(matrix).unwrap().kind else {
            return false;
        };
        if rows * (cols + 1) > 4096 {
            return false;
        }
        self.checkpoint();
        let old = self.arena.get(matrix).unwrap().children.clone();
        let mut children = vec![];
        let mut focus = None;
        for row in 0..rows {
            children.extend_from_slice(&old[row * cols..(row + 1) * cols]);
            let cell = self.arena.allocate(NodeKind::Placeholder, vec![]);
            self.arena.node_mut(cell).parent = Some(matrix);
            focus.get_or_insert(cell);
            children.push(cell);
        }
        self.arena.node_mut(matrix).children = children;
        self.arena.node_mut(matrix).kind = NodeKind::Matrix {
            rows,
            cols: cols + 1,
        };
        self.caret = Caret::at(focus.unwrap(), Direction::Indeterminate);
        self.selection = Selection::None;
        true
    }

    pub fn append_matrix_row(&mut self) -> bool {
        let mut here = self.caret.node;
        loop {
            if let NodeKind::Matrix { rows, cols } = self.arena.get(here).unwrap().kind {
                if (rows + 1) * cols > 4096 {
                    return false;
                }
                self.checkpoint();
                let mut focus = None;
                for _ in 0..cols {
                    let cell = self.arena.allocate(NodeKind::Placeholder, vec![]);
                    self.arena.node_mut(cell).parent = Some(here);
                    self.arena.node_mut(here).children.push(cell);
                    focus.get_or_insert(cell);
                }
                self.arena.node_mut(here).kind = NodeKind::Matrix {
                    rows: rows + 1,
                    cols,
                };
                self.caret = Caret::at(focus.unwrap(), Direction::Indeterminate);
                self.selection = Selection::None;
                return true;
            }
            let Some(parent) = self.arena.get(here).unwrap().parent else {
                return false;
            };
            here = parent;
        }
    }

    pub fn insert_definition(&mut self) -> NodeId {
        self.checkpoint();
        if let NodeKind::Call { argument_count } = self.arena.get(self.root).unwrap().kind {
            self.arena.node_mut(self.root).kind = NodeKind::Function {
                parameter_count: argument_count,
            };
        }
        self.selection = Selection::Subtree(self.root);
        self.wrap_binary_inner(NodeKind::Binary(BinaryOp::Define))
    }

    pub fn insert_evaluation(&mut self) -> NodeId {
        // EVALUATION is admitted at equation root or Define's RHS slot only.
        // Reuse an existing evaluation instead of nesting another result slot.
        let target =
            if self.arena.get(self.root).unwrap().kind == NodeKind::Binary(BinaryOp::Define) {
                self.arena.get(self.root).unwrap().children[1]
            } else {
                self.root
            };
        if self.arena.get(target).unwrap().kind == NodeKind::Binary(BinaryOp::Evaluate) {
            return target;
        }
        self.checkpoint();
        self.selection = Selection::Subtree(target);
        self.wrap_binary_inner(NodeKind::Binary(BinaryOp::Evaluate))
    }

    /// Independent structural deletion policy, preserving stable IDs of survivors.
    pub fn delete_structure(&mut self, backwards: bool) -> bool {
        if let Selection::Subtree(id) = self.selection {
            self.checkpoint();
            let children = self.arena.get(id).unwrap().children.clone();
            for child in children {
                self.retire(child, None);
            }
            let node = self.arena.node_mut(id);
            node.kind = NodeKind::Placeholder;
            node.children.clear();
            self.caret = Caret::at(id, Direction::Indeterminate);
            self.selection = Selection::None;
            return true;
        }
        let id = self.caret.node;
        let node = self.arena.get(id).unwrap();
        let (wrapper, survivor) = if !node.children.is_empty()
            && backwards
            && self.caret.caret_side == Direction::Right
        {
            let safe_unwrap = matches!(
                node.kind,
                NodeKind::Group | NodeKind::UnaryNeg | NodeKind::Absolute
            ) || matches!(node.kind, NodeKind::Call { argument_count: 0 });
            let empty_rhs = node
                .children
                .last()
                .is_some_and(|child| self.arena.get(*child).unwrap().kind == NodeKind::Placeholder);
            if safe_unwrap || empty_rhs {
                (id, node.children[0])
            } else {
                let mut leaf = *node.children.last().unwrap();
                while let Some(last) = self.arena.get(leaf).unwrap().children.last() {
                    leaf = *last;
                }
                self.caret = Caret::at(leaf, Direction::Right);
                return false;
            }
        } else if let Some(parent) = node.parent {
            let parent_node = self.arena.get(parent).unwrap();
            let slot = parent_node
                .children
                .iter()
                .position(|child| *child == id)
                .unwrap();
            if matches!(parent_node.kind, NodeKind::Matrix { .. }) {
                return false;
            }
            if node.kind != NodeKind::Placeholder {
                return false;
            }
            if slot > 0 && parent_node.children.len() > 2 {
                let next_kind = match parent_node.kind {
                    NodeKind::Call { argument_count } => Some(NodeKind::Call {
                        argument_count: argument_count - 1,
                    }),
                    NodeKind::Function { parameter_count } => Some(NodeKind::Function {
                        parameter_count: parameter_count - 1,
                    }),
                    NodeKind::Index { indices } => Some(NodeKind::Index {
                        indices: indices - 1,
                    }),
                    _ => None,
                };
                if let Some(kind) = next_kind {
                    let previous = parent_node.children[slot - 1];
                    self.checkpoint();
                    self.retire(id, None);
                    self.arena.node_mut(parent).children.remove(slot);
                    self.arena.node_mut(parent).kind = kind;
                    self.caret = Caret::at(previous, Direction::Right);
                    self.selection = Selection::None;
                    return true;
                }
            }
            if parent_node.children.len() == 1 {
                (parent, id)
            } else if backwards && slot > 0 {
                (parent, parent_node.children[0])
            } else if !backwards && slot == 0 {
                (parent, *parent_node.children.last().unwrap())
            } else {
                return false;
            }
        } else {
            return false;
        };
        self.checkpoint();
        let parent = self.arena.get(wrapper).unwrap().parent;
        self.replace_child(parent, wrapper, survivor);
        self.arena.node_mut(survivor).parent = parent;
        self.retire(wrapper, Some(survivor));
        self.caret = Caret::at(survivor, Direction::Right);
        self.selection = Selection::None;
        true
    }

    fn retire(&mut self, id: NodeId, preserve: Option<NodeId>) {
        if preserve == Some(id) {
            return;
        }
        let children = self.arena.get(id).unwrap().children.clone();
        for child in children {
            self.retire(child, preserve);
        }
        self.arena.slots[id.0] = None;
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

    /// Recover the original scoping direction from a text caret's UTF-16
    /// midpoint (strictly before the midpoint is Left; midpoint itself Right).
    /// E: ak.a 0x060003e9 IL_003e..00c0. Partial operand ranges are deferred.
    pub fn scope_direction(&self) -> Direction {
        if let Some(scope) = &self.scope {
            return scope.direction;
        }
        if let Some(range) = self.caret.value_range
            && let Some(text) = self.arena.get(self.caret.node).and_then(|n| n.kind.text())
        {
            let length = text.encode_utf16().count();
            return if range.focus.saturating_mul(2) < length {
                Direction::Left
            } else {
                Direction::Right
            };
        }
        match self.caret.caret_side {
            Direction::Left => Direction::Left,
            Direction::Right => Direction::Right,
            _ => match self.caret.node_side {
                Direction::Left => Direction::Left,
                _ => Direction::Right,
            },
        }
    }

    /// Original scope history/terminal cycle and text-midpoint direction.
    /// The represented whole-subtree subset grows through parents; native
    /// adjacent/partial operand sets are not silently modeled as extra subtrees.
    pub fn expand_scope(&mut self) -> bool {
        if self.selection == Selection::Subtree(self.root) {
            let origin = self
                .scope
                .take()
                .map(|scope| scope.origin)
                .unwrap_or(self.caret);
            self.caret = origin;
            self.selection = Selection::None;
            return true;
        }
        if self.selection == Selection::None || self.scope.is_none() {
            let direction = self.scope_direction();
            self.scope = Some(ScopeState {
                origin: self.caret,
                direction,
                history: Vec::new(),
            });
        }
        let mut next = match self.selection {
            Selection::None => Some(self.caret.node),
            Selection::Subtree(node) => self.arena.get(node).and_then(Node::parent),
        };
        while let Some(id) = next {
            let node = self.arena.get(id).unwrap();
            if node.kind != NodeKind::Placeholder {
                let scope = self.scope.as_mut().unwrap();
                scope.history.push((self.caret, self.selection));
                self.caret.caret_side = scope.direction;
                self.selection = Selection::Subtree(id);
                return true;
            }
            next = node.parent;
        }
        false
    }

    /// Undo the last scope expansion without creating a document undo command.
    /// E: ak.k 0x060003db pops its prior segment, then returns to origin.
    pub fn contract_scope(&mut self) -> bool {
        if self.selection == Selection::None {
            self.scope = None;
            return false;
        }
        let Some(scope) = self.scope.as_mut() else {
            return false;
        };
        let Some((caret, selection)) = scope.history.pop() else {
            return false;
        };
        self.caret = caret;
        self.selection = selection;
        if self.selection == Selection::None {
            self.scope = None;
        }
        true
    }

    /// Independent nearest-group policy for the observed ')' group-exit command.
    pub fn exit_group_right(&mut self) -> bool {
        let on_right_of_group = self.caret.caret_side == Direction::Right
            && matches!(
                self.arena.get(self.caret.node).unwrap().kind,
                NodeKind::Group
                    | NodeKind::Call { .. }
                    | NodeKind::Function { .. }
                    | NodeKind::Absolute
                    | NodeKind::Root
                    | NodeKind::Index { .. }
                    | NodeKind::Matrix { .. }
            );
        let mut current = if on_right_of_group {
            self.arena.get(self.caret.node).unwrap().parent
        } else {
            Some(self.caret.node)
        };
        while let Some(id) = current {
            let node = self.arena.get(id).unwrap();
            if matches!(
                node.kind,
                NodeKind::Group
                    | NodeKind::Call { .. }
                    | NodeKind::Function { .. }
                    | NodeKind::Absolute
                    | NodeKind::Root
                    | NodeKind::Index { .. }
                    | NodeKind::Matrix { .. }
            ) {
                if matches!(node.kind, NodeKind::Call { argument_count: 1 })
                    && self.arena.get(node.children[1]).unwrap().kind == NodeKind::Placeholder
                {
                    let empty = node.children[1];
                    self.checkpoint();
                    self.retire(empty, None);
                    self.arena.node_mut(id).children.pop();
                    self.arena.node_mut(id).kind = NodeKind::Call { argument_count: 0 };
                }
                self.caret = Caret::at(id, Direction::Right);
                self.selection = Selection::None;
                return true;
            }
            current = node.parent;
        }
        false
    }

    pub fn undo(&mut self) -> bool {
        if self.edit_group_depth != 0 {
            return false;
        }
        let Some(snapshot) = self.undo.pop() else {
            return false;
        };
        self.redo.push(self.snapshot());
        self.restore(snapshot);
        true
    }

    pub fn redo(&mut self) -> bool {
        if self.edit_group_depth != 0 {
            return false;
        }
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
        self.wrap_binary_inner(kind)
    }

    fn wrap_binary_inner(&mut self, kind: NodeKind) -> NodeId {
        let target = self.target();
        let old = self.arena.get(target).unwrap();
        let parent = old.parent;
        let empty = old.kind == NodeKind::Placeholder;
        // Insertion operation bl.bq IL_0394..03de fills the first operand
        // when scope direction is Right and the last operand when Left.
        let insert_left = self.selection != Selection::None
            && self.caret.caret_side == Direction::Left
            && !matches!(
                kind,
                NodeKind::Binary(BinaryOp::Define | BinaryOp::Evaluate)
            );
        let needs_group = !insert_left
            && kind == NodeKind::Power
            && matches!(old.kind, NodeKind::Power | NodeKind::Fraction);
        let first = if needs_group {
            self.arena.allocate(NodeKind::Group, vec![target])
        } else {
            target
        };
        let second = self.arena.allocate(NodeKind::Placeholder, vec![]);
        let children = if insert_left {
            vec![second, first]
        } else {
            vec![first, second]
        };
        let wrapper = self.arena.allocate(kind, children);
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
        if self.edit_group_depth == 0 || !self.edit_group_checkpointed {
            self.undo.push(self.snapshot());
            self.redo.clear();
            if self.edit_group_depth != 0 {
                self.edit_group_checkpointed = true;
            }
        }
        self.scope = None;
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            arena: self.arena.clone(),
            root: self.root,
            caret: self.caret,
            selection: self.selection,
            scope: self.scope.clone(),
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
        self.scope = snapshot.scope;
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
        assert!(e.expand_scope());
        assert_eq!(e.selection(), Selection::None);
        assert_eq!(e.caret().node, empty_right);
        e.place_caret(empty_right, Direction::Indeterminate)
            .unwrap();
        number(&mut e, "2");
        let fraction = e.insert_fraction();
        assert_eq!(e.arena().get(add).unwrap().children(), &[left, fraction]);
        assert_eq!(e.root(), add);
        assert_eq!(e.validate(), Ok(()));
    }

    #[test]
    fn evaluation_wraps_definition_rhs_once_and_undo_restores_selection_and_caret() {
        let mut e = Editor::new();
        let name = e.caret().node;
        e.set_atom(name, NodeKind::Identifier("a".into())).unwrap();
        let definition = e.insert_definition();
        let rhs = number(&mut e, "5");
        e.place_text_caret(rhs, 0).unwrap();
        e.select_subtree(rhs).unwrap();
        let before = e.to_portable();
        let caret = e.caret();
        let selection = e.selection();
        let evaluation = e.insert_evaluation();
        assert_eq!(e.root(), definition);
        assert_eq!(
            e.arena().get(definition).unwrap().children(),
            &[name, evaluation]
        );
        assert_eq!(e.arena().get(evaluation).unwrap().children()[0], rhs);
        let count = e.undo.len();
        let after = e.to_portable();
        let after_caret = e.caret();
        assert_eq!(e.insert_evaluation(), evaluation);
        assert_eq!(e.undo.len(), count);
        assert_eq!(e.to_portable(), after);
        assert_eq!(e.caret(), after_caret);
        assert!(e.undo());
        assert_eq!(e.to_portable(), before);
        assert_eq!(e.caret(), caret);
        assert_eq!(e.selection(), selection);
        assert!(e.redo());
        assert_eq!(e.to_portable(), after);
        assert_eq!(e.validate(), Ok(()));
    }

    fn sum_product() -> (Editor, NodeId, NodeId, NodeId, NodeId, NodeId) {
        let atom = |s: &str| PortableTree {
            kind: NodeKind::Identifier(s.into()),
            children: vec![],
        };
        let tree = PortableTree {
            kind: NodeKind::Binary(BinaryOp::Add),
            children: vec![
                atom("a"),
                PortableTree {
                    kind: NodeKind::Binary(BinaryOp::Multiply),
                    children: vec![atom("b"), atom("c")],
                },
            ],
        };
        let e = Editor::from_portable(&tree).unwrap();
        let add = e.root();
        let children = e.arena().get(add).unwrap().children();
        let a = children[0];
        let mult = children[1];
        let factors = e.arena().get(mult).unwrap().children();
        let b = factors[0];
        let c = factors[1];
        (e, a, b, c, mult, add)
    }

    #[test]
    fn scope_in_sum_product_contracts_and_cycles_back_to_exact_original_caret() {
        let (mut e, a, b, c, mult, add) = sum_product();
        for (leaf, path) in [
            (a, vec![a, add]),
            (b, vec![b, mult, add]),
            (c, vec![c, mult, add]),
        ] {
            e.place_text_caret(leaf, 1).unwrap();
            let origin = e.caret();
            let undo_count = e.undo.len();
            for selected in &path {
                assert!(e.expand_scope());
                assert_eq!(e.selection(), Selection::Subtree(*selected));
                assert_eq!(e.scope_direction(), Direction::Right);
            }
            assert!(e.contract_scope());
            assert_eq!(e.selection(), Selection::Subtree(path[path.len() - 2]));
            assert!(e.expand_scope());
            assert_eq!(e.selection(), Selection::Subtree(add));
            assert!(e.expand_scope());
            assert_eq!(e.selection(), Selection::None);
            assert_eq!(e.caret(), origin);
            assert!(!e.contract_scope());
            assert_eq!(e.undo.len(), undo_count);
        }
    }

    #[test]
    fn scope_uses_utf16_midpoint_and_infix_inserts_on_the_selected_side() {
        for (offset, direction) in [
            (0, Direction::Left),
            (1, Direction::Left),
            (2, Direction::Right),
            (4, Direction::Right),
        ] {
            let mut e = Editor::new();
            let target = e.caret().node;
            e.set_atom(target, NodeKind::Identifier("abcd".into()))
                .unwrap();
            e.place_text_caret(target, offset).unwrap();
            let before = e.to_portable();
            let origin = e.caret();
            assert!(e.expand_scope());
            assert_eq!(e.scope_direction(), direction);
            let selected = e.selection();
            let scoped_caret = e.caret();
            let fraction = e.insert_fraction();
            let operands = e.arena().get(fraction).unwrap().children();
            assert_eq!(
                operands[if direction == Direction::Right { 0 } else { 1 }],
                target
            );
            assert_eq!(
                e.arena().get(e.caret().node).unwrap().kind(),
                &NodeKind::Placeholder
            );
            assert!(e.undo());
            assert_eq!(e.to_portable(), before);
            assert_eq!(e.selection(), selected);
            assert_eq!(e.caret(), scoped_caret);
            assert!(e.contract_scope());
            assert_eq!(e.selection(), Selection::None);
            assert_eq!(e.caret(), origin);
            assert!(e.redo());
            assert_eq!(e.root(), fraction);
            assert_eq!(e.validate(), Ok(()));
        }
        let mut e = Editor::new();
        let target = e.caret().node;
        e.set_atom(target, NodeKind::Identifier("a𝑥b".into()))
            .unwrap();
        e.place_text_caret(target, 1).unwrap();
        assert_eq!(e.scope_direction(), Direction::Left);
        e.place_text_caret(target, 3).unwrap();
        assert_eq!(e.scope_direction(), Direction::Right);
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
    fn paste_edit_group_is_one_undo_and_preserves_stable_ids_across_branching() {
        let (mut e, _, b, _, mult, _) = sum_product();
        e.place_text_caret(b, 1).unwrap();
        e.expand_scope();
        e.expand_scope();
        assert_eq!(e.selection(), Selection::Subtree(mult));
        let before = e.to_portable();
        let caret = e.caret();
        let selection = e.selection();
        let count = e.undo.len();
        let fraction = e.edit_group(|editor| {
            let fraction = editor.insert_fraction();
            let denominator = editor.caret().node;
            editor
                .set_atom(denominator, NodeKind::Identifier("v".into()))
                .unwrap();
            editor.edit_group(|editor| {
                editor
                    .set_atom(denominator, NodeKind::Identifier("value".into()))
                    .unwrap();
                editor.insert_power();
                number(editor, "2");
                assert!(!editor.undo());
                assert!(!editor.redo());
            });
            fraction
        });
        assert_eq!(e.undo.len(), count + 1);
        let after = e.to_portable();
        let after_caret = e.caret();
        assert!(e.undo());
        assert_eq!(e.to_portable(), before);
        assert_eq!(e.caret(), caret);
        assert_eq!(e.selection(), selection);
        assert_eq!(e.scope_direction(), Direction::Right);
        assert!(e.redo());
        assert_eq!(e.to_portable(), after);
        assert_eq!(e.caret(), after_caret);
        assert!(e.undo());
        let next = e.insert_power();
        assert!(next.index() > fraction.index());
        assert!(!e.redo());
        assert_eq!(e.validate(), Ok(()));
    }

    #[test]
    fn empty_edit_groups_preserve_redo_and_group_state_recovers_after_panic() {
        let mut e = Editor::new();
        number(&mut e, "1");
        e.insert_fraction();
        assert!(e.undo());
        let undo_count = e.undo.len();
        let redo_count = e.redo.len();
        let returned = e.edit_group(|editor| {
            editor.edit_group(|editor| {
                let root = editor.root();
                editor.place_text_caret(root, 0).unwrap();
                assert!(editor.expand_scope());
                assert!(editor.contract_scope());
                root
            })
        });
        assert_eq!(returned, e.root());
        assert_eq!(e.undo.len(), undo_count);
        assert_eq!(e.redo.len(), redo_count);
        assert!(e.redo());
        let before = e.to_portable();
        let count = e.undo.len();
        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            e.edit_group(|editor| {
                number(editor, "4");
                panic!("synthetic edit failure");
            });
        }));
        assert!(panicked.is_err());
        assert_eq!(e.edit_group_depth, 0);
        assert_eq!(e.undo.len(), count + 1);
        assert!(e.undo());
        assert_eq!(e.to_portable(), before);
        assert_eq!(e.validate(), Ok(()));
    }

    #[test]
    fn left_scope_places_existing_expression_on_the_right_of_power_and_infix() {
        for incoming in [
            NodeKind::Power,
            NodeKind::Binary(BinaryOp::Add),
            NodeKind::Binary(BinaryOp::Multiply),
        ] {
            let (mut e, a, b, c, mult, add) = sum_product();
            e.place_text_caret(b, 0).unwrap();
            e.expand_scope();
            e.expand_scope();
            assert_eq!(e.selection(), Selection::Subtree(mult));
            assert_eq!(e.scope_direction(), Direction::Left);
            let before = e.to_portable();
            let caret = e.caret();
            let selection = e.selection();
            let wrapper = match incoming {
                NodeKind::Power => e.insert_power_infix(),
                NodeKind::Binary(op) => e.insert_infix(op),
                _ => unreachable!(),
            };
            assert_eq!(e.root(), add);
            assert_eq!(e.arena().get(add).unwrap().children(), &[a, wrapper]);
            let operands = e.arena().get(wrapper).unwrap().children();
            assert_eq!(operands, &[e.caret().node, mult]);
            assert_eq!(e.arena().get(mult).unwrap().children(), &[b, c]);
            assert_eq!(e.arena().get(wrapper).unwrap().kind(), &incoming);
            let after = e.to_portable();
            assert!(e.undo());
            assert_eq!(e.to_portable(), before);
            assert_eq!(e.caret(), caret);
            assert_eq!(e.selection(), selection);
            assert_eq!(e.scope_direction(), Direction::Left);
            assert!(e.redo());
            assert_eq!(e.to_portable(), after);
            assert_eq!(e.validate(), Ok(()));
        }
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

    #[test]
    fn text_selection_rejects_surrogate_splits_without_mutating_state() {
        let mut editor = Editor::new();
        let id = editor.root();
        editor
            .set_atom(id, NodeKind::Identifier("a😀b".into()))
            .unwrap();
        editor.place_text_selection(id, 4, 1).unwrap();
        let before = editor.caret();
        assert_eq!(
            editor.place_text_selection(id, 1, 2),
            Err(EditError::InvalidTextBoundary)
        );
        assert_eq!(editor.caret(), before);
        assert_eq!(
            editor.place_text_selection(id, 2, 4),
            Err(EditError::InvalidTextBoundary)
        );
        assert_eq!(editor.caret(), before);
    }

    #[test]
    fn document_history_copy_retains_live_scope_and_drops_nested_undo_stacks() {
        let mut editor = Editor::new();
        let id = editor.root();
        editor
            .set_atom(id, NodeKind::Identifier("abc".into()))
            .unwrap();
        editor.place_text_caret(id, 1).unwrap();
        editor.expand_scope();
        let mut copy = editor.clone_without_history();
        assert_eq!(copy.to_portable().unwrap(), editor.to_portable().unwrap());
        assert_eq!(copy.caret(), editor.caret());
        assert_eq!(copy.selection(), editor.selection());
        assert!(!copy.undo());
        copy.contract_scope();
        assert_eq!(copy.caret().value_range.unwrap().focus, 1);
        assert_ne!(copy.selection(), editor.selection());
        assert!(editor.undo());
    }
}
