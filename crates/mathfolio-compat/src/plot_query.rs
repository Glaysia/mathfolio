//! Prepared plot queries retain the existing resolved calculation IR.
//! Sampling replaces only outer value-name nodes; it never writes a Scope or
//! descends into function bodies/binders. No text/CoreExpr roundtrip is used.
use super::*;

const MAX_PREPARED_ELEMENTS: usize = 8192;

/// Opaque query prepared against a read-only CalculationContext. Preparing and
/// sampling cannot declare bindings or evaluate worksheet source definitions.
/// Initial budget:8MiB input,8192 labelled MathML elements, existing128 parse
/// depth. Sampling clones only this bounded query IR, once per sample.
#[derive(Debug)]
pub struct PreparedPlotQuery {
    expression: Expression,
    display_unit: Option<Expression>,
    sample_fault: Option<Fault>,
}
impl PreparedPlotQuery {
    /// Full resolved identity, including the calculation label and literal
    /// suffix separator. Call targets and display labels are not value names.
    pub fn root_symbol(&self) -> Option<&Symbol> {
        match &self.expression {
            Expression::Name(symbol) => Some(symbol),
            _ => None,
        }
    }
    /// Whether a supported outer value node matches this exact identity.
    /// Binder-bearing/ambiguous sample forms report unsupported, not false.
    pub fn contains_value_symbol(&self, symbol: &Symbol) -> Result<bool> {
        if let Some(fault) = &self.sample_fault {
            return Err(fault.clone());
        }
        contains(&self.expression, symbol, 0)
    }
}

fn resolve(context: &CalculationContext, xml: &str, bounded: bool) -> Result<PreparedPlotQuery> {
    if xml.len() > 8 * 1024 * 1024 {
        return Err(Fault::unsupported("MathML query exceeds 8 MiB"));
    }
    let labelled = editor_bridge::label_fragment(xml, &mut context.labels.clone())
        .map_err(Fault::unsupported)?;
    let wrapped = format!("<context xmlns:ml=\"{ML}\">{labelled}</context>");
    let document = roxmltree::Document::parse(&wrapped)
        .map_err(|error| Fault::unsupported(error.to_string()))?;
    let roots = children(document.root_element());
    let [root] = roots.as_slice() else {
        return Err(Fault::unsupported("Expected one evaluation query"));
    };
    if bounded && root.descendants().filter(Node::is_element).count() > MAX_PREPARED_ELEMENTS {
        return Err(Fault::unsupported(
            "Prepared plot query exceeds 8192 element budget",
        ));
    }
    let Statement::Evaluate {
        expression,
        display_unit,
    } = parse_statement(*root)?
    else {
        return Err(Fault::unsupported(
            "A query cannot define worksheet bindings",
        ));
    };
    // Unsupported substitution does not prohibit an ordinary prepared query.
    let sample_fault = if bounded {
        sample_safe(&context.scope, &expression, 0).err()
    } else {
        None
    };
    Ok(PreparedPlotQuery {
        expression,
        display_unit,
        sample_fault,
    })
}

pub(super) fn evaluate_mathml(context: &CalculationContext, xml: &str) -> Result<Value> {
    // No new element/substitution guards are imposed on the old query API.
    let query = resolve(context, xml, false)?;
    context.evaluate_prepared(&query)
}
impl CalculationContext {
    pub fn prepare_query(&self, xml: &str) -> Result<PreparedPlotQuery> {
        resolve(self, xml, true)
    }
    /// Evaluate the same resolved query using existing output/display semantics.
    pub fn evaluate_prepared(&self, query: &PreparedPlotQuery) -> Result<Value> {
        let value = self.scope.evaluate_output(&query.expression)?;
        self.scope
            .display(value, &query.display_unit)
            .map(|(value, _)| value)
    }
    /// Substitute exact value-name occurrences in the outer query only. In
    /// particular, function callees/captured scopes and unit names are intact.
    pub fn evaluate_prepared_with_sample(
        &self,
        query: &PreparedPlotQuery,
        symbol: &Symbol,
        sample: Quantity,
    ) -> Result<Value> {
        if let Some(fault) = &query.sample_fault {
            return Err(fault.clone());
        }
        if symbol.label != "VARIABLE" {
            return Err(Fault::unsupported(
                "Plot sample identity must resolve as VARIABLE",
            ));
        }
        if !sample.value.re.is_finite()
            || !sample.value.im.is_finite()
            || sample.value.im != 0.
            || sample.dimension.iter().any(|d| !d.is_finite())
        {
            return Err(Fault::unsupported(
                "Plot sample must be a finite real physical quantity",
            ));
        }
        // A query can be evaluated against another explicit context. Recheck
        // dynamic SCALE function roles rather than trust the preparation scope.
        sample_safe(&self.scope, &query.expression, 0)?;
        let mut expression = query.expression.clone();
        substitute(&mut expression, symbol, sample, 0)?;
        let value = self.scope.evaluate_output(&expression)?;
        self.scope
            .display(value, &query.display_unit)
            .map(|(value, _)| value)
    }
}

fn sample_safe(scope: &Scope, expression: &Expression, depth: usize) -> Result<()> {
    if depth > 128 {
        return Err(Fault::unsupported("Plot sample traversal depth exceeded"));
    }
    let visit = |e| sample_safe(scope, e, depth + 1);
    match expression {
        Expression::Scalar(_) | Expression::StringLiteral(_) | Expression::Name(_) => Ok(()),
        Expression::Neg(e) | Expression::Absolute(e) | Expression::Unary(_, e) => visit(e),
        Expression::Binary(op, _, rhs)
            if op == "scale" && scope.scaling_function(rhs).is_some() =>
        {
            Err(Fault::unsupported(
                "Plot sampling through function-role SCALE is not supported",
            ))
        }
        Expression::Binary(_, a, b) | Expression::Root(a, b) => {
            visit(a)?;
            visit(b)
        }
        Expression::Call(_, args) | Expression::Matrix { elements: args, .. } => {
            for arg in args {
                visit(arg)?;
            }
            Ok(())
        }
        Expression::Index(base, indices) => {
            visit(base)?;
            for index in indices {
                visit(index)?;
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
        Expression::Program(_)
        | Expression::Numerical(_)
        | Expression::Symbolic(_)
        | Expression::Logic(_) => Err(Fault::unsupported(
            "Plot sampling through binder/control/symbolic forms is not supported",
        )),
        Expression::Unsupported(fault) => Err(fault.clone()),
    }
}
fn contains(expression: &Expression, symbol: &Symbol, depth: usize) -> Result<bool> {
    if depth > 128 {
        return Err(Fault::unsupported("Plot sample traversal depth exceeded"));
    }
    let visit = |e| contains(e, symbol, depth + 1);
    match expression {
        Expression::Scalar(_) | Expression::StringLiteral(_) => Ok(false),
        Expression::Name(name) => Ok(name == symbol),
        Expression::Neg(e) | Expression::Absolute(e) | Expression::Unary(_, e) => visit(e),
        Expression::Binary(_, a, b) | Expression::Root(a, b) => Ok(visit(a)? || visit(b)?),
        Expression::Call(_, args) | Expression::Matrix { elements: args, .. } => {
            let mut found = false;
            for arg in args {
                found |= visit(arg)?;
            }
            Ok(found)
        }
        Expression::Index(base, indices) => {
            let mut found = visit(base)?;
            for index in indices {
                found |= visit(index)?;
            }
            Ok(found)
        }
        Expression::Range { start, second, end } => {
            Ok(visit(start)? || second.as_ref().map_or(Ok(false), |e| visit(e))? || visit(end)?)
        }
        _ => Err(Fault::unsupported("Unsupported plot value-name traversal")),
    }
}
fn substitute(
    expression: &mut Expression,
    symbol: &Symbol,
    sample: Quantity,
    depth: usize,
) -> Result<()> {
    if depth > 128 {
        return Err(Fault::unsupported("Plot sample traversal depth exceeded"));
    }
    let visit = |e| substitute(e, symbol, sample, depth + 1);
    match expression {
        Expression::Name(name) if name == symbol => *expression = Expression::Scalar(sample),
        Expression::Scalar(_) | Expression::Name(_) => (),
        Expression::Neg(e) | Expression::Absolute(e) | Expression::Unary(_, e) => visit(e)?,
        Expression::Binary(_, a, b) | Expression::Root(a, b) => {
            visit(a)?;
            visit(b)?;
        }
        Expression::Call(_, args) | Expression::Matrix { elements: args, .. } => {
            for arg in args {
                visit(arg)?;
            }
        }
        Expression::Index(base, indices) => {
            visit(base)?;
            for index in indices {
                visit(index)?;
            }
        }
        Expression::Range { start, second, end } => {
            visit(start)?;
            if let Some(second) = second {
                visit(second)?;
            }
            visit(end)?;
        }
        _ => return Err(Fault::unsupported("Unsupported plot sample substitution")),
    }
    Ok(())
}

#[cfg(test)]
#[path = "plot_query_tests.rs"]
mod tests;
