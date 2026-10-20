//! Structural editing primitives informed by the Prime 12 static observations.
//!
//! Descriptor numbers and fraction layout formulas are observed facts. Wrapping,
//! scope order, and initial caret placement are independent implementation
//! policies awaiting recorded comparison with Prime; this crate claims no parity.
//! See `docs/spec/mathcad12-editor-contract.json` and the editor analysis report.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

mod formatted_deletion;
mod global_vector_operators;
mod grouping_insertion;
mod label_cycle;
mod matrix_insertion;
mod matrix_selection;
mod radix_literal;
pub use radix_literal::NumericBase;
mod program_branches;
mod program_control;
mod program_insertion;
mod scalar_notation;
mod source_matrix_display;
mod source_matrix_navigation;
mod source_matrix_reshape;
pub use source_matrix_display::SourceMatrixDisplay;
mod scaling_labels;
mod string_literal;
mod structural_deletion;
mod structural_operators;
mod try_control;
mod typed_operator_insertion;
mod typeover;
pub use matrix_selection::MatrixCellSelection;

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
    GlobalDefine,
    CrossProduct,
    Polar,
    Evaluate,
    Equal,
    NotEqual,
    LessThan,
    LessOrEqual,
    GreaterThan,
    GreaterOrEqual,
    And,
    Or,
    Xor,
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
    /// Original NUMERIC source lexeme, including its canonical lowercase suffix.
    BasedNumber {
        literal: String,
        base: NumericBase,
    },
    /// STRING value contents only; displayed quote glyphs are not source text.
    StringLiteral(String),
    Imaginary(String),
    ImaginarySymbol {
        value: String,
        symbol: String,
    },
    UnaryNeg,
    LogicalNot,
    Root,
    Absolute,
    Factorial,
    Percent,
    Degree,
    Transpose,
    Conjugate,
    Vectorize,
    Determinant,
    Row,
    Column,
    Call {
        argument_count: usize,
    },
    Function {
        parameter_count: usize,
    },
    Matrix {
        rows: usize,
        cols: usize,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "source_matrix_display::non_null"
        )]
        display: Option<SourceMatrixDisplay>,
    },
    Index {
        indices: usize,
    },
    Range {
        has_step: bool,
    },
    Program {
        statement_count: usize,
    },
    LocalDefine,
    If {
        has_else: bool,
        #[serde(default = "default_if_branch_count")]
        branch_count: usize,
        #[serde(default)]
        alternate: bool,
    },
    For,
    While,
    Return,
    Break,
    Continue,
    /// Children are the try BODY, then the on-error BODY. Programs represent
    /// canonical BODY contents in this editor's supported subset.
    Try,
    Integral,
    Derivative {
        partial: bool,
    },
    Summation,
    Product,
    SymbolicEvaluation {
        command_count: usize,
    },
    SymbolicCommand {
        argument_count: usize,
    },
    Binary(BinaryOp),
    Fraction,
    Power,
    Group,
}

fn default_if_branch_count() -> usize {
    1
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
            | Self::BasedNumber { .. }
            | Self::StringLiteral(_)
            | Self::Imaginary(_)
            | Self::ImaginarySymbol { .. }
            | Self::Break
            | Self::Continue => i32::MAX,
            Self::Binary(BinaryOp::Define | BinaryOp::GlobalDefine) => 10,
            Self::Binary(BinaryOp::Evaluate) => 20,
            Self::Binary(
                BinaryOp::Equal
                | BinaryOp::NotEqual
                | BinaryOp::LessThan
                | BinaryOp::LessOrEqual
                | BinaryOp::GreaterThan
                | BinaryOp::GreaterOrEqual,
            ) => 80,
            Self::Binary(BinaryOp::And) => 72,
            Self::Binary(BinaryOp::Or) => 70,
            Self::Binary(BinaryOp::Xor) => 71,
            Self::Binary(BinaryOp::Add | BinaryOp::Subtract) => 100,
            Self::Binary(BinaryOp::Multiply | BinaryOp::InlineDivide) => 140,
            Self::Binary(BinaryOp::CrossProduct | BinaryOp::Polar) => 120,
            Self::Binary(BinaryOp::Scale) => 142,
            Self::Fraction => 143,
            Self::UnaryNeg | Self::LogicalNot => 150,
            Self::Power => 160,
            Self::Index { .. } => 170,
            Self::Root => 180,
            Self::Factorial | Self::Percent | Self::Degree => 180,
            Self::Transpose | Self::Row | Self::Column => 160,
            Self::Conjugate | Self::Vectorize => 130,
            Self::Absolute | Self::Determinant => 190,
            Self::Range { has_step: false } => 50,
            Self::Range { has_step: true } => 51,
            Self::Call { .. } | Self::Function { .. } => 300,
            Self::Matrix { .. } => 200,
            Self::Group => 299,
            Self::Program { .. } | Self::If { .. } | Self::For | Self::While | Self::Try => 290,
            Self::Return => 40,
            Self::LocalDefine => 41,
            Self::Integral
            | Self::Derivative { .. }
            | Self::Summation
            | Self::Product
            | Self::SymbolicEvaluation { .. }
            | Self::SymbolicCommand { .. } => 0,
        }
    }

    pub fn descriptor_name(&self) -> &'static str {
        match self {
            Self::Placeholder => "PLACEHOLDER",
            Self::Identifier(_)
            | Self::AnnotatedIdentifier { .. }
            | Self::NativeIdentifier { .. } => "IDENTIFIER",
            Self::Number(_) | Self::BasedNumber { .. } => "NUMERIC",
            Self::StringLiteral(_) => "STRING",
            Self::Imaginary(_) | Self::ImaginarySymbol { .. } => "NUMERIC",
            Self::UnaryNeg => "NEGATION",
            Self::LogicalNot => "LOGICAL_NOT",
            Self::Root => "NTHROOT",
            Self::Absolute => "ABSOLUTEVALUE",
            Self::Factorial => "FACTORIAL",
            Self::Percent => "PERCENT",
            Self::Degree => "DEGREE",
            Self::Transpose => "MATRIX_TRANSPOSE",
            Self::Conjugate => "CONJUGATE",
            Self::Vectorize => "VECTORIZATION",
            Self::Determinant => "DETERMINANT",
            Self::Row => "MATRIX_ROW",
            Self::Column => "MATRIX_COLUMN",
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
            Self::Binary(BinaryOp::GlobalDefine) => "GLOBAL_DEFINITION",
            Self::Binary(BinaryOp::CrossProduct) => "CROSSPRODUCT",
            Self::Binary(BinaryOp::Polar) => "POLAR",
            Self::Binary(BinaryOp::Evaluate) => "EVALUATION",
            Self::Binary(BinaryOp::Equal) => "EQUAL_TO",
            Self::Binary(BinaryOp::NotEqual) => "NOT_EQUAL_TO",
            Self::Binary(BinaryOp::LessThan) => "LESS_THAN",
            Self::Binary(BinaryOp::LessOrEqual) => "LESS_THAN_OR_EQUAL_TO",
            Self::Binary(BinaryOp::GreaterThan) => "GREATER_THAN",
            Self::Binary(BinaryOp::GreaterOrEqual) => "GREATER_THAN_OR_EQUAL_TO",
            Self::Binary(BinaryOp::And) => "LOGICAL_AND",
            Self::Binary(BinaryOp::Or) => "LOGICAL_OR",
            Self::Binary(BinaryOp::Xor) => "EXCLUSIVE_OR",
            Self::Fraction => "DIVISION",
            Self::Power => "EXPONENTIATION",
            Self::Group => "GROUPING",
            Self::Program { .. } => "PROGRAM",
            Self::LocalDefine => "ASSIGN",
            Self::If { .. } => "IF",
            Self::For => "FOR_LOOP",
            Self::While => "WHILE_LOOP",
            Self::Return => "RETURN",
            Self::Break => "BREAK",
            Self::Continue => "CONTINUE",
            Self::Try => "TRY",
            Self::Integral => "INTEGRAL",
            Self::Derivative { partial: false } => "DERIVATIVE",
            Self::Derivative { partial: true } => "PARTIAL_DERIVATIVE",
            Self::Summation => "SUMMATION",
            Self::Product => "PRODUCT",
            Self::SymbolicEvaluation { .. } => "SYMBOLIC_EVALUATION",
            Self::SymbolicCommand { .. } => "SYMBOLIC_COMMAND",
        }
    }

    fn arity(&self) -> usize {
        match self {
            Self::Placeholder
            | Self::Identifier(_)
            | Self::AnnotatedIdentifier { .. }
            | Self::NativeIdentifier { .. }
            | Self::Number(_)
            | Self::BasedNumber { .. }
            | Self::StringLiteral(_)
            | Self::Imaginary(_)
            | Self::ImaginarySymbol { .. }
            | Self::Break
            | Self::Continue => 0,
            Self::Group
            | Self::UnaryNeg
            | Self::LogicalNot
            | Self::Absolute
            | Self::Factorial
            | Self::Percent
            | Self::Degree
            | Self::Transpose
            | Self::Conjugate
            | Self::Vectorize
            | Self::Determinant => 1,
            Self::Binary(_)
            | Self::Fraction
            | Self::Power
            | Self::Root
            | Self::Row
            | Self::Column
            | Self::Try => 2,
            Self::Call { argument_count } => argument_count.saturating_add(1),
            Self::Function { parameter_count } => parameter_count.saturating_add(1),
            Self::Matrix { rows, cols, .. } => rows.checked_mul(*cols).unwrap_or(usize::MAX),
            Self::Index { indices } => indices.saturating_add(1),
            Self::Range { has_step } => {
                if *has_step {
                    3
                } else {
                    2
                }
            }
            Self::Program { statement_count } => *statement_count,
            Self::LocalDefine => 2,
            Self::If {
                has_else,
                branch_count,
                ..
            } => branch_count
                .saturating_mul(2)
                .saturating_add(usize::from(*has_else)),
            Self::For => 3,
            Self::While => 2,
            Self::Return => 1,
            Self::Integral | Self::Summation | Self::Product => 4,
            Self::Derivative { .. } => 3,
            Self::SymbolicEvaluation { command_count } => command_count.saturating_add(1),
            Self::SymbolicCommand { argument_count } => argument_count.saturating_add(1),
        }
    }

    fn text(&self) -> Option<&str> {
        match self {
            Self::Identifier(text)
            | Self::Number(text)
            | Self::StringLiteral(text)
            | Self::Imaginary(text) => Some(text),
            Self::AnnotatedIdentifier { name, .. } | Self::NativeIdentifier { name, .. } => {
                Some(name)
            }
            Self::ImaginarySymbol { value, .. } => Some(value),
            Self::BasedNumber { literal, .. } => Some(literal),
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
/// Interactive matrix budget; an independent resource limit, not Prime's limit.
pub const MAX_MATRIX_CELLS: usize = 4_096;
/// Independent conditional resource limit, not a recovered Prime limit.
pub const MAX_IF_BRANCHES: usize = 4_096;

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
        let mut stack = vec![(self, 1, false)];
        let mut count = 0;
        while let Some((node, depth, inside_program)) = stack.pop() {
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
                    NodeKind::BasedNumber { base, .. } if base.digits(text).is_none() => {
                        return Err(TreeError::InvalidNumber);
                    }
                    NodeKind::StringLiteral(_) if !string_literal::xml_text_valid(text) => {
                        return Err(TreeError::InvalidTree("string contains invalid XML text"));
                    }
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
            match &node.kind {
                NodeKind::Matrix {
                    rows,
                    cols,
                    display: Some(display),
                } => {
                    display.window([*rows, *cols]).map_err(|_| {
                        TreeError::InvalidTree("source matrix display outside supported bounds")
                    })?;
                }
                NodeKind::Binary(BinaryOp::Define | BinaryOp::GlobalDefine)
                | NodeKind::LocalDefine
                    if matches!(
                        node.children[0].kind,
                        NodeKind::Index { .. } | NodeKind::Row | NodeKind::Column
                    ) && !matches!(
                        node.children[0].children[0].kind,
                        NodeKind::Placeholder
                            | NodeKind::Identifier(_)
                            | NodeKind::AnnotatedIdentifier { .. }
                            | NodeKind::NativeIdentifier { .. }
                    ) =>
                {
                    return Err(TreeError::InvalidTree(
                        "an indexed or array slice definition target must be a named array",
                    ));
                }
                NodeKind::Index { indices } if !(1..=2).contains(indices) => {
                    return Err(TreeError::InvalidTree(
                        "index count outside supported one/two dimensional arrays",
                    ));
                }
                NodeKind::Program { statement_count }
                    if *statement_count == 0 || *statement_count > 4096 =>
                {
                    return Err(TreeError::InvalidTree("program line count outside bounds"));
                }
                NodeKind::SymbolicEvaluation { command_count } if *command_count > 16 => {
                    return Err(TreeError::InvalidTree(
                        "symbolic command count outside bounds",
                    ));
                }
                NodeKind::SymbolicCommand { argument_count } if *argument_count > 1 => {
                    return Err(TreeError::InvalidTree(
                        "symbolic command arity outside bounds",
                    ));
                }
                NodeKind::If {
                    branch_count,
                    has_else,
                    ..
                } => {
                    if !(1..=MAX_IF_BRANCHES).contains(branch_count) {
                        return Err(TreeError::InvalidTree("if branch count outside bounds"));
                    }
                    // Program bodies are the current supported BODY subset.
                    // The original pair-nary representation alternates tests and bodies.
                    let bodies = node.children[..branch_count * 2].iter().skip(1).step_by(2);
                    if !bodies
                        .chain(node.children[branch_count * 2..].iter())
                        .all(|child| matches!(child.kind, NodeKind::Program { .. }))
                        || node.children.len() != branch_count * 2 + usize::from(*has_else)
                    {
                        return Err(TreeError::InvalidTree("if bodies must be programs"));
                    }
                }
                NodeKind::For if !matches!(node.children[2].kind, NodeKind::Program { .. }) => {
                    return Err(TreeError::InvalidTree("for body must be a program"));
                }
                NodeKind::While if !matches!(node.children[1].kind, NodeKind::Program { .. }) => {
                    return Err(TreeError::InvalidTree("while body must be a program"));
                }
                NodeKind::Try
                    if !node
                        .children
                        .iter()
                        .all(|child| matches!(child.kind, NodeKind::Program { .. })) =>
                {
                    return Err(TreeError::InvalidTree(
                        "try and on-error bodies must be programs",
                    ));
                }
                NodeKind::SymbolicEvaluation { .. }
                    if !node.children[1..]
                        .iter()
                        .all(|child| matches!(child.kind, NodeKind::SymbolicCommand { .. })) =>
                {
                    return Err(TreeError::InvalidTree(
                        "symbolic evaluation children must be commands",
                    ));
                }
                NodeKind::SymbolicEvaluation { .. } if inside_program => {
                    return Err(TreeError::InvalidTree(
                        "symbolic evaluation cannot be used inside a program",
                    ));
                }
                NodeKind::LocalDefine
                    if !matches!(
                        node.children[0].kind,
                        NodeKind::Placeholder
                            | NodeKind::Identifier(_)
                            | NodeKind::AnnotatedIdentifier { .. }
                            | NodeKind::NativeIdentifier { .. }
                            | NodeKind::Index { .. }
                            | NodeKind::Row
                            | NodeKind::Column
                            | NodeKind::Function { .. }
                    ) =>
                {
                    return Err(TreeError::InvalidTree(
                        "program assignment target must be a name, named array selection or function declaration",
                    ));
                }
                NodeKind::LocalDefine
                    if matches!(node.children[0].kind, NodeKind::Function { .. })
                        && node.children[0].children.iter().any(|child| {
                            !matches!(
                                child.kind,
                                NodeKind::Placeholder
                                    | NodeKind::Identifier(_)
                                    | NodeKind::AnnotatedIdentifier { .. }
                                    | NodeKind::NativeIdentifier { .. }
                            )
                        }) =>
                {
                    return Err(TreeError::InvalidTree(
                        "local function name and parameters must be identifiers",
                    ));
                }
                NodeKind::For
                    if !matches!(
                        node.children[0].kind,
                        NodeKind::Placeholder
                            | NodeKind::Identifier(_)
                            | NodeKind::AnnotatedIdentifier { .. }
                            | NodeKind::NativeIdentifier { .. }
                    ) =>
                {
                    return Err(TreeError::InvalidTree("loop targets must be identifiers"));
                }
                NodeKind::Integral
                | NodeKind::Derivative { .. }
                | NodeKind::Summation
                | NodeKind::Product
                    if !matches!(
                        node.children[1].kind,
                        NodeKind::Placeholder
                            | NodeKind::Identifier(_)
                            | NodeKind::AnnotatedIdentifier { .. }
                            | NodeKind::NativeIdentifier { .. }
                    ) =>
                {
                    return Err(TreeError::InvalidTree(
                        "calculus binder must be an identifier",
                    ));
                }
                NodeKind::SymbolicCommand { .. }
                    if !matches!(
                        node.children[0].kind,
                        NodeKind::Placeholder
                            | NodeKind::Identifier(_)
                            | NodeKind::AnnotatedIdentifier { .. }
                            | NodeKind::NativeIdentifier { .. }
                    ) =>
                {
                    return Err(TreeError::InvalidTree(
                        "symbolic keyword must be an identifier",
                    ));
                }
                _ => {}
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
            let child_inside_program =
                inside_program || matches!(node.kind, NodeKind::Program { .. });
            stack.extend(
                node.children
                    .iter()
                    .map(|child| (child, depth + 1, child_inside_program)),
            );
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

#[derive(Clone, Debug)]
pub struct Arena {
    slots: Vec<Option<Node>>,
    revision: u64,
}
impl Default for Arena {
    fn default() -> Self {
        Self {
            slots: vec![],
            revision: next_arena_revision(),
        }
    }
}
impl PartialEq for Arena {
    fn eq(&self, other: &Self) -> bool {
        self.slots == other.slots
    }
}
impl Eq for Arena {}
fn next_arena_revision() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

impl Arena {
    /// O(1) identity for retained layout. Clones share a revision until either
    /// mutates; all mutation branches receive a globally distinct revision.
    /// Caret/selection changes do not mutate this arena.
    pub fn revision(&self) -> u64 {
        self.revision
    }
    fn touch(&mut self) {
        self.revision = next_arena_revision();
    }
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
        self.touch();
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
        self.touch();
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

/// A selection in the structural expression, including a rectangular matrix block.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Selection {
    None,
    Subtree(NodeId),
    MatrixCells(MatrixCellSelection),
}

#[derive(Clone, Debug)]
struct Snapshot {
    arena: Arena,
    root: NodeId,
    caret: Caret,
    selection: Selection,
    scope: Option<ScopeState>,
    typeover: Option<typeover::TypeoverState>,
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
    typeover: Option<typeover::TypeoverState>,
    expressions_only: bool,
    bin_oct_hex_mode: bool,
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
    InvalidOperatorCaret,
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
        editor.arena.touch();
        editor.root = import_node(&mut editor.arena, tree);
        editor.caret = Caret::at(first_leaf(&editor.arena, editor.root), Direction::Right);
        editor.clamp_source_matrix_caret();
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
        if self.expressions_only
            && matches!(
                tree.kind,
                NodeKind::Binary(BinaryOp::Define | BinaryOp::GlobalDefine)
            )
        {
            return Err(TreeError::InvalidTree(
                "definitions are not allowed in an expression-only editor",
            ));
        }
        if self.matrix_selection().is_some() {
            return Err(TreeError::InvalidTree(
                "matrix cell selection cannot be replaced by an expression",
            ));
        }
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
        self.clamp_source_matrix_caret();
        Ok(())
    }

    /// Validate before mutating. A successful import is one undoable operation.
    pub fn load_portable(&mut self, tree: &PortableTree) -> Result<(), TreeError> {
        tree.validate()?;
        let mut arena = self.arena.clone();
        arena.slots.iter_mut().for_each(|slot| *slot = None);
        arena.touch();
        let root = import_node(&mut arena, tree);
        self.checkpoint();
        self.arena = arena;
        self.root = root;
        self.caret = Caret::at(first_leaf(&self.arena, root), Direction::Right);
        self.selection = Selection::None;
        self.clamp_source_matrix_caret();
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
            typeover: None,
            expressions_only: false,
            bin_oct_hex_mode: false,
            edit_group_depth: 0,
            edit_group_checkpointed: false,
            undo: vec![],
            redo: vec![],
        }
    }

    pub fn arena(&self) -> &Arena {
        &self.arena
    }

    /// Editing context supplied by the host (for example, a SpecTable cell).
    /// This is not document content and does not create or restore undo entries.
    pub fn set_expressions_only(&mut self, expressions_only: bool) {
        self.expressions_only = expressions_only;
    }

    pub fn expressions_only(&self) -> bool {
        self.expressions_only
    }

    /// Host-supplied document input mode. Like expressions_only, this is not
    /// source/history and never reclassifies an existing Number or identifier.
    pub fn set_bin_oct_hex_mode(&mut self, enabled: bool) {
        self.bin_oct_hex_mode = enabled;
    }

    pub fn bin_oct_hex_mode(&self) -> bool {
        self.bin_oct_hex_mode
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
        self.typeover = None;
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
            typeover: self.typeover.clone(),
            expressions_only: self.expressions_only,
            bin_oct_hex_mode: self.bin_oct_hex_mode,
            edit_group_depth: 0,
            edit_group_checkpointed: false,
            undo: vec![],
            redo: vec![],
        }
    }

    pub fn select_subtree(&mut self, node: NodeId) -> Result<(), EditError> {
        self.arena.get(node).ok_or(EditError::MissingNode)?;
        self.typeover = None;
        self.selection = Selection::Subtree(node);
        self.scope = None;
        Ok(())
    }

    /// Select one complete structural operand and retain the side from which
    /// the selection was entered. The side determines where the next binary
    /// operator places the selected operand.
    pub fn select_subtree_from(&mut self, node: NodeId, side: Direction) -> Result<(), EditError> {
        self.arena.get(node).ok_or(EditError::MissingNode)?;
        self.caret = Caret::at(node, side);
        self.typeover = None;
        self.selection = Selection::Subtree(node);
        self.scope = None;
        Ok(())
    }

    /// Move to the first or last editable position in this math region.
    /// Navigation is not a document mutation and therefore leaves undo/redo
    /// intact. Prime exposes this separately from atom-local word movement.
    pub fn move_to_expression_extreme(&mut self, end: bool) -> bool {
        let node = if end {
            last_leaf(&self.arena, self.root)
        } else {
            first_leaf(&self.arena, self.root)
        };
        let side = if end {
            Direction::Right
        } else {
            Direction::Left
        };
        let suffix_end = end
            && self.arena.get(node).is_some_and(|node| {
                matches!(
                    &node.kind,
                    NodeKind::NativeIdentifier {
                        subscript: Some(_),
                        ..
                    }
                )
            });
        let next = if suffix_end {
            // The whole-name boundary includes its literal suffix. A normal
            // base-text offset would place region-end before that suffix.
            Caret::at(node, Direction::Right)
        } else if let Some(text) = self.arena.get(node).and_then(|node| node.kind.text()) {
            let offset = if end { text.encode_utf16().count() } else { 0 };
            let mut caret = Caret::at(node, Direction::Indeterminate);
            caret.value_range = Some(ValueRange {
                anchor: offset,
                focus: offset,
            });
            caret
        } else {
            Caret::at(node, side)
        };
        let changed = self.caret != next || self.selection != Selection::None;
        self.typeover = None;
        self.caret = next;
        self.selection = Selection::None;
        self.scope = None;
        changed
    }

    /// Replace an atomic node while retaining its ID. No text grammar is implied.
    pub fn set_atom(&mut self, node: NodeId, kind: NodeKind) -> Result<(), EditError> {
        if self.matrix_selection().is_some() {
            return Err(EditError::NotAnAtom);
        }
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
        if self.matrix_selection().is_some() {
            return;
        }
        self.caret.insertion = match self.caret.insertion {
            InsertionMode::Normal => InsertionMode::Subscript,
            InsertionMode::Subscript => InsertionMode::Normal,
        };
    }

    /// Enter or leave the literal suffix of a name. A literal suffix remains
    /// part of the identifier; it never creates an array Index expression.
    /// Returns whether the document changed (moving an existing caret does not).
    pub fn toggle_literal_subscript(&mut self) -> bool {
        if self.matrix_selection().is_some() {
            return false;
        }
        let node = self.caret.node;
        if self.caret.insertion == InsertionMode::Subscript {
            let suffix_end = self.arena.get(node).is_some_and(|node| {
                matches!(&node.kind, NodeKind::NativeIdentifier { subscript: Some(suffix), .. }
                    if self.caret.value_range.is_some_and(|range|
                        range.anchor == range.focus && range.focus == suffix.encode_utf16().count()))
            });
            // Native Ctrl+- retains the formatted-string offset (0078/1323).
            // Our suffix-local end maps to the whole-name right boundary;
            // intermediate formatted offsets remain outside generic splitting.
            if suffix_end {
                let _ = self.place_caret(node, Direction::Right);
                return false;
            }
            let length = self
                .arena
                .get(node)
                .unwrap()
                .kind
                .text()
                .map_or(0, |text| text.encode_utf16().count());
            let _ = self.place_text_caret(node, length);
            return false;
        }
        let kind = self.arena.get(node).unwrap().kind.clone();
        let (name, labels, contextual, subscript) = match kind {
            NodeKind::Identifier(name) => (name, "VARIABLE".into(), Some(true), None),
            NodeKind::AnnotatedIdentifier { name, labels } => (name, labels, None, None),
            NodeKind::NativeIdentifier {
                name,
                labels,
                contextual,
                subscript,
            } => (name, labels, contextual, subscript),
            _ => return false,
        };
        let changed = subscript.is_none();
        let suffix = subscript.unwrap_or_default();
        let offset = suffix.encode_utf16().count();
        if changed {
            let _ = self.set_atom(
                node,
                NodeKind::NativeIdentifier {
                    name,
                    labels,
                    contextual,
                    subscript: Some(suffix),
                },
            );
        }
        let _ = self.place_subscript_selection(node, offset, offset);
        changed
    }

    /// The active text field, including the literal suffix when editing it.
    pub fn editing_text(&self, node: NodeId) -> Option<&str> {
        let kind = &self.arena.get(node)?.kind;
        if self.caret.node == node
            && self.caret.insertion == InsertionMode::Subscript
            && let NodeKind::NativeIdentifier { subscript, .. } = kind
        {
            // The original deletion constructor retains insertion mode even
            // after the final suffix character/inline was removed. This is a
            // pending suffix caret, not an empty span in the saved source.
            return Some(subscript.as_deref().unwrap_or(""));
        }
        kind.text()
    }

    pub fn place_subscript_selection(
        &mut self,
        node: NodeId,
        anchor: usize,
        focus: usize,
    ) -> Result<(), EditError> {
        let NodeKind::NativeIdentifier { subscript, .. } =
            &self.arena.get(node).ok_or(EditError::MissingNode)?.kind
        else {
            return Err(EditError::NotAnAtom);
        };
        let text = subscript.as_deref().unwrap_or("");
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
        self.typeover = None;
        self.caret = Caret::at(node, Direction::Indeterminate);
        self.caret.insertion = InsertionMode::Subscript;
        self.caret.value_range = Some(ValueRange { anchor, focus });
        self.selection = Selection::None;
        self.scope = None;
        Ok(())
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
        if self.expressions_only && matches!(op, BinaryOp::Define | BinaryOp::GlobalDefine) {
            return self.root;
        }
        if op == BinaryOp::GlobalDefine {
            return self.insert_global_definition();
        }
        self.wrap_binary(NodeKind::Binary(op))
    }

    /// Independent linear-entry policy using recovered binding powers. Explicit
    /// selections and container boundaries take priority over automatic climbing.
    pub fn insert_infix(&mut self, op: BinaryOp) -> NodeId {
        if self.expressions_only && matches!(op, BinaryOp::Define | BinaryOp::GlobalDefine) {
            return self.root;
        }
        if op == BinaryOp::GlobalDefine {
            return self.insert_global_definition();
        }
        if let Some(range) = self.matrix_selection() {
            return range.matrix;
        }
        self.checkpoint();
        self.prepare_infix(&NodeKind::Binary(op));
        self.wrap_binary_inner(NodeKind::Binary(op))
    }

    /// Replace a pending two-key comparison without rebuilding its operands.
    pub fn replace_infix_operator(&mut self, node: NodeId, op: BinaryOp) -> bool {
        if self.expressions_only && matches!(op, BinaryOp::Define | BinaryOp::GlobalDefine) {
            return false;
        }
        if self.matrix_selection().is_some() {
            return false;
        }
        if !self
            .arena
            .get(node)
            .is_some_and(|node| matches!(node.kind, NodeKind::Binary(_)))
        {
            return false;
        }
        if self.arena.get(node).unwrap().kind == NodeKind::Binary(op) {
            return false;
        }
        self.checkpoint();
        self.arena.node_mut(node).kind = NodeKind::Binary(op);
        true
    }

    pub fn insert_fraction_infix(&mut self) -> NodeId {
        if let Some(range) = self.matrix_selection() {
            return range.matrix;
        }
        self.checkpoint();
        self.prepare_infix(&NodeKind::Fraction);
        self.wrap_binary_inner(NodeKind::Fraction)
    }

    pub fn insert_power_infix(&mut self) -> NodeId {
        if let Some(range) = self.matrix_selection() {
            return range.matrix;
        }
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
            let slot = node
                .children
                .iter()
                .position(|child| *child == target)
                .unwrap();
            if self.grouping_container_slot(parent, slot) {
                break;
            }
            let infix = matches!(node.kind, NodeKind::Binary(op) if !matches!(op, BinaryOp::Define | BinaryOp::GlobalDefine | BinaryOp::Evaluate))
                || matches!(
                    node.kind,
                    NodeKind::Fraction
                        | NodeKind::Power
                        | NodeKind::UnaryNeg
                        | NodeKind::LogicalNot
                        | NodeKind::Range { .. }
                );
            let stronger = node.kind.precedence() > incoming.precedence()
                || (node.kind.precedence() == incoming.precedence()
                    && !matches!(
                        incoming,
                        NodeKind::Power | NodeKind::Binary(BinaryOp::CrossProduct)
                    ));
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
    pub fn insert_logical_not(&mut self) -> NodeId {
        self.wrap_unary(NodeKind::LogicalNot)
    }
    pub fn insert_absolute(&mut self) -> NodeId {
        self.wrap_unary(NodeKind::Absolute)
    }

    fn wrap_unary(&mut self, kind: NodeKind) -> NodeId {
        if let Some(range) = self.matrix_selection() {
            return range.matrix;
        }
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
        if let Some(range) = self.matrix_selection() {
            return range.matrix;
        }
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
        if let Some(range) = self.matrix_selection() {
            return range.matrix;
        }
        self.checkpoint();
        self.prepare_infix(&NodeKind::Range { has_step: false });
        self.wrap_binary_inner(NodeKind::Range { has_step: false })
    }

    pub fn insert_step_range(&mut self) -> NodeId {
        if let Some(range) = self.matrix_selection() {
            return range.matrix;
        }
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

    /// Wrap the current operand in an ordered program block. Program line
    /// insertion/removal is deliberately separate from generic n-ary argument editing.
    pub fn insert_program(&mut self) -> NodeId {
        self.wrap_template(NodeKind::Program { statement_count: 1 }, 1, 0, 0)
    }

    fn program_line(&self) -> Option<(NodeId, NodeId, usize)> {
        let mut line = self.target();
        while let Some(parent) = self.arena.get(line)?.parent {
            let node = self.arena.get(parent)?;
            if matches!(node.kind, NodeKind::Program { .. }) {
                let index = node.children.iter().position(|id| *id == line)?;
                return Some((parent, line, index));
            }
            line = parent;
        }
        None
    }

    pub fn is_in_program(&self) -> bool {
        self.program_line().is_some()
            || matches!(
                self.arena.get(self.target()).map(|n| &n.kind),
                Some(NodeKind::Program { .. })
            )
    }

    /// Enter inserts above at the line's leftmost entry point, after otherwise.
    /// Focusing a loop's right boundary therefore inserts in its enclosing block.
    pub fn insert_program_line(&mut self) -> bool {
        if self.matrix_selection().is_some() {
            return false;
        }
        let (program, index) = if let Some((program, line, index)) = self.program_line() {
            (program, index + usize::from(!self.at_subtree_start(line)))
        } else if matches!(
            self.arena.get(self.target()).map(|n| &n.kind),
            Some(NodeKind::Program { .. })
        ) {
            let program = self.target();
            let count = self.arena.get(program).unwrap().children.len();
            (
                program,
                if self.caret.caret_side == Direction::Left {
                    0
                } else {
                    count
                },
            )
        } else {
            return false;
        };
        if self.arena.nodes().count() >= MAX_TREE_NODES {
            return false;
        }
        self.checkpoint();
        let line = self.arena.allocate(NodeKind::Placeholder, vec![]);
        self.arena.node_mut(line).parent = Some(program);
        let node = self.arena.node_mut(program);
        node.children.insert(index, line);
        node.kind = NodeKind::Program {
            statement_count: node.children.len(),
        };
        self.caret = Caret::at(line, Direction::Indeterminate);
        self.selection = Selection::None;
        self.scope = None;
        true
    }

    /// Delete only an empty direct program line. Keep its required final slot.
    /// This conservative policy does not unwrap populated branches or loops.
    pub fn delete_blank_program_line(&mut self, backwards: bool) -> bool {
        let Some((program, line, index)) = self.program_line() else {
            return false;
        };
        if self.arena.get(line).unwrap().kind != NodeKind::Placeholder {
            return false;
        }
        let children = self.arena.get(program).unwrap().children.clone();
        if children.len() == 1 {
            return false;
        }
        self.checkpoint();
        self.arena.node_mut(program).children.remove(index);
        self.arena.node_mut(program).kind = NodeKind::Program {
            statement_count: children.len() - 1,
        };
        self.retire(line, None);
        let next = if backwards && index > 0 {
            index - 1
        } else {
            index.min(children.len() - 2)
        };
        let sibling = self.arena.get(program).unwrap().children[next];
        self.caret = Caret::at(
            sibling,
            if backwards {
                Direction::Right
            } else {
                Direction::Left
            },
        );
        self.selection = Selection::None;
        self.scope = None;
        true
    }

    pub fn insert_local_definition(&mut self) -> NodeId {
        if let Some(range) = self.matrix_selection() {
            return range.matrix;
        }
        let target = self.target();
        let node = self.arena.get(target).unwrap();
        let Some(parent) = node.parent.filter(|parent| {
            matches!(
                self.arena.get(*parent).unwrap().kind,
                NodeKind::Program { .. }
            )
        }) else {
            return target;
        };
        let name = |id| {
            matches!(
                self.arena.get(id).unwrap().kind,
                NodeKind::Placeholder
                    | NodeKind::Identifier(_)
                    | NodeKind::AnnotatedIdentifier { .. }
                    | NodeKind::NativeIdentifier { .. }
            )
        };
        let allowed = match node.kind {
            NodeKind::Placeholder
            | NodeKind::Identifier(_)
            | NodeKind::AnnotatedIdentifier { .. }
            | NodeKind::NativeIdentifier { .. } => true,
            NodeKind::Index { .. } | NodeKind::Row | NodeKind::Column => name(node.children[0]),
            NodeKind::Call { .. } | NodeKind::Function { .. } => {
                node.children.iter().all(|child| name(*child))
            }
            _ => false,
        };
        if !allowed {
            return target;
        }
        let empty = node.kind == NodeKind::Placeholder;
        self.checkpoint();
        if let NodeKind::Call { argument_count } = self.arena.get(target).unwrap().kind {
            self.arena.node_mut(target).kind = NodeKind::Function {
                parameter_count: argument_count,
            };
        }
        let rhs = self.arena.allocate(NodeKind::Placeholder, vec![]);
        let assignment = self
            .arena
            .allocate(NodeKind::LocalDefine, vec![target, rhs]);
        self.replace_child(Some(parent), target, assignment);
        self.caret = Caret::at(if empty { target } else { rhs }, Direction::Indeterminate);
        self.selection = Selection::None;
        debug_assert!(self.validate().is_ok());
        assignment
    }

    pub fn insert_if_else(&mut self) -> NodeId {
        if let Some(range) = self.matrix_selection() {
            return range.matrix;
        }
        self.checkpoint();
        let target = self.target();
        let parent = self.arena.get(target).unwrap().parent;
        let then_value = self.arena.allocate(NodeKind::Placeholder, vec![]);
        let then_block = self
            .arena
            .allocate(NodeKind::Program { statement_count: 1 }, vec![then_value]);
        let else_value = self.arena.allocate(NodeKind::Placeholder, vec![]);
        let else_block = self
            .arena
            .allocate(NodeKind::Program { statement_count: 1 }, vec![else_value]);
        let wrapper = self.arena.allocate(
            NodeKind::If {
                has_else: true,
                branch_count: 1,
                alternate: false,
            },
            vec![target, then_block, else_block],
        );
        self.replace_child(parent, target, wrapper);
        self.caret = Caret::at(
            if self.arena.get(target).unwrap().kind == NodeKind::Placeholder {
                target
            } else {
                then_value
            },
            Direction::Indeterminate,
        );
        self.selection = Selection::None;
        debug_assert!(self.validate().is_ok());
        wrapper
    }

    pub fn insert_for_loop(&mut self) -> NodeId {
        let target = self.target();
        self.insert_program_control(NodeKind::For).unwrap_or(target)
    }

    pub fn insert_while_loop(&mut self) -> NodeId {
        let target = self.target();
        self.insert_program_control(NodeKind::While)
            .unwrap_or(target)
    }

    pub fn insert_return(&mut self) -> NodeId {
        let target = self.target();
        self.insert_program_control(NodeKind::Return)
            .unwrap_or(target)
    }

    pub fn insert_integral(&mut self) -> NodeId {
        self.wrap_template(NodeKind::Integral, 4, 0, 1)
    }

    pub fn insert_derivative(&mut self, partial: bool) -> NodeId {
        self.wrap_template(NodeKind::Derivative { partial }, 3, 0, 1)
    }

    pub fn insert_summation(&mut self) -> NodeId {
        self.wrap_template(NodeKind::Summation, 4, 0, 1)
    }

    pub fn insert_product(&mut self) -> NodeId {
        self.wrap_template(NodeKind::Product, 4, 0, 1)
    }

    pub fn insert_symbolic_evaluation(&mut self) -> NodeId {
        if let Some(range) = self.matrix_selection() {
            return range.matrix;
        }
        self.checkpoint();
        let target = self.target();
        let parent = self.arena.get(target).unwrap().parent;
        let keyword = self.arena.allocate(NodeKind::Placeholder, vec![]);
        let command = self.arena.allocate(
            NodeKind::SymbolicCommand { argument_count: 0 },
            vec![keyword],
        );
        let wrapper = self.arena.allocate(
            NodeKind::SymbolicEvaluation { command_count: 1 },
            vec![target, command],
        );
        self.replace_child(parent, target, wrapper);
        self.caret = Caret::at(keyword, Direction::Indeterminate);
        self.selection = Selection::None;
        debug_assert!(self.validate().is_ok());
        wrapper
    }

    /// Append a call/index argument, or move through an existing stepped range.
    pub fn append_argument(&mut self) -> bool {
        if self.matrix_selection().is_some() {
            return false;
        }
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
        if self.matrix_selection().is_some() {
            return None;
        }
        if rows == 0 || cols == 0 || rows.checked_mul(cols)? > MAX_MATRIX_CELLS {
            return None;
        }
        self.checkpoint();
        let target = self.target();
        let parent = self.arena.get(target)?.parent;
        let mut cells = vec![target];
        for _ in 1..rows * cols {
            cells.push(self.arena.allocate(NodeKind::Placeholder, vec![]));
        }
        let node = self.arena.allocate(
            NodeKind::Matrix {
                rows,
                cols,
                display: None,
            },
            cells,
        );
        self.replace_child(parent, target, node);
        self.caret = Caret::at(target, Direction::Indeterminate);
        self.selection = Selection::None;
        Some(node)
    }

    fn append_matrix_column(&mut self, matrix: NodeId) -> bool {
        let NodeKind::Matrix { rows, cols, .. } = self.arena.get(matrix).unwrap().kind else {
            return false;
        };
        self.insert_matrix_axis(matrix, rows, cols, cols, 0, false)
    }

    pub fn append_matrix_row(&mut self) -> bool {
        if self
            .matrix_selection()
            .is_some_and(|range| self.matrix_has_source_display(range.matrix))
        {
            return false;
        }
        if self.matrix_selection().is_some() {
            self.collapse_matrix_selection();
            return false;
        }
        let mut here = self.caret.node;
        loop {
            if let NodeKind::Matrix { rows, cols, .. } = self.arena.get(here).unwrap().kind {
                return self.insert_matrix_axis(here, rows, cols, rows, 0, true);
            }
            let Some(parent) = self.arena.get(here).unwrap().parent else {
                return false;
            };
            here = parent;
        }
    }

    /// Matrix Tab visits cells in row-major order, including populated cells.
    /// The last cell grows one empty row; Shift+Tab never grows the matrix.
    /// Some reports a handled key and whether its contents changed.
    pub fn matrix_tab(&mut self, forward: bool) -> Option<bool> {
        let (matrix, rows, cols, index, _) = self.matrix_cell()?;
        let children = self.arena.get(matrix)?.children.clone();
        if forward && index + 1 == children.len() {
            return Some(self.insert_matrix_axis(matrix, rows, cols, rows, 0, true));
        }
        let (node, side) = if forward {
            (
                first_leaf(&self.arena, children[index + 1]),
                Direction::Left,
            )
        } else if index == 0 {
            (matrix, Direction::Left)
        } else {
            (
                last_leaf(&self.arena, children[index - 1]),
                Direction::Right,
            )
        };
        self.typeover = None;
        self.caret = Caret::at(node, side);
        self.selection = Selection::None;
        self.scope = None;
        Some(false)
    }

    /// Insert above/below the current cell's row. Indeterminate implements the
    /// source command's cursor-dependent choice: before at the cell's left
    /// boundary, after otherwise. The new cell retains the current column.
    /// Other directions, a whole-matrix selection, and budget overflow are no-ops.
    pub fn insert_matrix_row(&mut self, direction: Direction) -> bool {
        self.insert_matrix_dimension(direction, true)
    }

    /// Insert left/right of the current cell's column. Indeterminate uses the
    /// same cell-boundary rule as insert_matrix_row; focus retains its row.
    pub fn insert_matrix_column(&mut self, direction: Direction) -> bool {
        self.insert_matrix_dimension(direction, false)
    }

    /// Remove the current cell's row, retaining the column and clamping the row
    /// to the next surviving row (previous at the bottom). Removing the final
    /// row leaves a 1x1 placeholder matrix, as in the original shape helper.
    pub fn delete_matrix_row(&mut self) -> bool {
        self.delete_matrix_dimension(true)
    }

    /// Remove the current cell's column and retain its row. Removing the final
    /// column leaves a 1x1 placeholder matrix. An already blank 1x1 is a no-op.
    pub fn delete_matrix_column(&mut self) -> bool {
        self.delete_matrix_dimension(false)
    }

    fn matrix_cell(&self) -> Option<(NodeId, usize, usize, usize, NodeId)> {
        let mut cell = self.target();
        if self.selection != Selection::None
            && matches!(self.arena.get(cell)?.kind, NodeKind::Matrix { .. })
        {
            return None;
        }
        while let Some(parent) = self.arena.get(cell)?.parent {
            let node = self.arena.get(parent)?;
            if let NodeKind::Matrix { rows, cols, .. } = node.kind {
                let index = node.children.iter().position(|id| *id == cell)?;
                return Some((parent, rows, cols, index, cell));
            }
            cell = parent;
        }
        None
    }

    fn at_subtree_start(&self, cell: NodeId) -> bool {
        if self.selection == Selection::Subtree(cell) {
            return self.caret.caret_side == Direction::Left;
        }
        if self.selection != Selection::None || self.caret.node != first_leaf(&self.arena, cell) {
            return false;
        }
        self.caret
            .value_range
            .map_or(self.caret.caret_side == Direction::Left, |range| {
                range.anchor == 0 && range.focus == 0
            })
    }

    fn insert_matrix_dimension(&mut self, direction: Direction, row_axis: bool) -> bool {
        if self
            .matrix_selection()
            .is_some_and(|range| self.matrix_has_source_display(range.matrix))
        {
            return false;
        }
        if self.matrix_selection().is_some() {
            self.collapse_matrix_selection();
            return false;
        }
        let Some((matrix, rows, cols, index, cell)) = self.matrix_cell() else {
            return false;
        };
        let before = match (row_axis, direction) {
            (true, Direction::Up) | (false, Direction::Left) => true,
            (true, Direction::Down) | (false, Direction::Right) => false,
            (_, Direction::Indeterminate) => self.at_subtree_start(cell),
            _ => return false,
        };
        let position = if row_axis { index / cols } else { index % cols };
        let retained = if row_axis { index % cols } else { index / cols };
        self.insert_matrix_axis(
            matrix,
            rows,
            cols,
            position + usize::from(!before),
            retained,
            row_axis,
        )
    }

    fn matrix_has_source_display(&self, matrix: NodeId) -> bool {
        self.arena.get(matrix).is_some_and(|node| {
            matches!(
                node.kind,
                NodeKind::Matrix {
                    display: Some(_),
                    ..
                }
            )
        })
    }

    fn insert_matrix_axis(
        &mut self,
        matrix: NodeId,
        rows: usize,
        cols: usize,
        position: usize,
        retained: usize,
        row_axis: bool,
    ) -> bool {
        let Some(new_rows) = rows.checked_add(usize::from(row_axis)) else {
            return false;
        };
        let Some(new_cols) = cols.checked_add(usize::from(!row_axis)) else {
            return false;
        };
        let Some(count) = new_rows.checked_mul(new_cols) else {
            return false;
        };
        let added = if row_axis { cols } else { rows };
        if count > MAX_MATRIX_CELLS || self.arena.nodes().count() + added > MAX_TREE_NODES {
            return false;
        }
        let Some(display) = self.matrix_display_after_reshape(
            matrix,
            [rows, cols],
            [new_rows, new_cols],
            source_matrix_reshape::Change::Insert {
                axis: usize::from(!row_axis),
                position,
            },
            if row_axis {
                [position, retained]
            } else {
                [retained, position]
            },
        ) else {
            return false;
        };
        let old = self.arena.get(matrix).unwrap().children.clone();
        self.checkpoint();
        let mut children = Vec::with_capacity(count);
        for row in 0..new_rows {
            for col in 0..new_cols {
                let inserted = if row_axis {
                    row == position
                } else {
                    col == position
                };
                let cell = if inserted {
                    let cell = self.arena.allocate(NodeKind::Placeholder, vec![]);
                    self.arena.node_mut(cell).parent = Some(matrix);
                    cell
                } else {
                    let old_row = row - usize::from(row_axis && row > position);
                    let old_col = col - usize::from(!row_axis && col > position);
                    old[old_row * cols + old_col]
                };
                children.push(cell);
            }
        }
        let focus_index = if row_axis {
            position * new_cols + retained
        } else {
            retained * new_cols + position
        };
        self.caret = Caret::at(children[focus_index], Direction::Indeterminate);
        let node = self.arena.node_mut(matrix);
        node.children = children;
        node.kind = NodeKind::Matrix {
            rows: new_rows,
            cols: new_cols,
            display,
        };
        self.selection = Selection::None;
        debug_assert!(self.validate().is_ok());
        true
    }

    fn delete_matrix_dimension(&mut self, row_axis: bool) -> bool {
        if self.matrix_selection().is_some() {
            return self.delete_selected_matrix_axis(row_axis);
        }
        let Some((matrix, rows, cols, index, _)) = self.matrix_cell() else {
            return false;
        };
        let old = self.arena.get(matrix).unwrap().children.clone();
        if rows == 1 && cols == 1 && self.arena.get(old[0]).unwrap().kind == NodeKind::Placeholder {
            return false;
        }
        let position = if row_axis { index / cols } else { index % cols };
        let reset = if row_axis { rows == 1 } else { cols == 1 };
        let next_shape = if reset {
            [1, 1]
        } else {
            [rows - usize::from(row_axis), cols - usize::from(!row_axis)]
        };
        let target = [
            (index / cols).min(next_shape[0] - 1),
            (index % cols).min(next_shape[1] - 1),
        ];
        let Some(display) = self.matrix_display_after_reshape(
            matrix,
            [rows, cols],
            next_shape,
            source_matrix_reshape::Change::Remove {
                axis: usize::from(!row_axis),
                count: 1,
            },
            target,
        ) else {
            return false;
        };
        if !reset && self.matrix_has_source_display(matrix) {
            let old_target = if row_axis {
                (target[0] + usize::from(target[0] >= position)) * cols + target[1]
            } else {
                target[0] * cols + target[1] + usize::from(target[1] >= position)
            };
            if !self.matrix_reshape_target_supported(old[old_target]) {
                return false;
            }
        }
        self.checkpoint();
        let (new_rows, new_cols, children, focus) = if reset {
            for cell in old {
                self.retire(cell, None);
            }
            let cell = self.arena.allocate(NodeKind::Placeholder, vec![]);
            self.arena.node_mut(cell).parent = Some(matrix);
            (1, 1, vec![cell], cell)
        } else {
            let new_rows = rows - usize::from(row_axis);
            let new_cols = cols - usize::from(!row_axis);
            let mut children = Vec::with_capacity(new_rows * new_cols);
            for (i, cell) in old.into_iter().enumerate() {
                let axis_index = if row_axis { i / cols } else { i % cols };
                if axis_index == position {
                    self.retire(cell, None);
                } else {
                    children.push(cell);
                }
            }
            let row = (index / cols).min(new_rows - 1);
            let col = (index % cols).min(new_cols - 1);
            let focus = first_leaf(&self.arena, children[row * new_cols + col]);
            (new_rows, new_cols, children, focus)
        };
        let node = self.arena.node_mut(matrix);
        node.children = children;
        node.kind = NodeKind::Matrix {
            rows: new_rows,
            cols: new_cols,
            display,
        };
        self.caret = Caret::at(focus, Direction::Left);
        self.selection = Selection::None;
        debug_assert!(self.validate().is_ok());
        true
    }

    pub fn insert_definition(&mut self) -> NodeId {
        if self.expressions_only {
            return self.root;
        }
        if matches!(
            self.arena.get(self.root).unwrap().kind,
            NodeKind::Binary(BinaryOp::GlobalDefine)
        ) {
            return self.root;
        }
        if let Some(range) = self.matrix_selection() {
            return range.matrix;
        }
        let target = self.arena.get(self.root).unwrap();
        if matches!(
            target.kind,
            NodeKind::Index { .. } | NodeKind::Row | NodeKind::Column
        ) && !matches!(
            self.arena.get(target.children[0]).unwrap().kind,
            NodeKind::Placeholder
                | NodeKind::Identifier(_)
                | NodeKind::AnnotatedIdentifier { .. }
                | NodeKind::NativeIdentifier { .. }
        ) {
            return self.root;
        }
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
        if let Some(range) = self.matrix_selection() {
            return range.matrix;
        }
        // EVALUATION is admitted at equation root or Define's RHS slot only.
        // Reuse an existing evaluation instead of nesting another result slot.
        let target = if matches!(
            self.arena.get(self.root).unwrap().kind,
            NodeKind::Binary(BinaryOp::Define | BinaryOp::GlobalDefine)
        ) {
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

    /// Delete using the represented native structural stages; unsupported
    /// repair paths leave both the source and navigation/history untouched.
    pub fn delete_structure(&mut self, backwards: bool) -> bool {
        if let Some(changed) = self.delete_literal_text(backwards) {
            return changed;
        }
        if self.typeover.is_some() {
            return self.delete_typeover();
        }
        if self.matrix_selection().is_some() {
            return self.delete_matrix_selection(backwards);
        }
        if let Some(changed) = self.delete_try_structure(backwards) {
            return changed;
        }
        if let Some((_, line, _)) = self.program_line()
            && self.arena.get(line).unwrap().kind == NodeKind::Placeholder
        {
            return self.delete_blank_program_line(backwards);
        }
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
            self.scope = None;
            return true;
        }
        let id = self.caret.node;
        let node = self.arena.get(id).unwrap();
        // REA inward deletion traverses the atomic control token to its center
        // and replaces it within the same command. Outward boundaries retain
        // the existing neighbor/parent path; they are not forced deletions.
        if self.selection == Selection::None
            && matches!(node.kind, NodeKind::Break | NodeKind::Continue)
            && ((backwards && self.caret.caret_side == Direction::Right)
                || (!backwards && self.caret.caret_side == Direction::Left))
        {
            self.checkpoint();
            self.arena.node_mut(id).kind = NodeKind::Placeholder;
            self.caret = Caret::at(id, Direction::Left);
            self.selection = Selection::None;
            self.scope = None;
            return true;
        }
        if self.delete_ordinary_structure(backwards) {
            return true;
        }
        self.begin_root_typeover(backwards);
        false
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
        self.arena.touch();
    }

    pub fn insert_group(&mut self) -> NodeId {
        if let Some(range) = self.matrix_selection() {
            return range.matrix;
        }
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
        if self.cancel_typeover() {
            return true;
        }
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
            Selection::MatrixCells(range) => Some(range.matrix),
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
                    | NodeKind::Determinant
                    | NodeKind::Conjugate
                    | NodeKind::Vectorize
                    | NodeKind::Row
                    | NodeKind::Column
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
                    | NodeKind::Determinant
                    | NodeKind::Conjugate
                    | NodeKind::Vectorize
                    | NodeKind::Row
                    | NodeKind::Column
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
            if let NodeKind::Matrix {
                rows,
                cols,
                display: Some(display),
            } = node.kind
            {
                display
                    .window([rows, cols])
                    .map_err(|_| "invalid source matrix display")?;
            }
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
        if let Selection::MatrixCells(range) = self.selection
            && !self.valid_matrix_selection(range)
        {
            return Err("matrix selection is invalid");
        }
        Ok(())
    }

    fn target(&self) -> NodeId {
        match self.selection {
            Selection::None => self.caret.node,
            Selection::Subtree(id) => id,
            Selection::MatrixCells(range) => range.matrix,
        }
    }

    fn wrap_template(
        &mut self,
        kind: NodeKind,
        arity: usize,
        target_slot: usize,
        initial_slot: usize,
    ) -> NodeId {
        if let Some(range) = self.matrix_selection() {
            return range.matrix;
        }
        debug_assert_eq!(kind.arity(), arity);
        debug_assert!(target_slot < arity && initial_slot < arity);
        self.checkpoint();
        let target = self.target();
        let parent = self.arena.get(target).unwrap().parent;
        let mut children = Vec::with_capacity(arity);
        for slot in 0..arity {
            children.push(if slot == target_slot {
                target
            } else {
                self.arena.allocate(NodeKind::Placeholder, vec![])
            });
        }
        let focus = if self.arena.get(target).unwrap().kind == NodeKind::Placeholder {
            target
        } else {
            children[initial_slot]
        };
        let wrapper = self.arena.allocate(kind, children);
        self.replace_child(parent, target, wrapper);
        self.caret = Caret::at(focus, Direction::Indeterminate);
        self.selection = Selection::None;
        debug_assert!(self.validate().is_ok());
        wrapper
    }

    fn wrap_binary(&mut self, kind: NodeKind) -> NodeId {
        if let Some(range) = self.matrix_selection() {
            return range.matrix;
        }
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
                NodeKind::Binary(BinaryOp::Define | BinaryOp::GlobalDefine | BinaryOp::Evaluate)
            );
        let needs_group = !insert_left
            && ((kind == NodeKind::Power
                && matches!(old.kind, NodeKind::Power | NodeKind::Fraction))
                || (matches!(kind, NodeKind::Row | NodeKind::Column)
                    && matches!(
                        old.kind,
                        NodeKind::Transpose | NodeKind::Row | NodeKind::Column
                    )));
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
        self.typeover = None;
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            arena: self.arena.clone(),
            root: self.root,
            caret: self.caret,
            selection: self.selection,
            scope: self.scope.clone(),
            typeover: self.typeover.clone(),
        }
    }

    fn restore(&mut self, mut snapshot: Snapshot) {
        // Tombstone abandoned future nodes so branch edits cannot reuse their IDs.
        snapshot
            .arena
            .slots
            .resize(self.arena.slots.len().max(snapshot.arena.slots.len()), None);
        snapshot.arena.touch();
        self.arena = snapshot.arena;
        self.root = snapshot.root;
        self.caret = snapshot.caret;
        self.selection = snapshot.selection;
        self.scope = snapshot.scope;
        self.typeover = snapshot.typeover;
    }
}

fn first_leaf(arena: &Arena, mut node: NodeId) -> NodeId {
    while let Some(child) = arena.get(node).unwrap().children.first() {
        node = *child;
    }
    node
}

fn last_leaf(arena: &Arena, mut node: NodeId) -> NodeId {
    while let Some(child) = arena.get(node).unwrap().children.last() {
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

    #[test]
    fn local_function_declaration_preserves_metadata_and_single_undo_with_formal_drafts() {
        let identifier = |name: &str| PortableTree {
            kind: NodeKind::Identifier(name.into()),
            children: vec![],
        };
        let source = PortableTree {
            kind: NodeKind::Program { statement_count: 1 },
            children: vec![PortableTree {
                kind: NodeKind::LocalDefine,
                children: vec![
                    PortableTree {
                        kind: NodeKind::Placeholder,
                        children: vec![],
                    },
                    identifier("body"),
                ],
            }],
        };
        let function = PortableTree {
            kind: NodeKind::Function { parameter_count: 1 },
            children: vec![
                PortableTree {
                    kind: NodeKind::NativeIdentifier {
                        name: "f".into(),
                        labels: "VARIABLE".into(),
                        contextual: Some(false),
                        subscript: Some("local".into()),
                    },
                    children: vec![],
                },
                identifier("m"),
            ],
        };
        let mut editor = Editor::from_portable(&source).unwrap();
        let statement = editor.arena().get(editor.root()).unwrap().children()[0];
        let target = editor.arena().get(statement).unwrap().children()[0];
        editor.select_subtree(target).unwrap();
        editor.replace_selection(&function).unwrap();
        assert_eq!(
            editor.to_portable().unwrap().children[0].children[0],
            function
        );
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), source);
        assert!(!editor.undo());
        assert!(editor.redo());
        let function_id = editor.arena().get(statement).unwrap().children()[0];
        let formal = editor.arena().get(function_id).unwrap().children()[1];
        editor.place_caret(formal, Direction::Right).unwrap();
        assert!(editor.append_argument());
        let draft = editor.to_portable().unwrap();
        assert_eq!(
            draft.children[0].children[0].kind,
            NodeKind::Function { parameter_count: 2 }
        );
        assert_eq!(
            draft.children[0].children[0].children[2].kind,
            NodeKind::Placeholder
        );
        assert!(editor.undo());
        assert_eq!(
            editor.to_portable().unwrap().children[0].children[0],
            function
        );
    }

    #[test]
    fn local_function_validation_rejects_expression_formals_calls_and_function_loop_binders() {
        let atom = |kind| PortableTree {
            kind,
            children: vec![],
        };
        let function = PortableTree {
            kind: NodeKind::Function { parameter_count: 1 },
            children: vec![
                atom(NodeKind::Identifier("f".into())),
                atom(NodeKind::Identifier("x".into())),
            ],
        };
        let program = |lhs| PortableTree {
            kind: NodeKind::Program { statement_count: 1 },
            children: vec![PortableTree {
                kind: NodeKind::LocalDefine,
                children: vec![lhs, atom(NodeKind::Number("3".into()))],
            }],
        };
        assert!(program(function.clone()).validate().is_ok());
        let mut bad_name = function.clone();
        bad_name.children[0] = atom(NodeKind::Number("1".into()));
        assert!(program(bad_name).validate().is_err());
        let mut bad_formal = function.clone();
        bad_formal.children[1] = atom(NodeKind::Number("2".into()));
        assert!(program(bad_formal).validate().is_err());
        let mut call = function.clone();
        call.kind = NodeKind::Call { argument_count: 1 };
        assert!(program(call).validate().is_err());
        let mut loop_target = program(function);
        loop_target.children[0].kind = NodeKind::For;
        loop_target.children[0]
            .children
            .push(atom(NodeKind::Placeholder));
        assert!(loop_target.validate().is_err());
    }

    #[test]
    fn program_local_assignment_uses_the_written_target_as_lhs_and_converts_calls_in_one_undo() {
        let name = || PortableTree {
            kind: NodeKind::NativeIdentifier {
                name: "f".into(),
                labels: "VARIABLE".into(),
                contextual: Some(false),
                subscript: Some("local".into()),
            },
            children: vec![],
        };
        let parameter = || PortableTree {
            kind: NodeKind::Identifier("x".into()),
            children: vec![],
        };
        let mut targets = vec![
            name(),
            PortableTree {
                kind: NodeKind::Placeholder,
                children: vec![],
            },
        ];
        for kind in [
            NodeKind::Call { argument_count: 1 },
            NodeKind::Function { parameter_count: 1 },
            NodeKind::Index { indices: 1 },
            NodeKind::Row,
            NodeKind::Column,
        ] {
            targets.push(PortableTree {
                kind,
                children: vec![name(), parameter()],
            });
        }
        for target_tree in targets {
            let source = PortableTree {
                kind: NodeKind::Program { statement_count: 1 },
                children: vec![target_tree.clone()],
            };
            let mut editor = Editor::from_portable(&source).unwrap();
            let program = editor.root();
            let target = editor.arena().get(program).unwrap().children()[0];
            editor.place_caret(target, Direction::Right).unwrap();
            let assignment = editor.insert_local_definition();
            assert_ne!(assignment, target);
            assert_eq!(
                editor.arena().get(program).unwrap().children(),
                &[assignment]
            );
            assert_eq!(
                editor.arena().get(assignment).unwrap().children()[0],
                target
            );
            let result = editor.to_portable().unwrap();
            let mut expected_target = target_tree.clone();
            if matches!(expected_target.kind, NodeKind::Call { .. }) {
                expected_target.kind = NodeKind::Function { parameter_count: 1 };
            }
            assert_eq!(result.children[0].children[0], expected_target);
            assert_eq!(result.children[0].children[1].kind, NodeKind::Placeholder);
            let expected_focus = if target_tree.kind == NodeKind::Placeholder {
                target
            } else {
                editor.arena().get(assignment).unwrap().children()[1]
            };
            assert_eq!(editor.caret().node, expected_focus);
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), source);
            assert!(!editor.undo());
            assert!(editor.redo());
            assert_eq!(editor.to_portable().unwrap(), result);
        }
    }

    #[test]
    fn local_assignment_rejects_non_program_expression_and_bad_function_targets_without_history() {
        let atom = |kind| PortableTree {
            kind,
            children: vec![],
        };
        let name = || atom(NodeKind::Identifier("f".into()));
        let mut bad_targets = vec![
            atom(NodeKind::Number("1".into())),
            PortableTree {
                kind: NodeKind::Binary(BinaryOp::Add),
                children: vec![name(), name()],
            },
            PortableTree {
                kind: NodeKind::Row,
                children: vec![atom(NodeKind::Number("1".into())), name()],
            },
            PortableTree {
                kind: NodeKind::Call { argument_count: 1 },
                children: vec![name(), atom(NodeKind::Number("2".into()))],
            },
        ];
        for target in bad_targets.drain(..) {
            let source = PortableTree {
                kind: NodeKind::Program { statement_count: 1 },
                children: vec![target],
            };
            let mut editor = Editor::from_portable(&source).unwrap();
            let target = editor.arena().get(editor.root()).unwrap().children()[0];
            editor.place_caret(target, Direction::Right).unwrap();
            let revision = editor.arena().revision();
            assert_eq!(editor.insert_local_definition(), target);
            assert_eq!(editor.to_portable().unwrap(), source);
            assert_eq!(editor.arena().revision(), revision);
            assert!(!editor.undo());
        }
        let source = name();
        let mut editor = Editor::from_portable(&source).unwrap();
        let root = editor.root();
        let revision = editor.arena().revision();
        assert_eq!(editor.insert_local_definition(), root);
        assert_eq!(editor.to_portable().unwrap(), source);
        assert_eq!(editor.arena().revision(), revision);
        assert!(!editor.undo());
        let source = PortableTree {
            kind: NodeKind::Program { statement_count: 1 },
            children: vec![PortableTree {
                kind: NodeKind::Call { argument_count: 1 },
                children: vec![name(), name()],
            }],
        };
        let mut editor = Editor::from_portable(&source).unwrap();
        // The initial caret is inside the call, not at its closed outer edge.
        let revision = editor.arena().revision();
        let caret = editor.caret().node;
        assert_eq!(editor.insert_local_definition(), caret);
        assert_eq!(editor.to_portable().unwrap(), source);
        assert_eq!(editor.arena().revision(), revision);
        assert!(!editor.undo());
    }
    #[test]
    fn arena_revisions_distinguish_clone_branches_and_restore_without_caret_churn() {
        let mut original = Editor::new();
        let first = original.arena().revision();
        original
            .place_caret(original.root(), Direction::Left)
            .unwrap();
        original.select_subtree(original.root()).unwrap();
        assert_eq!(first, original.arena().revision());
        let mut a = original.clone();
        let mut b = original.clone();
        a.set_atom(a.root(), NodeKind::Number("1".into())).unwrap();
        b.set_atom(b.root(), NodeKind::Number("2".into())).unwrap();
        assert_ne!(a.arena().revision(), b.arena().revision());
        assert_ne!(a.arena().revision(), first);
        let changed = a.arena().revision();
        assert!(a.undo());
        let undone = a.arena().revision();
        assert_ne!(changed, undone);
        assert_ne!(first, undone);
        assert!(a.redo());
        assert_ne!(undone, a.arena().revision());
        let tree = b.to_portable().unwrap();
        let before = a.arena().revision();
        a.load_portable(&tree).unwrap();
        assert_ne!(before, a.arena().revision());
        let matrix = a.insert_matrix(2, 2).unwrap();
        let before = a.arena().revision();
        let cell = a.arena().get(matrix).unwrap().children()[0];
        a.place_caret(cell, Direction::Right).unwrap();
        assert!(a.delete_matrix_row());
        assert_ne!(before, a.arena().revision());
        assert!(a.validate().is_ok());
    }

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
    fn expression_extremes_cross_nested_operands_without_creating_undo() {
        let tree = portable(
            NodeKind::Binary(BinaryOp::Add),
            vec![
                portable(NodeKind::Identifier("left".into()), vec![]),
                portable(
                    NodeKind::Fraction,
                    vec![
                        portable(NodeKind::Number("12".into()), vec![]),
                        portable(NodeKind::Identifier("right".into()), vec![]),
                    ],
                ),
            ],
        );
        let mut editor = Editor::from_portable(&tree).unwrap();
        let root = editor.root();
        let left = editor.arena().get(root).unwrap().children()[0];
        let fraction = editor.arena().get(root).unwrap().children()[1];
        let right = editor.arena().get(fraction).unwrap().children()[1];
        let undo = editor.undo.len();

        assert!(editor.move_to_expression_extreme(true));
        assert_eq!(editor.caret().node, right);
        assert_eq!(editor.caret().value_range.unwrap().focus, 5);
        assert!(editor.move_to_expression_extreme(false));
        assert_eq!(editor.caret().node, left);
        assert_eq!(editor.caret().value_range.unwrap().focus, 0);
        assert_eq!(editor.undo.len(), undo);
        assert_eq!(editor.to_portable().unwrap(), tree);
    }

    #[test]
    fn expression_end_includes_native_literal_suffix_without_source_or_history_change() {
        for suffix in ["", "dc𝛼"] {
            let tree = portable(
                NodeKind::NativeIdentifier {
                    name: "V".into(),
                    labels: "UNIT".into(),
                    contextual: Some(false),
                    subscript: Some(suffix.into()),
                },
                vec![],
            );
            let mut editor = Editor::from_portable(&tree).unwrap();
            editor.move_to_expression_extreme(false);
            assert_eq!(editor.caret().value_range.unwrap().focus, 0);
            editor.move_to_expression_extreme(true);
            assert_eq!(editor.caret().caret_side, Direction::Right);
            assert_eq!(editor.caret().value_range, None);
            assert_eq!(editor.caret().insertion, InsertionMode::Normal);
            assert_eq!(editor.to_portable().unwrap(), tree);
            assert!(editor.undo.is_empty() && editor.redo.is_empty());
        }
    }

    #[test]
    fn directional_subtree_selection_controls_operator_operand_side() {
        for (side, selected_slot) in [(Direction::Left, 1), (Direction::Right, 0)] {
            let mut editor = Editor::new();
            let target = editor.root();
            editor
                .set_atom(target, NodeKind::Identifier("x".into()))
                .unwrap();
            editor.select_subtree_from(target, side).unwrap();
            let fraction = editor.insert_fraction();
            assert_eq!(
                editor.arena().get(fraction).unwrap().children()[selected_slot],
                target
            );
            assert_eq!(editor.validate(), Ok(()));
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
    fn program_and_calculus_templates_are_structural_and_tab_navigable() {
        let mut program = Editor::new();
        let conditional = program.insert_if_else();
        assert_eq!(
            program.arena().get(conditional).unwrap().kind(),
            &NodeKind::If {
                has_else: true,
                branch_count: 1,
                alternate: false
            }
        );
        assert!(matches!(
            program
                .arena()
                .get(program.arena().get(conditional).unwrap().children()[1])
                .unwrap()
                .kind(),
            NodeKind::Program { statement_count: 1 }
        ));
        let initial = program.caret().node;
        assert_eq!(
            first_leaf(program.arena(), conditional),
            initial,
            "template caret starts at its first editable leaf"
        );
        assert!(program.validate().is_ok());

        let mut calculus = Editor::new();
        let integral = calculus.insert_integral();
        assert_eq!(calculus.arena().get(integral).unwrap().children().len(), 4);
        let body = calculus.arena().get(integral).unwrap().children()[0];
        let binder = calculus.arena().get(integral).unwrap().children()[1];
        assert_eq!(calculus.caret().node, body);
        calculus.place_caret(binder, Direction::Left).unwrap();
        assert_eq!(calculus.caret().node, binder);
        assert!(calculus.validate().is_ok());
    }

    #[test]
    fn extended_tree_validation_rejects_invalid_container_shapes() {
        assert!(matches!(
            portable(NodeKind::Program { statement_count: 0 }, vec![]).validate(),
            Err(TreeError::InvalidTree(_))
        ));
        assert!(matches!(
            portable(
                NodeKind::If {
                    has_else: false,
                    branch_count: 1,
                    alternate: false
                },
                vec![
                    portable(NodeKind::Placeholder, vec![]),
                    portable(NodeKind::Placeholder, vec![]),
                ],
            )
            .validate(),
            Err(TreeError::InvalidTree(_))
        ));
        assert!(matches!(
            portable(
                NodeKind::SymbolicEvaluation { command_count: 1 },
                vec![
                    portable(NodeKind::Placeholder, vec![]),
                    portable(NodeKind::Placeholder, vec![]),
                ],
            )
            .validate(),
            Err(TreeError::InvalidTree(_))
        ));
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

    fn matrix_fixture(rows: usize, cols: usize) -> (Editor, NodeId, Vec<NodeId>) {
        let tree = portable(
            NodeKind::Matrix {
                rows,
                cols,
                display: None,
            },
            (0..rows * cols)
                .map(|i| portable(NodeKind::Number((i + 1).to_string()), vec![]))
                .collect(),
        );
        let e = Editor::from_portable(&tree).unwrap();
        let matrix = e.root();
        let cells = e.arena().get(matrix).unwrap().children().to_vec();
        (e, matrix, cells)
    }

    #[test]
    fn matrix_insertions_are_relative_row_major_and_one_undo_command() {
        for direction in [Direction::Up, Direction::Down] {
            let (mut e, matrix, cells) = matrix_fixture(2, 3);
            e.place_text_caret(cells[4], 1).unwrap();
            let before = e.to_portable().unwrap();
            let before_caret = e.caret();
            let undo = e.undo.len();
            assert!(e.insert_matrix_row(direction));
            assert_eq!(e.undo.len(), undo + 1);
            assert_eq!(e.root(), matrix);
            let children = e.arena().get(matrix).unwrap().children();
            let at = if direction == Direction::Up { 1 } else { 2 };
            assert_eq!(&children[..at * 3], &cells[..at * 3]);
            assert_eq!(&children[(at + 1) * 3..], &cells[at * 3..]);
            assert_eq!(e.caret().node, children[at * 3 + 1]);
            let new_id = e.caret().node;
            let after = e.to_portable().unwrap();
            assert_eq!(
                after.kind,
                NodeKind::Matrix {
                    rows: 3,
                    cols: 3,
                    display: None
                }
            );
            assert!(e.undo());
            assert_eq!(e.caret(), before_caret);
            assert_eq!(e.to_portable().unwrap(), before);
            assert!(e.arena().get(new_id).is_none());
            assert!(e.redo());
            assert_eq!(e.to_portable().unwrap(), after);
            assert_eq!(e.validate(), Ok(()));
        }
        for direction in [Direction::Left, Direction::Right] {
            let (mut e, matrix, cells) = matrix_fixture(2, 3);
            e.place_caret(cells[4], Direction::Right).unwrap();
            assert!(e.insert_matrix_column(direction));
            let at = if direction == Direction::Left { 1 } else { 2 };
            let children = e.arena().get(matrix).unwrap().children();
            assert_eq!(e.caret().node, children[4 + at]);
            for row in 0..2 {
                for col in 0..3 {
                    assert_eq!(
                        children[row * 4 + col + usize::from(col >= at)],
                        cells[row * 3 + col]
                    );
                }
            }
            assert_eq!(
                e.to_portable().unwrap().kind,
                NodeKind::Matrix {
                    rows: 2,
                    cols: 4,
                    display: None
                }
            );
            assert_eq!(e.validate(), Ok(()));
        }
    }

    #[test]
    fn automatic_matrix_insertion_requires_the_whole_cell_left_boundary() {
        let fraction = portable(
            NodeKind::Fraction,
            vec![
                portable(NodeKind::Number("12".into()), vec![]),
                portable(NodeKind::Number("34".into()), vec![]),
            ],
        );
        let tree = portable(
            NodeKind::Matrix {
                rows: 1,
                cols: 1,
                display: None,
            },
            vec![fraction],
        );
        for (numerator, offset, before) in [(true, 0, true), (true, 1, false), (false, 0, false)] {
            let mut e = Editor::from_portable(&tree).unwrap();
            let matrix = e.root();
            let cell = e.arena().get(matrix).unwrap().children()[0];
            let atom = e.arena().get(cell).unwrap().children()[usize::from(!numerator)];
            e.place_text_caret(atom, offset).unwrap();
            assert!(e.insert_matrix_column(Direction::Indeterminate));
            let children = e.arena().get(matrix).unwrap().children();
            assert_eq!(children[usize::from(before)], cell);
            assert_eq!(e.caret().node, children[usize::from(!before)]);
            assert_eq!(e.validate(), Ok(()));
        }
    }

    #[test]
    fn matrix_deletion_retires_only_removed_subtrees_and_clamps_focus() {
        for row_axis in [true, false] {
            let (mut e, matrix, cells) = matrix_fixture(3, 3);
            e.place_text_selection(cells[8], 0, 1).unwrap();
            let before = e.to_portable().unwrap();
            let caret = e.caret();
            let old_len = e.undo.len();
            assert!(if row_axis {
                e.delete_matrix_row()
            } else {
                e.delete_matrix_column()
            });
            assert_eq!(e.undo.len(), old_len + 1);
            let expected: Vec<_> = cells
                .iter()
                .enumerate()
                .filter_map(|(i, id)| ((if row_axis { i / 3 } else { i % 3 }) != 2).then_some(*id))
                .collect();
            assert_eq!(e.arena().get(matrix).unwrap().children(), expected);
            let focus = if row_axis { cells[5] } else { cells[7] };
            assert_eq!(e.caret().node, focus);
            assert!(e.arena().get(cells[8]).is_none());
            assert_eq!(e.validate(), Ok(()));
            let after = e.to_portable().unwrap();
            assert!(e.undo());
            assert_eq!(e.to_portable().unwrap(), before);
            assert_eq!(e.caret(), caret);
            assert!(e.redo());
            assert_eq!(e.to_portable().unwrap(), after);
        }
    }

    #[test]
    fn deleting_final_matrix_axis_resets_one_placeholder_and_undo_restores_cells() {
        for row_axis in [true, false] {
            let (mut e, matrix, cells) =
                matrix_fixture(if row_axis { 1 } else { 3 }, if row_axis { 3 } else { 1 });
            e.place_caret(cells[1], Direction::Right).unwrap();
            let before = e.to_portable().unwrap();
            assert!(if row_axis {
                e.delete_matrix_row()
            } else {
                e.delete_matrix_column()
            });
            assert_eq!(e.root(), matrix);
            assert_eq!(
                e.to_portable().unwrap(),
                portable(
                    NodeKind::Matrix {
                        rows: 1,
                        cols: 1,
                        display: None
                    },
                    vec![portable(NodeKind::Placeholder, vec![])]
                )
            );
            assert!(cells.iter().all(|id| e.arena().get(*id).is_none()));
            let undo = e.undo.len();
            assert!(!e.delete_matrix_row());
            assert!(!e.delete_matrix_column());
            assert_eq!(e.undo.len(), undo);
            assert!(e.undo());
            assert_eq!(e.to_portable().unwrap(), before);
            assert_eq!(e.validate(), Ok(()));
        }
    }

    #[test]
    fn nested_matrix_commands_keep_outer_siblings_and_reject_ambiguous_selection() {
        let inner = portable(
            NodeKind::Matrix {
                rows: 1,
                cols: 1,
                display: None,
            },
            vec![portable(NodeKind::Number("2".into()), vec![])],
        );
        let tree = portable(
            NodeKind::Binary(BinaryOp::Add),
            vec![
                portable(NodeKind::Number("1".into()), vec![]),
                portable(
                    NodeKind::Matrix {
                        rows: 1,
                        cols: 2,
                        display: None,
                    },
                    vec![inner, portable(NodeKind::Number("3".into()), vec![])],
                ),
            ],
        );
        let mut e = Editor::from_portable(&tree).unwrap();
        let outer = e.arena().get(e.root()).unwrap().children()[1];
        let inner = e.arena().get(outer).unwrap().children()[0];
        let atom = e.arena().get(inner).unwrap().children()[0];
        e.place_text_caret(atom, 1).unwrap();
        assert!(e.insert_matrix_row(Direction::Down));
        assert_eq!(
            e.arena().get(outer).unwrap().kind(),
            &NodeKind::Matrix {
                rows: 1,
                cols: 2,
                display: None
            }
        );
        assert_eq!(
            e.arena().get(inner).unwrap().kind(),
            &NodeKind::Matrix {
                rows: 2,
                cols: 1,
                display: None
            }
        );
        e.select_subtree(inner).unwrap();
        let before = e.to_portable().unwrap();
        let undo = e.undo.len();
        assert!(!e.insert_matrix_column(Direction::Left));
        assert!(!e.delete_matrix_row());
        assert_eq!(e.to_portable().unwrap(), before);
        assert_eq!(e.undo.len(), undo);
        assert_eq!(e.validate(), Ok(()));
    }

    #[test]
    fn rejected_matrix_growth_preserves_redo_selection_and_caret() {
        let (mut e, _, cells) = matrix_fixture(64, 64);
        e.place_text_caret(cells[0], 1).unwrap();
        let before = e.to_portable().unwrap();
        let caret = e.caret();
        assert!(!e.insert_matrix_row(Direction::Down));
        assert!(!e.insert_matrix_column(Direction::Right));
        assert!(!e.insert_matrix_row(Direction::Left));
        assert_eq!(e.caret(), caret);
        assert_eq!(e.to_portable().unwrap(), before);
        assert!(e.undo.is_empty());
        assert!(e.delete_matrix_row());
        assert!(e.undo());
        assert!(!e.insert_matrix_column(Direction::Right));
        assert!(e.redo());
        assert_eq!(e.validate(), Ok(()));
    }

    #[test]
    fn program_enter_inserts_at_line_boundary_and_undo_restores_ids() {
        let tree = portable(
            NodeKind::Program { statement_count: 2 },
            vec![
                portable(NodeKind::Number("12".into()), vec![]),
                portable(NodeKind::Number("34".into()), vec![]),
            ],
        );
        for (offset, expected_index) in [(0, 1), (1, 2), (2, 2)] {
            let mut e = Editor::from_portable(&tree).unwrap();
            let program = e.root();
            let original = e.arena().get(program).unwrap().children().to_vec();
            e.place_text_caret(original[1], offset).unwrap();
            assert!(e.insert_program_line());
            let inserted = e.caret().node;
            let children = e.arena().get(program).unwrap().children();
            assert_eq!(children[expected_index], inserted);
            assert_eq!(
                e.arena().get(inserted).unwrap().kind(),
                &NodeKind::Placeholder
            );
            assert_eq!(
                e.to_portable().unwrap().kind,
                NodeKind::Program { statement_count: 3 }
            );
            assert!(e.undo());
            assert_eq!(e.to_portable().unwrap(), tree);
            assert_eq!(e.arena().get(program).unwrap().children(), original);
            assert!(e.redo());
            assert_eq!(e.validate(), Ok(()));
        }
    }

    #[test]
    fn program_enter_respects_nested_loop_body_and_outer_right_boundary() {
        let body = portable(
            NodeKind::Program { statement_count: 1 },
            vec![portable(NodeKind::Number("1".into()), vec![])],
        );
        let tree = portable(
            NodeKind::Program { statement_count: 1 },
            vec![portable(
                NodeKind::While,
                vec![portable(NodeKind::Number("0".into()), vec![]), body],
            )],
        );
        let mut e = Editor::from_portable(&tree).unwrap();
        let outer = e.root();
        let loop_id = e.arena().get(outer).unwrap().children()[0];
        let body = e.arena().get(loop_id).unwrap().children()[1];
        let inner_line = e.arena().get(body).unwrap().children()[0];
        e.place_caret(inner_line, Direction::Right).unwrap();
        assert!(e.insert_program_line());
        assert_eq!(e.arena().get(body).unwrap().children().len(), 2);
        assert_eq!(e.arena().get(outer).unwrap().children().len(), 1);
        assert!(e.delete_structure(true));
        assert_eq!(e.arena().get(body).unwrap().children(), &[inner_line]);
        e.place_caret(loop_id, Direction::Right).unwrap();
        assert!(e.insert_program_line());
        assert_eq!(e.arena().get(outer).unwrap().children().len(), 2);
        assert_eq!(e.arena().get(body).unwrap().children().len(), 1);
        assert_eq!(e.validate(), Ok(()));
    }

    #[test]
    fn deleting_empty_program_line_preserves_required_body_and_populated_siblings() {
        let mut e = Editor::new();
        let program = e.insert_program();
        assert!(!e.delete_structure(true));
        e.set_atom(e.caret().node, NodeKind::Number("7".into()))
            .unwrap();
        assert!(e.insert_program_line());
        let empty = e.caret().node;
        assert!(e.delete_structure(false));
        assert!(e.arena().get(empty).is_none());
        assert_eq!(e.arena().get(program).unwrap().children().len(), 1);
        assert!(e.undo());
        assert!(e.arena().get(empty).is_some());
        assert_eq!(e.caret().node, empty);
        assert_eq!(e.validate(), Ok(()));
    }

    #[test]
    fn matrix_tab_visits_filled_cells_and_last_cell_grows_one_undoable_row() {
        let (mut e, matrix, cells) = matrix_fixture(2, 2);
        e.place_caret(cells[0], Direction::Right).unwrap();
        assert_eq!(e.matrix_tab(true), Some(false));
        assert_eq!(e.caret().node, cells[1]);
        assert_eq!(e.matrix_tab(true), Some(false));
        assert_eq!(e.caret().node, cells[2]);
        assert_eq!(e.matrix_tab(false), Some(false));
        assert_eq!(e.caret().node, cells[1]);
        e.place_caret(cells[3], Direction::Right).unwrap();
        let before = e.to_portable().unwrap();
        assert_eq!(e.matrix_tab(true), Some(true));
        assert_eq!(
            e.arena().get(matrix).unwrap().kind(),
            &NodeKind::Matrix {
                rows: 3,
                cols: 2,
                display: None
            }
        );
        assert_eq!(e.caret().node, e.arena().get(matrix).unwrap().children()[4]);
        assert!(e.undo());
        assert_eq!(e.to_portable().unwrap(), before);
        assert_eq!(e.validate(), Ok(()));
    }
}
