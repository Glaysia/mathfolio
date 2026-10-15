//! Independently implemented, bounded Math50 programming subset.
//!
//! Schema structure and shipped help establish control flow, not the original
//! numerical interpreter. Budgets, strict comparison and unsupported cases below
//! are explicit project policies. No original process is run by this module.

use super::{
    Binding, Expression, Fault, ML, Result, Scope, Symbol, Value, check_attributes, children,
    is_math, parse_expression, symbol,
};
use mathcraft_core::Quantity;
use roxmltree::Node;
use std::collections::HashSet;

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
    If {
        branches: Vec<(Expression, Program)>,
        otherwise: Option<Program>,
        evaluate_all: bool,
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
            if items.len() != 2 || !is_math(items[0], "id") {
                return Err(bad("Only named local assignments are implemented"));
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
    value: Option<Value>,
}
struct Runtime {
    remaining: usize,
}
impl Runtime {
    fn tick(&mut self) -> Result<()> {
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
    let out = block(program, scope, runtime, depth)?;
    if matches!(out.control, Control::Break | Control::Continue) {
        return Err(Fault::new(
            "program_control",
            "Loop control used outside a loop",
        ));
    }
    out.value
        .ok_or_else(|| Fault::new("program_no_value", "Program produced no value"))
}

fn automatic_alias(name: &Symbol) -> Option<Symbol> {
    (name.label == "VARIABLE").then(|| Symbol {
        name: name.name.clone(),
        label: "*".into(),
    })
}

fn assign_local(scope: &mut Scope, name: &Symbol, value: Value) {
    if let Some(alias) = automatic_alias(name) {
        scope.bindings.insert(alias, Binding::Value(value.clone()));
    }
    scope.bindings.insert(name.clone(), Binding::Value(value));
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
                value: Some(scope.evaluate(expression, depth + 1)?),
            },
            Instruction::Assign { target, value } => {
                let value = scope.evaluate(value, depth + 1)?;
                if matches!(value, Value::Range { .. }) {
                    return Err(bad(
                        "Local range assignment requires vector materialization, not a local range binding",
                    ));
                }
                assign_local(scope, target, value.clone());
                Outcome {
                    control: Control::Normal,
                    value: Some(value),
                }
            }
            Instruction::Return(expression) => Outcome {
                control: Control::Return,
                value: Some(scope.evaluate(expression, depth + 1)?),
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
            Instruction::Assign { target, .. } => {
                names.insert(target.clone());
            }
            Instruction::For { variable, body, .. } => {
                names.insert(variable.clone());
                locals(body, names);
            }
            Instruction::While { body, .. } => locals(body, names),
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
    visitor: &mut impl FnMut(&Expression) -> Result<()>,
) -> Result<()> {
    for instruction in &program.instructions {
        match instruction {
            Instruction::Expression(e)
            | Instruction::Return(e)
            | Instruction::Assign { value: e, .. } => visitor(e)?,
            Instruction::For { iterable, body, .. } => {
                visitor(iterable)?;
                visit_expressions(body, visitor)?;
            }
            Instruction::While { condition, body } => {
                visitor(condition)?;
                visit_expressions(body, visitor)?;
            }
            Instruction::If {
                branches,
                otherwise,
                ..
            } => {
                for (test, body) in branches {
                    visitor(test)?;
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
    visit_expressions(program, &mut |expression| {
        scope.validate_names(expression, &bound, depth + 1)
    })
}
