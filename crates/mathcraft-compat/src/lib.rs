//! Independent execution of a bounded Mathcad MathML subset.
//!
//! Keeps identifier labels and expression trees; never translates through the
//! prototype text parser. Evidence: docs/analysis/0003 and tests/reference.
//! Top-level, already placed regions only. Grid snapping, nested scopes, exact
//! error arguments/handles and complete built-in/unit catalogs are not implemented.
use mathcraft_core::{Quantity, unit};
use num_complex::Complex64;
use roxmltree::Node;
use serde::Serialize;
use std::{collections::HashMap, sync::Arc};

pub mod arrays;
pub mod catalog;
pub mod editor_bridge;
pub mod numerics;
pub mod program;
pub mod reference;
pub mod scalars;
pub mod solve;
pub mod statistics;
pub mod symbolics;

const ML: &str = "http://schemas.mathsoft.com/math50";
const WS: &str = "http://schemas.mathsoft.com/worksheet50";
type Result<T> = std::result::Result<T, Fault>;

/// Original load-time placement, inferred from saved page settings and confirmed
/// by ApplyWorksheetData/AddItemAtLocation and AlignmentGrid IL. Ties snap down.
#[derive(Clone, Copy, Debug)]
pub struct PlacementGrid {
    step: f64,
}
impl PlacementGrid {
    pub fn from_presentation(xml: &str) -> std::result::Result<Self, String> {
        if xml.len() > 8 * 1024 * 1024 {
            return Err("Presentation XML exceeds limit".into());
        }
        let doc = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
        let page = doc
            .root_element()
            .children()
            .find(|n| {
                n.has_tag_name((
                    "http://schemas.ptc.com/mathcad/settings/presentation10",
                    "pageModel",
                ))
            })
            .ok_or("Missing pageModel")?;
        let standard = match page.attribute("paper-code") {
            Some("Letter" | "Legal" | "Ledger") => 0.2 * 96.0,
            Some("A1" | "A2" | "A3" | "A4" | "A5") => 0.5 * 96.0 / 2.54,
            _ => return Err("Unimplemented paper code; cannot infer placement grid".into()),
        };
        let step = match page.attribute("grid-size") {
            Some("Standard") => standard,
            Some("Fine") => standard * 0.5,
            _ => return Err("Unimplemented grid size".into()),
        };
        Ok(Self { step })
    }
    pub fn spacing(self) -> f64 {
        self.step
    }
    pub fn snap(self, value: f64) -> std::result::Result<f64, String> {
        if !value.is_finite() || value < 0. {
            return Err("Placement currently requires finite nonnegative coordinates".into());
        }
        let remainder = value % self.step;
        Ok(if remainder > self.step / 2. {
            value + (self.step - remainder)
        } else {
            value - remainder
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Symbol {
    pub name: String,
    pub label: String,
}

#[derive(Clone, Debug)]
pub enum Expression {
    Unsupported(Fault),
    Scalar(Quantity),
    Name(Symbol),
    Neg(Box<Self>),
    Binary(String, Box<Self>, Box<Self>),
    Root(Box<Self>, Box<Self>),
    Absolute(Box<Self>),
    Unary(String, Box<Self>),
    Program(Box<program::Program>),
    Logic(Box<program::Logic>),
    Numerical(numerics::NumericalExpression),
    Symbolic(Box<symbolics::SymbolicEvaluation>),
    Call(Symbol, Vec<Self>),
    Matrix {
        rows: usize,
        cols: usize,
        elements: Vec<Self>,
    },
    Index(Box<Self>, Vec<Self>),
    Range {
        start: Box<Self>,
        second: Option<Box<Self>>,
        end: Box<Self>,
    },
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Value {
    Symbolic {
        mathml: String,
    },
    Scalar {
        quantity: Quantity,
    },
    Matrix {
        rows: usize,
        cols: usize,
        /// Internal row-major order; native MathML XML uses column-major.
        elements: Vec<Quantity>,
    },
    Range {
        start: Quantity,
        step: Quantity,
        end: Quantity,
        count: usize,
    },
    /// Results of a top-level scalar expression iterated over one named range.
    /// Kept distinct from matrices, like Prime's unbracketed range output.
    RangeValues {
        elements: Vec<Quantity>,
    },
}
impl Value {
    fn scalar(quantity: Quantity) -> Self {
        Self::Scalar { quantity }
    }
    fn as_quantity(&self) -> Result<Quantity> {
        match self {
            Self::Scalar { quantity } => Ok(*quantity),
            _ => Err(Fault::unsupported("Expected a scalar value")),
        }
    }
    /// Numeric display payload, comparable with an original saved result.
    /// Physical dimensions remain separately available in `Value`.
    pub fn numeric_json(&self) -> serde_json::Value {
        fn number(q: &Quantity) -> serde_json::Value {
            serde_json::json!({"kind":"scalar", "real":q.value.re, "imag":q.value.im})
        }
        match self {
            Self::Symbolic { mathml } => serde_json::json!({"kind":"symbolic","mathml":mathml}),
            Self::Scalar { quantity } => number(quantity),
            Self::Matrix {
                rows,
                cols,
                elements,
            } => serde_json::json!({
                "kind":"matrix", "rows":rows, "cols":cols,
                "elements":(0..*cols).flat_map(|col|(0..*rows).map(move |row|number(&elements[row*cols+col]))).collect::<Vec<_>>()
            }),
            Self::Range {
                start,
                step,
                end,
                count,
            } => {
                serde_json::json!({"kind":"range","start":number(start),"step":number(step),"end":number(end),"count":count})
            }
            Self::RangeValues { elements } => {
                serde_json::json!({"kind":"range_values","elements":elements.iter().map(number).collect::<Vec<_>>()})
            }
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Fault {
    /// Original error family where established; not localized text or node IDs.
    pub code: String,
    pub detail: String,
    pub previous_region: Option<String>,
    pub unsupported: bool,
}
impl Fault {
    fn new(code: &str, detail: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            detail: detail.into(),
            previous_region: None,
            unsupported: false,
        }
    }
    fn unsupported(detail: impl Into<String>) -> Self {
        Self {
            unsupported: true,
            ..Self::new("unsupported", detail)
        }
    }
    fn unknown(symbol: &Symbol) -> Self {
        Self::new(
            "bad%_variable",
            format!("Undefined {} identifier {:?}", symbol.label, symbol.name),
        )
    }
}

#[derive(Clone, Debug)]
enum Statement {
    SolveBlock(solve::SolveBlock),
    Define {
        symbol: Symbol,
        parameters: Option<Vec<Symbol>>,
        body: Expression,
        global: bool,
        show_result: bool,
        display_unit: Option<Expression>,
    },
    Evaluate {
        expression: Expression,
        display_unit: Option<Expression>,
    },
}

#[derive(Clone, Debug, Serialize)]
pub struct RegionEvaluation {
    pub region_id: String,
    pub result_ref: Option<String>,
    pub left: f64,
    pub top: f64,
    pub value: Option<Value>,
    /// Unit-scaled display payload; `value` retains canonical physical quantity.
    pub display_value: Option<Value>,
    pub error: Option<Fault>,
    pub redefinition_warning: bool,
}

fn children<'a, 'input>(node: Node<'a, 'input>) -> Vec<Node<'a, 'input>> {
    node.children().filter(Node::is_element).collect()
}
fn is_math(node: Node<'_, '_>, name: &str) -> bool {
    node.has_tag_name((ML, name))
}
fn symbol(node: Node<'_, '_>) -> Result<Symbol> {
    if !is_math(node, "id") {
        return Err(Fault::unsupported("Expected labelled identifier"));
    }
    let (name, label, contextual, subscript) =
        editor_bridge::identifier_parts(node).map_err(Fault::unsupported)?;
    let name = editor_bridge::identifier_identity(&name, subscript.as_deref());
    // Prime persists resolved display labels separately from explicit choices.
    // Direct MathML has no contextual resolver; never force a stale cached label.
    let label = if contextual == Some(true) {
        None
    } else {
        label
    };
    let label = label
        .ok_or_else(|| Fault::unsupported("Automatic identifier labelling is not implemented"))?;
    Ok(Symbol { name, label })
}
fn parse_expression(node: Node<'_, '_>, depth: usize) -> Result<Expression> {
    if depth > 128 {
        return Err(Fault::unsupported("MathML nesting budget exceeded"));
    }
    if node.tag_name().namespace() != Some(ML) {
        return Err(Fault::unsupported("Foreign expression namespace"));
    }
    if is_math(node, "symEval") {
        return symbolics::parse(node, depth + 1).map(|s| Expression::Symbolic(Box::new(s)));
    }
    if !is_math(node, "id") {
        let allowed: &[&str] = match node.tag_name().name() {
            "imag" => &["symbol"],
            "matrix" => &["rows", "cols"],
            _ => &[],
        };
        check_attributes(node, allowed)?;
    }
    let items = children(node);
    let parse = |n| parse_expression(n, depth + 1);
    if let Some(expression) = numerics::parse_operator(node, depth + 1)? {
        return Ok(expression);
    }
    match node.tag_name().name() {
        "program" => {
            program::parse_program(node, depth + 1).map(|p| Expression::Program(Box::new(p)))
        }
        "real" | "imag" => {
            if !items.is_empty() || node.children().any(|n| !n.is_text()) {
                return Err(Fault::unsupported("Structured numeric literal"));
            }
            if is_math(node, "imag")
                && node
                    .attribute("symbol")
                    .is_some_and(|s| s != "i" && s != "j")
            {
                return Err(Fault::unsupported("Unknown imaginary symbol"));
            }
            let number: f64 = node
                .text()
                .unwrap_or("")
                .parse()
                .map_err(|_| Fault::unsupported("Invalid numeric literal"))?;
            if !number.is_finite() {
                return Err(Fault::unsupported("Nonfinite literal"));
            }
            let mut q = Quantity::scalar(number);
            if is_math(node, "imag") {
                q.value = Complex64::new(0., number);
            }
            Ok(Expression::Scalar(q))
        }
        "id" => Ok(Expression::Name(symbol(node)?)),
        "parens" if items.len() == 1 => parse(items[0]),
        "matrix" => {
            let rows: usize = node
                .attribute("rows")
                .unwrap_or("")
                .parse()
                .map_err(|_| Fault::unsupported("Invalid matrix rows"))?;
            let cols: usize = node
                .attribute("cols")
                .unwrap_or("")
                .parse()
                .map_err(|_| Fault::unsupported("Invalid matrix columns"))?;
            if rows.checked_mul(cols) != Some(items.len())
                || rows == 0
                || cols == 0
                || items.len() > 16384
            {
                return Err(Fault::unsupported("Invalid matrix size"));
            }
            let mut elements = Vec::with_capacity(items.len());
            for row in 0..rows {
                for col in 0..cols {
                    elements.push(parse(items[col * rows + row])?);
                }
            }
            Ok(Expression::Matrix {
                rows,
                cols,
                elements,
            })
        }
        "range" if items.len() == 2 => {
            let (start, second) = if is_math(items[0], "sequence") {
                check_attributes(items[0], &[])?;
                let pair = children(items[0]);
                if pair.len() != 2 {
                    return Err(Fault::unsupported("Range start/second structure"));
                }
                (parse(pair[0])?, Some(Box::new(parse(pair[1])?)))
            } else {
                (parse(items[0])?, None)
            };
            Ok(Expression::Range {
                start: Box::new(start),
                second,
                end: Box::new(parse(items[1])?),
            })
        }
        "apply" if !items.is_empty() => {
            let op = items[0];
            if is_math(op, "id") {
                let args = if items.len() == 2 && is_math(items[1], "sequence") {
                    check_attributes(items[1], &[])?;
                    children(items[1])
                } else {
                    items[1..].to_vec()
                };
                return Ok(Expression::Call(
                    symbol(op)?,
                    args.iter().copied().map(parse).collect::<Result<_>>()?,
                ));
            }
            if op.tag_name().namespace() != Some(ML) {
                return Err(Fault::unsupported("Foreign operator namespace"));
            }
            if matches!(
                op.tag_name().name(),
                "equal"
                    | "notEqual"
                    | "lessThan"
                    | "lessOrEqual"
                    | "greaterThan"
                    | "greaterOrEqual"
                    | "and"
                    | "or"
                    | "xor"
                    | "not"
            ) {
                return program::parse_logic(node, depth + 1)
                    .map(|l| Expression::Logic(Box::new(l)));
            }
            check_attributes(op, if is_math(op, "div") { &["inline"] } else { &[] })?;
            if !children(op).is_empty() {
                return Err(Fault::unsupported("Structured operator"));
            }
            match (op.tag_name().name(), &items[1..]) {
                ("neg", [a]) => Ok(Expression::Neg(Box::new(parse(*a)?))),
                ("absval", [a]) => Ok(Expression::Absolute(Box::new(parse(*a)?))),
                (
                    name @ ("transpose" | "conjugate" | "factorial" | "determinant" | "norm"),
                    [a],
                ) => Ok(Expression::Unary(name.into(), Box::new(parse(*a)?))),
                (name @ ("plus" | "minus" | "mult" | "div" | "pow" | "scale"), [a, b]) => Ok(
                    Expression::Binary(name.into(), Box::new(parse(*a)?), Box::new(parse(*b)?)),
                ),
                ("nthRoot", [degree, a]) => {
                    let degree = if is_math(*degree, "placeholder") {
                        Expression::Scalar(Quantity::scalar(2.))
                    } else {
                        parse(*degree)?
                    };
                    Ok(Expression::Root(Box::new(degree), Box::new(parse(*a)?)))
                }
                ("indexer", [a, i]) => {
                    let index = if is_math(*i, "sequence") {
                        check_attributes(*i, &[])?;
                        children(*i)
                    } else {
                        vec![*i]
                    };
                    if !(1..=2).contains(&index.len()) {
                        return Err(Fault::unsupported("Matrix index arity"));
                    }
                    Ok(Expression::Index(
                        Box::new(parse(*a)?),
                        index.into_iter().map(parse).collect::<Result<_>>()?,
                    ))
                }
                _ => Err(Fault::unsupported(format!(
                    "Operator/arity {} / {}",
                    op.tag_name().name(),
                    items.len() - 1
                ))),
            }
        }
        name => Err(Fault::unsupported(format!("MathML element {name}"))),
    }
}
fn parse_statement(node: Node<'_, '_>) -> Result<Statement> {
    if is_math(node, "symEval") {
        return Ok(Statement::Evaluate {
            expression: Expression::Symbolic(Box::new(symbolics::parse(node, 0)?)),
            display_unit: None,
        });
    }
    check_attributes(node, &[])?;
    let items = children(node);
    match (
        node.tag_name().namespace(),
        node.tag_name().name(),
        items.as_slice(),
    ) {
        (Some(ML), name @ ("define" | "globalDefine"), [lhs, rhs]) => {
            let (symbol, parameters, declaration_fault) = if is_math(*lhs, "function") {
                let parts = children(*lhs);
                let target = symbol(
                    *parts
                        .first()
                        .ok_or_else(|| Fault::unsupported("Missing function name"))?,
                )?;
                let parameters = (|| {
                    check_attributes(*lhs, &[])?;
                    match parts.as_slice() {
                        [name, bound] if is_math(*bound, "boundVars") => {
                            symbol(*name)?;
                            check_attributes(*bound, &[])?;
                            let parameters = children(*bound)
                                .into_iter()
                                .map(symbol)
                                .collect::<Result<Vec<_>>>()?;
                            if parameters.len() > 64 {
                                return Err(Fault::unsupported("Parameter budget exceeded"));
                            }
                            let mut unique = std::collections::HashSet::new();
                            if parameters.iter().any(|p| !unique.insert(p.clone())) {
                                return Err(Fault::unsupported("Duplicate parameters"));
                            }
                            Ok(parameters)
                        }
                        _ => Err(Fault::unsupported("Function declaration structure")),
                    }
                })();
                match parameters {
                    Ok(parameters) => (target, Some(parameters), None),
                    Err(fault) => (target, Some(vec![]), Some(fault)),
                }
            } else {
                (symbol(*lhs)?, None, None)
            };
            let (body, display_unit, show_result) = if is_math(*rhs, "eval") {
                let parts = children(*rhs);
                match parts.as_slice() {
                    [expression, unit] if is_math(*unit, "unitOverride") => (
                        parse_expression(*expression, 0).unwrap_or_else(Expression::Unsupported),
                        parse_display_unit(*unit)
                            .unwrap_or_else(|fault| Some(Expression::Unsupported(fault))),
                        true,
                    ),
                    _ => (
                        Expression::Unsupported(Fault::unsupported(
                            "Definition evaluation structure",
                        )),
                        None,
                        false,
                    ),
                }
            } else {
                (
                    parse_expression(*rhs, 0).unwrap_or_else(Expression::Unsupported),
                    None,
                    false,
                )
            };
            Ok(Statement::Define {
                symbol,
                parameters,
                // Retain the declaration even if its RHS is unsupported, so it
                // poisons dependent reads instead of exposing an earlier value.
                body: declaration_fault
                    .or_else(|| {
                        if is_math(*rhs, "eval") {
                            check_attributes(*rhs, &[]).err()
                        } else {
                            None
                        }
                    })
                    .map(Expression::Unsupported)
                    .unwrap_or(body),
                global: name == "globalDefine",
                show_result,
                display_unit,
            })
        }
        (Some(ML), "eval", [expression, unit]) if is_math(*unit, "unitOverride") => {
            let display_unit = parse_display_unit(*unit)?;
            Ok(Statement::Evaluate {
                expression: parse_expression(*expression, 0)?,
                display_unit,
            })
        }
        _ => Err(Fault::unsupported(format!(
            "Statement {}",
            node.tag_name().name()
        ))),
    }
}

fn parse_display_unit(node: Node<'_, '_>) -> Result<Option<Expression>> {
    check_attributes(node, &[])?;
    match children(node).as_slice() {
        [p] if is_math(*p, "placeholder") => Ok(None),
        [p] => Ok(Some(parse_expression(*p, 0)?)),
        _ => Err(Fault::unsupported("Display unit structure")),
    }
}
fn check_attributes(node: Node<'_, '_>, allowed: &[&str]) -> Result<()> {
    if node
        .attributes()
        .any(|a| a.namespace().is_some() || !allowed.contains(&a.name()))
    {
        return Err(Fault::unsupported(format!(
            "Unknown {} metadata",
            node.tag_name().name()
        )));
    }
    Ok(())
}

#[derive(Clone)]
enum Binding {
    Value(Value),
    Function {
        parameters: Vec<Symbol>,
        body: Expression,
        captured: Arc<Scope>,
    },
    Failed {
        fault: Fault,
        region: String,
    },
}
#[derive(Clone, Default)]
struct Scope {
    bindings: HashMap<Symbol, Binding>,
}
fn builtin(name: &Symbol) -> bool {
    name.label == "FUNCTION"
        && (scalars::is_builtin(&name.name)
            || arrays::is_builtin(&name.name)
            || statistics::is_builtin(&name.name)
            || numerics::is_builtin(&name.name))
}
impl Scope {
    fn display(&self, value: Value, unit: &Option<Expression>) -> Result<(Value, Option<Value>)> {
        let Some(unit) = unit else {
            return Ok((value, None));
        };
        let unit = self.evaluate(unit, 0)?.as_quantity()?;
        if unit.value.norm() == 0. {
            return Err(Fault::new("divide_by_zero", "Display unit is zero"));
        }
        let convert = |quantity: Quantity| -> Result<Quantity> {
            if !quantity.same_dimension(&unit) {
                return Err(Fault::new(
                    "bad%_dimensions",
                    "Display unit has incompatible dimensions",
                ));
            }
            let value = quantity.value / unit.value;
            if !value.re.is_finite() || !value.im.is_finite() {
                return Err(Fault::unsupported("Nonfinite display result"));
            }
            Ok(Quantity {
                value,
                dimension: [0.; 7],
            })
        };
        let display = match &value {
            Value::Symbolic { .. } => {
                return Err(Fault::unsupported(
                    "Symbolic results cannot have a numeric display unit",
                ));
            }
            Value::Scalar { quantity } => Value::scalar(convert(*quantity)?),
            Value::Matrix {
                rows,
                cols,
                elements,
            } => Value::Matrix {
                rows: *rows,
                cols: *cols,
                elements: elements
                    .iter()
                    .copied()
                    .map(convert)
                    .collect::<Result<_>>()?,
            },
            Value::Range {
                start,
                step,
                end,
                count,
            } => Value::Range {
                start: convert(*start)?,
                step: convert(*step)?,
                end: convert(*end)?,
                count: *count,
            },
            Value::RangeValues { elements } => Value::RangeValues {
                elements: elements
                    .iter()
                    .copied()
                    .map(convert)
                    .collect::<Result<_>>()?,
            },
        };
        Ok((value, Some(display)))
    }

    fn evaluate_output(&self, expression: &Expression) -> Result<Value> {
        // Evaluation of a bare range variable keeps the range descriptor.
        if matches!(expression, Expression::Name(_)) {
            return self.evaluate(expression, 0);
        }
        let mut names = std::collections::HashSet::new();
        self.collect_range_names(expression, &mut names, 0)?;
        if names.is_empty() {
            return self.evaluate(expression, 0);
        }
        if names.len() != 1 {
            return Err(Fault::unsupported(
                "Iteration over multiple ranges requires an established ordering contract",
            ));
        }
        let name = names.into_iter().next().unwrap();
        let Binding::Value(Value::Range {
            start, step, count, ..
        }) = self.lookup(&name)?
        else {
            unreachable!()
        };
        let mut elements = Vec::with_capacity(count);
        let mut scope = self.clone();
        for index in 0..count {
            let mut point = start;
            point.value.re = start.value.re + step.value.re * index as f64;
            scope
                .bindings
                .insert(name.clone(), Binding::Value(Value::scalar(point)));
            elements.push(scope.evaluate(expression, 0)?.as_quantity()?);
        }
        Ok(Value::RangeValues { elements })
    }

    fn collect_range_names(
        &self,
        expression: &Expression,
        names: &mut std::collections::HashSet<Symbol>,
        depth: usize,
    ) -> Result<()> {
        if depth > 128 {
            return Err(Fault::unsupported("Range dependency depth exceeded"));
        }
        let mut visit = |e| self.collect_range_names(e, names, depth + 1);
        match expression {
            Expression::Symbolic(_) => Ok(()),
            Expression::Name(name) => {
                if matches!(self.lookup(name), Ok(Binding::Value(Value::Range { .. }))) {
                    names.insert(name.clone());
                }
                Ok(())
            }
            Expression::Neg(a) | Expression::Absolute(a) | Expression::Unary(_, a) => visit(a),
            Expression::Program(_) => Ok(()),
            Expression::Numerical(n) => numerics::collect_range_names(self, n, names, depth + 1),
            Expression::Logic(l) => {
                for a in &l.arguments {
                    visit(a)?;
                }
                Ok(())
            }
            Expression::Binary(_, a, b) | Expression::Root(a, b) => {
                visit(a)?;
                visit(b)
            }
            Expression::Index(base, indices) => {
                visit(base)?;
                for i in indices {
                    visit(i)?;
                }
                Ok(())
            }
            Expression::Call(_, arguments)
            | Expression::Matrix {
                elements: arguments,
                ..
            } => {
                for a in arguments {
                    visit(a)?;
                }
                Ok(())
            }
            Expression::Range { start, second, end } => {
                visit(start)?;
                if let Some(second) = second {
                    visit(second)?;
                }
                visit(end)
            }
            Expression::Unsupported(_) | Expression::Scalar(_) => Ok(()),
        }
    }

    fn definition_result(
        &self,
        symbol: &Symbol,
        unit: &Option<Expression>,
    ) -> Result<(Value, Option<Value>)> {
        match self.lookup(symbol)? {
            Binding::Value(value) => self.display(value, unit),
            _ => Err(Fault::unsupported(
                "Function definitions do not have a scalar result",
            )),
        }
    }
    fn lookup(&self, name: &Symbol) -> Result<Binding> {
        if let Some(binding) = self.bindings.get(name) {
            return match binding {
                Binding::Failed { fault, region } => Err(Fault {
                    previous_region: Some(region.clone()),
                    ..fault.clone()
                }),
                _ => Ok(binding.clone()),
            };
        }
        let quantity = match (name.label.as_str(), name.name.as_str()) {
            ("UNIT", text) => unit(text),
            ("CONSTANT", "π") => Some(Quantity::scalar(std::f64::consts::PI)),
            ("CONSTANT", "e") => Some(Quantity::scalar(std::f64::consts::E)),
            ("CONSTANT", "γ") => Some(Quantity::scalar(std::f64::consts::EULER_GAMMA)),
            ("CONSTANT", "%") => Some(Quantity::scalar(0.01)),
            ("CONSTANT", text) => mathcraft_core::units::constant_lookup(text),
            ("SYSTEM", "ORIGIN") | ("VARIABLE", "ORIGIN") => Some(Quantity::scalar(0.)),
            ("SYSTEM", "TOL" | "CTOL") => Some(Quantity::scalar(0.001)),
            ("SYSTEM", "PRNPRECISION") => Some(Quantity::scalar(4.)),
            ("SYSTEM", "PRNCOLWIDTH") => Some(Quantity::scalar(8.)),
            _ => None,
        };
        quantity
            .map(|q| Binding::Value(Value::scalar(q)))
            .ok_or_else(|| Fault::unknown(name))
    }
    fn validate_names(&self, e: &Expression, parameters: &[Symbol], depth: usize) -> Result<()> {
        if depth > 128 {
            return Err(Fault::unsupported("Expression depth exceeded"));
        }
        match e {
            Expression::Symbolic(_) => Ok(()),
            Expression::Unsupported(fault) => Err(fault.clone()),
            Expression::Scalar(_) => Ok(()),
            Expression::Name(s) if parameters.contains(s) => Ok(()),
            Expression::Name(s) => self.lookup(s).map(|_| ()),
            Expression::Neg(a) | Expression::Absolute(a) | Expression::Unary(_, a) => {
                self.validate_names(a, parameters, depth + 1)
            }
            Expression::Program(p) => program::validate_names(p, self, parameters, depth + 1),
            Expression::Numerical(n) => numerics::validate_names(self, n, parameters, depth + 1),
            Expression::Logic(l) => {
                for a in &l.arguments {
                    self.validate_names(a, parameters, depth + 1)?;
                }
                Ok(())
            }
            Expression::Binary(_, a, b) | Expression::Root(a, b) => {
                self.validate_names(a, parameters, depth + 1)?;
                self.validate_names(b, parameters, depth + 1)
            }
            Expression::Index(base, indices) => {
                self.validate_names(base, parameters, depth + 1)?;
                for i in indices {
                    self.validate_names(i, parameters, depth + 1)?;
                }
                Ok(())
            }
            Expression::Range { start, second, end } => {
                self.validate_names(start, parameters, depth + 1)?;
                if let Some(second) = second {
                    self.validate_names(second, parameters, depth + 1)?;
                }
                self.validate_names(end, parameters, depth + 1)
            }
            Expression::Call(s, args) => {
                if !parameters.contains(s) && (self.bindings.contains_key(s) || !builtin(s)) {
                    self.lookup(s)?;
                }
                for a in args {
                    self.validate_names(a, parameters, depth + 1)?;
                }
                Ok(())
            }
            Expression::Matrix { elements, .. } => {
                for a in elements {
                    self.validate_names(a, parameters, depth + 1)?;
                }
                Ok(())
            }
        }
    }
    fn evaluate(&self, e: &Expression, depth: usize) -> Result<Value> {
        if depth > 128 {
            return Err(Fault::unsupported("Evaluation depth exceeded"));
        }
        let eval = |a| self.evaluate(a, depth + 1);
        let scalar = |a| eval(a)?.as_quantity();
        let core_error = |error: String| {
            let code = if error.contains("zero") {
                "divide_by_zero"
            } else if error.contains("dimension") {
                "bad%_dimensions"
            } else {
                "unsupported"
            };
            if code == "unsupported" {
                Fault::unsupported(error)
            } else {
                Fault::new(code, error)
            }
        };
        match e {
            Expression::Symbolic(s) => symbolics::evaluate(s, self),
            Expression::Program(p) => program::evaluate_in_scope(p, self, depth + 1),
            Expression::Logic(l) => program::evaluate_logic(l, self, depth + 1),
            Expression::Numerical(n) => numerics::evaluate(self, n, depth + 1),
            Expression::Unsupported(fault) => Err(fault.clone()),
            Expression::Scalar(q) => Ok(Value::scalar(*q)),
            Expression::Name(name) => match self.lookup(name)? {
                Binding::Value(v) => Ok(v),
                _ => Err(Fault::unsupported("Function used as a scalar")),
            },
            Expression::Neg(a) => arrays::unary("neg", &eval(a)?),
            Expression::Absolute(a) => arrays::unary("absval", &eval(a)?),
            Expression::Unary(op, a) => {
                if op == "factorial" {
                    scalars::factorial(&eval(a)?)
                } else {
                    arrays::unary(op, &eval(a)?)
                }
            }
            Expression::Binary(op, a, b) => arrays::binary(op, &eval(a)?, &eval(b)?),
            Expression::Root(degree, a) => {
                let d = scalar(degree)?;
                let exponent = Quantity::scalar(1.).multiply(d, true).map_err(core_error)?;
                Ok(Value::scalar(
                    scalar(a)?.power(exponent).map_err(core_error)?,
                ))
            }
            Expression::Matrix {
                rows,
                cols,
                elements,
            } => Ok(Value::Matrix {
                rows: *rows,
                cols: *cols,
                elements: elements.iter().map(scalar).collect::<Result<_>>()?,
            }),
            Expression::Range { start, second, end } => {
                let start = scalar(start)?;
                let end = scalar(end)?;
                if start.value.im != 0. || end.value.im != 0. {
                    return Err(Fault::unsupported("Range bounds must be real"));
                }
                if !start.same_dimension(&end) {
                    return Err(Fault::new(
                        "bad%_dimensions",
                        "Range bounds have incompatible units",
                    ));
                }
                let step = if let Some(second) = second {
                    let second = scalar(second)?;
                    if second.value.im != 0. {
                        return Err(Fault::unsupported("Range second value must be real"));
                    }
                    second.add(start, true).map_err(core_error)?
                } else {
                    if !start.is_scalar() {
                        return Err(Fault::new(
                            "bad%_dimensions%_unitless",
                            "A range with units needs an explicit second value",
                        ));
                    }
                    Quantity::scalar(1.)
                };
                if step.value.re == 0. {
                    return Err(Fault::unsupported("Range step is zero"));
                }
                let steps = (end.value.re - start.value.re) / step.value.re;
                if !steps.is_finite() || steps < 0. || steps > 16383. {
                    return Err(Fault::unsupported(
                        "Range direction or cardinality outside implemented subset",
                    ));
                }
                // Independent finite-range policy; account for representation
                // error at a mathematically integral floating-point endpoint.
                let rounded = steps.round();
                let steps = if (rounded - steps).abs() <= 8. * f64::EPSILON * steps.abs().max(1.) {
                    rounded
                } else {
                    steps.floor()
                };
                Ok(Value::Range {
                    start,
                    step,
                    end,
                    count: steps as usize + 1,
                })
            }
            Expression::Index(a, indices) => {
                let matrix = eval(a)?;
                let origin_symbol = Symbol {
                    name: "ORIGIN".into(),
                    label: "SYSTEM".into(),
                };
                let origin = match self.lookup(&origin_symbol)? {
                    Binding::Value(v) => v.as_quantity()?,
                    _ => return Err(Fault::unsupported("Non-scalar SYSTEM ORIGIN")),
                };
                if !origin.is_scalar() || origin.value.im != 0. || origin.value.re.fract() != 0. {
                    return Err(Fault::unsupported(
                        "SYSTEM ORIGIN must be a real, dimensionless integer",
                    ));
                }
                let mut offsets = Vec::new();
                for i in indices {
                    let index = scalar(i)?;
                    if !index.is_scalar() || index.value.im != 0. || index.value.re.fract() != 0. {
                        return Err(Fault::unsupported("Non-integer/unit-bearing matrix index"));
                    }
                    let offset = index.value.re - origin.value.re;
                    if offset < 0. || offset > 16383. {
                        return Err(Fault::unsupported("Matrix index outside implemented range"));
                    }
                    offsets.push(offset as usize);
                }
                match matrix {
                    Value::Matrix {
                        rows,
                        cols,
                        elements,
                    } => {
                        let (r, c) = match offsets.as_slice() {
                            [i] if cols == 1 => (*i, 0),
                            [r, c] => (*r, *c),
                            _ => {
                                return Err(Fault::unsupported(
                                    "A general matrix needs row and column indices",
                                ));
                            }
                        };
                        if r >= rows || c >= cols {
                            return Err(Fault::unsupported(
                                "Matrix index outside implemented range",
                            ));
                        }
                        Ok(Value::scalar(elements[r * cols + c]))
                    }
                    _ => Err(Fault::unsupported("Matrix index outside implemented range")),
                }
            }
            Expression::Call(name, arguments) => {
                let args = arguments.iter().map(eval).collect::<Result<Vec<_>>>()?;
                // An explicit user definition takes priority over a built-in with
                // the SAME labelled identity; other label classes stay distinct.
                if self.bindings.contains_key(name) {
                    let binding = self.lookup(name)?;
                    if let Binding::Function {
                        parameters,
                        body,
                        captured,
                    } = binding
                    {
                        if args.len() != parameters.len() {
                            return Err(Fault::unsupported("Function argument count"));
                        }
                        let mut call = (*captured).clone();
                        for (parameter, value) in parameters.into_iter().zip(args) {
                            call.bindings.insert(parameter, Binding::Value(value));
                        }
                        return call.evaluate(&body, depth + 1);
                    }
                    return Err(Fault::unsupported("Calling a non-function"));
                }
                if !builtin(name) {
                    return Err(Fault::unknown(name));
                }
                if scalars::is_builtin(&name.name) {
                    return scalars::call(&name.name, &args);
                }
                if statistics::is_builtin(&name.name) {
                    return statistics::call(&name.name, &args);
                }
                if numerics::is_builtin(&name.name) {
                    return numerics::call(&name.name, &args);
                }
                let origin = if matches!(name.name.as_str(), "last" | "submatrix") {
                    match self.lookup(&Symbol {
                        name: "ORIGIN".into(),
                        label: "SYSTEM".into(),
                    })? {
                        Binding::Value(value) => value.as_quantity()?,
                        _ => return Err(Fault::unsupported("Non-scalar SYSTEM ORIGIN")),
                    }
                } else {
                    Quantity::scalar(0.)
                };
                arrays::call(&name.name, &args, origin)
            }
        }
    }
    fn define(
        &mut self,
        symbol: &Symbol,
        parameters: &Option<Vec<Symbol>>,
        body: &Expression,
        region: &str,
    ) -> Result<()> {
        let binding = if let Some(parameters) = parameters {
            self.validate_names(body, parameters, 0)
                .map(|()| Binding::Function {
                    parameters: parameters.clone(),
                    body: body.clone(),
                    captured: Arc::new(self.clone()),
                })
        } else {
            self.evaluate(body, 0).and_then(|value| {
                if matches!(value,Value::Range{..}) && !matches!(body,Expression::Range{..}) {
                    Err(Fault::unsupported("A range is not a scalar; ordinary aliases of range variables are not implemented"))
                } else {Ok(Binding::Value(value))}
            })
        };
        match binding {
            Ok(binding) => {
                self.bindings.insert(symbol.clone(), binding);
                Ok(())
            }
            Err(fault) => {
                self.bindings.insert(
                    symbol.clone(),
                    Binding::Failed {
                        fault: fault.clone(),
                        region: region.into(),
                    },
                );
                Err(fault)
            }
        }
    }
}

/// Evaluate already positioned, top-level math regions, in geometric order.
/// Import does not execute scripts, XAML, external relationships, or the original engine.
pub fn evaluate_worksheet(xml: &str) -> std::result::Result<Vec<RegionEvaluation>, String> {
    evaluate_worksheet_with_grid(xml, None)
}

/// Supply the document's presentation grid to reproduce Prime's load-time snap.
pub fn evaluate_worksheet_with_grid(
    xml: &str,
    grid: Option<PlacementGrid>,
) -> std::result::Result<Vec<RegionEvaluation>, String> {
    if xml.len() > 8 * 1024 * 1024 {
        return Err("Worksheet XML exceeds 8 MiB".into());
    }
    let doc = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
    if !doc.root_element().has_tag_name((WS, "worksheet")) {
        return Err("Expected worksheet50 root".into());
    }
    let regions = doc
        .root_element()
        .children()
        .find(|n| n.has_tag_name((WS, "regions")))
        .ok_or("Missing regions")?;
    let mut inputs = Vec::new();
    for region in regions.children().filter(Node::is_element) {
        let content = children(region);
        if !content.is_empty()
            && content
                .iter()
                .all(|n| n.has_tag_name((WS, "text")) || n.has_tag_name((WS, "image")))
        {
            continue;
        }
        if inputs.len() >= 10000 {
            return Err("Region budget exceeded".into());
        }
        let coordinate = |name| -> std::result::Result<f64, String> {
            let value: f64 = region
                .attribute(name)
                .ok_or_else(|| format!("Missing {name}"))?
                .parse()
                .map_err(|_| format!("Invalid {name}"))?;
            if !value.is_finite() {
                return Err(format!("Nonfinite {name}"));
            }
            if let Some(grid) = grid {
                grid.snap(value)
            } else {
                Ok(value)
            }
        };
        let math = region.children().find(|n| n.has_tag_name((WS, "math")));
        let statement = if let Some(math) = math {
            let all = children(math);
            let c: Vec<_> = all
                .iter()
                .copied()
                .filter(|n| n.tag_name().namespace() == Some(ML))
                .collect();
            if math
                .attributes()
                .any(|a| a.namespace().is_some() || a.name() != "resultRef")
            {
                Err(Fault::unsupported("Math region execution metadata"))
            } else if all.iter().any(|n| {
                n.tag_name().namespace() != Some(ML)
                    && !n.has_tag_name((WS, "resultFormat"))
                    && !n.has_tag_name((WS, "formattingOverride"))
            }) {
                Err(Fault::unsupported("Unknown math region sibling"))
            } else if c.len() == 1 {
                parse_statement(c[0])
            } else {
                Err(Fault::unsupported("Math region has multiple roots"))
            }
        } else if content.len() == 1 && content[0].has_tag_name((WS, "solveblock")) {
            solve::parse_block(content[0], grid).map(Statement::SolveBlock)
        } else {
            Err(Fault::unsupported(
                "Non-math/nested region requires document model",
            ))
        };
        inputs.push((
            RegionEvaluation {
                region_id: region.attribute("region-id").unwrap_or("").into(),
                result_ref: math
                    .and_then(|n| n.attribute("resultRef"))
                    .map(str::to_string)
                    .or_else(|| match &statement {
                        Ok(Statement::SolveBlock(block)) => block.solver_result_ref(),
                        _ => None,
                    }),
                left: coordinate("left")?,
                top: coordinate("top")?,
                value: None,
                display_value: None,
                error: None,
                redefinition_warning: false,
            },
            statement,
        ));
    }
    // Exact overlap uses stable input order as an explicit independent policy.
    // Original tie behavior is not established beyond the one retained probe.
    inputs.sort_by(|a, b| {
        a.0.top
            .total_cmp(&b.0.top)
            .then(a.0.left.total_cmp(&b.0.left))
    });
    let mut globals = Scope::default();
    let mut global_symbols = std::collections::HashSet::new();
    let mut global_counts = HashMap::<Symbol, usize>::new();
    for (_, statement) in &inputs {
        if let Ok(Statement::Define {
            symbol,
            global: true,
            ..
        }) = statement
        {
            *global_counts.entry(symbol.clone()).or_default() += 1;
        }
    }
    for (row, statement) in &mut inputs {
        if let Ok(Statement::Define {
            symbol,
            parameters,
            body,
            global: true,
            show_result,
            display_unit,
        }) = statement
        {
            global_symbols.insert(symbol.clone());
            if global_counts[symbol] > 1 {
                let fault = Fault::unsupported(
                    "Duplicate global definitions need exact conflict diagnostics",
                );
                globals.bindings.insert(
                    symbol.clone(),
                    Binding::Failed {
                        fault: fault.clone(),
                        region: row.region_id.clone(),
                    },
                );
                row.error = Some(fault);
            } else {
                row.error = globals
                    .define(symbol, parameters, body, &row.region_id)
                    .err();
                if row.error.is_none() && *show_result {
                    match globals.definition_result(symbol, display_unit) {
                        Ok((v, d)) => {
                            row.value = Some(v);
                            row.display_value = d;
                        }
                        Err(e) => row.error = Some(e),
                    }
                }
            }
        }
    }
    let mut scope = globals;
    for (row, statement) in &mut inputs {
        let statement = match statement {
            Ok(s) => s,
            Err(fault) => {
                row.error = Some(fault.clone());
                continue;
            }
        };
        match statement {
            Statement::SolveBlock(block) => {
                let outputs = block.exports();
                if outputs.iter().any(|symbol| global_symbols.contains(symbol)) {
                    row.error = Some(Fault::new(
                        "normal_and_global_definitions",
                        "Solve result identifier is already globally defined",
                    ));
                    continue;
                }
                let redefinition = outputs
                    .iter()
                    .any(|symbol| scope.bindings.contains_key(symbol));
                match solve::evaluate(&mut scope, block, &row.region_id) {
                    Ok((value, display)) => {
                        row.value = value;
                        row.display_value = display;
                        row.redefinition_warning = redefinition;
                    }
                    Err(error) => row.error = Some(error),
                }
            }
            Statement::Define { global: true, .. } => (),
            Statement::Define {
                symbol,
                parameters,
                body,
                show_result,
                display_unit,
                ..
            } => {
                if global_symbols.contains(symbol) {
                    row.error = Some(Fault::new(
                        "normal_and_global_definitions",
                        "Identifier is already globally defined",
                    ));
                    continue;
                }
                let redefinition = scope.bindings.contains_key(symbol);
                row.error = scope.define(symbol, parameters, body, &row.region_id).err();
                row.redefinition_warning = redefinition && row.error.is_none();
                if row.error.is_none() && *show_result {
                    match scope.definition_result(symbol, display_unit) {
                        Ok((v, d)) => {
                            row.value = Some(v);
                            row.display_value = d;
                        }
                        Err(e) => row.error = Some(e),
                    }
                }
            }
            Statement::Evaluate {
                expression,
                display_unit,
            } => {
                let result = scope
                    .evaluate_output(expression)
                    .and_then(|value| scope.display(value, display_unit));
                match result {
                    Ok((value, display)) => {
                        row.value = Some(value);
                        row.display_value = display;
                    }
                    Err(error) => row.error = Some(error),
                }
            }
        }
    }
    Ok(inputs.into_iter().map(|(row, _)| row).collect())
}
