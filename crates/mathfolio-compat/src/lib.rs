//! Independent execution of a bounded Mathcad MathML subset.
//!
//! Keeps identifier labels and expression trees; never translates through the
//! prototype text parser. Evidence: docs/analysis/0003 and tests/reference.
//! Positioned worksheet regions and bounded Area child blocks. General nested
//! scopes, exact error arguments/handles and complete catalogs remain incomplete.
use mathfolio_core::{Quantity, unit};
use num_complex::Complex64;
use roxmltree::Node;
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

mod area_calculation;
#[cfg(test)]
mod area_legacy_tests;
pub mod arrays;
#[cfg(test)]
mod cancellation_tests;
pub mod catalog;
pub mod editor_bridge;
#[cfg(test)]
mod indexed_definition_tests;
mod matrix_origin;
pub mod numerics;
mod plot_query;
pub mod program;
pub use plot_query::PreparedPlotQuery;
pub mod radix_integral;
#[cfg(test)]
mod radix_tests;
pub mod reference;
pub mod scalars;
pub mod solve;
pub mod solve_builder;
mod source_matrix_display;
mod spec_table;
pub mod statistics;
#[cfg(test)]
mod string_tests;
mod string_value;
pub use string_value::StringValue;
pub mod strings;
pub mod symbolics;

const ML: &str = "http://schemas.mathsoft.com/math50";
const WS: &str = "http://schemas.mathsoft.com/worksheet50";
type Result<T> = std::result::Result<T, Fault>;

/// A cooperative, per-job stop request. A fresh control should be used for each
/// background job; stopping never becomes an ordinary worksheet error.
#[derive(Clone, Debug, Default)]
pub struct CalculationControl {
    inner: Arc<ControlState>,
}
#[derive(Debug, Default)]
struct ControlState {
    cancelled: AtomicBool,
    #[cfg(test)]
    remaining_checks: std::sync::atomic::AtomicUsize,
    #[cfg(test)]
    check_budget_enabled: AtomicBool,
}
thread_local! {
    static ACTIVE_CONTROL: RefCell<Option<CalculationControl>> = const {RefCell::new(None)};
}
impl CalculationControl {
    pub fn cancel(&self) {
        self.inner.cancelled.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.inner.cancelled.load(Ordering::Acquire)
    }
    /// Apply this job's control to calculation and temporary plot queries on the
    /// current worker thread. Nested calls restore the previous control on exit,
    /// including unwinding, so stored function environments never retain tokens.
    pub fn run<T>(&self, operation: impl FnOnce() -> T) -> T {
        struct Guard(Option<CalculationControl>);
        impl Drop for Guard {
            fn drop(&mut self) {
                ACTIVE_CONTROL.with(|active| {
                    active.replace(self.0.take());
                });
            }
        }
        let _guard = Guard(ACTIVE_CONTROL.with(|active| active.replace(Some(self.clone()))));
        operation()
    }
    #[cfg(test)]
    fn stop_after_checks(checks: usize) -> Self {
        let control = Self::default();
        control
            .inner
            .remaining_checks
            .store(checks, Ordering::Relaxed);
        control
            .inner
            .check_budget_enabled
            .store(true, Ordering::Relaxed);
        control
    }
}

fn cancellation_requested() -> bool {
    ACTIVE_CONTROL.with(|active| {
        active
            .borrow()
            .as_ref()
            .is_some_and(CalculationControl::is_cancelled)
    })
}

fn check_cancelled() -> Result<()> {
    ACTIVE_CONTROL.with(|active| {
        let active = active.borrow();
        let Some(control) = active.as_ref() else {
            return Ok(());
        };
        #[cfg(test)]
        if control.inner.check_budget_enabled.load(Ordering::Relaxed)
            && control
                .inner
                .remaining_checks
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_sub(1))
                .is_err()
        {
            control.cancel();
        }
        if control.is_cancelled() {
            Err(Fault::new("CANCELLED", "Calculation stopped"))
        } else {
            Ok(())
        }
    })
}

/// Original load-time placement, inferred from saved page settings and confirmed
/// by ApplyWorksheetData/AddItemAtLocation and AlignmentGrid IL. Ties snap down.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(target_arch = "wasm32", derive(Serialize, Deserialize))]
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
    StringLiteral(String),
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Value {
    String {
        value: StringValue,
    },
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
    pub fn as_string(&self) -> Result<&str> {
        match self {
            Self::String { value } => {
                strings::validate(value)?;
                Ok(value)
            }
            _ => Err(Fault::new("invalid_argument", "must be string")),
        }
    }
    /// Numeric display payload, comparable with an original saved result.
    /// Physical dimensions remain separately available in `Value`.
    pub fn numeric_json(&self) -> serde_json::Value {
        fn number(q: &Quantity) -> serde_json::Value {
            serde_json::json!({"kind":"scalar", "real":q.value.re, "imag":q.value.im})
        }
        match self {
            Self::String { value } => serde_json::json!({"kind":"string","value":value}),
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
#[cfg_attr(target_arch = "wasm32", derive(Deserialize))]
pub struct Fault {
    /// Original error family where established; not localized text or node IDs.
    pub code: String,
    pub detail: String,
    pub previous_region: Option<String>,
    pub unsupported: bool,
}
impl Fault {
    pub fn is_cancelled(&self) -> bool {
        self.code == "CANCELLED"
    }
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
    SpecTable(spec_table::SpecTable),
    Define {
        symbol: Symbol,
        parameters: Option<Vec<Symbol>>,
        body: Expression,
        global: bool,
        show_result: bool,
        display_unit: Option<Expression>,
    },
    IndexedDefine {
        symbol: Symbol,
        indices: Vec<Expression>,
        slice: Option<&'static str>,
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
#[cfg_attr(target_arch = "wasm32", derive(Deserialize))]
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
    /// Result-associated metadata captured from an already executing Rust scope.
    /// Unknown/cold/unexecuted/pending/error publications never invent an origin.
    /// This is neither matrix data nor a native MatrixFormat/source attribute.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matrix_origin: Option<i32>,
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
    let label = editor_bridge::explicit_identifier_label(label.as_deref(), contextual)
        .ok_or_else(|| Fault::unsupported("Automatic identifier labelling is not implemented"))?;
    Ok(Symbol {
        name,
        label: label.into(),
    })
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
    if is_math(node, "str") {
        return strings::parse_literal(node).map(Expression::StringLiteral);
    }
    if !is_math(node, "id") {
        let allowed: &[&str] = match node.tag_name().name() {
            "real" => &["base"],
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
        "tryCatch" => {
            program::parse_try_expression(node, depth + 1).map(|p| Expression::Program(Box::new(p)))
        }
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
            let digits = node.text().unwrap_or("");
            // Native base10/absent keep the established decimal contract. This
            // bounded decoder never changes the authoritative worksheet XML.
            let number: f64 = match node.attribute("base") {
                None | Some("10") => digits
                    .parse()
                    .map_err(|_| Fault::unsupported("Invalid numeric literal"))?,
                Some(base @ ("2" | "8" | "16")) => {
                    let base = match base {
                        "2" => 2,
                        "8" => 8,
                        "16" => 16,
                        _ => unreachable!(),
                    };
                    radix_integral::decode_unsigned_integral(digits, base)
                        .map_err(|e| Fault::unsupported(e.to_string()))?
                        .exact_f64()
                }
                Some(_) => return Err(Fault::unsupported("Unsupported numeric base metadata")),
            };
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
            // Probe only the first child on the ordinary legacy path. A later
            // display (including a foreign mimic) is still rejected by the
            // existing full-cell parse below; it can never be skipped as data.
            let source = if items
                .first()
                .is_some_and(|n| n.tag_name().name() == "display")
            {
                Some(
                    source_matrix_display::parse(node, rows, cols, &items)
                        .map_err(Fault::unsupported)?,
                )
            } else {
                None
            };
            let items = source.as_ref().map_or(items.as_slice(), |s| s.cells);
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
                    name @ ("transpose" | "conjugate" | "factorial" | "determinant" | "vectorize"
                    | "percent" | "degree_op"),
                    [a],
                ) => Ok(Expression::Unary(name.into(), Box::new(parse(*a)?))),
                (
                    name @ ("plus" | "minus" | "mult" | "div" | "pow" | "scale" | "matcol"
                    | "matrow" | "crossProduct" | "polar"),
                    [a, b],
                ) => Ok(Expression::Binary(
                    name.into(),
                    Box::new(parse(*a)?),
                    Box::new(parse(*b)?),
                )),
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
            let mut indexed = None;
            let (symbol, parameters, declaration_fault) =
                if let Some((target, indices, slice)) = array_definition_target(*lhs)? {
                    indexed = Some((indices, slice));
                    (target, None, None)
                } else if is_math(*lhs, "function") {
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
            let body = declaration_fault
                .or_else(|| {
                    if is_math(*rhs, "eval") {
                        check_attributes(*rhs, &[]).err()
                    } else {
                        None
                    }
                })
                .map(Expression::Unsupported)
                .unwrap_or(body);
            if let Some((indices, slice)) = indexed {
                return Ok(Statement::IndexedDefine {
                    symbol,
                    indices,
                    slice,
                    body,
                    global: name == "globalDefine",
                    show_result,
                    display_unit,
                });
            }
            Ok(Statement::Define {
                symbol,
                parameters,
                // Retain the declaration even if its RHS is unsupported, so it
                // poisons dependent reads instead of exposing an earlier value.
                body,
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

/// Keep the declared base even when an index/operand is malformed, so failed
/// array definitions poison subsequent reads rather than expose an old value.
type ArrayDefinitionTarget = (Symbol, Vec<Expression>, Option<&'static str>);
fn array_definition_target(node: Node<'_, '_>) -> Result<Option<ArrayDefinitionTarget>> {
    if !is_math(node, "apply") {
        return Ok(None);
    }
    let parts = children(node);
    let Some(op) = parts.first() else {
        return Ok(None);
    };
    let slice = match op.tag_name().name() {
        "matrow" if is_math(*op, "matrow") => Some("matrow"),
        "matcol" if is_math(*op, "matcol") => Some("matcol"),
        "indexer" if is_math(*op, "indexer") => None,
        _ => return Ok(None),
    };
    let target = symbol(
        *parts
            .get(1)
            .ok_or_else(|| Fault::unsupported("Missing array definition name"))?,
    )?;
    let indices = match parse_expression(node, 0) {
        Ok(Expression::Index(_, indices)) if slice.is_none() => indices,
        Ok(Expression::Binary(_, _, index)) if slice.is_some() => vec![*index],
        Ok(_) => vec![Expression::Unsupported(Fault::unsupported(
            "Array definition structure",
        ))],
        Err(fault) => vec![Expression::Unsupported(fault)],
    };
    Ok(Some((target, indices, slice)))
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

#[cfg(test)]
mod array_reference_tests;
mod scaling;
mod unit_functions;

#[derive(Clone)]
enum Binding {
    Value(Value),
    IndexedArray {
        value: Value,
        zero_dimension: Option<[f64; 7]>,
    },
    Function {
        parameters: Vec<Symbol>,
        body: Expression,
        captured: Arc<Scope>,
        local_guard: Option<program::CaptureGuard>,
    },
    Failed {
        fault: Fault,
        region: String,
    },
}
#[derive(Clone)]
struct Scope {
    strings_use_origin: Option<bool>,
    bindings: Bindings,
    program_frame: Option<Arc<program::CaptureFrame>>,
}
impl Default for Scope {
    fn default() -> Self {
        Self {
            strings_use_origin: Some(false),
            bindings: Bindings::default(),
            program_frame: None,
        }
    }
}

type BindingDelta = Vec<(Symbol, Arc<Binding>)>;
type BindingUndo = Vec<(Symbol, Option<Arc<Binding>>)>;

/// Bindings are replaced atomically, never mutated through a scope reference.
/// A copied scope/checkpoint can share immutable payloads, including fill-unit
/// metadata and captured functions, without copying every matrix element.
#[derive(Clone, Default)]
struct Bindings {
    values: HashMap<Symbol, Arc<Binding>>,
}
impl Bindings {
    fn get(&self, name: &Symbol) -> Option<&Binding> {
        self.values.get(name).map(Arc::as_ref)
    }
    fn shared(&self, name: &Symbol) -> Option<Arc<Binding>> {
        self.values.get(name).cloned()
    }
    fn contains_key(&self, name: &Symbol) -> bool {
        self.values.contains_key(name)
    }
    fn insert(&mut self, name: Symbol, binding: Binding) -> Option<Arc<Binding>> {
        self.insert_shared(name, Arc::new(binding))
    }
    fn insert_shared(&mut self, name: Symbol, binding: Arc<Binding>) -> Option<Arc<Binding>> {
        self.values.insert(name, binding)
    }
    fn remove(&mut self, name: &Symbol) -> Option<Arc<Binding>> {
        self.values.remove(name)
    }
    fn extend(&mut self, bindings: impl IntoIterator<Item = (Symbol, Arc<Binding>)>) {
        self.values.extend(bindings);
    }
}
fn builtin(name: &Symbol) -> bool {
    unit_functions::is_builtin(name)
        || (name.label == "FUNCTION"
            && (strings::is_builtin(&name.name)
                || scalars::is_builtin(&name.name)
                || arrays::is_builtin(&name.name)
                || statistics::is_builtin(&name.name)
                || numerics::is_builtin(&name.name)))
}
impl Scope {
    fn origin(&self) -> Result<Quantity> {
        let symbol = Symbol {
            name: "ORIGIN".into(),
            label: "SYSTEM".into(),
        };
        let origin = match self.stored_binding(&symbol)? {
            Some(Binding::Value(value) | Binding::IndexedArray { value, .. }) => {
                value.as_quantity()?
            }
            Some(_) => return Err(Fault::unsupported("Non-scalar SYSTEM ORIGIN")),
            None => Quantity::scalar(0.),
        };
        if !origin.is_scalar()
            || origin.value.im != 0.
            || !origin.value.re.is_finite()
            || origin.value.re.fract() != 0.
        {
            return Err(Fault::unsupported(
                "SYSTEM ORIGIN must be a real, dimensionless integer",
            ));
        }
        Ok(origin)
    }

    fn define_indexed(
        &mut self,
        symbol: &Symbol,
        indices: &[Expression],
        body: &Expression,
        region: &str,
    ) -> Result<bool> {
        self.define_array(symbol, indices, None, body, region)
    }

    fn define_array(
        &mut self,
        symbol: &Symbol,
        indices: &[Expression],
        slice: Option<&str>,
        body: &Expression,
        region: &str,
    ) -> Result<bool> {
        self.define_array_at_depth(symbol, (indices, slice), body, region, 0)
            .map(|(ranged, _)| ranged)
    }

    fn define_array_at_depth(
        &mut self,
        symbol: &Symbol,
        (indices, slice): (&[Expression], Option<&str>),
        body: &Expression,
        region: &str,
        depth: usize,
    ) -> Result<(bool, Value)> {
        let result: Result<(Binding, bool, Value)> = (|| {
            if slice.is_some_and(|operator| !matches!(operator, "matrow" | "matcol")) {
                return Err(Fault::unsupported("Unknown array assignment operator"));
            }
            let origin = self.origin()?;
            let mut names = std::collections::HashSet::new();
            for index in indices {
                self.collect_range_names(index, &mut names, depth)?;
            }
            let mut names: Vec<_> = names.into_iter().collect();
            names.sort_by(|a, b| a.name.cmp(&b.name).then(a.label.cmp(&b.label)));
            let ranged = !names.is_empty();
            if ranged && slice.is_some() {
                return Err(Fault::unsupported(
                    "Range-valued row/column assignment indices need an established original lowering contract",
                ));
            }
            if ranged {
                // Self-dependent range recurrence has an observable loop order
                // that this static pass has not recovered. Reject it instead
                // of silently reading either a partly updated or stale array.
                let mut validation = self.clone();
                validation.bindings.insert(symbol.clone(),Binding::Failed{fault:Fault::new("EA041_RANGE_RECURRENCE","Indexed range recurrence needs an established execution-order contract"),region:region.into()});
                validation
                    .validate_names(body, &[], depth)
                    .map_err(|fault| {
                        if fault.code == "EA041_RANGE_RECURRENCE" {
                            Fault::unsupported(fault.detail)
                        } else {
                            fault
                        }
                    })?;
            }
            let mut ranges = Vec::new();
            let mut count = 1usize;
            for name in names {
                let Binding::Value(Value::Range {
                    start,
                    step,
                    count: range_count,
                    ..
                }) = self.lookup(&name)?
                else {
                    unreachable!()
                };
                count = count
                    .checked_mul(range_count)
                    .filter(|count| *count > 0 && *count <= 16_384)
                    .ok_or_else(|| Fault::unsupported("Indexed range work budget exceeded"))?;
                ranges.push((name, start, step, range_count));
            }
            let (existing, dimension) = match self.bindings.get(symbol) {
                Some(Binding::IndexedArray {
                    value,
                    zero_dimension,
                }) => (Some(value.clone()), *zero_dimension),
                Some(Binding::Value(value)) => (Some(value.clone()), None),
                Some(Binding::Failed { .. }) => {
                    self.lookup(symbol)?;
                    unreachable!()
                }
                _ => (None, None),
            };
            let mut array = arrays::IndexedArray::new(existing, dimension)?;
            let mut point = self.clone();
            let mut last_value = None;
            for ordinal in 0..count {
                check_cancelled()?;
                let mut remainder = ordinal;
                for (name, start, step, len) in ranges.iter().rev() {
                    let index = remainder % len;
                    remainder /= len;
                    let mut value = *start;
                    value.value = start.value + step.value * index as f64;
                    point
                        .bindings
                        .insert(name.clone(), Binding::Value(Value::scalar(value)));
                }
                let offsets = indices
                    .iter()
                    .map(|index| {
                        point
                            .evaluate(index, depth)?
                            .as_quantity()
                            .and_then(|index| arrays::index_offset(index, origin))
                    })
                    .collect::<Result<Vec<_>>>()?;
                let value = point.evaluate(body, depth)?;
                if let Some(operator) = slice {
                    let [offset] = offsets.as_slice() else {
                        return Err(Fault::unsupported(
                            "Row/column definition requires one index",
                        ));
                    };
                    let rhs_dimension = match body {
                        Expression::Name(name) => match point.bindings.get(name) {
                            Some(Binding::IndexedArray { zero_dimension, .. }) => *zero_dimension,
                            _ => None,
                        },
                        _ => None,
                    };
                    array.put_slice(operator == "matcol", *offset, &value, rhs_dimension)?;
                } else {
                    array.put(&offsets, value.as_quantity()?)?;
                }
                last_value = Some(value);
            }
            let (value, zero_dimension) = array.finish();
            Ok((
                Binding::IndexedArray {
                    value,
                    zero_dimension,
                },
                ranged,
                last_value.expect("Nonempty bounded array assignment iteration"),
            ))
        })();
        match result {
            Ok((binding, ranged, value)) => {
                self.bindings.insert(symbol.clone(), binding);
                Ok((ranged, value))
            }
            Err(fault) => {
                if !fault.is_cancelled() {
                    self.bindings.insert(
                        symbol.clone(),
                        Binding::Failed {
                            fault: fault.clone(),
                            region: region.into(),
                        },
                    );
                }
                Err(fault)
            }
        }
    }

    fn indexed_definition_result(
        &self,
        symbol: &Symbol,
        indices: &[Expression],
        unit: &Option<Expression>,
    ) -> Result<(Value, Option<Value>)> {
        // The shipped array-authoring help explicitly supports single-index
        // writes to row vectors. Preserve that addressed-cell meaning for the
        // definition's inline result without broadening general index reads.
        if let [index] = indices
            && let Value::Matrix {
                rows: 1,
                cols,
                elements,
            } = self.evaluate(&Expression::Name(symbol.clone()), 0)?
            && cols > 1
        {
            let offset =
                arrays::index_offset(self.evaluate(index, 0)?.as_quantity()?, self.origin()?)?;
            let quantity = elements
                .get(offset)
                .copied()
                .ok_or_else(|| Fault::unsupported("Matrix index outside implemented range"))?;
            return self.display(Value::scalar(quantity), unit);
        }
        let value = self.evaluate(
            &Expression::Index(Box::new(Expression::Name(symbol.clone())), indices.to_vec()),
            0,
        )?;
        self.display(value, unit)
    }

    fn array_definition_result(
        &self,
        symbol: &Symbol,
        indices: &[Expression],
        slice: Option<&str>,
        unit: &Option<Expression>,
    ) -> Result<(Value, Option<Value>)> {
        if let Some(operator) = slice {
            let [index] = indices else {
                return Err(Fault::unsupported(
                    "Row/column definition requires one index",
                ));
            };
            let value = self.evaluate(
                &Expression::Binary(
                    operator.into(),
                    Box::new(Expression::Name(symbol.clone())),
                    Box::new(index.clone()),
                ),
                0,
            )?;
            self.display(value, unit)
        } else {
            self.indexed_definition_result(symbol, indices, unit)
        }
    }

    fn display(&self, value: Value, unit: &Option<Expression>) -> Result<(Value, Option<Value>)> {
        let Some(unit) = unit else {
            return Ok((value, None));
        };
        // UNIT functions have a registered inverse, not a linear factor.
        // Preserve worksheet bindings (including failed definitions) first.
        if let Expression::Name(name) = unit
            && !self.bindings.contains_key(name)
            && unit_functions::is_builtin(name)
        {
            let display = unit_functions::inverse_display(name, &value)?;
            return Ok((value, Some(display)));
        }
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
            Value::String { .. } => {
                return Err(Fault::unsupported(
                    "Strings cannot have a numeric display unit",
                ));
            }
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
                if matches!(
                    self.bindings.get(name),
                    Some(Binding::Value(Value::Range { .. }))
                ) {
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
            Expression::Binary(op, a, b) if op == "scale" && self.scaling_function(b).is_some() => {
                visit(a)
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
            Expression::Unsupported(_) | Expression::Scalar(_) | Expression::StringLiteral(_) => {
                Ok(())
            }
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
        if let Some(binding) = self.stored_binding(name)? {
            return match binding {
                Binding::IndexedArray { value, .. } => Ok(Binding::Value(value.clone())),
                _ => Ok(binding.clone()),
            };
        }
        let quantity = match (name.label.as_str(), name.name.as_str()) {
            ("UNIT", text) => unit(text),
            ("CONSTANT", "π") => Some(Quantity::scalar(std::f64::consts::PI)),
            ("CONSTANT", "e") => Some(Quantity::scalar(std::f64::consts::E)),
            ("CONSTANT", "γ") => Some(Quantity::scalar(std::f64::consts::EULER_GAMMA)),
            ("CONSTANT", "%") => Some(Quantity::scalar(0.01)),
            ("CONSTANT", text) => mathfolio_core::units::constant_lookup(text),
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
    fn stored_binding(&self, name: &Symbol) -> Result<Option<&Binding>> {
        match self.bindings.get(name) {
            Some(Binding::Failed { fault, region }) => Err(Fault {
                previous_region: Some(region.clone()),
                ..fault.clone()
            }),
            binding => Ok(binding),
        }
    }
    fn validate_name(&self, name: &Symbol) -> Result<()> {
        if self.stored_binding(name)?.is_some() {
            Ok(())
        } else {
            self.lookup(name).map(|_| ())
        }
    }
    fn evaluate_reference<'a>(
        &'a self,
        expression: &Expression,
        depth: usize,
    ) -> Result<std::borrow::Cow<'a, Value>> {
        if let Expression::Name(name) = expression {
            check_cancelled()?;
            if depth > 128 {
                return Err(Fault::unsupported("Evaluation depth exceeded"));
            }
            if let Some(Binding::Value(value) | Binding::IndexedArray { value, .. }) =
                self.stored_binding(name)?
            {
                return Ok(std::borrow::Cow::Borrowed(value));
            }
        }
        self.evaluate(expression, depth)
            .map(std::borrow::Cow::Owned)
    }
    fn validate_names(&self, e: &Expression, parameters: &[Symbol], depth: usize) -> Result<()> {
        check_cancelled()?;
        if depth > 128 {
            return Err(Fault::unsupported("Expression depth exceeded"));
        }
        match e {
            Expression::Symbolic(_) => Ok(()),
            Expression::Unsupported(fault) => Err(fault.clone()),
            Expression::Scalar(_) | Expression::StringLiteral(_) => Ok(()),
            Expression::Name(s) if parameters.contains(s) => Ok(()),
            Expression::Name(s) => self.validate_name(s),
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
            Expression::Binary(op, a, b) if op == "scale" && self.scaling_function(b).is_some() => {
                self.validate_names(a, parameters, depth + 1)?;
                self.validate_call_name(self.scaling_function(b).unwrap(), parameters)
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
                self.validate_call_name(s, parameters)?;
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
        check_cancelled()?;
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
            Expression::StringLiteral(value) => {
                strings::validate(value)?;
                Ok(Value::String {
                    value: StringValue::from(value.as_str()),
                })
            }
            Expression::Name(name) => match self.lookup(name)? {
                Binding::Value(v) => Ok(v),
                _ => Err(Fault::unsupported("Function used as a scalar")),
            },
            Expression::Neg(a) => arrays::unary("neg", &eval(a)?),
            Expression::Absolute(a) => arrays::unary("absval", &eval(a)?),
            Expression::Unary(op, a) if op == "vectorize" => self.evaluate_vectorized(a, depth + 1),
            Expression::Unary(op, a) => {
                let value = self.evaluate_reference(a, depth + 1)?;
                if op == "factorial" {
                    scalars::factorial(&value)
                } else {
                    arrays::unary(op, &value)
                }
            }
            Expression::Binary(op, a, b) if matches!(op.as_str(), "matcol" | "matrow") => {
                let matrix = self.evaluate_reference(a, depth + 1)?;
                let index = scalar(b)?;
                arrays::row_or_column(op, &matrix, index, self.origin()?)
            }
            Expression::Binary(op, a, b) if matches!(op.as_str(), "crossProduct" | "polar") => {
                let left = self.evaluate_reference(a, depth + 1)?;
                let right = self.evaluate_reference(b, depth + 1)?;
                if op == "polar" {
                    arrays::polar(&left, &right)
                } else {
                    arrays::cross_product(&left, &right)
                }
            }
            Expression::Binary(op, a, b) if op == "scale" && self.scaling_function(b).is_some() => {
                let argument = eval(a)?;
                self.call_named(self.scaling_function(b).unwrap(), vec![argument], depth)
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
                let matrix = self.evaluate_reference(a, depth + 1)?;
                let origin = self.origin()?;
                let mut offsets = Vec::new();
                for i in indices {
                    let index = scalar(i)?;
                    offsets.push(arrays::index_offset(index, origin)?);
                }
                match matrix.as_ref() {
                    Value::Matrix {
                        rows,
                        cols,
                        elements,
                    } => {
                        let (r, c) = match offsets.as_slice() {
                            [i] if *cols == 1 => (*i, 0),
                            [r, c] => (*r, *c),
                            _ => {
                                return Err(Fault::unsupported(
                                    "A general matrix needs row and column indices",
                                ));
                            }
                        };
                        if r >= *rows || c >= *cols {
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
                self.call_named(name, args, depth)
            }
        }
    }
    /// The arrow changes each application in this expression, but does not
    /// rewrite a separately defined function body or a stored intermediate.
    fn evaluate_vectorized(&self, expression: &Expression, depth: usize) -> Result<Value> {
        check_cancelled()?;
        if depth > 128 {
            return Err(Fault::unsupported("Vectorized expression depth exceeded"));
        }
        let evaluate = |value| self.evaluate_vectorized(value, depth + 1);
        let scalar_expression = |value: &Value| value.as_quantity().map(Expression::Scalar);
        match expression {
            Expression::Scalar(_) | Expression::StringLiteral(_) | Expression::Name(_) => {
                let value = self.evaluate(expression, depth + 1)?;
                if matches!(value, Value::Scalar { .. } | Value::Matrix { .. }) {
                    Ok(value)
                } else {
                    Err(Fault::unsupported(
                        "Vectorization requires a numeric scalar or matrix",
                    ))
                }
            }
            Expression::Unsupported(fault) => Err(fault.clone()),
            Expression::Unary(op, child) if op == "vectorize" => evaluate(child),
            Expression::Neg(child) | Expression::Absolute(child) | Expression::Unary(_, child) => {
                let value = evaluate(child)?;
                arrays::vectorize(&[value], |point| {
                    let child = Box::new(scalar_expression(&point[0])?);
                    let operation = match expression {
                        Expression::Neg(_) => Expression::Neg(child),
                        Expression::Absolute(_) => Expression::Absolute(child),
                        Expression::Unary(op, _) => Expression::Unary(op.clone(), child),
                        _ => unreachable!(),
                    };
                    self.evaluate(&operation, depth + 1)
                })
            }
            Expression::Binary(op, left, right)
                if op == "scale" && self.scaling_function(right).is_some() =>
            {
                let name = self.scaling_function(right).unwrap();
                let argument = evaluate(left)?;
                arrays::vectorize(&[argument], |point| {
                    self.evaluate(
                        &Expression::Call(name.clone(), vec![scalar_expression(&point[0])?]),
                        depth + 1,
                    )
                })
            }
            Expression::Binary(op, left, right) => {
                let values = [evaluate(left)?, evaluate(right)?];
                arrays::vectorize(&values, |point| {
                    self.evaluate(
                        &Expression::Binary(
                            op.clone(),
                            Box::new(scalar_expression(&point[0])?),
                            Box::new(scalar_expression(&point[1])?),
                        ),
                        depth + 1,
                    )
                })
            }
            Expression::Root(degree, value) => {
                let values = [evaluate(degree)?, evaluate(value)?];
                arrays::vectorize(&values, |point| {
                    self.evaluate(
                        &Expression::Root(
                            Box::new(scalar_expression(&point[0])?),
                            Box::new(scalar_expression(&point[1])?),
                        ),
                        depth + 1,
                    )
                })
            }
            Expression::Call(name, arguments) => {
                let values = arguments.iter().map(evaluate).collect::<Result<Vec<_>>>()?;
                arrays::vectorize(&values, |point| {
                    let arguments = point.iter().map(scalar_expression).collect::<Result<_>>()?;
                    self.evaluate(&Expression::Call(name.clone(), arguments), depth + 1)
                })
            }
            Expression::Matrix {
                rows,
                cols,
                elements,
            } => Ok(Value::Matrix {
                rows: *rows,
                cols: *cols,
                elements: elements
                    .iter()
                    .map(|cell| evaluate(cell)?.as_quantity())
                    .collect::<Result<_>>()?,
            }),
            Expression::Logic(logic) => {
                let values = logic
                    .arguments
                    .iter()
                    .map(evaluate)
                    .collect::<Result<Vec<_>>>()?;
                arrays::vectorize(&values, |point| {
                    let arguments = point.iter().map(scalar_expression).collect::<Result<_>>()?;
                    self.evaluate(
                        &Expression::Logic(Box::new(program::Logic {
                            operator: logic.operator,
                            arguments,
                        })),
                        depth + 1,
                    )
                })
            }
            Expression::Index(_, _)
            | Expression::Range { .. }
            | Expression::Program(_)
            | Expression::Numerical(_)
            | Expression::Symbolic(_) => Err(Fault::unsupported(
                "Vectorization of index/range/program/numerical/symbolic special forms is not established",
            )),
        }
    }
    fn define(
        &mut self,
        symbol: &Symbol,
        parameters: &Option<Vec<Symbol>>,
        body: &Expression,
        region: &str,
    ) -> Result<()> {
        check_cancelled()?;
        if parameters.is_none()
            && let Expression::Name(source) = body
            && let Some(binding) = self.bindings.shared(source)
            && matches!(
                binding.as_ref(),
                Binding::IndexedArray { .. } | Binding::Value(Value::Matrix { .. })
            )
        {
            // Later writes publish a fresh binding; sharing this immutable
            // version keeps aliases isolated and retains private fill metadata.
            self.bindings.insert_shared(symbol.clone(), binding);
            return Ok(());
        }
        let binding = if let Some(parameters) = parameters {
            self.validate_names(body, parameters, 0)
                .map(|()| Binding::Function {
                    parameters: parameters.clone(),
                    body: body.clone(),
                    captured: Arc::new(self.clone()),
                    local_guard: None,
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
                if fault.is_cancelled() {
                    return Err(fault);
                }
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

type WorksheetInput = (RegionEvaluation, Result<Statement>);
type ParsedWorksheet = (Vec<WorksheetInput>, Vec<String>, Vec<(f64, f64)>);

/// Calculation commands are independent of automatic-calculation settings.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(target_arch = "wasm32", derive(Serialize, Deserialize))]
pub enum CalculationRequest {
    Worksheet,
    /// Initialize global prerequisite bindings without publishing worksheet rows.
    Prerequisites,
    ThroughRegion(String),
    ActiveRegion(String),
}

/// Rows retain their last result while pending, independently of synchronization.
#[derive(Clone, Debug)]
#[cfg_attr(target_arch = "wasm32", derive(Serialize, Deserialize))]
pub struct CalculationUpdate {
    pub rows: Vec<RegionEvaluation>,
    pub pending_region_ids: Vec<String>,
    /// Includes global definitions executed as prerequisites outside the requested prefix.
    pub executed_region_ids: Vec<String>,
    /// Complete rows are reusable; the interrupted row retains its previous
    /// payload and remains pending. Cancellation is not published as an error.
    pub cancelled: bool,
}

/// Read-only worksheet environment for temporary queries, such as plot axes.
/// It holds computed bindings; querying never executes source definitions.
pub struct CalculationContext {
    scope: Scope,
    labels: HashMap<String, String>,
}

impl CalculationContext {
    pub fn with_strings_use_origin(mut self, enabled: bool) -> Self {
        self.scope.strings_use_origin = Some(enabled);
        self
    }
    pub fn with_strings_use_origin_unavailable(mut self) -> Self {
        self.scope.strings_use_origin = None;
        self
    }
    /// Evaluate one MathML eval without changing the worksheet environment.
    /// Display-unit metadata is validated, while the returned value retains its
    /// canonical physical quantity, matching RegionEvaluation::value.
    pub fn evaluate_mathml(&self, xml: &str) -> Result<Value> {
        plot_query::evaluate_mathml(self, xml)
    }
}

/// Incremental, geometric-order calculator with reusable binding checkpoints.
/// Invalidation is conservatively downstream, not a recovered dependency graph.
pub struct WorksheetCalculator {
    strings_use_origin: Option<bool>,
    source: Option<String>,
    source_grid: Option<PlacementGrid>,
    context_positions: Vec<(f64, f64)>,
    inputs: Vec<WorksheetInput>,
    signatures: Vec<String>,
    pending: Vec<bool>,
    scopes: Vec<Option<BindingDelta>>,
    globals: Option<Scope>,
    globals_complete: bool,
    global_symbols: std::collections::HashSet<Symbol>,
    global_rows: HashMap<String, RegionEvaluation>,
    global_signature: Vec<String>,
}

impl Default for WorksheetCalculator {
    fn default() -> Self {
        Self {
            strings_use_origin: Some(false),
            source: None,
            source_grid: None,
            context_positions: Vec::new(),
            inputs: Vec::new(),
            signatures: Vec::new(),
            pending: Vec::new(),
            scopes: Vec::new(),
            globals: None,
            globals_complete: false,
            global_symbols: Default::default(),
            global_rows: HashMap::new(),
            global_signature: Vec::new(),
        }
    }
}

impl WorksheetCalculator {
    /// Restored browser workers retain presentation payloads, never bindings or
    /// freshness. Only subsequent explicit engine requests execute definitions.
    #[cfg(target_arch = "wasm32")]
    pub fn restore_display_rows(&mut self, previous: &[RegionEvaluation]) {
        for (row, _) in &mut self.inputs {
            if let Some(old) = previous.iter().find(|old| old.region_id == row.region_id) {
                row.value = old.value.clone();
                row.display_value = old.display_value.clone();
                row.error = old.error.clone();
                row.matrix_origin = old.matrix_origin;
            }
        }
    }
    /// Explicit worksheet setting; false preserves native default zero indexing.
    /// A change invalidates all calculated bindings, including captured globals.
    pub fn set_strings_use_origin(&mut self, enabled: bool) -> bool {
        self.set_string_origin_policy(Some(enabled))
    }
    /// Malformed/unavailable package metadata blocks only index-sensitive calls.
    pub fn set_strings_use_origin_unavailable(&mut self) -> bool {
        self.set_string_origin_policy(None)
    }
    fn set_string_origin_policy(&mut self, enabled: Option<bool>) -> bool {
        if self.strings_use_origin == enabled {
            return false;
        }
        self.strings_use_origin = enabled;
        self.pending.fill(true);
        self.scopes.fill(None);
        self.globals = None;
        self.globals_complete = false;
        self.global_rows.clear();
        true
    }
    /// Snapshot computed bindings strictly before this geometric position.
    /// Global definitions participate regardless of their worksheet position.
    /// Pending preceding regions are rejected rather than executed implicitly.
    pub fn context_before(
        &self,
        left: f64,
        top: f64,
    ) -> std::result::Result<CalculationContext, String> {
        if !left.is_finite() || !top.is_finite() {
            return Err("Nonfinite calculation context position".into());
        }
        let mut scope = match &self.globals {
            Some(globals) if self.globals_complete => globals.clone(),
            Some(_) => return Err("Global definitions are pending calculation".into()),
            None if self.global_signature.is_empty() => Scope::default(),
            None => return Err("Global definitions are pending calculation".into()),
        };
        scope.strings_use_origin = self.strings_use_origin;
        let mut labels = HashMap::new();
        for (_, statement) in &self.inputs {
            if let Some(symbol) = global_symbol(statement) {
                labels.insert(symbol.name.clone(), symbol.label.clone());
            }
        }
        for (index, (row, _)) in self.inputs.iter().enumerate() {
            let (context_left, context_top) = self.context_positions[index];
            if !(context_top < top || (context_top == top && context_left < left)) {
                break;
            }
            if self.pending[index] {
                return Err(format!(
                    "Calculation context has pending region {}",
                    row.region_id
                ));
            }
            let saved = self.scopes[index]
                .as_ref()
                .ok_or_else(|| format!("Missing calculated bindings for {}", row.region_id))?;
            for (symbol, binding) in saved {
                labels.insert(symbol.name.clone(), symbol.label.clone());
                scope
                    .bindings
                    .insert_shared(symbol.clone(), binding.clone());
            }
        }
        Ok(CalculationContext { scope, labels })
    }

    /// Parse and invalidate without executing any expression. Existing payloads
    /// are retained by region identity until an explicit calculation replaces them.
    pub fn update(
        &mut self,
        xml: &str,
        grid: Option<PlacementGrid>,
    ) -> std::result::Result<CalculationUpdate, String> {
        if self.source.as_deref() == Some(xml) && self.source_grid == grid {
            return Ok(self.snapshot(Vec::new()));
        }
        let (mut inputs, signatures, context_positions) = parse_worksheet(xml, grid)?;
        check_cancelled().map_err(|fault| fault.detail)?;
        let global_signature: Vec<_> = inputs
            .iter()
            .zip(&signatures)
            .filter(|((_, statement), _)| is_global(statement))
            .map(|(_, signature)| signature.clone())
            .collect();
        let globals_changed = self.global_signature != global_signature || self.source_grid != grid;
        let prefix = if globals_changed {
            0
        } else {
            self.signatures
                .iter()
                .zip(&signatures)
                .take_while(|(a, b)| a == b)
                .count()
        };
        let mut previous: HashMap<_, _> = std::mem::take(&mut self.inputs)
            .into_iter()
            .map(|(row, _)| (row.region_id.clone(), row))
            .collect();
        for (row, _) in &mut inputs {
            if let Some(old) = previous.remove(&row.region_id) {
                row.value = old.value;
                row.display_value = old.display_value;
                row.error = old.error;
                row.redefinition_warning = old.redefinition_warning;
                row.matrix_origin = old.matrix_origin;
            }
        }
        let mut pending = vec![true; inputs.len()];
        let mut scopes = std::mem::take(&mut self.scopes);
        scopes.truncate(prefix);
        scopes.resize_with(inputs.len(), || None);
        pending[..prefix].copy_from_slice(&self.pending[..prefix]);
        self.inputs = inputs;
        self.context_positions = context_positions;
        self.signatures = signatures;
        self.pending = pending;
        self.scopes = scopes;
        if globals_changed {
            self.globals = None;
            self.globals_complete = false;
            self.global_rows.clear();
        }
        self.global_signature = global_signature;
        self.source = Some(xml.into());
        self.source_grid = grid;
        Ok(self.snapshot(Vec::new()))
    }

    pub fn calculate(
        &mut self,
        request: CalculationRequest,
    ) -> std::result::Result<CalculationUpdate, String> {
        let (limit, force) = match &request {
            CalculationRequest::Worksheet => (self.inputs.len(), None),
            CalculationRequest::Prerequisites => (0, None),
            CalculationRequest::ThroughRegion(id) | CalculationRequest::ActiveRegion(id) => {
                let index = self
                    .inputs
                    .iter()
                    .position(|(row, _)| row.region_id == *id)
                    .ok_or_else(|| format!("Unknown calculation region {id}"))?;
                (
                    index + 1,
                    matches!(request, CalculationRequest::ActiveRegion(_)).then_some(index),
                )
            }
        };
        if matches!(request, CalculationRequest::Worksheet) {
            self.pending.fill(true);
            self.scopes.fill(None);
            self.globals = None;
            self.globals_complete = false;
            self.global_rows.clear();
        }
        if let Some(index) = force {
            self.pending[index..].fill(true);
            self.scopes[index..].fill(None);
            if is_global(&self.inputs[index].1) {
                self.globals = None;
                self.globals_complete = false;
                self.global_rows.clear();
                self.pending.fill(true);
                self.scopes.fill(None);
            }
        }
        let mut executed = Vec::new();
        if check_cancelled().is_err() {
            return Ok(self.snapshot(executed));
        }
        if !self.globals_complete {
            let mut counts = HashMap::<Symbol, usize>::new();
            for (_, statement) in &self.inputs {
                if let Some(symbol) = global_symbol(statement) {
                    *counts.entry(symbol.clone()).or_default() += 1;
                }
            }
            self.global_symbols = counts.keys().cloned().collect();
            let mut globals = self.globals.take().unwrap_or_default();
            globals.strings_use_origin = self.strings_use_origin;
            for (row, statement) in &self.inputs {
                if !is_global(statement) || self.global_rows.contains_key(&row.region_id) {
                    continue;
                }
                if check_cancelled().is_err() {
                    self.globals = Some(globals);
                    return Ok(self.snapshot(executed));
                }
                let names = statement_binding_names(statement);
                let before = save_bindings(&globals, &names);
                let mut candidate = row.clone();
                clear_result(&mut candidate);
                evaluate_global(&mut candidate, statement, &mut globals, &counts);
                candidate.matrix_origin = matrix_origin::capture(&globals, &candidate);
                if candidate.error.as_ref().is_some_and(Fault::is_cancelled)
                    || cancellation_requested()
                {
                    restore_bindings(&mut globals, before);
                    self.globals = Some(globals);
                    return Ok(self.snapshot(executed));
                }
                executed.push(row.region_id.clone());
                self.global_rows.insert(row.region_id.clone(), candidate);
            }
            self.globals = Some(globals);
            self.globals_complete = true;
        }
        let mut scope = self.globals.clone().unwrap_or_default();
        scope.strings_use_origin = self.strings_use_origin;
        for index in 0..limit {
            if check_cancelled().is_err() {
                return Ok(self.snapshot(executed));
            }
            if !self.pending[index]
                && let Some(saved) = &self.scopes[index]
            {
                // Replay already computed bindings, not expression evaluation.
                // Keeping only each statement's delta avoids a full copy of all
                // earlier definitions for every worksheet result region.
                scope.bindings.extend(saved.iter().cloned());
                continue;
            }
            let original = self.inputs[index].0.clone();
            let names = statement_binding_names(&self.inputs[index].1);
            let before = save_bindings(&scope, &names);
            let (row, statement) = &mut self.inputs[index];
            if is_global(statement) {
                if let Some(saved) = self.global_rows.get(&row.region_id) {
                    *row = saved.clone();
                }
            } else {
                clear_result(row);
                scope = evaluate_local(
                    std::slice::from_mut(&mut self.inputs[index]),
                    scope,
                    &self.global_symbols,
                );
                self.inputs[index].0.matrix_origin =
                    matrix_origin::capture(&scope, &self.inputs[index].0);
                if self.inputs[index]
                    .0
                    .error
                    .as_ref()
                    .is_some_and(Fault::is_cancelled)
                    || cancellation_requested()
                {
                    restore_bindings(&mut scope, before);
                    self.inputs[index].0 = original;
                    return Ok(self.snapshot(executed));
                }
                executed.push(self.inputs[index].0.region_id.clone());
            }
            let names = match &self.inputs[index].1 {
                Ok(
                    Statement::Define {
                        symbol,
                        global: false,
                        ..
                    }
                    | Statement::IndexedDefine {
                        symbol,
                        global: false,
                        ..
                    },
                ) => vec![symbol.clone()],
                Ok(Statement::SolveBlock(block)) => block.exports(),
                Ok(Statement::SpecTable(table)) => table.binding_names(),
                _ => Vec::new(),
            };
            self.pending[index] = false;
            self.scopes[index] = Some(
                names
                    .into_iter()
                    .filter_map(|symbol| {
                        scope
                            .bindings
                            .shared(&symbol)
                            .map(|binding| (symbol, binding))
                    })
                    .collect(),
            );
        }
        Ok(self.snapshot(executed))
    }

    pub fn calculate_with_control(
        &mut self,
        request: CalculationRequest,
        control: &CalculationControl,
    ) -> std::result::Result<CalculationUpdate, String> {
        control.run(|| self.calculate(request))
    }

    fn snapshot(&self, executed_region_ids: Vec<String>) -> CalculationUpdate {
        CalculationUpdate {
            rows: self
                .inputs
                .iter()
                .zip(&self.pending)
                .map(|((row, _), pending)| {
                    let mut published = row.clone();
                    if *pending || published.error.is_some() {
                        published.matrix_origin = None;
                    }
                    published
                })
                .collect(),
            pending_region_ids: self
                .inputs
                .iter()
                .zip(&self.pending)
                .filter(|(_, pending)| **pending)
                .map(|((row, _), _)| row.region_id.clone())
                .collect(),
            executed_region_ids,
            cancelled: cancellation_requested(),
        }
    }
}

fn clear_result(row: &mut RegionEvaluation) {
    row.matrix_origin = None;
    row.value = None;
    row.display_value = None;
    row.error = None;
    row.redefinition_warning = false;
}

fn is_global(statement: &Result<Statement>) -> bool {
    global_symbol(statement).is_some()
}
fn global_symbol(statement: &Result<Statement>) -> Option<&Symbol> {
    match statement {
        Ok(
            Statement::Define {
                global: true,
                symbol,
                ..
            }
            | Statement::IndexedDefine {
                global: true,
                symbol,
                ..
            },
        ) => Some(symbol),
        _ => None,
    }
}

/// Evaluate positioned regions in sibling geometric order, with Area child blocks.
/// Import does not execute scripts, XAML, external relationships, or the original engine.
pub fn evaluate_worksheet(xml: &str) -> std::result::Result<Vec<RegionEvaluation>, String> {
    evaluate_worksheet_with_grid(xml, None)
}

/// Supply the document's presentation grid to reproduce Prime's load-time snap.
pub fn evaluate_worksheet_with_grid(
    xml: &str,
    grid: Option<PlacementGrid>,
) -> std::result::Result<Vec<RegionEvaluation>, String> {
    let mut calculator = WorksheetCalculator::default();
    calculator.update(xml, grid)?;
    Ok(calculator.calculate(CalculationRequest::Worksheet)?.rows)
}

fn parse_worksheet(
    xml: &str,
    grid: Option<PlacementGrid>,
) -> std::result::Result<ParsedWorksheet, String> {
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
    let mut table_labels = HashMap::new();
    let hierarchical = regions.children().any(|region| {
        region
            .children()
            .any(|node| node.has_tag_name((WS, "Area")))
    });
    for positioned in area_calculation::ordered_regions(regions, grid, hierarchical)? {
        let region = positioned.region;
        let content = children(region);
        let math = region.children().find(|n| n.has_tag_name((WS, "math")));
        let statement = if let Some(fault) = positioned.fault {
            Err(fault)
        } else if let Some(math) = math {
            let all = children(math);
            let c: Vec<_> = all
                .iter()
                .copied()
                .filter(|n| n.tag_name().namespace() == Some(ML))
                .collect();
            if math.attributes().any(|a| {
                a.namespace().is_some()
                    || (a.name() != "resultRef"
                        && !(positioned.in_area
                            && a.name() == "disable-calc"
                            && matches!(a.value(), "false" | "0")))
            }) {
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
        } else if content.len() == 1 && content[0].has_tag_name((WS, "spec-table")) {
            spec_table::parse(content[0], xml, &mut table_labels).map(Statement::SpecTable)
        } else {
            Err(Fault::unsupported(
                "Non-math/nested region requires document model",
            ))
        };
        for symbol in statement_binding_names(&statement) {
            table_labels.insert(symbol.name, symbol.label);
        }
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
                left: positioned.left,
                top: positioned.top,
                value: None,
                display_value: None,
                error: None,
                redefinition_warning: false,
                matrix_origin: None,
            },
            statement,
            xml[region.range()].to_owned(),
            positioned.context_position,
        ));
    }
    // Preserve the independent legacy non-Area parser: table label state is
    // collected in XML order before rows are sorted for execution. Area blocks
    // instead have their explicit sibling-slot traversal above.
    if !hierarchical {
        inputs.sort_by(|a, b| {
            a.0.top
                .total_cmp(&b.0.top)
                .then(a.0.left.total_cmp(&b.0.left))
        });
    }
    let context_positions = inputs.iter().map(|row| row.3).collect();
    let (inputs, signatures) = inputs
        .into_iter()
        .map(|(row, statement, source, _)| ((row, statement), source))
        .unzip();
    Ok((inputs, signatures, context_positions))
}

fn evaluate_global(
    row: &mut RegionEvaluation,
    statement: &Result<Statement>,
    globals: &mut Scope,
    global_counts: &HashMap<Symbol, usize>,
) {
    if let Ok(Statement::IndexedDefine {
        symbol,
        indices,
        slice,
        body,
        global: true,
        show_result,
        display_unit,
    }) = statement
    {
        if global_counts[symbol] > 1 {
            let fault =
                Fault::unsupported("Duplicate global definitions need exact conflict diagnostics");
            globals.bindings.insert(
                symbol.clone(),
                Binding::Failed {
                    fault: fault.clone(),
                    region: row.region_id.clone(),
                },
            );
            row.error = Some(fault);
        } else {
            let defined = if slice.is_none() {
                globals.define_indexed(symbol, indices, body, &row.region_id)
            } else {
                globals.define_array(symbol, indices, *slice, body, &row.region_id)
            };
            match defined {
                Ok(ranged) => {
                    if *show_result && !ranged {
                        match globals.array_definition_result(symbol, indices, *slice, display_unit)
                        {
                            Ok((value, display)) => {
                                row.value = Some(value);
                                row.display_value = display;
                            }
                            Err(fault) => row.error = Some(fault),
                        }
                    }
                }
                Err(fault) => row.error = Some(fault),
            }
        }
        return;
    }
    if let Ok(Statement::Define {
        symbol,
        parameters,
        body,
        global: true,
        show_result,
        display_unit,
    }) = statement
    {
        if global_counts[symbol] > 1 {
            let fault =
                Fault::unsupported("Duplicate global definitions need exact conflict diagnostics");
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

fn statement_binding_names(statement: &Result<Statement>) -> Vec<Symbol> {
    match statement {
        Ok(Statement::Define { symbol, .. } | Statement::IndexedDefine { symbol, .. }) => {
            vec![symbol.clone()]
        }
        Ok(Statement::SolveBlock(block)) => block.exports(),
        Ok(Statement::SpecTable(table)) => table.binding_names(),
        _ => Vec::new(),
    }
}
fn save_bindings(scope: &Scope, names: &[Symbol]) -> BindingUndo {
    names
        .iter()
        .map(|name| (name.clone(), scope.bindings.shared(name)))
        .collect()
}
fn restore_bindings(scope: &mut Scope, saved: BindingUndo) {
    for (name, binding) in saved {
        if let Some(binding) = binding {
            scope.bindings.insert_shared(name, binding);
        } else {
            scope.bindings.remove(&name);
        }
    }
}

fn evaluate_local(
    inputs: &mut [WorksheetInput],
    mut scope: Scope,
    global_symbols: &std::collections::HashSet<Symbol>,
) -> Scope {
    for (row, statement) in inputs.iter_mut() {
        let statement = match statement {
            Ok(s) => s,
            Err(fault) => {
                row.error = Some(fault.clone());
                continue;
            }
        };
        match statement {
            Statement::IndexedDefine { global: true, .. } => (),
            Statement::IndexedDefine {
                symbol,
                indices,
                slice,
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
                let defined = if slice.is_none() {
                    scope.define_indexed(symbol, indices, body, &row.region_id)
                } else {
                    scope.define_array(symbol, indices, *slice, body, &row.region_id)
                };
                match defined {
                    Ok(ranged) => {
                        row.redefinition_warning = redefinition;
                        if *show_result && !ranged {
                            match scope.array_definition_result(
                                symbol,
                                indices,
                                *slice,
                                display_unit,
                            ) {
                                Ok((value, display)) => {
                                    row.value = Some(value);
                                    row.display_value = display;
                                }
                                Err(fault) => row.error = Some(fault),
                            }
                        }
                    }
                    Err(fault) => row.error = Some(fault),
                }
            }
            Statement::SpecTable(table) => table.evaluate(row, &mut scope, global_symbols),
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
    scope
}
