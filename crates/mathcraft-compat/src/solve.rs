//! Native worksheet50 solve-block subset. The iteration below is independent
//! code, not the original solver; see docs/analysis/0011-solve-semantics.md.
use super::{
    Binding, Expression, Fault, PlacementGrid, Result, Scope, Statement, Symbol, Value, WS,
    children, is_math, parse_expression, parse_statement,
};
use mathcraft_core::Quantity;
use roxmltree::Node;
use std::collections::HashSet;

#[derive(Clone, Debug)]
struct Constraint {
    left: Expression,
    right: Expression,
}
#[derive(Clone, Debug)]
struct Solver {
    variables: Vec<Symbol>,
    target: Option<Symbol>,
    show_result: bool,
    display_unit: Option<Expression>,
    minerr: bool,
}
#[derive(Clone, Debug)]
pub(crate) struct SolveBlock {
    statements: Vec<Statement>,
    constraints: Vec<Constraint>,
    solver: Option<Solver>,
    result_ref: Option<String>,
    declared_outputs: Vec<Symbol>,
    fault: Option<Fault>,
}
impl SolveBlock {
    pub(crate) fn exports(&self) -> Vec<Symbol> {
        self.declared_outputs.clone()
    }
    pub(crate) fn solver_result_ref(&self) -> Option<String> {
        self.result_ref.clone()
    }
}

fn math_root<'a, 'input>(math: Node<'a, 'input>) -> Result<Node<'a, 'input>> {
    super::check_attributes(math, &["resultRef"])?;
    let all = children(math);
    let mut roots = all
        .iter()
        .copied()
        .filter(|n| n.tag_name().namespace() == Some(super::ML));
    let root = roots
        .next()
        .ok_or_else(|| Fault::unsupported("Missing solve-block math root"))?;
    if roots.next().is_some()
        || all.iter().any(|n| {
            n.tag_name().namespace() != Some(super::ML)
                && !n.has_tag_name((WS, "resultFormat"))
                && !n.has_tag_name((WS, "formattingOverride"))
        })
    {
        return Err(Fault::unsupported("Unknown solve-block math sibling"));
    }
    Ok(root)
}
fn solver_name(name: &Symbol) -> bool {
    name.label == "KEYWORD"
        && matches!(
            name.name.as_str(),
            "find"
                | "Find"
                | "minerr"
                | "Minerr"
                | "minimize"
                | "Minimize"
                | "maximize"
                | "Maximize"
                | "odesolve"
                | "Odesolve"
        )
}
fn solver_expression(statement: &Statement) -> Option<&Expression> {
    match statement {
        Statement::Define { body, .. } => Some(body),
        Statement::Evaluate { expression, .. } => Some(expression),
        _ => None,
    }
}
fn solver(statement: &Statement) -> Result<Solver> {
    let (target, show_result, display_unit) = match statement {
        Statement::Define {
            symbol,
            parameters: None,
            global: false,
            show_result,
            display_unit,
            ..
        } => (Some(symbol.clone()), *show_result, display_unit.clone()),
        Statement::Evaluate { display_unit, .. } => (None, true, display_unit.clone()),
        _ => {
            return Err(Fault::unsupported(
                "Parameterized/global/tuple solve-block result definitions are not implemented",
            ));
        }
    };
    let Some(Expression::Call(name, args)) = solver_expression(statement) else {
        return Err(Fault::unsupported(
            "Solver must be the outer expression of an evaluation or definition",
        ));
    };
    if !solver_name(name) || !matches!(name.name.as_str(), "find" | "Find" | "minerr" | "Minerr") {
        return Err(Fault::unsupported(
            "This solve-block subset implements Find and Minerr equalities only",
        ));
    }
    let mut seen = HashSet::new();
    let mut variables = Vec::new();
    for arg in args {
        let Expression::Name(var) = arg else {
            return Err(Fault::unsupported(
                "Scalar solver unknowns must be identifiers",
            ));
        };
        if var.label != "VARIABLE" || !seen.insert(var.clone()) {
            return Err(Fault::unsupported(
                "Solver unknowns require unique Variable-labelled identifiers",
            ));
        }
        variables.push(var.clone());
    }
    if variables.is_empty() || variables.len() > 16 {
        return Err(Fault::unsupported(
            "Solver unknown count must be between 1 and 16",
        ));
    }
    Ok(Solver {
        variables,
        target,
        show_result,
        display_unit,
        minerr: matches!(name.name.as_str(), "minerr" | "Minerr"),
    })
}

pub(crate) fn parse_block(node: Node<'_, '_>, grid: Option<PlacementGrid>) -> Result<SolveBlock> {
    if !node.has_tag_name((WS, "solveblock")) {
        return Err(Fault::unsupported("Expected native worksheet50 solveblock"));
    }
    let mut block = SolveBlock {
        statements: vec![],
        constraints: vec![],
        solver: None,
        result_ref: None,
        declared_outputs: vec![],
        fault: None,
    };
    // Preserve the declared result even if another child is unsupported. A
    // failed solve must poison an old value of its output instead of reusing it.
    for math in node.descendants().filter(|n| n.has_tag_name((WS, "math"))) {
        let Some(root) = math
            .children()
            .find(|n| is_math(*n, "define") || is_math(*n, "globalDefine"))
        else {
            continue;
        };
        let parts = children(root);
        let [lhs, rhs] = parts.as_slice() else {
            continue;
        };
        let known_solver = rhs
            .descendants()
            .filter(|n| is_math(*n, "apply"))
            .filter_map(|n| children(n).first().copied())
            .filter_map(|n| super::symbol(n).ok())
            .any(|n| solver_name(&n));
        if known_solver
            && let Ok(symbol) = super::symbol(*lhs)
            && !block.declared_outputs.contains(&symbol)
        {
            block.declared_outputs.push(symbol);
        }
    }
    let parsed = (|| {
        super::check_attributes(node, &["disable-calc"])?;
        if node
            .attribute("disable-calc")
            .is_some_and(|v| v != "false" && v != "0")
        {
            return Err(Fault::unsupported("Disabled solve block is not evaluated"));
        }
        let all = children(node);
        let [regions] = all.as_slice() else {
            return Err(Fault::unsupported("Solveblock must contain regions"));
        };
        if !regions.has_tag_name((WS, "regions")) {
            return Err(Fault::unsupported("Solveblock regions namespace"));
        }
        super::check_attributes(*regions, &[])?;
        let mut items = Vec::new();
        for region in regions.children().filter(Node::is_element) {
            if !region.has_tag_name((WS, "region")) || items.len() >= 256 {
                return Err(Fault::unsupported("Solve-block region shape or budget"));
            }
            let coordinate = |name: &str| -> Result<f64> {
                let value = region
                    .attribute(name)
                    .ok_or_else(|| Fault::unsupported("Missing solve-block coordinate"))?
                    .parse::<f64>()
                    .map_err(|_| Fault::unsupported("Invalid solve-block coordinate"))?;
                if !value.is_finite() {
                    return Err(Fault::unsupported("Nonfinite solve-block coordinate"));
                }
                grid.map(|g| g.snap(value).map_err(Fault::unsupported))
                    .unwrap_or(Ok(value))
            };
            items.push((coordinate("top")?, coordinate("left")?, region));
        }
        items.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        for (_, _, region) in items {
            let category = region.attribute("solve-block-category");
            let Some(math) = region.children().find(|n| n.has_tag_name((WS, "math"))) else {
                if children(region)
                    .iter()
                    .all(|n| n.has_tag_name((WS, "text")) || n.has_tag_name((WS, "image")))
                {
                    continue;
                }
                return Err(Fault::unsupported(
                    "Nested/non-math solve-block participant",
                ));
            };
            let root = math_root(math)?;
            let ch = children(root);
            if is_math(root, "apply") && ch.first().is_some_and(|n| is_math(*n, "equal")) {
                if category.is_some_and(|v| v != "constraint") {
                    return Err(Fault::unsupported("Inconsistent constraint category"));
                }
                super::check_attributes(root, &[])?;
                if ch.len() != 3 {
                    return Err(Fault::unsupported("Constraint equality arity"));
                }
                super::check_attributes(ch[0], &[])?;
                if !children(ch[0]).is_empty() {
                    return Err(Fault::unsupported("Structured constraint operator"));
                }
                block.constraints.push(Constraint {
                    left: parse_expression(ch[1], 0)?,
                    right: parse_expression(ch[2], 0)?,
                });
                if block.constraints.len() > 64 {
                    return Err(Fault::unsupported("Constraint budget exceeded"));
                }
                continue;
            }
            let statement = parse_statement(root)?;
            let is_solver = solver_expression(&statement)
                .is_some_and(|e| matches!(e,Expression::Call(n,_) if solver_name(n)));
            if is_solver || category == Some("solver") {
                let parsed_solver = solver(&statement)?;
                block.result_ref = math.attribute("resultRef").map(str::to_string);
                block.solver = Some(parsed_solver);
                // The original aggregate includes only children before the
                // selected solver. Later children do not enter its equations.
                break;
            }
            match &statement {
                Statement::Define { global: false, .. } => {
                    if category.is_some_and(|v| v != "guess-value") {
                        return Err(Fault::unsupported(
                            "Inconsistent initial-definition category",
                        ));
                    }
                    block.statements.push(statement);
                }
                Statement::Evaluate { .. } if category.is_none() => (), // nonparticipating display
                _ => {
                    return Err(Fault::unsupported(
                        "Unsupported solve-block participant or inequality constraint",
                    ));
                }
            }
        }
        if block.solver.is_none() {
            return Err(Fault::unsupported("No supported solver in solve block"));
        }
        Ok(())
    })();
    block.fault = parsed.err();
    if block.fault.is_none() {
        block.declared_outputs = block
            .solver
            .as_ref()
            .and_then(|s| s.target.clone())
            .into_iter()
            .collect();
    }
    Ok(block)
}

pub(crate) fn evaluate(
    scope: &mut Scope,
    block: &SolveBlock,
    region: &str,
) -> Result<(Option<Value>, Option<Value>)> {
    let result = evaluate_inner(scope, block, region);
    if let Err(fault) = &result {
        for name in &block.declared_outputs {
            scope.bindings.insert(
                name.clone(),
                Binding::Failed {
                    fault: fault.clone(),
                    region: region.into(),
                },
            );
        }
    }
    result
}
fn evaluate_inner(
    scope: &mut Scope,
    block: &SolveBlock,
    region: &str,
) -> Result<(Option<Value>, Option<Value>)> {
    if let Some(fault) = &block.fault {
        return Err(fault.clone());
    }
    let solver = block
        .solver
        .as_ref()
        .ok_or_else(|| Fault::unsupported("Missing solver"))?;
    let mut local = scope.clone();
    for statement in &block.statements {
        let Statement::Define {
            symbol,
            parameters,
            body,
            global: false,
            ..
        } = statement
        else {
            return Err(Fault::unsupported("Unsupported local solver statement"));
        };
        local.define(symbol, parameters, body, region)?;
    }
    let quantities = solve_system(&local, solver, &block.constraints)?;
    let value = if quantities.len() == 1 {
        Value::scalar(quantities[0])
    } else {
        Value::Matrix {
            rows: quantities.len(),
            cols: 1,
            elements: quantities,
        }
    };
    let (value, display) = local.display(value, &solver.display_unit)?;
    if let Some(target) = &solver.target {
        scope
            .bindings
            .insert(target.clone(), Binding::Value(value.clone()));
    }
    Ok(if solver.show_result {
        (Some(value), display)
    } else {
        (None, None)
    })
}

fn number(q: Quantity) -> Result<f64> {
    if q.value.im != 0. {
        return Err(Fault::unsupported(
            "Complex solve-block unknowns/constraints are not implemented",
        ));
    }
    if !q.value.re.is_finite() || q.dimension.iter().any(|d| !d.is_finite()) {
        return Err(Fault::new("solve_domain", "Nonfinite solve-block value"));
    }
    Ok(q.value.re)
}
fn tolerance(scope: &Scope, name: &str) -> Result<f64> {
    let symbol = Symbol {
        name: name.into(),
        label: "SYSTEM".into(),
    };
    let Binding::Value(value) = scope.lookup(&symbol)? else {
        return Err(Fault::new(
            "solve_domain",
            "Solver tolerance must be scalar",
        ));
    };
    let q = value.as_quantity()?;
    let x = number(q)?;
    if !q.is_scalar() || x <= 0. || x > 1. {
        return Err(Fault::new(
            "solve_domain",
            "Solver tolerance must be dimensionless and in (0,1]",
        ));
    }
    Ok(x)
}
struct Residuals<'a> {
    local: Scope,
    variables: &'a [Symbol],
    constraints: &'a [Constraint],
    dimensions: Vec<[f64; 7]>,
    scales: Vec<f64>,
    evaluations: usize,
}
impl Residuals<'_> {
    fn at(&mut self, x: &[Quantity]) -> Result<Vec<f64>> {
        if self.evaluations >= 10000 {
            return Err(Fault::new(
                "solve_nonconvergence",
                "Solve-block residual budget exhausted",
            ));
        }
        self.evaluations += 1;
        for (symbol, q) in self.variables.iter().zip(x) {
            number(*q)?;
            self.local
                .bindings
                .insert(symbol.clone(), Binding::Value(Value::scalar(*q)));
        }
        let mut samples = Vec::with_capacity(self.constraints.len());
        for (i, c) in self.constraints.iter().enumerate() {
            let a = self.local.evaluate(&c.left, 0)?.as_quantity()?;
            let b = self.local.evaluate(&c.right, 0)?.as_quantity()?;
            let av = number(a)?;
            let bv = number(b)?;
            if !a.same_dimension(&b) {
                return Err(Fault::new(
                    "bad%_dimensions",
                    "Constraint equality operands have incompatible units",
                ));
            }
            if self.dimensions.get(i).is_some_and(|dimension| {
                !a.same_dimension(&Quantity {
                    value: a.value,
                    dimension: *dimension,
                })
            }) {
                return Err(Fault::new(
                    "bad%_dimensions",
                    "Constraint dimensions changed during iteration",
                ));
            }
            samples.push((a, av, bv));
        }
        if self.dimensions.is_empty() {
            for (a, _, _) in &samples {
                self.dimensions.push(a.dimension);
                // Use a common scale for equal-dimensional equations, so
                // least-squares terms with the same units keep equal weight.
                self.scales.push(
                    samples
                        .iter()
                        .filter(|(b, _, _)| a.same_dimension(b))
                        .map(|(_, av, bv)| av.abs().max(bv.abs()))
                        .fold(1., f64::max),
                );
            }
        }
        Ok(samples
            .iter()
            .zip(&self.scales)
            .map(|((_, av, bv), scale)| (av - bv) / scale)
            .collect())
    }
}
fn norm(values: &[f64]) -> f64 {
    values.iter().map(|x| x * x).sum::<f64>().sqrt()
}
fn solve_system(
    scope: &Scope,
    solver: &Solver,
    constraints: &[Constraint],
) -> Result<Vec<Quantity>> {
    let n = solver.variables.len();
    let m = constraints.len();
    if m == 0 || (!solver.minerr && m != n) || (solver.minerr && m < n) {
        return Err(Fault::unsupported(
            "Find requires a square equality system; Minerr requires at least as many equalities as unknowns",
        ));
    }
    let mut x = Vec::with_capacity(n);
    for variable in &solver.variables {
        let Binding::Value(value) = scope.lookup(variable)? else {
            return Err(Fault::new(
                "solve_domain",
                "A scalar initial guess is required for every unknown",
            ));
        };
        let q = value.as_quantity()?;
        number(q)?;
        x.push(q);
    }
    let ctol = tolerance(scope, "CTOL")?;
    let tol = tolerance(scope, "TOL")?;
    let mut residuals = Residuals {
        local: scope.clone(),
        variables: &solver.variables,
        constraints,
        dimensions: vec![],
        scales: vec![],
        evaluations: 0,
    };
    let mut r = residuals.at(&x)?;
    let scales: Vec<_> = x.iter().map(|q| q.value.re.abs().max(1.)).collect();
    let mut damping = 1e-6;
    for _ in 0..100 {
        if r.iter().all(|v| v.abs() <= ctol) {
            return Ok(x);
        }
        let mut jacobian = vec![vec![0.; n]; m];
        for (j, scale) in scales.iter().enumerate() {
            let h = f64::EPSILON.cbrt() * (x[j].value.re.abs() + scale);
            let mut plus = x.clone();
            plus[j].value.re += h;
            let mut minus = x.clone();
            minus[j].value.re -= h;
            let rp = residuals.at(&plus)?;
            let rm = residuals.at(&minus)?;
            for (i, row) in jacobian.iter_mut().enumerate() {
                row[j] = (rp[i] - rm[i]) / (2. * h) * scale;
            }
        }
        let (matrix, rhs) = if solver.minerr {
            let mut matrix = vec![vec![0.; n]; n];
            let mut rhs = vec![0.; n];
            for (j, row) in matrix.iter_mut().enumerate() {
                for (k, entry) in row.iter_mut().enumerate() {
                    *entry = jacobian.iter().map(|v| v[j] * v[k]).sum();
                }
                row[j] += damping;
                rhs[j] = -jacobian.iter().zip(&r).map(|(v, r)| v[j] * r).sum::<f64>();
            }
            if norm(&rhs) <= tol.min(1e-8) {
                return Ok(x);
            }
            (matrix, rhs)
        } else {
            (jacobian, r.iter().map(|v| -v).collect())
        };
        let step = linear_solve(matrix, rhs)?;
        let old_norm = norm(&r);
        let mut accepted = None;
        let mut alpha = 1.;
        for _ in 0..16 {
            let mut next = x.clone();
            for ((q, s), scale) in next.iter_mut().zip(&step).zip(&scales) {
                q.value.re += alpha * s * scale;
            }
            match residuals.at(&next) {
                Ok(next_r) if norm(&next_r) < old_norm => {
                    accepted = Some((next, next_r));
                    break;
                }
                Ok(_) => (),
                Err(fault) if fault.unsupported || fault.code == "bad%_dimensions" => {
                    return Err(fault);
                }
                Err(_) => (),
            }
            alpha *= 0.5;
        }
        if let Some((next, next_r)) = accepted {
            let improvement = old_norm - norm(&next_r);
            x = next;
            r = next_r;
            damping = (damping * 0.3).max(1e-12);
            if solver.minerr && improvement <= tol.min(1e-10) * old_norm.max(1.) {
                return Ok(x);
            }
        } else if solver.minerr {
            damping *= 10.;
            if damping > 1e12 {
                return Ok(x);
            }
        } else {
            return Err(Fault::new(
                "solve_nonconvergence",
                "Find failed to decrease the equality residual",
            ));
        }
    }
    if solver.minerr {
        // Shipped help explicitly distinguishes Minerr by returning the last
        // iterate when its iteration limit is reached without convergence.
        Ok(x)
    } else {
        Err(Fault::new(
            "solve_nonconvergence",
            "Solve-block iteration budget exhausted",
        ))
    }
}
fn linear_solve(mut matrix: Vec<Vec<f64>>, mut rhs: Vec<f64>) -> Result<Vec<f64>> {
    let n = rhs.len();
    for j in 0..n {
        let pivot = (j..n)
            .max_by(|&a, &b| matrix[a][j].abs().total_cmp(&matrix[b][j].abs()))
            .unwrap();
        if !matrix[pivot][j].is_finite() || matrix[pivot][j].abs() < 1e-14 {
            return Err(Fault::new(
                "solve_nonconvergence",
                "Singular numerical solve-block Jacobian",
            ));
        }
        matrix.swap(j, pivot);
        rhs.swap(j, pivot);
        let factor = matrix[j][j];
        for v in &mut matrix[j][j..] {
            *v /= factor;
        }
        rhs[j] /= factor;
        let row = matrix[j].clone();
        let value = rhs[j];
        for (i, other) in matrix.iter_mut().enumerate() {
            if i == j {
                continue;
            }
            let factor = other[j];
            for (a, b) in other[j..].iter_mut().zip(&row[j..]) {
                *a -= factor * b;
            }
            rhs[i] -= factor * value;
        }
    }
    if rhs.iter().any(|x| !x.is_finite()) {
        return Err(Fault::new(
            "solve_nonconvergence",
            "Nonfinite solve-block step",
        ));
    }
    Ok(rhs)
}
