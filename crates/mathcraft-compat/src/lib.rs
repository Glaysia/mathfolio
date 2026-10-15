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

pub mod editor_bridge;
pub mod reference;

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
    Call(Symbol, Vec<Self>),
    Matrix {
        rows: usize,
        cols: usize,
        elements: Vec<Self>,
    },
    Index(Box<Self>, Box<Self>),
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Value {
    Scalar {
        quantity: Quantity,
    },
    Matrix {
        rows: usize,
        cols: usize,
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
            _ => Err(Fault::unsupported("Matrix arithmetic is not implemented")),
        }
    }
    /// Numeric display payload, comparable with an original saved result.
    /// Physical dimensions remain separately available in `Value`.
    pub fn numeric_json(&self) -> serde_json::Value {
        fn number(q: &Quantity) -> serde_json::Value {
            serde_json::json!({"kind":"scalar", "real":q.value.re, "imag":q.value.im})
        }
        match self {
            Self::Scalar { quantity } => number(quantity),
            Self::Matrix {
                rows,
                cols,
                elements,
            } => serde_json::json!({
                "kind":"matrix", "rows":rows, "cols":cols,
                "elements":elements.iter().map(number).collect::<Vec<_>>()
            }),
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
    Define {
        symbol: Symbol,
        parameters: Option<Vec<Symbol>>,
        body: Expression,
        global: bool,
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
    let name = node
        .text()
        .ok_or_else(|| Fault::unsupported("Empty identifier"))?
        .to_string();
    let label = node
        .attribute("labels")
        .ok_or_else(|| Fault::unsupported("Automatic identifier labelling is not implemented"))?
        .to_string();
    if !children(node).is_empty() {
        return Err(Fault::unsupported(
            "Structured identifier subscripts are not implemented",
        ));
    }
    Ok(Symbol { name, label })
}
fn parse_expression(node: Node<'_, '_>, depth: usize) -> Result<Expression> {
    if depth > 128 {
        return Err(Fault::unsupported("MathML nesting budget exceeded"));
    }
    if node.tag_name().namespace() != Some(ML) {
        return Err(Fault::unsupported("Foreign expression namespace"));
    }
    let items = children(node);
    let parse = |n| parse_expression(n, depth + 1);
    match node.tag_name().name() {
        "real" | "imag" => {
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
            // Current runtime reference covers column vectors; storage order of
            // general matrices needs separate evidence before execution support.
            if cols != 1 {
                return Err(Fault::unsupported(
                    "Only column matrices have runtime reference coverage",
                ));
            }
            Ok(Expression::Matrix {
                rows,
                cols,
                elements: items.into_iter().map(parse).collect::<Result<_>>()?,
            })
        }
        "apply" if !items.is_empty() => {
            let op = items[0];
            if is_math(op, "id") {
                return Ok(Expression::Call(
                    symbol(op)?,
                    items[1..]
                        .iter()
                        .copied()
                        .map(parse)
                        .collect::<Result<_>>()?,
                ));
            }
            if op.tag_name().namespace() != Some(ML) {
                return Err(Fault::unsupported("Foreign operator namespace"));
            }
            match (op.tag_name().name(), &items[1..]) {
                ("neg", [a]) => Ok(Expression::Neg(Box::new(parse(*a)?))),
                ("absval", [a]) => Ok(Expression::Absolute(Box::new(parse(*a)?))),
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
                ("indexer", [a, i]) => Ok(Expression::Index(
                    Box::new(parse(*a)?),
                    Box::new(parse(*i)?),
                )),
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
    let items = children(node);
    match (
        node.tag_name().namespace(),
        node.tag_name().name(),
        items.as_slice(),
    ) {
        (Some(ML), name @ ("define" | "globalDefine"), [lhs, rhs]) => {
            let (symbol, parameters) = if is_math(*lhs, "function") {
                let parts = children(*lhs);
                match parts.as_slice() {
                    [name, bound] if is_math(*bound, "boundVars") => {
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
                        (symbol(*name)?, Some(parameters))
                    }
                    _ => return Err(Fault::unsupported("Function declaration structure")),
                }
            } else {
                (symbol(*lhs)?, None)
            };
            Ok(Statement::Define {
                symbol,
                parameters,
                // Retain the declaration even if its RHS is unsupported, so it
                // poisons dependent reads instead of exposing an earlier value.
                body: parse_expression(*rhs, 0).unwrap_or_else(Expression::Unsupported),
                global: name == "globalDefine",
            })
        }
        (Some(ML), "eval", [expression, unit]) if is_math(*unit, "unitOverride") => {
            let units = children(*unit);
            let display_unit = match units.as_slice() {
                [p] if is_math(*p, "placeholder") => None,
                [p] => Some(parse_expression(*p, 0)?),
                _ => return Err(Fault::unsupported("Display unit structure")),
            };
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
        && matches!(
            name.name.as_str(),
            "sin" | "cos" | "tan" | "ln" | "exp" | "sqrt" | "abs"
        )
}
impl Scope {
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
            ("SYSTEM" | "VARIABLE", "ORIGIN") => Some(Quantity::scalar(0.)),
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
            Expression::Unsupported(fault) => Err(fault.clone()),
            Expression::Scalar(_) => Ok(()),
            Expression::Name(s) if parameters.contains(s) => Ok(()),
            Expression::Name(s) => self.lookup(s).map(|_| ()),
            Expression::Neg(a) | Expression::Absolute(a) => {
                self.validate_names(a, parameters, depth + 1)
            }
            Expression::Binary(_, a, b) | Expression::Root(a, b) | Expression::Index(a, b) => {
                self.validate_names(a, parameters, depth + 1)?;
                self.validate_names(b, parameters, depth + 1)
            }
            Expression::Call(s, args) => {
                if !parameters.contains(s) && !builtin(s) {
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
            Expression::Unsupported(fault) => Err(fault.clone()),
            Expression::Scalar(q) => Ok(Value::scalar(*q)),
            Expression::Name(name) => match self.lookup(name)? {
                Binding::Value(v) => Ok(v),
                _ => Err(Fault::unsupported("Function used as a scalar")),
            },
            Expression::Neg(a) => {
                let mut q = scalar(a)?;
                q.value = -q.value;
                Ok(Value::scalar(q))
            }
            Expression::Absolute(a) => {
                let mut q = scalar(a)?;
                q.value = Complex64::new(q.value.norm(), 0.);
                Ok(Value::scalar(q))
            }
            Expression::Binary(op, a, b) => {
                let a = scalar(a)?;
                let b = scalar(b)?;
                let q = match op.as_str() {
                    "plus" => a.add(b, false),
                    "minus" => a.add(b, true),
                    "mult" | "scale" => a.multiply(b, false),
                    "div" => a.multiply(b, true),
                    "pow" => a.power(b),
                    _ => unreachable!(),
                }
                .map_err(core_error)?;
                Ok(Value::scalar(q))
            }
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
            Expression::Index(a, i) => {
                let matrix = eval(a)?;
                let index = scalar(i)?;
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
                if !index.is_scalar() || index.value.im != 0. || index.value.re.fract() != 0. {
                    return Err(Fault::unsupported("Non-integer/unit-bearing matrix index"));
                }
                let offset = index.value.re - origin.value.re;
                match matrix {
                    Value::Matrix {
                        cols: 1, elements, ..
                    } if offset >= 0. && offset < elements.len() as f64 => {
                        Ok(Value::scalar(elements[offset as usize]))
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
                let [arg] = args.as_slice() else {
                    return Err(Fault::unsupported("Built-in argument count"));
                };
                let mut q = arg.as_quantity()?;
                if name.name == "sqrt" {
                    return Ok(Value::scalar(
                        q.power(Quantity::scalar(0.5)).map_err(core_error)?,
                    ));
                }
                if name.name == "abs" {
                    q.value = Complex64::new(q.value.norm(), 0.);
                    return Ok(Value::scalar(q));
                }
                if !q.is_scalar() {
                    return Err(Fault::new(
                        "bad%_dimensions%_unitless",
                        "This argument must be unitless",
                    ));
                }
                q.value = match name.name.as_str() {
                    "sin" => q.value.sin(),
                    "cos" => q.value.cos(),
                    "tan" => q.value.tan(),
                    "ln" => q.value.ln(),
                    "exp" => q.value.exp(),
                    _ => unreachable!(),
                };
                if !q.value.re.is_finite() || !q.value.im.is_finite() {
                    return Err(Fault::unsupported("Nonfinite built-in result"));
                }
                Ok(Value::scalar(q))
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
            self.evaluate(body, 0).map(Binding::Value)
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
            let c = children(math);
            if c.len() == 1 {
                parse_statement(c[0])
            } else {
                Err(Fault::unsupported("Math region has multiple roots"))
            }
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
                    .map(str::to_string),
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
            Statement::Define { global: true, .. } => (),
            Statement::Define {
                symbol,
                parameters,
                body,
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
            }
            Statement::Evaluate {
                expression,
                display_unit,
            } => {
                let result = scope.evaluate(expression, 0).and_then(|value| {
                    if let Some(unit) = display_unit {
                        let unit = scope.evaluate(unit, 0)?.as_quantity()?;
                        let quantity = value.as_quantity()?;
                        if !quantity.same_dimension(&unit) {
                            return Err(Fault::new(
                                "bad%_dimensions",
                                "Display unit has incompatible dimensions",
                            ));
                        }
                        if unit.value.norm() == 0. {
                            return Err(Fault::new("divide_by_zero", "Display unit is zero"));
                        }
                        let display = Value::scalar(Quantity {
                            value: quantity.value / unit.value,
                            dimension: [0.; 7],
                        });
                        Ok((value, Some(display)))
                    } else {
                        Ok((value, None))
                    }
                });
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
