//! Independently implemented, bounded Math50 programming subset.
//!
//! Schema structure and shipped help establish control flow, not the original
//! numerical interpreter. Budgets, strict comparison and unsupported cases below
//! are explicit project policies. No original process is run by this module.

use super::{
    Binding, Expression, Fault, ML, Result, Scope, Symbol, Value, check_attributes, children,
    is_math, parse_expression, symbol,
};
use mathfolio_core::Quantity;
use roxmltree::Node;
use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

#[cfg(test)]
#[path = "program/try_tests.rs"]
mod try_tests;

pub const DEFAULT_BUDGET: usize = 100_000;
const MAX_LINES: usize = 4096;

#[derive(Clone, Debug)]
pub struct Program {
    pub instructions: Vec<Instruction>,
}

#[derive(Clone, Debug)]
pub enum Instruction {
    Expression(Expression),
    Assign {
        target: Symbol,
        value: Expression,
    },
    AssignArray {
        target: Symbol,
        indices: Vec<Expression>,
        slice: Option<&'static str>,
        value: Expression,
    },
    DefineFunction {
        target: Symbol,
        parameters: Vec<Symbol>,
        body: Expression,
    },
    If {
        branches: Vec<(Expression, Program)>,
        otherwise: Option<Program>,
        evaluate_all: bool,
    },
    Try {
        attempt: Program,
        handler: Program,
    },
    For {
        variable: Symbol,
        iterable: Expression,
        body: Program,
    },
    While {
        condition: Expression,
        body: Program,
    },
    Return(Expression),
    Break,
    Continue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogicOp {
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
    Xor,
    Not,
}

#[derive(Clone, Debug)]
pub struct Logic {
    pub operator: LogicOp,
    pub arguments: Vec<Expression>,
}

fn bad(message: &str) -> Fault {
    Fault::unsupported(message)
}

pub(crate) fn parse_logic(node: Node<'_, '_>, depth: usize) -> Result<Logic> {
    if depth > 128 || !is_math(node, "apply") {
        return Err(bad("Logical expression structure/depth"));
    }
    check_attributes(node, &[])?;
    let items = children(node);
    let Some(op) = items.first() else {
        return Err(bad("Missing logical operator"));
    };
    if op.tag_name().namespace() != Some(ML) || !children(*op).is_empty() {
        return Err(bad("Logical operator namespace/body"));
    }
    check_attributes(*op, &[])?;
    let operator = match op.tag_name().name() {
        "equal" => LogicOp::Equal,
        "notEqual" => LogicOp::NotEqual,
        "lessThan" => LogicOp::Less,
        "lessOrEqual" => LogicOp::LessEqual,
        "greaterThan" => LogicOp::Greater,
        "greaterOrEqual" => LogicOp::GreaterEqual,
        "and" => LogicOp::And,
        "or" => LogicOp::Or,
        "xor" => LogicOp::Xor,
        "not" => LogicOp::Not,
        _ => return Err(bad("Unsupported logical operator")),
    };
    let arity = if operator == LogicOp::Not { 1 } else { 2 };
    if items.len() != arity + 1 {
        return Err(bad("Logical operator arity"));
    }
    Ok(Logic {
        operator,
        arguments: items[1..]
            .iter()
            .map(|node| parse_expression(*node, depth + 1))
            .collect::<Result<_>>()?,
    })
}

pub(crate) fn parse_program(node: Node<'_, '_>, depth: usize) -> Result<Program> {
    if depth > 128 || !is_math(node, "program") {
        return Err(bad("Program namespace/depth"));
    }
    // The shipped symbolic evaluation operator help explicitly excludes its
    // use on a program line. Reject descendants too, including assignment RHS.
    if node.descendants().any(|node| is_math(node, "symEval")) {
        return Err(bad("Symbolic evaluation cannot be used inside a program"));
    }
    check_attributes(node, &[])?;
    let items = children(node);
    if items.is_empty() || items.len() > MAX_LINES {
        return Err(bad("Program line count outside implemented bounds"));
    }
    Ok(Program {
        instructions: items
            .into_iter()
            .map(|node| parse_instruction(node, depth + 1))
            .collect::<Result<_>>()?,
    })
}

fn body(node: Node<'_, '_>, depth: usize) -> Result<Program> {
    if is_math(node, "program") {
        parse_program(node, depth)
    } else {
        Ok(Program {
            instructions: vec![parse_instruction(node, depth + 1)?],
        })
    }
}

fn test_then(node: Node<'_, '_>, depth: usize) -> Result<(Expression, Program)> {
    check_attributes(node, &[])?;
    let parts = children(node);
    if parts.len() < 2 || !is_math(parts[0], "test") || !is_math(parts[1], "then") {
        return Err(bad("If test/then structure"));
    }
    check_attributes(parts[0], &[])?;
    check_attributes(parts[1], &[])?;
    let test = children(parts[0]);
    let then = children(parts[1]);
    if test.len() != 1 || then.len() != 1 || !is_math(then[0], "program") {
        return Err(bad("If test/then arity"));
    }
    Ok((
        parse_expression(test[0], depth + 1)?,
        parse_program(then[0], depth + 1)?,
    ))
}

fn parse_instruction(node: Node<'_, '_>, depth: usize) -> Result<Instruction> {
    if depth > 128 || node.tag_name().namespace() != Some(ML) {
        return Err(bad("Program instruction namespace/depth"));
    }
    let items = children(node);
    match node.tag_name().name() {
        "localDefine" => {
            check_attributes(node, &["style"])?;
            if node
                .attribute("style")
                .is_some_and(|s| !matches!(s, "default" | "left-arrow" | "equal"))
            {
                return Err(bad("Local assignment style"));
            }
            if items.len() != 2 {
                return Err(bad("Local assignment arity"));
            }
            if is_math(items[0], "function") {
                check_attributes(items[0], &[])?;
                let declaration = children(items[0]);
                let [name, bound] = declaration.as_slice() else {
                    return Err(bad("Local function declaration structure"));
                };
                if !is_math(*bound, "boundVars") {
                    return Err(bad("Local function requires boundVars"));
                }
                check_attributes(*bound, &[])?;
                let parameters = children(*bound)
                    .into_iter()
                    .map(symbol)
                    .collect::<Result<Vec<_>>>()?;
                if parameters.len() > 64
                    || parameters.iter().collect::<HashSet<_>>().len() != parameters.len()
                {
                    return Err(bad("Local function parameter count or duplicates"));
                }
                return Ok(Instruction::DefineFunction {
                    target: symbol(*name)?,
                    parameters,
                    body: parse_expression(items[1], depth + 1)?,
                });
            }
            if let Some((target, indices, slice)) = super::array_definition_target(items[0])? {
                return Ok(Instruction::AssignArray {
                    target,
                    indices,
                    slice,
                    value: parse_expression(items[1], depth + 1)?,
                });
            }
            Ok(Instruction::Assign {
                target: symbol(items[0])?,
                value: parse_expression(items[1], depth + 1)?,
            })
        }
        "if" => {
            let first = test_then(node, depth + 1)?;
            let mut branches = vec![first];
            let mut otherwise = None;
            let mut mode = None;
            for (index, part) in items[2..].iter().enumerate() {
                match part.tag_name().name() {
                    "elseif" | "alsoif" if part.tag_name().namespace() == Some(ML) => {
                        let all = is_math(*part, "alsoif");
                        if mode.is_some_and(|old| old != all) || otherwise.is_some() {
                            return Err(bad("Mixed or misplaced conditional branches"));
                        }
                        if children(*part).len() != 2 {
                            return Err(bad("Conditional branch arity"));
                        }
                        mode = Some(all);
                        branches.push(test_then(*part, depth + 1)?);
                    }
                    "else" if is_math(*part, "else") && index + 3 == items.len() => {
                        check_attributes(*part, &[])?;
                        let values = children(*part);
                        if values.len() != 1 || !is_math(values[0], "program") {
                            return Err(bad("Else program structure"));
                        }
                        otherwise = Some(parse_program(values[0], depth + 1)?);
                    }
                    _ => return Err(bad("Unsupported conditional branch")),
                }
            }
            Ok(Instruction::If {
                branches,
                otherwise,
                evaluate_all: mode.unwrap_or(false),
            })
        }
        "for" => {
            check_attributes(node, &[])?;
            if items.len() != 3 || !is_math(items[0], "id") {
                return Err(bad("For variable/range/body structure"));
            }
            Ok(Instruction::For {
                variable: symbol(items[0])?,
                iterable: parse_expression(items[1], depth + 1)?,
                body: body(items[2], depth + 1)?,
            })
        }
        "while" => {
            check_attributes(node, &[])?;
            if items.len() != 2 {
                return Err(bad("While condition/body structure"));
            }
            Ok(Instruction::While {
                condition: parse_expression(items[0], depth + 1)?,
                body: body(items[1], depth + 1)?,
            })
        }
        "tryCatch" => {
            check_attributes(node, &[])?;
            if items.len() != 2 {
                return Err(bad("Try/on error requires exactly two bodies"));
            }
            Ok(Instruction::Try {
                attempt: body(items[0], depth + 1)?,
                handler: body(items[1], depth + 1)?,
            })
        }
        "return" => {
            check_attributes(node, &[])?;
            if items.len() != 1 {
                return Err(bad("Return expression arity"));
            }
            Ok(Instruction::Return(parse_expression(items[0], depth + 1)?))
        }
        "break" | "continue" => {
            check_attributes(node, &[])?;
            if !items.is_empty() {
                return Err(bad("Loop control takes no arguments"));
            }
            Ok(if is_math(node, "break") {
                Instruction::Break
            } else {
                Instruction::Continue
            })
        }
        _ => Ok(Instruction::Expression(parse_expression(node, depth + 1)?)),
    }
}

/// A standalone public entry point, useful for testing exact native MathML.
pub fn from_mathml(xml: &str) -> Result<Program> {
    if xml.len() > 8 * 1024 * 1024 {
        return Err(bad("Program XML byte budget exceeded"));
    }
    let document = roxmltree::Document::parse(xml).map_err(|_| bad("Malformed program XML"))?;
    parse_program(document.root_element(), 0)
}

/// Native Try is also an expression-class node. Represent an isolated Try in
/// this interpreter's existing local program frame; do not extend this policy
/// to every programming operator by analogy (source contract 0103).
pub(crate) fn parse_try_expression(node: Node<'_, '_>, depth: usize) -> Result<Program> {
    if !is_math(node, "tryCatch") {
        return Err(bad("Expected a Try/on error expression"));
    }
    Ok(Program {
        instructions: vec![parse_instruction(node, depth + 1)?],
    })
}

/// Host bindings are cloned. Program locals never modify the caller's worksheet.
pub fn execute(program: &Program, bindings: &[(Symbol, Value)], budget: usize) -> Result<Value> {
    let mut scope = Scope::default();
    for (name, value) in bindings {
        scope
            .bindings
            .insert(name.clone(), Binding::Value(value.clone()));
    }
    run(program, &mut scope, &mut Runtime { remaining: budget }, 0)
}

pub(crate) fn evaluate_in_scope(program: &Program, scope: &Scope, depth: usize) -> Result<Value> {
    run(
        program,
        &mut scope.clone(),
        &mut Runtime {
            remaining: DEFAULT_BUDGET,
        },
        depth,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Control {
    Normal,
    Return,
    Break,
    Continue,
}
struct Outcome {
    control: Control,
    value: Option<Result<Value>>,
}
struct Runtime {
    remaining: usize,
}
impl Runtime {
    fn tick(&mut self) -> Result<()> {
        super::check_cancelled()?;
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or_else(|| Fault::new("program_budget", "Program instruction budget exceeded"))?;
        Ok(())
    }
}

fn run(program: &Program, scope: &mut Scope, runtime: &mut Runtime, depth: usize) -> Result<Value> {
    // Shipped program examples declare VARIABLE locals but also reference them
    // with automatic labels. Resolve only names declared by this program; this
    // does not implement general automatic label resolution for the worksheet.
    let mut declared = HashSet::new();
    locals(program, &mut declared);
    for name in declared {
        if let Some(alias) = automatic_alias(&name)
            && let Ok(binding) = scope.lookup(&name)
        {
            scope.bindings.insert(alias, binding);
        }
    }
    // Each program expression owns its mutation ledger. Captured functions
    // refer to this ledger, not the argument bindings of later callers.
    scope.program_frame = Some(Arc::new(CaptureFrame::default()));
    let out = block(program, scope, runtime, depth)?;
    if matches!(out.control, Control::Break | Control::Continue) {
        return Err(Fault::new(
            "program_control",
            "Loop control used outside a loop",
        ));
    }
    out.value
        .unwrap_or_else(|| Err(Fault::new("program_no_value", "Program produced no value")))
}

fn automatic_alias(name: &Symbol) -> Option<Symbol> {
    (name.label == "VARIABLE").then(|| Symbol {
        name: name.name.clone(),
        label: "*".into(),
    })
}

fn assign_local(scope: &mut Scope, name: &Symbol, value: Value) {
    let before = local_before(scope, name);
    scope.bindings.insert(name.clone(), Binding::Value(value));
    finish_local_assignment(scope, name, before);
}

#[derive(Default)]
pub(crate) struct CaptureFrame {
    watched: Mutex<HashMap<Symbol, u64>>,
    active: AtomicBool,
}

#[derive(Clone)]
pub(crate) struct CaptureGuard {
    frame: Arc<CaptureFrame>,
    revisions: Vec<(Symbol, u64)>,
}
impl CaptureGuard {
    fn new(
        scope: &Scope,
        target: &Symbol,
        parameters: &[Symbol],
        body: &Expression,
    ) -> Result<Option<Self>> {
        let Some(frame) = &scope.program_frame else {
            return Ok(None);
        };
        let mut names = HashSet::new();
        let mut bound = parameters.iter().cloned().collect::<HashSet<_>>();
        bound.extend(parameters.iter().filter_map(automatic_alias));
        free_names(body, &bound, &mut names, 0)?;
        // A redefinition may refer to its previous function binding. Publishing
        // the new function must not invalidate that deliberate backreference.
        names.remove(target);
        if let Some(alias) = automatic_alias(target) {
            names.remove(&alias);
        }
        let mut watched = frame
            .watched
            .lock()
            .map_err(|_| bad("Local capture ledger unavailable"))?;
        let revisions = names
            .into_iter()
            .map(|name| {
                let revision = *watched.entry(name.clone()).or_default();
                (name, revision)
            })
            .collect();
        frame.active.store(!watched.is_empty(), Ordering::Relaxed);
        Ok(Some(Self {
            frame: frame.clone(),
            revisions,
        }))
    }
    pub(crate) fn check(&self) -> Result<()> {
        let watched = self
            .frame
            .watched
            .lock()
            .map_err(|_| bad("Local capture ledger unavailable"))?;
        if self
            .revisions
            .iter()
            .any(|(name, revision)| watched.get(name) != Some(revision))
        {
            return Err(bad(
                "Calling a local function after changing one of its captured bindings needs an established original capture contract",
            ));
        }
        Ok(())
    }
}

type LocalBefore = Vec<(Symbol, Option<Arc<Binding>>)>;
fn same_capture_quantity(left: &Quantity, right: &Quantity) -> bool {
    left.value.re.to_bits() == right.value.re.to_bits()
        && left.value.im.to_bits() == right.value.im.to_bits()
        && left
            .dimension
            .iter()
            .zip(right.dimension)
            .all(|(left, right)| left.to_bits() == right.to_bits())
}
fn same_capture_value(left: &Value, right: &Value) -> bool {
    let quantities = |left: &[Quantity], right: &[Quantity]| {
        left.len() == right.len()
            && left
                .iter()
                .zip(right)
                .all(|(left, right)| same_capture_quantity(left, right))
    };
    match (left, right) {
        (Value::Scalar { quantity: left }, Value::Scalar { quantity: right }) => {
            same_capture_quantity(left, right)
        }
        (
            Value::Matrix {
                rows: lr,
                cols: lc,
                elements: left,
            },
            Value::Matrix {
                rows: rr,
                cols: rc,
                elements: right,
            },
        ) => (lr, lc) == (rr, rc) && quantities(left, right),
        (Value::RangeValues { elements: left }, Value::RangeValues { elements: right }) => {
            quantities(left, right)
        }
        (
            Value::Range {
                start: ls,
                step: lt,
                end: le,
                count: lc,
            },
            Value::Range {
                start: rs,
                step: rt,
                end: re,
                count: rc,
            },
        ) => {
            lc == rc
                && same_capture_quantity(ls, rs)
                && same_capture_quantity(lt, rt)
                && same_capture_quantity(le, re)
        }
        (Value::Symbolic { mathml: left }, Value::Symbolic { mathml: right }) => left == right,
        (Value::String { value: left }, Value::String { value: right }) => left == right,
        _ => false,
    }
}
fn local_before(scope: &Scope, name: &Symbol) -> LocalBefore {
    if !scope
        .program_frame
        .as_ref()
        .is_some_and(|frame| frame.active.load(Ordering::Relaxed))
    {
        return vec![];
    }
    std::iter::once(name.clone())
        .chain(automatic_alias(name))
        .map(|name| {
            let binding = scope.bindings.shared(&name);
            (name, binding)
        })
        .collect()
}
fn finish_local_assignment(scope: &mut Scope, name: &Symbol, before: LocalBefore) {
    if let Some(alias) = automatic_alias(name)
        && let Some(binding) = scope.bindings.shared(name)
    {
        scope.bindings.insert_shared(alias, binding);
    }
    if before.is_empty() {
        return;
    }
    let Some(frame) = &scope.program_frame else {
        return;
    };
    let Ok(mut watched) = frame.watched.lock() else {
        return;
    };
    for (name, old) in before {
        let Some(revision) = watched.get_mut(&name) else {
            continue;
        };
        let new = scope.bindings.shared(&name);
        let same = match (&old, &new) {
            (Some(old), Some(new)) if Arc::ptr_eq(old, new) => true,
            (Some(old), Some(new)) => {
                fn value(binding: &Binding) -> Option<&Value> {
                    match binding {
                        Binding::Value(value) | Binding::IndexedArray { value, .. } => Some(value),
                        _ => None,
                    }
                }
                fn fill_dimension(binding: &Binding) -> Option<[f64; 7]> {
                    match binding {
                        Binding::IndexedArray { zero_dimension, .. } => *zero_dimension,
                        Binding::Value(Value::Matrix { elements, .. }) => elements
                            .first()
                            .filter(|first| elements.iter().all(|q| q.dimension == first.dimension))
                            .map(|q| q.dimension),
                        _ => None,
                    }
                }
                match (value(old), value(new)) {
                    (Some(old_value), Some(new_value)) => {
                        let same_fill = match (fill_dimension(old), fill_dimension(new)) {
                            (Some(left), Some(right)) => left
                                .iter()
                                .zip(right)
                                .all(|(left, right)| left.to_bits() == right.to_bits()),
                            (None, None) => true,
                            _ => false,
                        };
                        same_capture_value(old_value, new_value) && same_fill
                    }
                    _ => false,
                }
            }
            (None, None) => true,
            _ => false,
        };
        if !same {
            *revision = revision.saturating_add(1);
        }
    }
}

fn free_names(
    expression: &Expression,
    bound: &HashSet<Symbol>,
    names: &mut HashSet<Symbol>,
    depth: usize,
) -> Result<()> {
    if depth > 128 {
        return Err(bad("Local function dependency depth exceeded"));
    }
    let mut visit = |child| free_names(child, bound, names, depth + 1);
    match expression {
        Expression::Name(name) => {
            if !bound.contains(name) {
                names.insert(name.clone());
            }
        }
        Expression::Scalar(_) | Expression::StringLiteral(_) | Expression::Unsupported(_) => (),
        Expression::Neg(child) | Expression::Absolute(child) | Expression::Unary(_, child) => {
            visit(child)?
        }
        Expression::Binary(_, left, right) | Expression::Root(left, right) => {
            visit(left)?;
            visit(right)?;
        }
        Expression::Matrix { elements, .. } => {
            for child in elements {
                visit(child)?;
            }
        }
        Expression::Index(base, indices) => {
            visit(base)?;
            for child in indices {
                visit(child)?;
            }
        }
        Expression::Range { start, second, end } => {
            visit(start)?;
            if let Some(second) = second {
                visit(second)?;
            }
            visit(end)?;
        }
        Expression::Call(name, args) => {
            for arg in args {
                visit(arg)?;
            }
            if !bound.contains(name) {
                names.insert(name.clone());
            }
        }
        Expression::Logic(logic) => {
            for child in &logic.arguments {
                visit(child)?;
            }
        }
        Expression::Program(program) => {
            let mut local = bound.clone();
            let mut declared = HashSet::new();
            locals(program, &mut declared);
            local.extend(declared.iter().filter_map(automatic_alias));
            local.extend(declared);
            visit_expressions(program, &mut |child, parameters| {
                let mut child_bound = local.clone();
                child_bound.extend(parameters.iter().cloned());
                child_bound.extend(parameters.iter().filter_map(automatic_alias));
                free_names(child, &child_bound, names, depth + 1)
            })?;
        }
        Expression::Numerical(numerical) => {
            use super::numerics::NumericalExpression::*;
            let (body, variable) = match numerical {
                Root { body, variable, .. }
                | Derivative { body, variable, .. }
                | Integral { body, variable, .. }
                | Fold { body, variable, .. } => (body, variable),
            };
            let mut local = bound.clone();
            local.insert(variable.clone());
            if let Some(alias) = automatic_alias(variable) {
                local.insert(alias);
            }
            free_names(body, &local, names, depth + 1)?;
            match numerical {
                Root {
                    bounds: Some((lower, upper)),
                    ..
                }
                | Integral { lower, upper, .. }
                | Fold { lower, upper, .. } => {
                    free_names(lower, bound, names, depth + 1)?;
                    free_names(upper, bound, names, depth + 1)?;
                }
                Root {
                    bounds: None,
                    variable,
                    ..
                } => {
                    if !bound.contains(variable) {
                        names.insert(variable.clone());
                    }
                }
                Derivative {
                    variable, order, ..
                } => {
                    free_names(order, bound, names, depth + 1)?;
                    if !bound.contains(variable) {
                        names.insert(variable.clone());
                    }
                }
            }
        }
        Expression::Symbolic(_) => {
            return Err(bad("Symbolic local function capture is not established"));
        }
    }
    Ok(())
}

fn truth(value: Value) -> Result<bool> {
    let quantity = value.as_quantity()?;
    if !quantity.is_scalar() {
        return Err(Fault::new(
            "bad%_dimensions%_unitless",
            "Boolean test must be dimensionless in this implementation",
        ));
    }
    Ok(quantity.value.re != 0.0 || quantity.value.im != 0.0)
}

pub(crate) fn evaluate_logic(logic: &Logic, scope: &Scope, depth: usize) -> Result<Value> {
    if depth > 128 || logic.arguments.len() != if logic.operator == LogicOp::Not { 1 } else { 2 } {
        return Err(bad("Logical expression arity/depth"));
    }
    // Logical operators are eager in this bounded implementation. Only control
    // branches have a shipped, explicitly established lazy-evaluation contract.
    let values = logic
        .arguments
        .iter()
        .map(|arg| scope.evaluate(arg, depth + 1))
        .collect::<Result<Vec<_>>>()?;
    let boolean = match logic.operator {
        LogicOp::Not => !truth(values[0].clone())?,
        LogicOp::And => truth(values[0].clone())? & truth(values[1].clone())?,
        LogicOp::Or => truth(values[0].clone())? | truth(values[1].clone())?,
        LogicOp::Xor => truth(values[0].clone())? ^ truth(values[1].clone())?,
        op => {
            let a = values[0].as_quantity()?;
            let b = values[1].as_quantity()?;
            if !a.same_dimension(&b) {
                return Err(Fault::new(
                    "bad%_dimensions",
                    "Comparison dimensions differ",
                ));
            }
            match op {
                LogicOp::Equal => a.value == b.value,
                LogicOp::NotEqual => a.value != b.value,
                _ => {
                    if a.value.im != 0.0 || b.value.im != 0.0 {
                        return Err(bad("Ordered complex comparisons are not implemented"));
                    }
                    match op {
                        LogicOp::Less => a.value.re < b.value.re,
                        LogicOp::LessEqual => a.value.re <= b.value.re,
                        LogicOp::Greater => a.value.re > b.value.re,
                        LogicOp::GreaterEqual => a.value.re >= b.value.re,
                        _ => unreachable!(),
                    }
                }
            }
        }
    };
    Ok(Value::scalar(Quantity::scalar(if boolean {
        1.0
    } else {
        0.0
    })))
}

/// A bounded interpreter policy, not a reconstruction of the original fault
/// taxonomy. Calculation faults can enter on-error; implementation gaps,
/// upstream failed regions and host stop/resource limits must remain visible.
fn try_catches(fault: &Fault) -> bool {
    !fault.unsupported
        && !fault.is_cancelled()
        && fault.previous_region.is_none()
        && !matches!(
            fault.code.as_str(),
            "unsupported"
                | "program_budget"
                | "program_control"
                | "program_no_value"
                | "EA041_RANGE_RECURRENCE"
        )
}

fn block(
    program: &Program,
    scope: &mut Scope,
    runtime: &mut Runtime,
    depth: usize,
) -> Result<Outcome> {
    if depth > 128 {
        return Err(bad("Program execution depth exceeded"));
    }
    let mut last = None;
    for instruction in &program.instructions {
        runtime.tick()?;
        let out = match instruction {
            Instruction::Expression(expression) => Outcome {
                control: Control::Normal,
                value: Some(Ok(scope.evaluate(expression, depth + 1)?)),
            },
            Instruction::Assign { target, value } => {
                let shared_array = match value {
                    Expression::Name(name) => scope.bindings.shared(name).filter(|binding| {
                        matches!(
                            binding.as_ref(),
                            Binding::IndexedArray { .. } | Binding::Value(Value::Matrix { .. })
                        )
                    }),
                    _ => None,
                };
                let value = scope.evaluate(value, depth + 1)?;
                if matches!(value, Value::Range { .. }) {
                    return Err(bad(
                        "Local range assignment requires vector materialization, not a local range binding",
                    ));
                }
                if let Some(binding) = shared_array {
                    let before = local_before(scope, target);
                    scope.bindings.insert_shared(target.clone(), binding);
                    finish_local_assignment(scope, target, before);
                } else {
                    assign_local(scope, target, value.clone());
                }
                Outcome {
                    control: Control::Normal,
                    value: Some(Ok(value)),
                }
            }
            Instruction::AssignArray {
                target,
                indices,
                slice,
                value,
            } => {
                let before = local_before(scope, target);
                let (_, assigned) = scope.define_array_at_depth(
                    target,
                    (indices, *slice),
                    value,
                    "program-local",
                    depth + 1,
                )?;
                // Share the complete binding, including its zero-fill basis.
                finish_local_assignment(scope, target, before);
                Outcome {
                    control: Control::Normal,
                    value: Some(Ok(assigned)),
                }
            }
            Instruction::DefineFunction {
                target,
                parameters,
                body,
            } => {
                let guard = CaptureGuard::new(scope, target, parameters, body)?;
                let before = local_before(scope, target);
                scope.define(
                    target,
                    &Some(parameters.clone()),
                    body,
                    "program-local-function",
                )?;
                let Some(Binding::Function {
                    parameters,
                    body,
                    captured,
                    ..
                }) = scope.bindings.get(target)
                else {
                    unreachable!()
                };
                scope.bindings.insert(
                    target.clone(),
                    Binding::Function {
                        parameters: parameters.clone(),
                        body: body.clone(),
                        captured: captured.clone(),
                        local_guard: guard,
                    },
                );
                finish_local_assignment(scope, target, before);
                Outcome {
                    control: Control::Normal,
                    value: Some(Err(bad(
                        "Returning a function value from a program is not implemented",
                    ))),
                }
            }
            Instruction::Return(expression) => Outcome {
                control: Control::Return,
                value: Some(Ok(scope.evaluate(expression, depth + 1)?)),
            },
            Instruction::Break => Outcome {
                control: Control::Break,
                value: None,
            },
            Instruction::Continue => Outcome {
                control: Control::Continue,
                value: None,
            },
            Instruction::If {
                branches,
                otherwise,
                evaluate_all,
            } => {
                let mut matched = false;
                let mut out = Outcome {
                    control: Control::Normal,
                    value: None,
                };
                for (test, body) in branches {
                    runtime.tick()?;
                    if truth(scope.evaluate(test, depth + 1)?)? {
                        matched = true;
                        out = block(body, scope, runtime, depth + 1)?;
                        if out.control != Control::Normal || !*evaluate_all {
                            break;
                        }
                    }
                }
                if !matched && let Some(body) = otherwise {
                    out = block(body, scope, runtime, depth + 1)?;
                }
                out
            }
            Instruction::Try { attempt, handler } => {
                match block(attempt, scope, runtime, depth + 1) {
                    Ok(out) => {
                        // Return/break/continue are successful control flow,
                        // not errors. A deferred final value fault still has
                        // the same boundary as an immediately raised fault.
                        if out.control == Control::Normal
                            && out
                                .value
                                .as_ref()
                                .is_some_and(|value| value.as_ref().is_err_and(try_catches))
                        {
                            block(handler, scope, runtime, depth + 1)?
                        } else {
                            out
                        }
                    }
                    Err(fault) if try_catches(&fault) => {
                        // Preserve this interpreter's sequential local state;
                        // original rollback semantics have not been observed.
                        // Do not reset the host's shared instruction budget.
                        block(handler, scope, runtime, depth + 1)?
                    }
                    Err(fault) => return Err(fault),
                }
            }
            Instruction::While { condition, body } => {
                let mut out = Outcome {
                    control: Control::Normal,
                    value: None,
                };
                loop {
                    runtime.tick()?;
                    if !truth(scope.evaluate(condition, depth + 1)?)? {
                        break;
                    }
                    let step = block(body, scope, runtime, depth + 1)?;
                    if step.value.is_some() {
                        out.value = step.value;
                    }
                    if step.control == Control::Return {
                        out.control = Control::Return;
                        break;
                    }
                    if step.control == Control::Break {
                        break;
                    }
                }
                out
            }
            Instruction::For {
                variable,
                iterable,
                body,
            } => {
                let values = iteration_values(scope.evaluate(iterable, depth + 1)?)?;
                let mut out = Outcome {
                    control: Control::Normal,
                    value: None,
                };
                for value in values {
                    runtime.tick()?;
                    assign_local(scope, variable, Value::scalar(value));
                    let step = block(body, scope, runtime, depth + 1)?;
                    if step.value.is_some() {
                        out.value = step.value;
                    }
                    if step.control == Control::Return {
                        out.control = Control::Return;
                        break;
                    }
                    if step.control == Control::Break {
                        break;
                    }
                }
                out
            }
        };
        if out.value.is_some() {
            last = out.value;
        }
        if out.control != Control::Normal {
            return Ok(Outcome {
                control: out.control,
                value: last,
            });
        }
    }
    Ok(Outcome {
        control: Control::Normal,
        value: last,
    })
}

fn iteration_values(value: Value) -> Result<Vec<Quantity>> {
    match value {
        Value::Scalar { quantity } => Ok(vec![quantity]),
        Value::Range {
            start, step, count, ..
        } => {
            if count > 16_384 {
                return Err(bad("For range cardinality budget"));
            }
            Ok((0..count)
                .map(|index| {
                    let mut q = start;
                    q.value += step.value * index as f64;
                    q
                })
                .collect())
        }
        Value::Matrix {
            rows,
            cols,
            elements,
        } if rows == 1 || cols == 1 => Ok(elements),
        Value::RangeValues { elements } => Ok(elements),
        _ => Err(bad(
            "For iteration over a non-vector matrix is not implemented",
        )),
    }
}

fn locals(program: &Program, names: &mut HashSet<Symbol>) {
    for instruction in &program.instructions {
        match instruction {
            Instruction::Assign { target, .. }
            | Instruction::AssignArray { target, .. }
            | Instruction::DefineFunction { target, .. } => {
                names.insert(target.clone());
            }
            Instruction::For { variable, body, .. } => {
                names.insert(variable.clone());
                locals(body, names);
            }
            Instruction::While { body, .. } => locals(body, names),
            Instruction::Try { attempt, handler } => {
                locals(attempt, names);
                locals(handler, names);
            }
            Instruction::If {
                branches,
                otherwise,
                ..
            } => {
                for (_, body) in branches {
                    locals(body, names);
                }
                if let Some(body) = otherwise {
                    locals(body, names);
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn visit_expressions(
    program: &Program,
    visitor: &mut impl FnMut(&Expression, &[Symbol]) -> Result<()>,
) -> Result<()> {
    for instruction in &program.instructions {
        match instruction {
            Instruction::Expression(e)
            | Instruction::Return(e)
            | Instruction::Assign { value: e, .. } => visitor(e, &[])?,
            Instruction::DefineFunction {
                body, parameters, ..
            } => visitor(body, parameters)?,
            Instruction::AssignArray { indices, value, .. } => {
                for index in indices {
                    visitor(index, &[])?;
                }
                visitor(value, &[])?;
            }
            Instruction::For { iterable, body, .. } => {
                visitor(iterable, &[])?;
                visit_expressions(body, visitor)?;
            }
            Instruction::While { condition, body } => {
                visitor(condition, &[])?;
                visit_expressions(body, visitor)?;
            }
            Instruction::Try { attempt, handler } => {
                visit_expressions(attempt, visitor)?;
                visit_expressions(handler, visitor)?;
            }
            Instruction::If {
                branches,
                otherwise,
                ..
            } => {
                for (test, body) in branches {
                    visitor(test, &[])?;
                    visit_expressions(body, visitor)?;
                }
                if let Some(body) = otherwise {
                    visit_expressions(body, visitor)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

pub(crate) fn validate_names(
    program: &Program,
    scope: &Scope,
    parameters: &[Symbol],
    depth: usize,
) -> Result<()> {
    let mut names = HashSet::new();
    locals(program, &mut names);
    let mut bound = parameters.to_vec();
    bound.extend(names.iter().filter_map(automatic_alias));
    bound.extend(names);
    visit_expressions(program, &mut |expression, parameters| {
        let mut local = bound.clone();
        local.extend(parameters.iter().cloned());
        local.extend(parameters.iter().filter_map(automatic_alias));
        scope.validate_names(expression, &local, depth + 1)
    })
}

#[cfg(test)]
mod local_function_tests {
    use super::*;
    use crate::CalculationControl;

    #[test]
    fn cancelling_a_local_function_call_preserves_the_caller_and_fresh_retry() {
        let name = |name: &str| Symbol {
            name: name.into(),
            label: "VARIABLE".into(),
        };
        let number = |value: f64| Expression::Scalar(Quantity::scalar(value));
        let body = Program {
            instructions: vec![
                Instruction::Assign {
                    target: name("total"),
                    value: number(0.),
                },
                Instruction::For {
                    variable: name("i"),
                    iterable: Expression::Range {
                        start: Box::new(number(0.)),
                        second: None,
                        end: Box::new(number(999.)),
                    },
                    body: Program {
                        instructions: vec![Instruction::Assign {
                            target: name("total"),
                            value: Expression::Binary(
                                "plus".into(),
                                Box::new(Expression::Name(name("total"))),
                                Box::new(Expression::Name(name("x"))),
                            ),
                        }],
                    },
                },
                Instruction::Expression(Expression::Name(name("total"))),
            ],
        };
        let program = Program {
            instructions: vec![
                Instruction::DefineFunction {
                    target: name("f"),
                    parameters: vec![name("x")],
                    body: Expression::Program(Box::new(body)),
                },
                Instruction::Expression(Expression::Call(name("f"), vec![number(3.)])),
            ],
        };
        let mut scope = Scope::default();
        scope
            .define(&name("external"), &None, &number(5.), "caller")
            .unwrap();
        let before = scope.bindings.shared(&name("external")).unwrap();
        let control = CalculationControl::stop_after_checks(300);
        assert!(
            control
                .run(|| evaluate_in_scope(&program, &scope, 0))
                .unwrap_err()
                .is_cancelled()
        );
        assert!(Arc::ptr_eq(
            &before,
            &scope.bindings.shared(&name("external")).unwrap()
        ));
        assert!(scope.program_frame.is_none());
        assert!(!scope.bindings.contains_key(&name("f")));
        assert_eq!(
            evaluate_in_scope(&program, &scope, 0)
                .unwrap()
                .as_quantity()
                .unwrap()
                .value
                .re,
            3000.
        );
    }
}
