//! Independent bounded exact polynomial algebra, not the original CAS.
//! Source contracts and limitations: docs/analysis/0012-symbolics-semantics.md.
use crate::{
    Binding, Fault, ML, Result, Scope, Symbol, Value, check_attributes, children, is_math,
};
use roxmltree::Node;
use std::collections::{BTreeMap, BTreeSet};

const MAX_TERMS: usize = 512;
const MAX_POWER: u32 = 32;

fn bad(message: &str) -> Fault {
    Fault::unsupported(message)
}
fn overflow() -> Fault {
    bad("Exact symbolic rational overflow")
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Rational {
    n: i128,
    d: i128,
}
impl Rational {
    const ZERO: Self = Self { n: 0, d: 1 };
    const ONE: Self = Self { n: 1, d: 1 };
    fn new(n: i128, d: i128) -> Result<Self> {
        if d == 0 {
            return Err(Fault::new(
                "divide_by_zero",
                "Exact symbolic division by zero",
            ));
        }
        if n == i128::MIN || d == i128::MIN {
            return Err(overflow());
        }
        let (n, d) = if d < 0 { (-n, -d) } else { (n, d) };
        let mut a = n.abs();
        let mut b = d;
        while b != 0 {
            let r = a % b;
            a = b;
            b = r;
        }
        Ok(Self { n: n / a, d: d / a })
    }
    fn neg(self) -> Self {
        Self {
            n: -self.n,
            d: self.d,
        }
    }
    fn add(self, other: Self) -> Result<Self> {
        Self::new(
            self.n
                .checked_mul(other.d)
                .and_then(|a| other.n.checked_mul(self.d).and_then(|b| a.checked_add(b)))
                .ok_or_else(overflow)?,
            self.d.checked_mul(other.d).ok_or_else(overflow)?,
        )
    }
    fn mul(self, other: Self) -> Result<Self> {
        let a = Self::new(self.n, other.d)?;
        let b = Self::new(other.n, self.d)?;
        Self::new(
            a.n.checked_mul(b.n).ok_or_else(overflow)?,
            a.d.checked_mul(b.d).ok_or_else(overflow)?,
        )
    }
    fn div(self, other: Self) -> Result<Self> {
        self.mul(Self::new(other.d, other.n)?)
    }
    fn parse(text: &str) -> Result<Self> {
        let text = text.trim();
        if text.len() > 80 {
            return Err(overflow());
        }
        let (mantissa, exponent) = match text.find(['e', 'E']) {
            Some(index) => (
                &text[..index],
                text[index + 1..]
                    .parse::<i32>()
                    .map_err(|_| bad("Invalid exact exponent"))?,
            ),
            None => (text, 0),
        };
        if exponent.unsigned_abs() > 36 {
            return Err(overflow());
        }
        let negative = mantissa.starts_with('-');
        let mantissa = mantissa.strip_prefix(['-', '+']).unwrap_or(mantissa);
        let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
        if whole.is_empty() && fraction.is_empty()
            || !whole
                .chars()
                .chain(fraction.chars())
                .all(|c| c.is_ascii_digit())
        {
            return Err(bad("Invalid exact numeric literal"));
        }
        let digits = format!("{whole}{fraction}")
            .parse::<i128>()
            .map_err(|_| overflow())?;
        let power = exponent - fraction.len() as i32;
        let scale = 10i128
            .checked_pow(power.unsigned_abs())
            .ok_or_else(overflow)?;
        Self::new(if negative { -digits } else { digits }, 1).and_then(|r| {
            if power >= 0 {
                r.mul(Self::new(scale, 1)?)
            } else {
                r.div(Self::new(scale, 1)?)
            }
        })
    }
    fn xml(self) -> String {
        if self.d == 1 {
            format!("<real>{}</real>", self.n)
        } else {
            op(
                "div",
                &format!("<real>{}</real>", self.n),
                &format!("<real>{}</real>", self.d),
            )
        }
    }
}
type Monomial = Vec<(String, u32)>;
#[derive(Clone, Debug)]
struct Polynomial(BTreeMap<Monomial, Rational>);
impl Polynomial {
    fn constant(r: Rational) -> Self {
        Self(if r.n == 0 {
            BTreeMap::new()
        } else {
            BTreeMap::from([(vec![], r)])
        })
    }
    fn variable(name: String) -> Self {
        Self(BTreeMap::from([(vec![(name, 1)], Rational::ONE)]))
    }
    fn add(mut self, other: Self) -> Result<Self> {
        super::check_cancelled()?;
        for (monomial, coefficient) in other.0 {
            let sum = self
                .0
                .get(&monomial)
                .copied()
                .unwrap_or(Rational::ZERO)
                .add(coefficient)?;
            if sum.n == 0 {
                self.0.remove(&monomial);
            } else {
                self.0.insert(monomial, sum);
            }
            if self.0.len() > MAX_TERMS {
                return Err(bad("Symbolic term budget exceeded"));
            }
        }
        Ok(self)
    }
    fn scale(self, r: Rational) -> Result<Self> {
        if r.n == 0 {
            return Ok(Self::constant(Rational::ZERO));
        }
        Ok(Self(
            self.0
                .into_iter()
                .map(|(m, c)| Ok((m, c.mul(r)?)))
                .collect::<Result<_>>()?,
        ))
    }
    fn mul(self, other: Self) -> Result<Self> {
        if self
            .0
            .len()
            .checked_mul(other.0.len())
            .is_none_or(|count| count > MAX_TERMS * MAX_TERMS)
        {
            return Err(bad("Symbolic product budget exceeded"));
        }
        let mut result = Self::constant(Rational::ZERO);
        for (a, ca) in &self.0 {
            super::check_cancelled()?;
            for (b, cb) in &other.0 {
                let mut m = a.iter().cloned().collect::<BTreeMap<_, _>>();
                for (name, power) in b {
                    let total = m.entry(name.clone()).or_default();
                    *total = total.checked_add(*power).ok_or_else(overflow)?;
                    if *total > MAX_POWER {
                        return Err(bad("Symbolic exponent budget exceeded"));
                    }
                }
                result = result.add(Self(BTreeMap::from([(
                    m.into_iter().collect(),
                    ca.mul(*cb)?,
                )])))?;
            }
        }
        Ok(result)
    }
    fn pow(self, power: u32) -> Result<Self> {
        if power > MAX_POWER {
            return Err(bad("Symbolic exponent budget exceeded"));
        }
        if power == 0 && self.0.is_empty() {
            return Err(bad("Indeterminate symbolic zero power zero"));
        }
        let mut result = Self::constant(Rational::ONE);
        for _ in 0..power {
            result = result.mul(self.clone())?;
        }
        Ok(result)
    }
    fn scalar(&self) -> Result<Rational> {
        if self.0.is_empty() {
            return Ok(Rational::ZERO);
        }
        match self.0.first_key_value() {
            Some((m, c)) if self.0.len() == 1 && m.is_empty() => Ok(*c),
            _ => Err(bad(
                "Symbolic division/exponent requires a rational constant",
            )),
        }
    }
    fn derivative(self, name: &str) -> Result<Self> {
        let mut result = Self::constant(Rational::ZERO);
        for (mut m, c) in self.0 {
            if let Some(index) = m.iter().position(|(n, _)| n == name) {
                let power = m[index].1;
                m[index].1 -= 1;
                if m[index].1 == 0 {
                    m.remove(index);
                }
                result = result.add(Self(BTreeMap::from([(
                    m,
                    c.mul(Rational::new(power as i128, 1)?)?,
                )])))?;
            }
        }
        Ok(result)
    }
    fn substitute(self, name: &str, value: Rational) -> Result<Self> {
        let mut result = Self::constant(Rational::ZERO);
        for (mut monomial, mut coefficient) in self.0 {
            if let Some(index) = monomial.iter().position(|(candidate, _)| candidate == name) {
                let power = monomial[index].1;
                monomial.remove(index);
                for _ in 0..power {
                    coefficient = coefficient.mul(value)?;
                }
            }
            result = result.add(Self(BTreeMap::from([(monomial, coefficient)])))?;
        }
        Ok(result)
    }
    fn xml(&self) -> String {
        let mut terms = self.0.iter().collect::<Vec<_>>();
        terms.sort_by(|(a, _), (b, _)| {
            b.iter()
                .map(|(_, p)| p)
                .sum::<u32>()
                .cmp(&a.iter().map(|(_, p)| p).sum::<u32>())
                .then_with(|| a.cmp(b))
        });
        let mut output: Option<String> = None;
        for (m, c) in terms {
            if c.n == 0 {
                continue;
            }
            let mut factors = Vec::new();
            let absolute = Rational {
                n: c.n.abs(),
                d: c.d,
            };
            if absolute != Rational::ONE || m.is_empty() {
                factors.push(absolute.xml());
            }
            for (name, power) in m {
                let id = format!("<id labels=\"VARIABLE\">{}</id>", escape(name));
                factors.push(if *power == 1 {
                    id
                } else {
                    op("pow", &id, &format!("<real>{power}</real>"))
                });
            }
            let term = factors
                .into_iter()
                .reduce(|a, b| op("mult", &a, &b))
                .unwrap();
            output = Some(match output {
                None => {
                    if c.n < 0 {
                        format!("<apply><neg/>{term}</apply>")
                    } else {
                        term
                    }
                }
                Some(previous) => op(if c.n < 0 { "minus" } else { "plus" }, &previous, &term),
            });
        }
        output.unwrap_or_else(|| "<real>0</real>".into())
    }
}

#[derive(Clone, Debug)]
enum Command {
    Normalize,
    Solve(Option<String>),
}
#[derive(Clone, Debug)]
pub struct SymbolicEvaluation {
    polynomial: Polynomial,
    commands: Vec<Command>,
    equation: bool,
    source_names: BTreeSet<String>,
}

fn identifier(node: Node<'_, '_>) -> Result<String> {
    if !is_math(node, "id") || !children(node).is_empty() {
        return Err(bad("Symbolic identifiers require a simple text id"));
    }
    check_attributes(node, &["labels", "label-is-contextual"])?;
    if node
        .attribute("labels")
        .is_some_and(|label| !matches!(label, "VARIABLE" | "*"))
    {
        return Err(bad(
            "Symbolic unit/constant/function label is outside polynomial subset",
        ));
    }
    if node
        .attribute("label-is-contextual")
        .is_some_and(|v| !matches!(v, "true" | "false" | "0" | "1"))
    {
        return Err(bad("Invalid symbolic contextual flag"));
    }
    let name = node.text().unwrap_or("").to_owned();
    if name.is_empty() || name.len() > 256 {
        return Err(bad("Invalid symbolic name"));
    }
    Ok(name)
}
fn polynomial(node: Node<'_, '_>, depth: usize) -> Result<Polynomial> {
    if depth > 64 || node.tag_name().namespace() != Some(ML) {
        return Err(bad("Symbolic expression namespace/depth"));
    }
    let parts = children(node);
    match node.tag_name().name() {
        "real" => {
            check_attributes(node, &[])?;
            if !parts.is_empty() {
                return Err(bad("Real literal children"));
            }
            Ok(Polynomial::constant(Rational::parse(
                node.text().unwrap_or(""),
            )?))
        }
        "id" => Ok(Polynomial::variable(identifier(node)?)),
        "parens" if parts.len() == 1 => {
            check_attributes(node, &[])?;
            polynomial(parts[0], depth + 1)
        }
        "apply" => {
            check_attributes(node, &[])?;
            let (operator, args) = parts
                .split_first()
                .ok_or_else(|| bad("Empty symbolic apply"))?;
            if operator.tag_name().namespace() != Some(ML) || !children(*operator).is_empty() {
                return Err(bad("Symbolic operator structure"));
            }
            if operator.tag_name().name() == "limit" {
                check_attributes(*operator, &["direction"])?;
                if operator
                    .attribute("direction")
                    .is_some_and(|direction| !matches!(direction, "left" | "right"))
                {
                    return Err(bad("Invalid symbolic limit direction"));
                }
            } else {
                check_attributes(*operator, &[])?;
            }
            match (operator.tag_name().name(), args) {
                ("neg", [a]) => polynomial(*a, depth + 1)?.scale(Rational::ONE.neg()),
                ("plus", [a, b]) => polynomial(*a, depth + 1)?.add(polynomial(*b, depth + 1)?),
                ("minus", [a, b]) => polynomial(*a, depth + 1)?
                    .add(polynomial(*b, depth + 1)?.scale(Rational::ONE.neg())?),
                ("mult" | "scale", [a, b]) => {
                    polynomial(*a, depth + 1)?.mul(polynomial(*b, depth + 1)?)
                }
                ("div", [a, b]) => polynomial(*a, depth + 1)?
                    .scale(Rational::ONE.div(polynomial(*b, depth + 1)?.scalar()?)?),
                ("pow", [a, b]) => {
                    let power = polynomial(*b, depth + 1)?.scalar()?;
                    if power.d != 1 || power.n < 0 || power.n > MAX_POWER as i128 {
                        return Err(bad(
                            "Polynomial powers require bounded nonnegative integers",
                        ));
                    }
                    polynomial(*a, depth + 1)?.pow(power.n as u32)
                }
                ("derivative" | "partDerivative", [lambda, degree]) => {
                    check_attributes(*lambda, &[])?;
                    check_attributes(*degree, &[])?;
                    let l = children(*lambda);
                    let d = children(*degree);
                    if !is_math(*lambda, "lambda")
                        || !is_math(*degree, "degree")
                        || l.len() != 2
                        || !is_math(l[0], "boundVars")
                        || d.len() != 1
                    {
                        return Err(bad("Symbolic derivative structure"));
                    }
                    check_attributes(l[0], &[])?;
                    let bound = children(l[0]);
                    if bound.len() != 1 {
                        return Err(bad("One symbolic derivative variable required"));
                    }
                    let name = identifier(bound[0])?;
                    let count = if is_math(d[0], "placeholder") {
                        check_attributes(d[0], &[])?;
                        if !children(d[0]).is_empty() {
                            return Err(bad("Invalid derivative placeholder"));
                        }
                        1
                    } else {
                        let count = polynomial(d[0], depth + 1)?.scalar()?;
                        if count.d != 1 || count.n < 1 || count.n > MAX_POWER as i128 {
                            return Err(bad("Derivative degree limit"));
                        }
                        count.n as u32
                    };
                    let mut p = polynomial(l[1], depth + 1)?;
                    for _ in 0..count {
                        p = p.derivative(&name)?;
                    }
                    Ok(p)
                }
                ("limit", [lambda, value]) => {
                    check_attributes(*lambda, &[])?;
                    let lambda_parts = children(*lambda);
                    if !is_math(*lambda, "lambda")
                        || lambda_parts.len() != 2
                        || !is_math(lambda_parts[0], "boundVars")
                    {
                        return Err(bad("Symbolic limit lambda structure"));
                    }
                    check_attributes(lambda_parts[0], &[])?;
                    let bound = children(lambda_parts[0]);
                    if bound.len() != 1 {
                        return Err(bad("One symbolic limit variable required"));
                    }
                    let name = identifier(bound[0])?;
                    let point = polynomial(*value, depth + 1)?.scalar()?;
                    polynomial(lambda_parts[1], depth + 1)?.substitute(&name, point)
                }
                _ => Err(bad("Unsupported symbolic polynomial operator or arity")),
            }
        }
        _ => Err(bad("Unsupported symbolic polynomial expression")),
    }
}

pub(crate) fn parse(node: Node<'_, '_>, depth: usize) -> Result<SymbolicEvaluation> {
    if depth > 64 || !is_math(node, "symEval") {
        return Err(bad("Expected bounded symEval"));
    }
    check_attributes(node, &["hide-lhs", "hide-keywords", "style"])?;
    for attr in ["hide-lhs", "hide-keywords"] {
        if node
            .attribute(attr)
            .is_some_and(|v| !matches!(v, "true" | "false" | "0" | "1"))
        {
            return Err(bad("Invalid symEval boolean"));
        }
    }
    if node
        .attribute("style")
        .is_some_and(|v| !matches!(v, "default" | "right-arrow" | "equals"))
    {
        return Err(bad("Invalid symEval style"));
    }
    let parts = children(node);
    let first = *parts.first().ok_or_else(|| bad("Missing symbolic input"))?;
    let mut equation = false;
    let p = if is_math(first, "apply") {
        let c = children(first);
        if c.first().is_some_and(|n| is_math(*n, "equal")) {
            check_attributes(first, &[])?;
            if c.len() != 3 {
                return Err(bad("Symbolic equation arity"));
            }
            check_attributes(c[0], &[])?;
            if !children(c[0]).is_empty() {
                return Err(bad("Symbolic equality children"));
            }
            equation = true;
            polynomial(c[1], depth + 1)?
                .add(polynomial(c[2], depth + 1)?.scale(Rational::ONE.neg())?)?
        } else {
            polynomial(first, depth + 1)?
        }
    } else {
        polynomial(first, depth + 1)?
    };
    let mut commands = Vec::new();
    let mut cached = false;
    for item in &parts[1..] {
        if is_math(*item, "symResult") {
            check_attributes(*item, &[])?;
            if cached || children(*item).len() != 1 {
                return Err(bad("Invalid cached symbolic result"));
            }
            cached = true;
            continue;
        }
        if cached || !is_math(*item, "command") {
            return Err(bad("Symbolic command/result order"));
        }
        check_attributes(*item, &[])?;
        let args = children(*item);
        if args.is_empty() || args.len() > 2 || commands.len() >= 16 {
            return Err(bad("Symbolic command arity/budget"));
        }
        if is_math(args[0], "placeholder") && args.len() == 1 {
            check_attributes(args[0], &[])?;
            if !children(args[0]).is_empty() {
                return Err(bad("Invalid symbolic keyword placeholder"));
            }
            continue;
        }
        let keyword = identifier(args[0])?;
        commands.push(match keyword.as_str() {
            "simplify" | "expand" if args.len() == 1 => Command::Normalize,
            "solve" => Command::Solve(if args.len() == 2 {
                Some(identifier(args[1])?)
            } else {
                None
            }),
            _ => return Err(bad("Unsupported symbolic keyword/modifier")),
        });
    }
    // Retain source names even when canonical cancellation removes a term, so
    // unsupported existing worksheet values cannot silently disappear.
    fn names(
        node: Node<'_, '_>,
        bound: &BTreeSet<String>,
        out: &mut BTreeSet<String>,
    ) -> Result<()> {
        if is_math(node, "id") {
            let name = identifier(node)?;
            if !bound.contains(&name) {
                out.insert(name);
            }
        } else if is_math(node, "lambda") {
            let parts = children(node);
            if parts.len() != 2 || !is_math(parts[0], "boundVars") {
                return Err(bad("Symbolic lambda structure"));
            }
            let mut nested = bound.clone();
            for variable in children(parts[0]) {
                nested.insert(identifier(variable)?);
            }
            names(parts[1], &nested, out)?;
        } else {
            for child in children(node) {
                names(child, bound, out)?;
            }
        }
        Ok(())
    }
    let mut source_names = BTreeSet::new();
    names(first, &BTreeSet::new(), &mut source_names)?;
    Ok(SymbolicEvaluation {
        polynomial: p,
        commands,
        equation,
        source_names,
    })
}

/// Parse standalone native Math50 symEval without consulting numeric bindings.
pub fn from_mathml(xml: &str) -> Result<SymbolicEvaluation> {
    if xml.len() > 1024 * 1024 {
        return Err(bad("Symbolic XML input budget exceeded"));
    }
    let doc = roxmltree::Document::parse(xml).map_err(|_| bad("Invalid symbolic XML"))?;
    parse(doc.root_element(), 0)
}
fn op(operator: &str, a: &str, b: &str) -> String {
    format!("<apply><{operator}/>{a}{b}</apply>")
}
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
fn exact_square_root(value: Rational) -> Result<Option<Rational>> {
    if value.n < 0 {
        return Ok(None);
    }
    fn sqrt(n: u128) -> u128 {
        let mut lo = 0;
        let mut hi = 1u128 << 64;
        while lo + 1 < hi {
            let mid = (lo + hi) / 2;
            if mid <= n / mid {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo
    }
    let a = sqrt(value.n as u128);
    let b = sqrt(value.d as u128);
    if a * a == value.n as u128 && b * b == value.d as u128 {
        Ok(Some(Rational::new(a as i128, b as i128)?))
    } else {
        Ok(None)
    }
}
fn solve(p: &Polynomial, variable: Option<&str>) -> Result<String> {
    let names =
        p.0.keys()
            .flat_map(|m| m.iter().map(|(n, _)| n.as_str()))
            .collect::<BTreeSet<_>>();
    let variable = match variable {
        Some(v) => v,
        None if names.len() == 1 => *names.first().unwrap(),
        _ => return Err(bad("Solve requires one unambiguous variable")),
    };
    if names.iter().any(|&n| n != variable) {
        return Err(bad(
            "Parameterized or multivariate solve is outside this subset",
        ));
    }
    let mut coefficients = [Rational::ZERO; 3];
    for (m, c) in &p.0 {
        let degree = if m.is_empty() { 0 } else { m[0].1 };
        if degree > 2 {
            return Err(bad("Solve supports degree at most two"));
        }
        coefficients[degree as usize] = coefficients[degree as usize].add(*c)?;
    }
    let [c, b, a] = coefficients;
    if a.n == 0 {
        if b.n == 0 {
            return Err(bad(
                "Constant/identical equation solution sets are unsupported",
            ));
        }
        return Ok(c.neg().div(b)?.xml());
    }
    let discriminant = b.mul(b)?.add(a.mul(c)?.mul(Rational::new(-4, 1)?)?)?;
    let denominator = a.mul(Rational::new(2, 1)?)?;
    if discriminant.n == 0 {
        return Ok(b.neg().div(denominator)?.xml());
    }
    if let Some(root) = exact_square_root(discriminant)? {
        let minus = b.neg().add(root.neg())?.div(denominator)?.xml();
        let plus = b.neg().add(root)?.div(denominator)?.xml();
        return Ok(format!(
            "<matrix rows=\"2\" cols=\"1\">{minus}{plus}</matrix>"
        ));
    }
    let root = format!("<apply><sqrt/>{}</apply>", discriminant.xml());
    let minus = op(
        "div",
        &op("minus", &b.neg().xml(), &root),
        &denominator.xml(),
    );
    let plus = op(
        "div",
        &op("plus", &b.neg().xml(), &root),
        &denominator.xml(),
    );
    Ok(format!(
        "<matrix rows=\"2\" cols=\"1\">{minus}{plus}</matrix>"
    ))
}

pub(crate) fn evaluate(evaluation: &SymbolicEvaluation, scope: &Scope) -> Result<Value> {
    let mut substitutions = BTreeMap::new();
    for name in &evaluation.source_names {
        let symbol = Symbol {
            name: name.clone(),
            label: "VARIABLE".into(),
        };
        if let Some(binding) = scope.bindings.get(&symbol) {
            let number = match binding {
                Binding::Value(Value::Scalar { quantity })
                    if quantity.is_scalar()
                        && quantity.value.im == 0.
                        && quantity.value.re.is_finite()
                        && quantity.value.re.fract() == 0.
                        && quantity.value.re.abs() <= 9_007_199_254_740_992. =>
                {
                    Rational::new(quantity.value.re as i128, 1)?
                }
                Binding::Failed { fault, .. } => return Err(fault.clone()),
                _ => {
                    return Err(bad(
                        "Symbolic worksheet substitution supports only exact dimensionless integer scalars",
                    ));
                }
            };
            substitutions.insert(name.clone(), number);
        }
    }
    let mut polynomial = Polynomial::constant(Rational::ZERO);
    for (monomial, coefficient) in &evaluation.polynomial.0 {
        let mut c = *coefficient;
        let mut remainder = Vec::new();
        for (name, power) in monomial {
            if let Some(number) = substitutions.get(name) {
                for _ in 0..*power {
                    c = c.mul(*number)?;
                }
            } else {
                remainder.push((name.clone(), *power));
            }
        }
        polynomial = polynomial.add(Polynomial(BTreeMap::from([(remainder, c)])))?;
    }
    let mut result = None;
    for command in &evaluation.commands {
        if result.is_some() {
            return Err(bad("Commands after solve are outside the symbolic subset"));
        }
        match command {
            Command::Normalize => (),
            Command::Solve(variable) => result = Some(solve(&polynomial, variable.as_deref())?),
        }
    }
    if evaluation.equation && result.is_none() {
        return Err(bad("Symbolic equations require solve in this subset"));
    }
    let expression = result.unwrap_or_else(|| polynomial.xml());
    Ok(Value::Symbolic {
        mathml: format!("<symResult xmlns=\"{ML}\">{expression}</symResult>"),
    })
}
pub fn evaluate_mathml(xml: &str) -> Result<Value> {
    evaluate(&from_mathml(xml)?, &Scope::default())
}
