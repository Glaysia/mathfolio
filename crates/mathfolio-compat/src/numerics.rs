//! Bounded independent numerical algorithms behind observed Math50 operators.
//!
//! Parsing/scoping follows shipped schemas, help and front-end evidence. Simpson
//! quadrature, secant/bisection and finite differences below are new code, not
//! recovered Prime algorithms; see docs/analysis/0007-numerics-semantics.md.
use super::{
    Binding, Expression, Fault, Result, Scope, Symbol, Value, children, is_math, parse_expression,
};
use mathfolio_core::Quantity;
use num_complex::Complex64;
use roxmltree::Node;
use std::{cell::Cell, collections::HashSet};

#[derive(Clone, Debug)]
pub enum NumericalExpression {
    Root {
        body: Box<Expression>,
        variable: Symbol,
        bounds: Option<(Box<Expression>, Box<Expression>)>,
    },
    Derivative {
        body: Box<Expression>,
        variable: Symbol,
        order: Box<Expression>,
    },
    Integral {
        body: Box<Expression>,
        variable: Symbol,
        lower: Box<Expression>,
        upper: Box<Expression>,
    },
    Fold {
        body: Box<Expression>,
        variable: Symbol,
        lower: Box<Expression>,
        upper: Box<Expression>,
        product: bool,
    },
}

fn no_attrs(node: Node<'_, '_>) -> Result<()> {
    super::check_attributes(node, &[])
}

fn binder(node: Node<'_, '_>) -> Result<Symbol> {
    if !is_math(node, "id") {
        return Err(Fault::unsupported("Numerical binder must be an identifier"));
    }
    let (name, label, contextual, subscript) =
        super::editor_bridge::identifier_parts(node).map_err(Fault::unsupported)?;
    if matches!(label.as_deref(), Some("*" | "NONE")) || contextual == Some(true) {
        return Err(Fault::unsupported(
            "Automatic identifier labelling is not implemented",
        ));
    }
    let label = label.unwrap_or_else(|| "VARIABLE".into());
    if label != "VARIABLE" {
        return Err(Fault::unsupported(
            "Numerical binder requires the Variable label",
        ));
    }
    Ok(Symbol {
        name: super::editor_bridge::identifier_identity(&name, subscript.as_deref()),
        label,
    })
}

fn lambda(node: Node<'_, '_>, depth: usize) -> Result<(Symbol, Expression)> {
    if !is_math(node, "lambda") {
        return Err(Fault::unsupported("Calculus requires a Math50 lambda"));
    }
    no_attrs(node)?;
    let parts = children(node);
    let [bound, body] = parts.as_slice() else {
        return Err(Fault::unsupported("Calculus lambda arity"));
    };
    if !is_math(*bound, "boundVars") {
        return Err(Fault::unsupported("Calculus requires boundVars"));
    }
    no_attrs(*bound)?;
    let vars = children(*bound);
    let [var] = vars.as_slice() else {
        return Err(Fault::unsupported(
            "Only one calculus bound variable is supported",
        ));
    };
    Ok((binder(*var)?, parse_expression(*body, depth + 1)?))
}

fn bound(node: Node<'_, '_>, name: &str, depth: usize) -> Result<Box<Expression>> {
    if !is_math(node, name) {
        return Err(Fault::unsupported("Calculus bound order/shape"));
    }
    no_attrs(node)?;
    let ch = children(node);
    let [value] = ch.as_slice() else {
        return Err(Fault::unsupported(
            "Calculus bound must contain one expression",
        ));
    };
    Ok(Box::new(parse_expression(*value, depth + 1)?))
}

/// Parser hook before ordinary eager Call parsing. root's first argument and
/// variable must not be evaluated before establishing the local binding.
pub(crate) fn parse_operator(node: Node<'_, '_>, depth: usize) -> Result<Option<Expression>> {
    if !is_math(node, "apply") {
        return Ok(None);
    }
    let items = children(node);
    let Some(op) = items.first().copied() else {
        return Ok(None);
    };
    if is_math(op, "id") && op.text() == Some("root") && op.attribute("labels") == Some("KEYWORD") {
        no_attrs(node)?;
        let _ = super::editor_bridge::identifier_parts(op).map_err(Fault::unsupported)?;
        let args = if items.len() == 2 && is_math(items[1], "sequence") {
            no_attrs(items[1])?;
            children(items[1])
        } else {
            items[1..].to_vec()
        };
        if args.len() != 2 && args.len() != 4 {
            return Err(Fault::unsupported(
                "root requires (expression, variable[, lower, upper])",
            ));
        }
        let variable = binder(args[1])?;
        let bounds = if args.len() == 4 {
            Some((
                Box::new(parse_expression(args[2], depth + 1)?),
                Box::new(parse_expression(args[3], depth + 1)?),
            ))
        } else {
            None
        };
        return Ok(Some(Expression::Numerical(NumericalExpression::Root {
            body: Box::new(parse_expression(args[0], depth + 1)?),
            variable,
            bounds,
        })));
    }
    if !is_math(op, "derivative")
        && !is_math(op, "partDerivative")
        && !is_math(op, "integral")
        && !is_math(op, "summation")
        && !is_math(op, "product")
    {
        return Ok(None);
    }
    no_attrs(node)?;
    if !children(op).is_empty() {
        return Err(Fault::unsupported("Structured calculus operator"));
    }
    let lambda_node = *items
        .get(1)
        .ok_or_else(|| Fault::unsupported("Missing calculus lambda"))?;
    let (variable, body) = lambda(lambda_node, depth + 1)?;
    let numerical = if is_math(op, "summation") || is_math(op, "product") {
        no_attrs(op)?;
        if items.len() != 4 {
            return Err(Fault::unsupported(
                "Summation/product currently requires explicit lower and upper bounds",
            ));
        }
        NumericalExpression::Fold {
            body: Box::new(body),
            variable,
            lower: bound(items[2], "lowerBound", depth + 1)?,
            upper: bound(items[3], "upperBound", depth + 1)?,
            product: is_math(op, "product"),
        }
    } else if is_math(op, "integral") {
        super::check_attributes(op, &["algorithm", "auto-algorithm"])?;
        if op.attribute("algorithm").is_some_and(|v| v != "adaptive")
            || op
                .attribute("auto-algorithm")
                .is_some_and(|v| v != "true" && v != "false" && v != "1" && v != "0")
        {
            return Err(Fault::unsupported(
                "Only finite adaptive integration is implemented",
            ));
        }
        if items.len() != 4 {
            return Err(Fault::unsupported(
                "Only a definite integral with lower/upper bounds is implemented",
            ));
        }
        NumericalExpression::Integral {
            body: Box::new(body),
            variable,
            lower: bound(items[2], "lowerBound", depth + 1)?,
            upper: bound(items[3], "upperBound", depth + 1)?,
        }
    } else {
        no_attrs(op)?;
        let order = match &items[2..] {
            [] => Box::new(Expression::Scalar(Quantity::scalar(1.0))),
            [n] if is_math(*n, "degree") => {
                no_attrs(*n)?;
                let ch = children(*n);
                match ch.as_slice() {
                    [p] if is_math(*p, "placeholder") => {
                        no_attrs(*p)?;
                        Box::new(Expression::Scalar(Quantity::scalar(1.0)))
                    }
                    [e] => Box::new(parse_expression(*e, depth + 1)?),
                    _ => return Err(Fault::unsupported("Derivative degree shape")),
                }
            }
            _ => return Err(Fault::unsupported("Derivative operator shape")),
        };
        NumericalExpression::Derivative {
            body: Box::new(body),
            variable,
            order,
        }
    };
    Ok(Some(Expression::Numerical(numerical)))
}

pub(crate) fn validate_names(
    scope: &Scope,
    expr: &NumericalExpression,
    parameters: &[Symbol],
    depth: usize,
) -> Result<()> {
    let (body, variable) = match expr {
        NumericalExpression::Root { body, variable, .. }
        | NumericalExpression::Derivative { body, variable, .. }
        | NumericalExpression::Integral { body, variable, .. }
        | NumericalExpression::Fold { body, variable, .. } => (body, variable),
    };
    let mut local = parameters.to_vec();
    local.push(variable.clone());
    scope.validate_names(body, &local, depth + 1)?;
    match expr {
        NumericalExpression::Root {
            variable,
            bounds: None,
            ..
        } => scope.validate_names(&Expression::Name(variable.clone()), parameters, depth + 1),
        NumericalExpression::Root {
            bounds: Some((a, b)),
            ..
        } => {
            scope.validate_names(a, parameters, depth + 1)?;
            scope.validate_names(b, parameters, depth + 1)
        }
        NumericalExpression::Derivative {
            variable, order, ..
        } => {
            scope.validate_names(&Expression::Name(variable.clone()), parameters, depth + 1)?;
            scope.validate_names(order, parameters, depth + 1)
        }
        NumericalExpression::Integral { lower, upper, .. }
        | NumericalExpression::Fold { lower, upper, .. } => {
            scope.validate_names(lower, parameters, depth + 1)?;
            scope.validate_names(upper, parameters, depth + 1)
        }
    }
}

pub(crate) fn collect_range_names(
    scope: &Scope,
    expr: &NumericalExpression,
    names: &mut HashSet<Symbol>,
    depth: usize,
) -> Result<()> {
    let (body, variable) = match expr {
        NumericalExpression::Root { body, variable, .. }
        | NumericalExpression::Derivative { body, variable, .. }
        | NumericalExpression::Integral { body, variable, .. }
        | NumericalExpression::Fold { body, variable, .. } => (body, variable),
    };
    let mut local = scope.clone();
    local.bindings.insert(
        variable.clone(),
        Binding::Value(Value::scalar(Quantity::scalar(0.0))),
    );
    local.collect_range_names(body, names, depth + 1)?;
    match expr {
        NumericalExpression::Root {
            variable,
            bounds: None,
            ..
        } => scope.collect_range_names(&Expression::Name(variable.clone()), names, depth + 1),
        NumericalExpression::Root {
            bounds: Some((a, b)),
            ..
        } => {
            scope.collect_range_names(a, names, depth + 1)?;
            scope.collect_range_names(b, names, depth + 1)
        }
        NumericalExpression::Derivative {
            variable, order, ..
        } => {
            scope.collect_range_names(&Expression::Name(variable.clone()), names, depth + 1)?;
            scope.collect_range_names(order, names, depth + 1)
        }
        NumericalExpression::Integral { lower, upper, .. }
        | NumericalExpression::Fold { lower, upper, .. } => {
            scope.collect_range_names(lower, names, depth + 1)?;
            scope.collect_range_names(upper, names, depth + 1)
        }
    }
}

thread_local! { static BUDGET: Cell<(usize, usize)> = const { Cell::new((0, 0)) }; }
struct Guard;
impl Guard {
    fn enter() -> Result<Self> {
        BUDGET.with(|budget| {
            let (depth, remaining) = budget.get();
            if depth >= 8 {
                return Err(Fault::unsupported(
                    "Nested numerical operator depth exceeds 8",
                ));
            }
            budget.set((depth + 1, if depth == 0 { 10_000 } else { remaining }));
            Ok(Self)
        })
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        BUDGET.with(|b| {
            let (d, n) = b.get();
            b.set((d - 1, n));
        });
    }
}
fn consume() -> Result<()> {
    super::check_cancelled()?;
    BUDGET.with(|budget| {
        let (depth, remaining) = budget.get();
        if remaining == 0 {
            return Err(Fault::new(
                "numeric_nonconvergence",
                "Numerical evaluation budget exhausted",
            ));
        }
        budget.set((depth, remaining - 1));
        Ok(())
    })
}
fn finite(q: Quantity) -> Result<Quantity> {
    if q.value.re.is_finite() && q.value.im.is_finite() && q.dimension.iter().all(|d| d.is_finite())
    {
        Ok(q)
    } else {
        Err(Fault::new("numeric_domain", "Nonfinite numerical sample"))
    }
}
fn tol(scope: &Scope) -> Result<f64> {
    let id = Symbol {
        name: "TOL".into(),
        label: "SYSTEM".into(),
    };
    let q = match scope.lookup(&id)? {
        Binding::Value(v) => v.as_quantity()?,
        _ => return Err(Fault::new("numeric_domain", "TOL must be a scalar")),
    };
    if !q.is_scalar()
        || q.value.im != 0.0
        || !q.value.re.is_finite()
        || q.value.re <= 0.0
        || q.value.re > 1.0
    {
        return Err(Fault::new(
            "numeric_domain",
            "TOL must be dimensionless and in (0,1]",
        ));
    }
    Ok(q.value.re)
}
fn sample(
    scope: &mut Scope,
    body: &Expression,
    variable: &Symbol,
    point: Quantity,
    depth: usize,
) -> Result<Quantity> {
    consume()?;
    scope.bindings.insert(
        variable.clone(),
        Binding::Value(Value::scalar(finite(point)?)),
    );
    finite(scope.evaluate(body, depth + 1)?.as_quantity()?)
}
fn guess(scope: &Scope, variable: &Symbol) -> Result<Quantity> {
    match scope.lookup(variable)? {
        Binding::Value(v) => finite(v.as_quantity()?),
        _ => Err(Fault::new(
            "numeric_domain",
            "A scalar initial value is required",
        )),
    }
}
fn equal_dimension(a: Quantity, b: Quantity) -> Result<()> {
    if a.same_dimension(&b) {
        Ok(())
    } else {
        Err(Fault::new(
            "bad%_dimensions",
            "Numerical samples or bounds have incompatible units",
        ))
    }
}

pub(crate) fn evaluate(scope: &Scope, expr: &NumericalExpression, depth: usize) -> Result<Value> {
    let _guard = Guard::enter()?;
    let output = match expr {
        NumericalExpression::Root {
            body,
            variable,
            bounds,
        } => root(scope, body, variable, bounds.as_ref(), depth)?,
        NumericalExpression::Derivative {
            body,
            variable,
            order,
        } => {
            let n = scope.evaluate(order, depth + 1)?.as_quantity()?;
            if !n.is_scalar()
                || n.value.im != 0.0
                || !n.value.re.is_finite()
                || n.value.re.fract() != 0.0
                || !(0.0..=5.0).contains(&n.value.re)
            {
                return Err(Fault::new(
                    "numeric_domain",
                    "Numeric derivative order must be an integer from 0 to 5",
                ));
            }
            derivative(scope, body, variable, n.value.re as usize, depth)?
        }
        NumericalExpression::Integral {
            body,
            variable,
            lower,
            upper,
        } => {
            let a = finite(scope.evaluate(lower, depth + 1)?.as_quantity()?)?;
            let b = finite(scope.evaluate(upper, depth + 1)?.as_quantity()?)?;
            equal_dimension(a, b)?;
            if a.value.im != 0.0 || b.value.im != 0.0 {
                return Err(Fault::unsupported(
                    "Integration currently requires finite real bounds",
                ));
            }
            integral(scope, body, variable, a, b, depth)?
        }
        NumericalExpression::Fold {
            body,
            variable,
            lower,
            upper,
            product,
        } => {
            let a = finite(scope.evaluate(lower, depth + 1)?.as_quantity()?)?;
            let b = finite(scope.evaluate(upper, depth + 1)?.as_quantity()?)?;
            fold(scope, body, variable, a, b, *product, depth)?
        }
    };
    Ok(Value::scalar(finite(output)?))
}

fn fold(
    scope: &Scope,
    body: &Expression,
    variable: &Symbol,
    a: Quantity,
    b: Quantity,
    product: bool,
    depth: usize,
) -> Result<Quantity> {
    for q in [a, b] {
        if !q.is_scalar() || q.value.im != 0. || q.value.re.abs() > 9_007_199_254_740_991. {
            return Err(Fault::new(
                "numeric_domain",
                "Local sum/product bounds must be finite dimensionless real scalars",
            ));
        }
        if q.value.re.fract() != 0. {
            return Err(Fault::unsupported(
                "Noninteger summation/product limit iteration has not been reconstructed",
            ));
        }
    }
    if b.value.re < a.value.re {
        return Err(Fault::new(
            "numeric_domain",
            "Local sum/product upper bound precedes its lower bound",
        ));
    }
    let count = b.value.re - a.value.re + 1.;
    if count > 10_000. {
        return Err(Fault::new(
            "numeric_nonconvergence",
            "Local sum/product exceeds the numerical evaluation budget",
        ));
    }
    let mut local = scope.clone();
    let mut total = None;
    for index in 0..count as usize {
        let q = sample(
            &mut local,
            body,
            variable,
            Quantity::scalar(a.value.re + index as f64),
            depth,
        )?;
        total = Some(match total {
            None => q,
            Some(previous) => {
                let previous: Quantity = previous;
                if product {
                    previous.multiply(q, false)
                } else {
                    previous.add(q, false)
                }
                .map_err(|e| Fault::new("numeric_domain", e))?
            }
        });
    }
    finite(total.expect("inclusive ordered integer bounds have at least one value"))
}

fn root(
    scope: &Scope,
    body: &Expression,
    variable: &Symbol,
    bounds: Option<&(Box<Expression>, Box<Expression>)>,
    depth: usize,
) -> Result<Quantity> {
    let tolerance = tol(scope)?.min(1e-5);
    let mut local = scope.clone();
    if let Some((lo, hi)) = bounds {
        let mut a = finite(scope.evaluate(lo, depth + 1)?.as_quantity()?)?;
        let mut b = finite(scope.evaluate(hi, depth + 1)?.as_quantity()?)?;
        equal_dimension(a, b)?;
        if a.value.im != 0.0 || b.value.im != 0.0 || a.value.re >= b.value.re {
            return Err(Fault::new(
                "numeric_domain",
                "Root brackets require finite real a < b",
            ));
        }
        let mut fa = sample(&mut local, body, variable, a, depth)?;
        let fb = sample(&mut local, body, variable, b, depth)?;
        equal_dimension(fa, fb)?;
        if fa.value.im != 0.0 || fb.value.im != 0.0 {
            return Err(Fault::unsupported(
                "Bracketed root requires real function samples",
            ));
        }
        if fa.value.re == 0.0 {
            return Ok(a);
        }
        if fb.value.re == 0.0 {
            return Ok(b);
        }
        if fa.value.re.is_sign_positive() == fb.value.re.is_sign_positive() {
            return Err(Fault::new(
                "numeric_domain",
                "Root bracket endpoints must have opposite signs",
            ));
        }
        for _ in 0..256 {
            let mut mid = a;
            mid.value.re = a.value.re * 0.5 + b.value.re * 0.5;
            let fm = sample(&mut local, body, variable, mid, depth)?;
            equal_dimension(fa, fm)?;
            if fm.value.im != 0.0 {
                return Err(Fault::unsupported("Bracketed root sample became complex"));
            }
            if fm.value.norm() <= tolerance {
                return Ok(mid);
            }
            if mid.value.re == a.value.re || mid.value.re == b.value.re {
                break;
            }
            if fa.value.re.is_sign_positive() == fm.value.re.is_sign_positive() {
                a = mid;
                fa = fm;
            } else {
                b = mid;
            }
        }
    } else {
        let mut z0 = guess(scope, variable)?;
        let h = 0.01 * (z0.value.norm() + 1.0);
        let mut z1 = z0;
        z1.value += Complex64::new(h, if z0.value.im == 0.0 { 0.0 } else { 0.1 * h });
        let mut f0 = sample(&mut local, body, variable, z0, depth)?;
        if f0.value.norm() <= tolerance {
            return Ok(z0);
        }
        for _ in 0..128 {
            let f1 = sample(&mut local, body, variable, z1, depth)?;
            equal_dimension(f0, f1)?;
            if f1.value.norm() <= tolerance {
                return Ok(z1);
            }
            let delta = f1.value - f0.value;
            if delta.norm() == 0.0 {
                break;
            }
            let next = z1.value - f1.value * (z1.value - z0.value) / delta;
            z0 = z1;
            f0 = f1;
            z1.value = next;
        }
    }
    Err(Fault::new(
        "numeric_nonconvergence",
        "Root solver did not produce a sufficiently small residual",
    ))
}

fn derivative(
    scope: &Scope,
    body: &Expression,
    variable: &Symbol,
    order: usize,
    depth: usize,
) -> Result<Quantity> {
    let point = guess(scope, variable)?;
    let mut local = scope.clone();
    if order == 0 {
        return sample(&mut local, body, variable, point, depth);
    }
    let weights = difference_weights(order)?;
    let mut h = (point.value.norm().max(1.0)) * 0.25;
    let mut previous: Option<Complex64> = None;
    let mut best: Option<(f64, Complex64)> = None;
    let mut dimension = None;
    let target = 1e-8 * 10_f64.powi(order as i32 - 1);
    for _ in 0..12 {
        let mut value = Complex64::new(0.0, 0.0);
        for (i, w) in weights.iter().enumerate() {
            let mut x = point;
            x.value.re += (i as f64 - 4.0) * h;
            let f = sample(&mut local, body, variable, x, depth)?;
            if let Some(d) = dimension {
                equal_dimension(
                    Quantity {
                        dimension: d,
                        value: f.value,
                    },
                    f,
                )?;
            } else {
                dimension = Some(f.dimension);
            }
            value += f.value * w;
        }
        value /= h.powi(order as i32);
        if let Some(old) = previous {
            let error = (value - old).norm();
            if best.as_ref().is_none_or(|b| error < b.0) {
                best = Some((error, value));
            }
            if error <= target * value.norm().max(1.0) {
                let dim = dimension.unwrap();
                return Ok(Quantity {
                    value,
                    dimension: std::array::from_fn(|i| dim[i] - point.dimension[i] * order as f64),
                });
            }
        }
        previous = Some(value);
        h /= 1.4;
    }
    let (error, value) = best
        .ok_or_else(|| Fault::new("numeric_nonconvergence", "Derivative estimate unavailable"))?;
    if error > target * value.norm().max(1.0) * 10.0 {
        return Err(Fault::new(
            "numeric_nonconvergence",
            "Derivative estimates did not stabilize",
        ));
    }
    let dim = dimension.unwrap();
    Ok(Quantity {
        value,
        dimension: std::array::from_fn(|i| dim[i] - point.dimension[i] * order as f64),
    })
}

// Solve the moment equations for a centered nine-point derivative stencil.
// This is an independent generic construction, not a product coefficient table.
fn difference_weights(order: usize) -> Result<[f64; 9]> {
    let mut rows = [[0.0; 10]; 9];
    for (power, row) in rows.iter_mut().enumerate() {
        for (i, v) in row.iter_mut().take(9).enumerate() {
            *v = (i as f64 - 4.0).powi(power as i32);
        }
        if power == order {
            row[9] = (1..=order).product::<usize>() as f64;
        }
    }
    for column in 0..9 {
        let pivot = (column..9)
            .max_by(|&a, &b| rows[a][column].abs().total_cmp(&rows[b][column].abs()))
            .unwrap();
        rows.swap(column, pivot);
        let scale = rows[column][column];
        if scale == 0.0 {
            return Err(Fault::new(
                "numeric_domain",
                "Degenerate derivative stencil",
            ));
        }
        for value in &mut rows[column][column..] {
            *value /= scale;
        }
        let pivot_row = rows[column];
        for (i, row) in rows.iter_mut().enumerate() {
            if i != column {
                let factor = row[column];
                for (value, pivot_value) in row[column..].iter_mut().zip(&pivot_row[column..]) {
                    *value -= factor * pivot_value;
                }
            }
        }
    }
    Ok(std::array::from_fn(|i| rows[i][9]))
}

#[derive(Clone, Copy)]
struct Segment {
    a: f64,
    b: f64,
    fa: Complex64,
    fm: Complex64,
    fb: Complex64,
    whole: Complex64,
    tolerance: f64,
    depth: usize,
}
fn simpson(a: f64, b: f64, fa: Complex64, fm: Complex64, fb: Complex64) -> Complex64 {
    (fa + 4.0 * fm + fb) * ((b - a) / 6.0)
}
fn integral(
    scope: &Scope,
    body: &Expression,
    variable: &Symbol,
    a: Quantity,
    b: Quantity,
    depth: usize,
) -> Result<Quantity> {
    let mut local = scope.clone();
    let mut output_dimension = None;
    let mut eval = |x: f64| -> Result<Complex64> {
        let mut point = a;
        point.value.re = x;
        let value = sample(&mut local, body, variable, point, depth)?;
        if let Some(d) = output_dimension {
            equal_dimension(
                Quantity {
                    value: value.value,
                    dimension: d,
                },
                value,
            )?;
        } else {
            output_dimension = Some(value.dimension);
        }
        Ok(value.value)
    };
    let x = a.value.re;
    let y = b.value.re;
    let middle = x * 0.5 + y * 0.5;
    let fa = eval(x)?;
    let fm = eval(middle)?;
    let fb = eval(y)?;
    let whole = simpson(x, y, fa, fm, fb);
    let tolerance = tol(scope)? * whole.norm().max(1.0);
    let mut stack = vec![Segment {
        a: x,
        b: y,
        fa,
        fm,
        fb,
        whole,
        tolerance,
        depth: 0,
    }];
    let mut total = Complex64::new(0.0, 0.0);
    while let Some(s) = stack.pop() {
        let middle = s.a * 0.5 + s.b * 0.5;
        let left = s.a * 0.5 + middle * 0.5;
        let right = middle * 0.5 + s.b * 0.5;
        let fl = eval(left)?;
        let fr = eval(right)?;
        let sl = simpson(s.a, middle, s.fa, fl, s.fm);
        let sr = simpson(middle, s.b, s.fm, fr, s.fb);
        let delta = sl + sr - s.whole;
        if delta.norm() <= 15.0 * s.tolerance {
            total += sl + sr + delta / 15.0;
            continue;
        }
        if s.depth >= 20 || left == s.a || right == s.b {
            return Err(Fault::new(
                "numeric_nonconvergence",
                "Integral subdivision did not converge",
            ));
        }
        stack.push(Segment {
            a: middle,
            b: s.b,
            fa: s.fm,
            fm: fr,
            fb: s.fb,
            whole: sr,
            tolerance: s.tolerance * 0.5,
            depth: s.depth + 1,
        });
        stack.push(Segment {
            a: s.a,
            b: middle,
            fa: s.fa,
            fm: fl,
            fb: s.fm,
            whole: sl,
            tolerance: s.tolerance * 0.5,
            depth: s.depth + 1,
        });
    }
    let dim = output_dimension.unwrap();
    Ok(Quantity {
        value: total,
        dimension: std::array::from_fn(|i| dim[i] + a.dimension[i]),
    })
}

pub(crate) fn is_builtin(name: &str) -> bool {
    name == "polyroots"
}

/// Coefficients use vector storage order, constant first, independently of
/// ORIGIN. The original wrapper takes ORIGIN to interpret its indexed value;
/// this representation already exposes the ordered element storage directly.
pub(crate) fn call(name: &str, args: &[Value]) -> Result<Value> {
    if !is_builtin(name) {
        return Err(Fault::unsupported("Unknown numerical builtin"));
    }
    let [
        Value::Matrix {
            rows,
            cols: 1,
            elements,
        },
    ] = args
    else {
        return Err(Fault::new(
            "numeric_domain",
            "polyroots requires one coefficient column vector",
        ));
    };
    if !(2..=99).contains(rows) || elements.len() != *rows {
        return Err(Fault::new(
            "numeric_domain",
            "polyroots requires between 2 and 99 coefficients",
        ));
    }
    for q in elements {
        finite(*q)?;
        equal_dimension(elements[0], *q)?;
    }
    let mut coefficients: Vec<_> = elements.iter().map(|q| q.value).collect();
    if coefficients.last().is_none_or(|q| q.norm() == 0.) {
        return Err(Fault::new(
            "numeric_domain",
            "The highest polynomial coefficient must be nonzero",
        ));
    }
    let scale = coefficients.iter().map(|q| q.norm()).fold(0_f64, f64::max);
    for q in &mut coefficients {
        *q /= scale;
    }
    let original = coefficients.clone();
    let _guard = Guard::enter()?;
    let mut roots = Vec::with_capacity(rows - 1);
    while coefficients.len() > 2 {
        let root = laguerre(&coefficients, Complex64::new(0., 0.))?;
        roots.push(root);
        let n = coefficients.len() - 1;
        let mut quotient = vec![Complex64::new(0., 0.); n];
        quotient[n - 1] = coefficients[n];
        for j in (1..n).rev() {
            quotient[j - 1] = coefficients[j] + root * quotient[j];
        }
        if quotient
            .iter()
            .any(|q| !q.re.is_finite() || !q.im.is_finite())
        {
            return Err(Fault::new(
                "numeric_nonconvergence",
                "Polynomial deflation overflowed",
            ));
        }
        coefficients = quotient;
    }
    let final_root = -coefficients[0] / coefficients[1];
    roots.push(final_root);
    let mut output = Vec::with_capacity(roots.len());
    for root in roots {
        // Polish against the original polynomial to limit deflation drift.
        let value = laguerre(&original, root)?;
        output.push(finite(Quantity {
            value,
            dimension: [0.; 7],
        })?);
    }
    Ok(Value::Matrix {
        rows: output.len(),
        cols: 1,
        elements: output,
    })
}

// Independent implementation of complex Laguerre iteration. It uses no PTC
// coefficient tables, native code, or recovered implementation bodies.
fn laguerre(coefficients: &[Complex64], mut x: Complex64) -> Result<Complex64> {
    let n = (coefficients.len() - 1) as f64;
    for iteration in 0..128 {
        consume()?;
        let mut p = *coefficients.last().unwrap();
        let mut first = Complex64::new(0., 0.);
        let mut second = first;
        let mut bound = p.norm();
        for a in coefficients[..coefficients.len() - 1].iter().rev() {
            second = second * x + 2. * first;
            first = first * x + p;
            p = p * x + a;
            bound = bound * x.norm() + a.norm();
        }
        if !p.re.is_finite() || !p.im.is_finite() || !bound.is_finite() {
            return Err(Fault::new(
                "numeric_nonconvergence",
                "Polynomial evaluation overflowed",
            ));
        }
        if p.norm() <= 1e-13 * bound.max(f64::MIN_POSITIVE) {
            return Ok(x);
        }
        let g = first / p;
        let h = g * g - second / p;
        let radical = ((n - 1.) * (n * h - g * g)).sqrt();
        let plus = g + radical;
        let minus = g - radical;
        let denominator = if plus.norm() > minus.norm() {
            plus
        } else {
            minus
        };
        let step = if denominator.norm() == 0. {
            Complex64::from_polar(
                1. + x.norm(),
                (iteration as f64 + 1.) * 2.399_963_229_728_653,
            )
        } else {
            Complex64::new(n, 0.) / denominator
        };
        x -= step;
        if !x.re.is_finite() || !x.im.is_finite() {
            return Err(Fault::new(
                "numeric_nonconvergence",
                "Polynomial root iteration became nonfinite",
            ));
        }
    }
    Err(Fault::new(
        "numeric_nonconvergence",
        "Polynomial root iteration did not converge",
    ))
}
