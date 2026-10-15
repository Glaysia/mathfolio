//! Read-only mathematical result presentation; rounding never changes stored values.
use crate::{typography::MathStyle, worksheet_math};
use mathcraft_compat::Value;
use mathcraft_core::{Quantity, format_complex};
use mathcraft_document::canvas::{MathFormat, ResultFormat};
use mathcraft_editor::{BinaryOp, NodeKind, PortableTree};

#[derive(Clone, Debug)]
pub(crate) enum DisplayMath {
    Text(String, MathStyle),
    Row(Vec<Self>),
    Power(Box<Self>, Box<Self>),
    Subscript(Box<Self>, Box<Self>),
    Fraction(Box<Self>, Box<Self>),
    Group(Box<Self>),
    Root(Box<Self>),
}
impl DisplayMath {
    fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into(), MathStyle::Operator)
    }
    #[cfg(test)]
    fn plain(&self) -> String {
        match self {
            Self::Text(s, _) => s.clone(),
            Self::Row(parts) => parts.iter().map(Self::plain).collect(),
            Self::Power(a, b) => format!("{}^{}", a.plain(), b.plain()),
            Self::Subscript(a, b) => format!("{}_{}", a.plain(), b.plain()),
            Self::Fraction(a, b) => format!("{}/{}", a.plain(), b.plain()),
            Self::Group(a) => format!("({})", a.plain()),
            Self::Root(a) => format!("√{}", a.plain()),
        }
    }
}
#[derive(Clone, Debug)]
pub(crate) struct FormattedResult {
    pub rows: usize,
    pub cols: usize,
    pub cells: Vec<DisplayMath>,
    pub brackets: bool,
}

fn trimmed(value: f64, places: usize) -> String {
    let text = format!("{value:.places$}");
    let text = if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        text
    };
    if text == "-0" {
        "0".into()
    } else {
        text.replace('-', "−")
    }
}
fn scientific(value: f64, places: usize, engineering: bool) -> DisplayMath {
    if value == 0. {
        return DisplayMath::text("0");
    }
    // Parsing Rust's decimal exponent avoids powi underflow for subnormal values.
    let source = format!("{value:e}");
    let (mantissa, exponent) = source.split_once('e').unwrap();
    let exponent: i32 = exponent.parse().unwrap();
    let mut exp = if engineering {
        exponent.div_euclid(3) * 3
    } else {
        exponent
    };
    let mut coefficient = mantissa.parse::<f64>().unwrap() * 10_f64.powi(exponent - exp);
    let limit = if engineering { 1000. } else { 10. };
    let rounded = coefficient.abs() * 10_f64.powi(places as i32);
    if rounded.round() / 10_f64.powi(places as i32) >= limit {
        coefficient /= limit;
        exp += if engineering { 3 } else { 1 };
    }
    let coefficient = DisplayMath::text(trimmed(coefficient, places));
    if exp == 0 {
        return coefficient;
    }
    let power = if exp == 1 {
        DisplayMath::text("10")
    } else {
        DisplayMath::Power(
            Box::new(DisplayMath::text("10")),
            Box::new(DisplayMath::text(exp.to_string().replace('-', "−"))),
        )
    };
    DisplayMath::Row(vec![coefficient, DisplayMath::text(" × "), power])
}
fn real(value: f64, style: &MathFormat) -> DisplayMath {
    if !value.is_finite() {
        return DisplayMath::text(if value.is_nan() {
            "NaN"
        } else if value.is_sign_negative() {
            "−∞"
        } else {
            "∞"
        });
    }
    let value = if value.abs() < 1e-15 { 0. } else { value };
    let places = usize::from(style.precision.min(15));
    match style.result_format {
        ResultFormat::Scientific => scientific(value, places, false),
        ResultFormat::Engineering => scientific(value, places, true),
        ResultFormat::Decimal => DisplayMath::text(trimmed(value, places)),
        ResultFormat::General => {
            if value != 0. && (value.abs() >= 1000. || value.abs() < 0.01) {
                scientific(value, places, false)
            } else {
                DisplayMath::text(trimmed(value, places))
            }
        }
    }
}
fn product(parts: Vec<DisplayMath>) -> DisplayMath {
    let mut output = vec![];
    for part in parts {
        if !output.is_empty() {
            output.push(DisplayMath::text("·"));
        }
        output.push(part);
    }
    if output.is_empty() {
        DisplayMath::text("1")
    } else {
        DisplayMath::Row(output)
    }
}
fn quantity(value: &Quantity, style: &MathFormat) -> DisplayMath {
    let re = if value.value.re.abs() < 1e-15 {
        0.
    } else {
        value.value.re
    };
    let im = if value.value.im.abs() < 1e-15 {
        0.
    } else {
        value.value.im
    };
    let mut numeric = if im == 0. {
        real(re, style)
    } else {
        let imaginary = DisplayMath::Row(vec![real(im.abs(), style), DisplayMath::text("i")]);
        if re == 0. {
            if im < 0. {
                DisplayMath::Row(vec![DisplayMath::text("−"), imaginary])
            } else {
                imaginary
            }
        } else {
            DisplayMath::Row(vec![
                real(re, style),
                DisplayMath::text(if im < 0. { " − " } else { " + " }),
                imaginary,
            ])
        }
    };
    let mut numerator = vec![];
    let mut denominator = vec![];
    for (unit, power) in ["m", "kg", "s", "A", "K", "mol", "cd"]
        .into_iter()
        .zip(value.dimension)
    {
        if power.abs() < 1e-9 {
            continue;
        }
        let base = DisplayMath::Text(unit.into(), MathStyle::Unit);
        let factor = if (power.abs() - 1.).abs() < 1e-9 {
            base
        } else {
            DisplayMath::Power(
                Box::new(base),
                Box::new(DisplayMath::text(format_complex(
                    mathcraft_core::Quantity::scalar(power.abs()).value,
                ))),
            )
        };
        if power > 0. {
            numerator.push(factor)
        } else {
            denominator.push(factor)
        }
    }
    if numerator.is_empty() && denominator.is_empty() {
        return numeric;
    }
    if re != 0. && im != 0. {
        numeric = DisplayMath::Group(Box::new(numeric));
    }
    let units = if denominator.is_empty() {
        product(numerator)
    } else {
        DisplayMath::Fraction(Box::new(product(numerator)), Box::new(product(denominator)))
    };
    DisplayMath::Row(vec![numeric, DisplayMath::text(" "), units])
}
fn symbolic(tree: &PortableTree, budget: usize) -> DisplayMath {
    if budget == 0 {
        return DisplayMath::text("…");
    }
    let children: Vec<_> = tree
        .children
        .iter()
        .map(|t| symbolic(t, budget - 1))
        .collect();
    let unary = || {
        Box::new(
            children
                .first()
                .cloned()
                .unwrap_or_else(|| DisplayMath::text("?")),
        )
    };
    match &tree.kind {
        NodeKind::Number(n) => DisplayMath::text(n),
        NodeKind::Identifier(name) => DisplayMath::Text(name.clone(), MathStyle::Variable),
        NodeKind::AnnotatedIdentifier { name, labels } => {
            DisplayMath::Text(name.clone(), MathStyle::from_label(labels))
        }
        NodeKind::NativeIdentifier {
            name,
            labels,
            subscript,
            ..
        } => {
            let base = DisplayMath::Text(name.clone(), MathStyle::from_label(labels));
            if let Some(sub) = subscript {
                DisplayMath::Subscript(
                    Box::new(base),
                    Box::new(DisplayMath::Text(
                        sub.clone(),
                        MathStyle::from_label(labels),
                    )),
                )
            } else {
                base
            }
        }
        NodeKind::Power if children.len() == 2 => {
            DisplayMath::Power(Box::new(children[0].clone()), Box::new(children[1].clone()))
        }
        NodeKind::Fraction if children.len() == 2 => {
            DisplayMath::Fraction(Box::new(children[0].clone()), Box::new(children[1].clone()))
        }
        NodeKind::Group => DisplayMath::Group(unary()),
        NodeKind::Root if children.len() == 2 => {
            if matches!(&tree.children[0].kind,NodeKind::Number(n) if n=="2") {
                DisplayMath::Root(Box::new(children[1].clone()))
            } else {
                DisplayMath::Row(vec![
                    DisplayMath::Text("root".into(), MathStyle::Function),
                    DisplayMath::Group(Box::new(DisplayMath::Row(vec![
                        children[0].clone(),
                        DisplayMath::text(", "),
                        children[1].clone(),
                    ]))),
                ])
            }
        }
        NodeKind::UnaryNeg => DisplayMath::Row(vec![DisplayMath::text("−"), *unary()]),
        NodeKind::Binary(op) if children.len() == 2 => {
            let symbol = match op {
                BinaryOp::Add => " + ",
                BinaryOp::Subtract => " − ",
                BinaryOp::Multiply => "·",
                BinaryOp::Scale => " ",
                BinaryOp::InlineDivide => "/",
                BinaryOp::Define => ":=",
                BinaryOp::Evaluate => "=",
            };
            let precedence = tree.kind.precedence();
            let left = if tree.children[0].kind.precedence() < precedence {
                DisplayMath::Group(Box::new(children[0].clone()))
            } else {
                children[0].clone()
            };
            let right = if tree.children[1].kind.precedence() < precedence
                || (tree.children[1].kind.precedence() == precedence
                    && matches!(op, BinaryOp::Subtract | BinaryOp::InlineDivide))
            {
                DisplayMath::Group(Box::new(children[1].clone()))
            } else {
                children[1].clone()
            };
            DisplayMath::Row(vec![left, DisplayMath::text(symbol), right])
        }
        NodeKind::Call { .. } | NodeKind::Function { .. } if !children.is_empty() => {
            let mut args = vec![];
            for child in children.iter().skip(1) {
                if !args.is_empty() {
                    args.push(DisplayMath::text(", "));
                }
                args.push(child.clone());
            }
            let name = if let NodeKind::Identifier(name) = &tree.children[0].kind {
                DisplayMath::Text(name.clone(), MathStyle::Function)
            } else {
                children[0].clone()
            };
            DisplayMath::Row(vec![
                name,
                DisplayMath::Group(Box::new(DisplayMath::Row(args))),
            ])
        }
        _ => DisplayMath::text("지원하지 않는 기호 결과"),
    }
}

pub(crate) fn format(value: &Value, style: &MathFormat) -> FormattedResult {
    let scalar = |cell| FormattedResult {
        rows: 1,
        cols: 1,
        cells: vec![cell],
        brackets: false,
    };
    match value {
        Value::Scalar { quantity: q } => scalar(quantity(q, style)),
        Value::Symbolic { mathml } => scalar(
            worksheet_math::tree_from_mathml(mathml)
                .map(|t| symbolic(&t, 48))
                .unwrap_or_else(|_| DisplayMath::text("지원하지 않는 기호 결과")),
        ),
        Value::Matrix {
            rows,
            cols,
            elements,
        } => {
            let r = (*rows).min(12);
            let c = (*cols).min(12);
            FormattedResult {
                rows: r,
                cols: c,
                brackets: true,
                cells: (0..r)
                    .flat_map(|row| {
                        (0..c).map(move |col| {
                            if (*rows > 12 && row == 11) || (*cols > 12 && col == 11) {
                                DisplayMath::text("…")
                            } else {
                                quantity(&elements[row * cols + col], style)
                            }
                        })
                    })
                    .collect(),
            }
        }
        Value::Range {
            start, step, count, ..
        } => {
            if *count == 0 {
                return scalar(DisplayMath::text("∅"));
            }
            FormattedResult {
                rows: (*count).min(12),
                cols: 1,
                brackets: false,
                cells: (0..(*count).min(12))
                    .map(|i| {
                        if *count > 12 && i == 11 {
                            DisplayMath::text("…")
                        } else {
                            quantity(
                                &Quantity {
                                    value: start.value + step.value * i as f64,
                                    ..*start
                                },
                                style,
                            )
                        }
                    })
                    .collect(),
            }
        }
        Value::RangeValues { elements } => {
            if elements.is_empty() {
                return scalar(DisplayMath::text("∅"));
            }
            FormattedResult {
                rows: elements.len().min(12),
                cols: 1,
                brackets: false,
                cells: elements
                    .iter()
                    .take(12)
                    .enumerate()
                    .map(|(i, q)| {
                        if elements.len() > 12 && i == 11 {
                            DisplayMath::text("…")
                        } else {
                            quantity(q, style)
                        }
                    })
                    .collect(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn units_are_typeset_as_fraction_and_powers_without_rounding_physical_values() {
        let mut value = Quantity::scalar(1.23456789);
        value.dimension = [1., 1., -2., 0., 0., 0., 0.];
        let before = value;
        let display = quantity(&value, &MathFormat::default()).plain();
        assert_eq!(display, "1.235 m·kg/s^2");
        assert_eq!(value, before);
    }
    #[test]
    fn engineering_rounding_carries_and_subnormal_exponents_stay_finite() {
        let mut style = MathFormat {
            result_format: ResultFormat::Engineering,
            ..Default::default()
        };
        assert_eq!(real(999999.99, &style).plain(), "1 × 10^6");
        style.result_format = ResultFormat::Scientific;
        assert_eq!(real(f64::from_bits(1), &style).plain(), "0");
        assert_eq!(real(1e-15, &style).plain(), "1 × 10^−15");
        style.result_format = ResultFormat::Decimal;
        assert_eq!(real(-0.00001, &style).plain(), "0");
    }
    #[test]
    fn general_format_uses_recovered_exponential_and_zero_boundaries() {
        let style = MathFormat::default();
        for (value, expected) in [
            (0., "0"),
            (0.009, "9 × 10^−3"),
            (0.01, "0.01"),
            (999., "999"),
            (1000., "1 × 10^3"),
            (1e-16, "0"),
            (1e-15, "1 × 10^−15"),
        ] {
            assert_eq!(real(value, &style).plain(), expected);
        }
    }
}
