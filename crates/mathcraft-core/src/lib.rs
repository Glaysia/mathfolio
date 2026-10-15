//! Independent prototype semantics. Mathcad compatibility requires separate evidence.
use num_complex::Complex64;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt, sync::Arc};

pub mod units;

pub type Result<T> = std::result::Result<T, String>;

/// SI base dimensions: length, mass, time, current, temperature, amount, luminosity.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Quantity {
    pub value: Complex64,
    pub dimension: [f64; 7],
}

impl Quantity {
    pub fn scalar(value: f64) -> Self {
        Self {
            value: Complex64::new(value, 0.0),
            dimension: [0.0; 7],
        }
    }
    pub fn same_dimension(&self, other: &Self) -> bool {
        self.dimension
            .iter()
            .zip(other.dimension)
            .all(|(a, b)| (a - b).abs() < 1e-9)
    }
    pub fn is_scalar(&self) -> bool {
        self.same_dimension(&Self::scalar(1.0))
    }
    fn finite(self) -> Result<Self> {
        if self.value.re.is_finite() && self.value.im.is_finite() {
            Ok(self)
        } else {
            Err("Result is not finite".into())
        }
    }
    pub fn add(self, rhs: Self, subtract: bool) -> Result<Self> {
        if !self.same_dimension(&rhs) {
            return Err("Cannot add quantities with different dimensions".into());
        }
        Self {
            value: if subtract {
                self.value - rhs.value
            } else {
                self.value + rhs.value
            },
            ..self
        }
        .finite()
    }
    pub fn multiply(self, rhs: Self, divide: bool) -> Result<Self> {
        if divide && rhs.value.norm() == 0.0 {
            return Err("Division by zero".into());
        }
        let mut dimension = self.dimension;
        for (d, r) in dimension.iter_mut().zip(rhs.dimension) {
            *d += if divide { -r } else { r };
        }
        Self {
            value: if divide {
                self.value / rhs.value
            } else {
                self.value * rhs.value
            },
            dimension,
        }
        .finite()
    }
    pub fn power(self, rhs: Self) -> Result<Self> {
        if !rhs.is_scalar() {
            return Err("Exponent must be dimensionless".into());
        }
        if !self.is_scalar() && rhs.value.im != 0.0 {
            return Err("A dimensional quantity needs a real exponent".into());
        }
        if self.value.norm() == 0.0 && (rhs.value.re < 0.0 || rhs.value.im != 0.0) {
            return Err("Undefined power of zero".into());
        }
        let value = if self.value.im == 0.0
            && rhs.value.im == 0.0
            && (self.value.re >= 0.0 || rhs.value.re.fract() == 0.0)
        {
            Complex64::new(self.value.re.powf(rhs.value.re), 0.0)
        } else {
            self.value.powc(rhs.value)
        };
        Self {
            value,
            dimension: self.dimension.map(|d| d * rhs.value.re),
        }
        .finite()
    }
    pub fn format_in(self, unit: Self, label: &str) -> Result<String> {
        if !self.same_dimension(&unit) {
            return Err(format!(
                "Requested unit '{label}' has a different dimension"
            ));
        }
        if unit.value.norm() == 0.0 {
            return Err("Display unit cannot be zero".into());
        }
        Ok(format!(
            "{} {}",
            format_complex(self.value / unit.value),
            label
        ))
    }
}

fn format_number(value: f64) -> String {
    if value == 0.0 {
        return "0".into();
    }
    if value.abs() < 1e-5 || value.abs() >= 1e7 {
        return format!("{value:.7e}");
    }
    format!("{value:.8}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}
pub fn format_complex(value: Complex64) -> String {
    if value.im == 0.0 {
        format_number(value.re)
    } else if value.re == 0.0 {
        format!("{}i", format_number(value.im))
    } else {
        format!(
            "{} {} {}i",
            format_number(value.re),
            if value.im < 0.0 { "-" } else { "+" },
            format_number(value.im.abs())
        )
    }
}
impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", format_complex(self.value))?;
        for (name, exponent) in ["m", "kg", "s", "A", "K", "mol", "cd"]
            .iter()
            .zip(self.dimension)
        {
            if exponent.abs() > 1e-9 {
                if (exponent - 1.0).abs() < 1e-9 {
                    write!(f, " {name}")?;
                } else {
                    write!(f, " {name}^{}", format_number(exponent))?;
                }
            }
        }
        Ok(())
    }
}

pub fn unit(name: &str) -> Option<Quantity> {
    if let Some(value) = units::lookup(name) {
        return Some(value);
    }
    let base = |name: &str| -> Option<Quantity> {
        let (scale, dimension) = match name {
            "m" | "meter" => (1.0, [1., 0., 0., 0., 0., 0., 0.]),
            "kg" | "kilogram" => (1.0, [0., 1., 0., 0., 0., 0., 0.]),
            "g" => (0.001, [0., 1., 0., 0., 0., 0., 0.]),
            "s" | "sec" | "second" => (1.0, [0., 0., 1., 0., 0., 0., 0.]),
            "A" | "ampere" => (1.0, [0., 0., 0., 1., 0., 0., 0.]),
            "K" | "kelvin" => (1.0, [0., 0., 0., 0., 1., 0., 0.]),
            "mol" => (1.0, [0., 0., 0., 0., 0., 1., 0.]),
            "cd" => (1.0, [0., 0., 0., 0., 0., 0., 1.]),
            "Hz" | "hertz" => (1.0, [0., 0., -1., 0., 0., 0., 0.]),
            "N" | "newton" => (1.0, [1., 1., -2., 0., 0., 0., 0.]),
            "Pa" | "pascal" => (1.0, [-1., 1., -2., 0., 0., 0., 0.]),
            "J" | "joule" => (1.0, [2., 1., -2., 0., 0., 0., 0.]),
            "W" | "watt" => (1.0, [2., 1., -3., 0., 0., 0., 0.]),
            "V" | "volt" => (1.0, [2., 1., -3., -1., 0., 0., 0.]),
            "ohm" | "Ω" | "Ω" => (1.0, [2., 1., -3., -2., 0., 0., 0.]),
            "F" | "farad" => (1.0, [-2., -1., 4., 2., 0., 0., 0.]),
            "H" | "henry" => (1.0, [2., 1., -2., -2., 0., 0., 0.]),
            "C" | "coulomb" => (1.0, [0., 0., 1., 1., 0., 0., 0.]),
            "S" | "siemens" => (1.0, [-2., -1., 3., 2., 0., 0., 0.]),
            "rad" => (1.0, [0.; 7]),
            "deg" | "°" => (std::f64::consts::PI / 180.0, [0.; 7]),
            "min" => (60.0, [0., 0., 1., 0., 0., 0., 0.]),
            "hour" | "hr" => (3600.0, [0., 0., 1., 0., 0., 0., 0.]),
            "inch" | "in" => (0.0254, [1., 0., 0., 0., 0., 0., 0.]),
            _ => return None,
        };
        Some(Quantity {
            value: Complex64::new(scale, 0.0),
            dimension,
        })
    };
    if let Some(value) = base(name) {
        return Some(value);
    }
    for (prefix, factor) in [
        ("da", 1e1),
        ("G", 1e9),
        ("M", 1e6),
        ("k", 1e3),
        ("h", 1e2),
        ("d", 1e-1),
        ("c", 1e-2),
        ("m", 1e-3),
        ("u", 1e-6),
        ("µ", 1e-6),
        ("μ", 1e-6),
        ("n", 1e-9),
        ("p", 1e-12),
    ] {
        if let Some(tail) = name.strip_prefix(prefix)
            && let Some(mut quantity) = base(tail)
        {
            quantity.value *= factor;
            return Some(quantity);
        }
    }
    None
}

#[derive(Clone, Debug)]
pub enum Expr {
    Number(f64),
    Name(String),
    Neg(Box<Expr>),
    Binary(char, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Number(f64),
    Name(String),
    Op(char),
    End,
}

fn tokens(source: &str) -> Result<Vec<Token>> {
    if source.len() > 4096 {
        return Err("Expression exceeds 4096 bytes".into());
    }
    let chars: Vec<char> = source.chars().collect();
    let mut out = Vec::new();
    let mut p = 0;
    while p < chars.len() {
        let c = chars[p];
        if c.is_whitespace() {
            p += 1;
            continue;
        }
        if c.is_ascii_digit() || (c == '.' && chars.get(p + 1).is_some_and(char::is_ascii_digit)) {
            let start = p;
            p += 1;
            while p < chars.len() && (chars[p].is_ascii_digit() || chars[p] == '.') {
                p += 1;
            }
            if p < chars.len() && matches!(chars[p], 'e' | 'E') {
                let before = p;
                p += 1;
                if p < chars.len() && matches!(chars[p], '+' | '-') {
                    p += 1;
                }
                let digits = p;
                while p < chars.len() && chars[p].is_ascii_digit() {
                    p += 1;
                }
                if p == digits {
                    p = before;
                }
            }
            let value: f64 = chars[start..p]
                .iter()
                .collect::<String>()
                .parse()
                .map_err(|_| "Invalid number")?;
            if !value.is_finite() {
                return Err("Number is not finite".into());
            }
            out.push(Token::Number(value));
        } else if c.is_alphabetic() || c == '_' || matches!(c, 'Ω' | 'Ω' | '°') {
            let start = p;
            p += 1;
            while p < chars.len()
                && (chars[p].is_alphanumeric() || chars[p] == '_' || chars[p] == '.')
            {
                p += 1;
            }
            out.push(Token::Name(chars[start..p].iter().collect()));
        } else if "+-*/^(),".contains(c) {
            out.push(Token::Op(c));
            p += 1;
        } else if matches!(c, '×' | '·' | '÷' | '−') {
            out.push(Token::Op(match c {
                '÷' => '/',
                '−' => '-',
                _ => '*',
            }));
            p += 1;
        } else {
            return Err(format!("Unexpected character '{c}'"));
        }
        if out.len() > 256 {
            return Err("Expression exceeds 256 tokens".into());
        }
    }
    out.push(Token::End);
    Ok(out)
}

pub fn parse(source: &str) -> Result<Expr> {
    let mut parser = Parser {
        tokens: tokens(source)?,
        position: 0,
        depth: 0,
    };
    let expression = parser.expression(0)?;
    if parser.peek() != &Token::End {
        return Err("Unexpected trailing expression".into());
    }
    Ok(expression)
}
struct Parser {
    tokens: Vec<Token>,
    position: usize,
    depth: usize,
}
impl Parser {
    fn peek(&self) -> &Token {
        &self.tokens[self.position]
    }
    fn next(&mut self) -> Token {
        let t = self.tokens[self.position].clone();
        if t != Token::End {
            self.position += 1;
        }
        t
    }
    fn expect(&mut self, c: char) -> Result<()> {
        if self.next() == Token::Op(c) {
            Ok(())
        } else {
            Err(format!("Expected '{c}'"))
        }
    }
    fn expression(&mut self, min: u8) -> Result<Expr> {
        self.depth += 1;
        if self.depth > 64 {
            return Err("Expression nesting exceeds 64 levels".into());
        }
        let mut lhs = match self.next() {
            Token::Number(n) => Expr::Number(n),
            Token::Name(name) => {
                if self.peek() == &Token::Op('(') {
                    self.next();
                    let mut args = Vec::new();
                    if self.peek() != &Token::Op(')') {
                        loop {
                            args.push(self.expression(0)?);
                            if self.peek() != &Token::Op(',') {
                                break;
                            }
                            self.next();
                        }
                    }
                    self.expect(')')?;
                    Expr::Call(name, args)
                } else {
                    Expr::Name(name)
                }
            }
            Token::Op('-') => Expr::Neg(Box::new(self.expression(5)?)),
            Token::Op('+') => self.expression(5)?,
            Token::Op('(') => {
                let value = self.expression(0)?;
                self.expect(')')?;
                value
            }
            _ => return Err("Expected a number, variable, or parenthesized expression".into()),
        };
        loop {
            let (op, left, right, implicit) = match self.peek() {
                Token::Op(c @ ('+' | '-')) => (*c, 1, 2, false),
                Token::Op(c @ ('*' | '/')) => (*c, 3, 4, false),
                Token::Op('^') => ('^', 7, 6, false),
                Token::Number(_) | Token::Name(_) | Token::Op('(') => ('*', 3, 4, true),
                _ => break,
            };
            if left < min {
                break;
            }
            if !implicit {
                self.next();
            }
            lhs = Expr::Binary(op, Box::new(lhs), Box::new(self.expression(right)?));
        }
        self.depth -= 1;
        Ok(lhs)
    }
}

#[derive(Clone, Default)]
pub struct Environment {
    pub variables: HashMap<String, Quantity>,
    functions: HashMap<String, Function>,
}
#[derive(Clone)]
struct Function {
    parameters: Vec<String>,
    body: Expr,
    definition_environment: Arc<Environment>,
}
impl Environment {
    pub fn evaluate(&self, source: &str) -> Result<Quantity> {
        self.eval(&parse(source)?, &HashMap::new(), 0)
    }
    fn eval(
        &self,
        expr: &Expr,
        locals: &HashMap<String, Quantity>,
        depth: usize,
    ) -> Result<Quantity> {
        if depth > 64 {
            return Err("Evaluation recursion limit reached".into());
        }
        match expr {
            Expr::Number(n) => Ok(Quantity::scalar(*n)),
            Expr::Name(name) => {
                if let Some(value) = locals.get(name).or_else(|| self.variables.get(name)) {
                    return Ok(*value);
                }
                match name.as_str() {
                    "pi" | "π" => Ok(Quantity::scalar(std::f64::consts::PI)),
                    "e" => Ok(Quantity::scalar(std::f64::consts::E)),
                    "i" | "j" => Ok(Quantity {
                        value: Complex64::new(0., 1.),
                        ..Quantity::scalar(0.)
                    }),
                    _ => unit(name).ok_or_else(|| format!("Undefined variable or unit: {name}")),
                }
            }
            Expr::Neg(value) => {
                let mut q = self.eval(value, locals, depth + 1)?;
                q.value = -q.value;
                Ok(q)
            }
            Expr::Binary(op, lhs, rhs) => {
                let a = self.eval(lhs, locals, depth + 1)?;
                let b = self.eval(rhs, locals, depth + 1)?;
                match op {
                    '+' => a.add(b, false),
                    '-' => a.add(b, true),
                    '*' => a.multiply(b, false),
                    '/' => a.multiply(b, true),
                    '^' => a.power(b),
                    _ => Err("Unknown operator".into()),
                }
            }
            Expr::Call(name, args) => {
                let values = args
                    .iter()
                    .map(|a| self.eval(a, locals, depth + 1))
                    .collect::<Result<Vec<_>>>()?;
                if let Some(function) = self.functions.get(name) {
                    if function.parameters.len() != values.len() {
                        return Err(format!(
                            "{name} expects {} arguments",
                            function.parameters.len()
                        ));
                    }
                    let scope = function.parameters.iter().cloned().zip(values).collect();
                    return function
                        .definition_environment
                        .eval(&function.body, &scope, depth + 1);
                }
                if values.len() != 1 {
                    return Err(format!("{name} expects one argument"));
                }
                let q = values[0];
                match name.as_str() {
                    "sqrt" => q.power(Quantity::scalar(0.5)),
                    "abs" => Ok(Quantity {
                        value: Complex64::new(q.value.norm(), 0.),
                        ..q
                    }),
                    "re" | "Re" => Ok(Quantity {
                        value: Complex64::new(q.value.re, 0.),
                        ..q
                    }),
                    "im" | "Im" => Ok(Quantity {
                        value: Complex64::new(q.value.im, 0.),
                        ..q
                    }),
                    "conj" => Ok(Quantity {
                        value: q.value.conj(),
                        ..q
                    }),
                    "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "exp" | "ln" | "log"
                    | "log10" => {
                        if !q.is_scalar() {
                            return Err(format!("{name} needs a dimensionless argument"));
                        }
                        let v = q.value;
                        Quantity {
                            value: match name.as_str() {
                                "sin" => v.sin(),
                                "cos" => v.cos(),
                                "tan" => v.tan(),
                                "asin" => v.asin(),
                                "acos" => v.acos(),
                                "atan" => v.atan(),
                                "exp" => v.exp(),
                                "ln" => v.ln(),
                                _ => v.log(10.),
                            },
                            ..q
                        }
                        .finite()
                    }
                    _ => Err(format!("Unknown function: {name}")),
                }
            }
        }
    }
    pub fn line(&mut self, source: &str) -> Result<Option<LineValue>> {
        let source = source.split('#').next().unwrap_or("").trim();
        if source.is_empty() {
            return Ok(None);
        }
        let (statement, display_unit) = source
            .split_once("->")
            .map_or((source, None), |(a, b)| (a.trim(), Some(b.trim())));
        let statement = statement.trim_end_matches('=').trim();
        let (label, expr) = if let Some((lhs, rhs)) = statement.split_once(":=") {
            let lhs = lhs.trim();
            if let Some((name, params)) = lhs.split_once('(') {
                let params = params
                    .strip_suffix(')')
                    .ok_or("Function definition needs a closing parenthesis")?;
                let name = valid_name(name.trim())?;
                let parameters = if params.trim().is_empty() {
                    Vec::new()
                } else {
                    params
                        .split(',')
                        .map(|p| valid_name(p.trim()))
                        .collect::<Result<Vec<_>>>()?
                };
                if parameters.len() > 16 {
                    return Err("At most 16 function parameters are supported".into());
                }
                let mut unique = parameters.clone();
                unique.sort();
                unique.dedup();
                if unique.len() != parameters.len() {
                    return Err("Duplicate function parameters".into());
                }
                let function = Function {
                    parameters,
                    body: parse(rhs)?,
                    definition_environment: Arc::new(self.clone()),
                };
                self.functions.insert(name.clone(), function);
                return Ok(Some(LineValue {
                    value: None,
                    text: format!("{lhs} defined"),
                    label: Some(name),
                }));
            }
            (Some(valid_name(lhs)?), rhs.trim())
        } else {
            (None, statement)
        };
        let value = match self.evaluate(expr) {
            Ok(q) => q,
            Err(e) => {
                if let Some(name) = &label {
                    self.variables.remove(name);
                }
                return Err(e);
            }
        };
        let text = if let Some(display) = display_unit {
            value.format_in(self.evaluate(display)?, display)?
        } else {
            value.to_string()
        };
        if let Some(name) = &label {
            self.variables.insert(name.clone(), value);
        }
        Ok(Some(LineValue {
            value: Some(value),
            text,
            label,
        }))
    }
}
fn valid_name(name: &str) -> Result<String> {
    match tokens(name)?.as_slice() {
        [Token::Name(n), Token::End] if n == name => Ok(n.clone()),
        _ => Err(format!("Invalid name: {name}")),
    }
}
#[derive(Clone, Debug)]
pub struct LineValue {
    pub value: Option<Quantity>,
    pub text: String,
    pub label: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn real(s: &str) -> f64 {
        Environment::default().evaluate(s).unwrap().value.re
    }
    #[test]
    fn precedence_and_associativity() {
        assert_eq!(real("-2^2"), -4.);
        assert_eq!(real("2^3^2"), 512.);
        assert_eq!(real("(2+3)*4"), 20.);
        assert_eq!(real("2(3+4)"), 14.);
    }
    #[test]
    fn unit_dimensions_and_conversion() {
        let mut e = Environment::default();
        assert_eq!(
            e.line("d := 3 m + 20 cm -> cm").unwrap().unwrap().text,
            "320 cm"
        );
        assert!(e.evaluate("3 m + 2 s").is_err());
        let q = e.evaluate("sqrt(4 m^2)").unwrap();
        assert!((q.value.re - 2.).abs() < 1e-12);
        assert!(q.same_dimension(&unit("m").unwrap()));
    }
    #[test]
    fn variables_and_functions() {
        let mut e = Environment::default();
        e.line("a := 3").unwrap();
        e.line("f(x) := a*x^2").unwrap();
        assert_eq!(e.evaluate("f(4)").unwrap().value.re, 48.);
        assert!(e.line("g(x,x) := x").is_err());
        assert!(e.evaluate("future+1").is_err());
    }
    #[test]
    fn original_reference_definition_time_binding_and_zero_power() {
        // Fresh Prime 12 references: function-capture and zero-power-zero.
        let mut e = Environment::default();
        e.line("a := 2").unwrap();
        e.line("f(x) := a*x").unwrap();
        e.line("a := 3").unwrap();
        assert_eq!(e.evaluate("f(4)").unwrap().value.re, 8.);
        assert_eq!(e.evaluate("0^0").unwrap().value.re, 1.);
    }
    #[test]
    fn engineering_complex_arithmetic() {
        let q = Environment::default().evaluate("(3+4i) ohm").unwrap();
        assert_eq!(q.value, Complex64::new(3., 4.));
        assert!(q.same_dimension(&unit("ohm").unwrap()));
        assert!(
            (Environment::default()
                .evaluate("abs(3+4i)")
                .unwrap()
                .value
                .re
                - 5.)
                .abs()
                < 1e-12
        );
        assert!((real("sin(90 deg)") - 1.).abs() < 1e-12);
    }
    #[test]
    fn errors_are_explicit() {
        for s in [
            "1/0",
            "0^-1",
            "exp(10000)",
            "sin(2 m)",
            "2 m ^ (1 s)",
            "1..2",
            "(1+2",
        ] {
            assert!(Environment::default().evaluate(s).is_err(), "{s}");
        }
    }
    #[test]
    fn recursion_and_input_budgets() {
        let mut e = Environment::default();
        e.line("f(x) := f(x)").unwrap();
        assert!(e.evaluate("f(1)").is_err());
        assert!(parse(&"(".repeat(100)).is_err());
    }
    #[test]
    fn failed_redefinition_does_not_leave_stale_value() {
        let mut e = Environment::default();
        e.line("x := 3").unwrap();
        assert!(e.line("x := missing").is_err());
        assert!(e.evaluate("x").is_err());
    }
}
