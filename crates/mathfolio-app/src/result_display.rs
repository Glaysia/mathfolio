//! Read-only mathematical result presentation; rounding never changes stored values.
use crate::{typography::MathStyle, worksheet_math};
use mathfolio_compat::Value;
use mathfolio_core::{Quantity, format_complex};
use mathfolio_document::canvas::{ComplexFormat, MathFormat, ResultFormat};
use mathfolio_editor::{BinaryOp, NodeKind, PortableTree};
mod matrix_projection;
mod numeric_literal;
#[cfg(test)]
pub(crate) use matrix_projection::take_formatted_matrix_cells;
pub(crate) use matrix_projection::{
    MatrixProjection, MatrixWindow, PreparedNativeMatrix, PreparedValueMatrix,
};

/// A bounded cold preview backed by the complete retained native matrix. This
/// does not change the legacy v1 reader or use a clipped preview as full data.
pub(crate) fn from_cached_native_result(xml: &str, style: &MathFormat) -> Option<FormattedResult> {
    if let Ok(matrix) = PreparedNativeMatrix::parse(xml)
        && let Ok(view) = matrix.project(
            MatrixWindow {
                row_offset: 0,
                col_offset: 0,
                rows: 12,
                cols: 12,
            },
            style,
        )
    {
        let preview = FormattedResult {
            rows: view.rows,
            cols: view.cols,
            cells: view.cells,
            brackets: true,
        };
        if !preview.to_saved().is_empty() {
            return Some(preview);
        }
    }
    from_native_result(xml, style)
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum DisplayMath {
    Text(String, MathStyle),
    Row(Vec<Self>),
    Power(Box<Self>, Box<Self>),
    Subscript(Box<Self>, Box<Self>),
    Fraction(Box<Self>, Box<Self>),
    Group(Box<Self>),
    Root(Box<Self>),
    Overline(Box<Self>),
    OverArrow(Box<Self>),
    Bars(Box<Self>, bool),
    AngleFence(Box<Self>, bool),
}
impl DisplayMath {
    fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into(), MathStyle::Operator)
    }
    #[cfg(test)]
    pub(crate) fn plain(&self) -> String {
        match self {
            Self::Text(s, _) => s.clone(),
            Self::Row(parts) => parts.iter().map(Self::plain).collect(),
            Self::Power(a, b) => format!("{}^{}", a.plain(), b.plain()),
            Self::Subscript(a, b) => format!("{}_{}", a.plain(), b.plain()),
            Self::Fraction(a, b) => format!("{}/{}", a.plain(), b.plain()),
            Self::Group(a) => format!("({})", a.plain()),
            Self::Root(a) => format!("√{}", a.plain()),
            Self::Overline(a) => format!("overline({})", a.plain()),
            Self::OverArrow(a) => format!("vectorize({})", a.plain()),
            Self::Bars(a, double) => {
                let bar = if *double { "‖" } else { "|" };
                format!("{bar}{}{bar}", a.plain())
            }
            Self::AngleFence(a, vertical) => {
                if *vertical {
                    format!("vertical-angle({})", a.plain())
                } else {
                    format!("〈{}〉", a.plain())
                }
            }
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FormattedResult {
    pub rows: usize,
    pub cols: usize,
    pub cells: Vec<DisplayMath>,
    pub brackets: bool,
}

const MAX_CACHE_BYTES: usize = 1024 * 1024;
const MAX_DISPLAY_NODES: usize = 4096;
const MAX_DISPLAY_TEXT: usize = 64 * 1024;
const MAX_DISPLAY_DEPTH: usize = 48;

#[derive(Default)]
struct DisplayBudget {
    nodes: usize,
    text: usize,
}
impl DisplayBudget {
    fn take(&mut self, depth: usize, text: usize) -> Option<()> {
        self.nodes = self.nodes.checked_add(1)?;
        self.text = self.text.checked_add(text)?;
        (depth <= MAX_DISPLAY_DEPTH
            && self.nodes <= MAX_DISPLAY_NODES
            && self.text <= MAX_DISPLAY_TEXT)
            .then_some(())
    }
}

impl FormattedResult {
    /// Saved presentation only. This contains neither executable input nor an
    /// engine value; it can be shown before explicit worksheet calculation.
    pub(crate) fn to_saved(&self) -> String {
        use serde_json::json;
        fn encode(
            node: &DisplayMath,
            depth: usize,
            b: &mut DisplayBudget,
        ) -> Option<serde_json::Value> {
            b.take(depth, 0)?;
            Some(match node {
                DisplayMath::Text(text, style) => {
                    b.text = b.text.checked_add(text.len())?;
                    if b.text > MAX_DISPLAY_TEXT {
                        return None;
                    }
                    let style = match style {
                        MathStyle::Operator => "operator",
                        MathStyle::Variable => "variable",
                        MathStyle::Unit => "unit",
                        MathStyle::Constant => "constant",
                        MathStyle::Function => "function",
                        MathStyle::Keyword => "keyword",
                    };
                    json!(["text", text, style])
                }
                DisplayMath::Row(parts) => json!([
                    "row",
                    parts
                        .iter()
                        .map(|p| encode(p, depth + 1, b))
                        .collect::<Option<Vec<_>>>()?
                ]),
                DisplayMath::Power(a, z)
                | DisplayMath::Subscript(a, z)
                | DisplayMath::Fraction(a, z) => {
                    let tag = match node {
                        DisplayMath::Power(..) => "power",
                        DisplayMath::Subscript(..) => "subscript",
                        _ => "fraction",
                    };
                    json!([tag, encode(a, depth + 1, b)?, encode(z, depth + 1, b)?])
                }
                DisplayMath::Group(a) | DisplayMath::Root(a) => json!([
                    if matches!(node, DisplayMath::Group(_)) {
                        "group"
                    } else {
                        "root"
                    },
                    encode(a, depth + 1, b)?
                ]),
                DisplayMath::Overline(a) | DisplayMath::OverArrow(a) | DisplayMath::Bars(a, _) => {
                    let tag = match node {
                        DisplayMath::Overline(_) => "overline",
                        DisplayMath::OverArrow(_) => "over-arrow",
                        DisplayMath::Bars(_, false) => "bars",
                        _ => "double-bars",
                    };
                    json!([tag, encode(a, depth + 1, b)?])
                }
                DisplayMath::AngleFence(a, vertical) => {
                    json!(["angle-fence", encode(a, depth + 1, b)?, vertical])
                }
            })
        }
        if !(1..=12).contains(&self.rows)
            || !(1..=12).contains(&self.cols)
            || self.cells.len() != self.rows * self.cols
        {
            return String::new();
        }
        let mut budget = DisplayBudget::default();
        let Some(cells) = self
            .cells
            .iter()
            .map(|n| encode(n, 0, &mut budget))
            .collect::<Option<Vec<_>>>()
        else {
            return String::new();
        };
        json!({"version":1,"rows":self.rows,"cols":self.cols,"brackets":self.brackets,"cells":cells}).to_string()
    }

    pub(crate) fn from_saved(source: &str) -> Option<Self> {
        fn decode(
            v: &serde_json::Value,
            depth: usize,
            b: &mut DisplayBudget,
        ) -> Option<DisplayMath> {
            b.take(depth, 0)?;
            let a = v.as_array()?;
            let tag = a.first()?.as_str()?;
            Some(match (tag, a.len()) {
                ("text", 3) => {
                    let text = a[1].as_str()?;
                    b.text = b.text.checked_add(text.len())?;
                    if b.text > MAX_DISPLAY_TEXT {
                        return None;
                    }
                    let style = match a[2].as_str()? {
                        "operator" => MathStyle::Operator,
                        "variable" => MathStyle::Variable,
                        "unit" => MathStyle::Unit,
                        "constant" => MathStyle::Constant,
                        "function" => MathStyle::Function,
                        "keyword" => MathStyle::Keyword,
                        _ => return None,
                    };
                    DisplayMath::Text(text.into(), style)
                }
                ("row", 2) => DisplayMath::Row(
                    a[1].as_array()?
                        .iter()
                        .map(|n| decode(n, depth + 1, b))
                        .collect::<Option<Vec<_>>>()?,
                ),
                ("power" | "subscript" | "fraction", 3) => {
                    let left = Box::new(decode(&a[1], depth + 1, b)?);
                    let right = Box::new(decode(&a[2], depth + 1, b)?);
                    match tag {
                        "power" => DisplayMath::Power(left, right),
                        "subscript" => DisplayMath::Subscript(left, right),
                        _ => DisplayMath::Fraction(left, right),
                    }
                }
                ("group" | "root", 2) => {
                    let inner = Box::new(decode(&a[1], depth + 1, b)?);
                    if tag == "group" {
                        DisplayMath::Group(inner)
                    } else {
                        DisplayMath::Root(inner)
                    }
                }
                ("overline" | "over-arrow" | "bars" | "double-bars", 2) => {
                    let inner = Box::new(decode(&a[1], depth + 1, b)?);
                    match tag {
                        "overline" => DisplayMath::Overline(inner),
                        "over-arrow" => DisplayMath::OverArrow(inner),
                        _ => DisplayMath::Bars(inner, tag == "double-bars"),
                    }
                }
                ("angle-fence", 3) => {
                    DisplayMath::AngleFence(Box::new(decode(&a[1], depth + 1, b)?), a[2].as_bool()?)
                }
                _ => return None,
            })
        }
        if source.len() > MAX_CACHE_BYTES {
            return None;
        }
        let v: serde_json::Value = serde_json::from_str(source).ok()?;
        if v["version"].as_u64()? != 1 {
            return None;
        }
        let rows = usize::try_from(v["rows"].as_u64()?).ok()?;
        let cols = usize::try_from(v["cols"].as_u64()?).ok()?;
        let cells = v["cells"].as_array()?;
        if !(1..=12).contains(&rows) || !(1..=12).contains(&cols) || cells.len() != rows * cols {
            return None;
        }
        let mut budget = DisplayBudget::default();
        Some(Self {
            rows,
            cols,
            brackets: v["brackets"].as_bool()?,
            cells: cells
                .iter()
                .map(|n| decode(n, 0, &mut budget))
                .collect::<Option<_>>()?,
        })
    }
}

/// Read inert saved result MathML, never the worksheet's input expression.
/// Unrecognized structures stay uninterpreted in the owning native cache.
pub(crate) fn from_native_result(xml: &str, style: &MathFormat) -> Option<FormattedResult> {
    const ML: &str = "http://schemas.mathsoft.com/math50";
    const UNITS: &str = "http://schemas.mathsoft.com/units10";
    fn children<'a, 'input>(node: roxmltree::Node<'a, 'input>) -> Vec<roxmltree::Node<'a, 'input>> {
        node.children()
            .filter(roxmltree::Node::is_element)
            .collect()
    }
    fn literal(node: roxmltree::Node<'_, '_>, style: &MathFormat) -> Option<DisplayMath> {
        let parts = children(node);
        if node.has_tag_name((ML, "str")) {
            if node.attributes().any(|a| {
                a.namespace() != Some("http://www.w3.org/XML/1998/namespace")
                    || a.name() != "space"
                    || a.value() != "preserve"
            }) || node.children().any(|n| !n.is_text())
            {
                return None;
            }
            return Some(string_literal(
                &node.children().filter_map(|n| n.text()).collect::<String>(),
            ));
        }
        if node.has_tag_name((ML, "real")) && parts.is_empty() {
            return numeric_literal::coefficient(node, false).map(|n| real(n, style));
        }
        if node.has_tag_name((ML, "complex"))
            && node.attributes().len() == 0
            && parts.len() == 2
            && parts[0].has_tag_name((ML, "real"))
            && parts[1].has_tag_name((ML, "imag"))
            && parts.iter().all(|n| children(*n).is_empty())
        {
            let re = numeric_literal::coefficient(parts[0], false)?;
            let im = numeric_literal::coefficient(parts[1], true)?;
            let mut q = Quantity::scalar(re);
            q.value.im = im;
            return Some(quantity(&q, style));
        }
        None
    }
    fn units(node: roxmltree::Node<'_, '_>) -> Option<DisplayMath> {
        if !node.has_tag_name((UNITS, "unitMonomial")) {
            return None;
        }
        let mut top = Vec::new();
        let mut bottom = Vec::new();
        for unit in children(node) {
            if !unit.has_tag_name((UNITS, "unitReference")) || !children(unit).is_empty() {
                return None;
            }
            let name = unit.attribute("unit")?;
            if name.is_empty() || name.len() > 256 {
                return None;
            }
            let numerator: i32 = unit.attribute("power-numerator")?.parse().ok()?;
            let denominator: u32 = unit.attribute("power-denominator")?.parse().ok()?;
            if denominator == 0 || numerator == i32::MIN {
                return None;
            }
            if numerator == 0 {
                continue;
            }
            let name = match name {
                "meter" => "m",
                "kilogram" => "kg",
                "second" => "s",
                "ampere" => "A",
                "kelvin" => "K",
                "mole" => "mol",
                "candela" => "cd",
                "volt" => "V",
                "ohm" => "Ω",
                "henry" => "H",
                "farad" => "F",
                "hertz" => "Hz",
                "newton" => "N",
                "pascal" => "Pa",
                "joule" => "J",
                "watt" => "W",
                "coulomb" => "C",
                "siemens" => "S",
                other => other,
            };
            let base = DisplayMath::Text(name.into(), MathStyle::Unit);
            let power = numerator.unsigned_abs();
            let factor = if power == 1 && denominator == 1 {
                base
            } else {
                let exponent = if denominator == 1 {
                    DisplayMath::text(power.to_string())
                } else {
                    DisplayMath::Fraction(
                        Box::new(DisplayMath::text(power.to_string())),
                        Box::new(DisplayMath::text(denominator.to_string())),
                    )
                };
                DisplayMath::Power(Box::new(base), Box::new(exponent))
            };
            if numerator > 0 {
                top.push(factor);
            } else {
                bottom.push(factor);
            }
        }
        Some(if bottom.is_empty() {
            product(top)
        } else {
            DisplayMath::Fraction(Box::new(product(top)), Box::new(product(bottom)))
        })
    }
    fn cell(node: roxmltree::Node<'_, '_>, style: &MathFormat) -> Option<DisplayMath> {
        if let Some(n) = literal(node, style) {
            return Some(n);
        }
        let parts = children(node);
        if node.has_tag_name((ML, "unitedValue")) && parts.len() == 2 {
            if parts[0].has_tag_name((ML, "str")) {
                return None;
            }
            let number = literal(parts[0], style)?;
            let number = if needs_numeric_group(&number) {
                DisplayMath::Group(Box::new(number))
            } else {
                number
            };
            return Some(DisplayMath::Row(vec![
                number,
                DisplayMath::text(" "),
                units(parts[1])?,
            ]));
        }
        None
    }
    if xml.len() > MAX_CACHE_BYTES {
        return None;
    }
    let doc = roxmltree::Document::parse(xml).ok()?;
    let mut budget = DisplayBudget::default();
    for n in doc.descendants() {
        if n.is_element() {
            budget.take(n.ancestors().count(), 0)?;
        }
        if n.is_text() {
            budget.text = budget.text.checked_add(n.text()?.len())?;
            if budget.text > MAX_DISPLAY_TEXT {
                return None;
            }
        }
    }
    let mut node = doc.root_element();
    if node.has_tag_name((ML, "result")) {
        let parts = children(node);
        if parts.len() != 1 {
            return None;
        }
        node = parts[0];
    }
    let result = if node.has_tag_name((ML, "symResult")) {
        let parts = children(node);
        if parts.len() != 1 {
            return None;
        }
        let symbolic_xml = format!(
            "<ml:symResult xmlns:ml=\"{ML}\" xmlns=\"{ML}\">{}</ml:symResult>",
            &xml[parts[0].range()]
        );
        let tree = worksheet_math::tree_from_mathml(&symbolic_xml).ok()?;
        let mut pending = vec![&tree];
        while let Some(part) = pending.pop() {
            if !matches!(
                &part.kind,
                NodeKind::Number(_)
                    | NodeKind::Imaginary(_)
                    | NodeKind::ImaginarySymbol { .. }
                    | NodeKind::Identifier(_)
                    | NodeKind::AnnotatedIdentifier { .. }
                    | NodeKind::NativeIdentifier { .. }
                    | NodeKind::Power
                    | NodeKind::Fraction
                    | NodeKind::Group
                    | NodeKind::Root
                    | NodeKind::UnaryNeg
                    | NodeKind::LogicalNot
                    | NodeKind::Absolute
                    | NodeKind::Factorial
                    | NodeKind::Percent
                    | NodeKind::Degree
                    | NodeKind::Transpose
                    | NodeKind::Conjugate
                    | NodeKind::Vectorize
                    | NodeKind::Determinant
                    | NodeKind::Row
                    | NodeKind::Column
                    | NodeKind::Binary(
                        BinaryOp::Add
                            | BinaryOp::Subtract
                            | BinaryOp::Multiply
                            | BinaryOp::CrossProduct
                            | BinaryOp::Polar
                            | BinaryOp::Scale
                            | BinaryOp::InlineDivide
                    )
                    | NodeKind::Call { .. }
                    | NodeKind::Function { .. }
            ) {
                return None;
            }
            pending.extend(&part.children);
        }
        FormattedResult {
            rows: 1,
            cols: 1,
            cells: vec![symbolic(&tree, 48)],
            brackets: false,
        }
    } else if node.has_tag_name((ML, "matrix")) {
        let rows = node.attribute("rows")?.parse::<usize>().ok()?;
        let cols = node.attribute("cols")?.parse::<usize>().ok()?;
        let elements = children(node);
        if rows == 0 || cols == 0 || rows.checked_mul(cols)? != elements.len() {
            return None;
        }
        let cells = elements
            .iter()
            .map(|n| {
                #[cfg(test)]
                matrix_projection::record_matrix_cell();
                cell(*n, style)
            })
            .collect::<Option<Vec<_>>>()?;
        let r = rows.min(12);
        let c = cols.min(12);
        FormattedResult {
            rows: r,
            cols: c,
            brackets: true,
            cells: (0..r)
                .flat_map(|row| (0..c).map(move |col| (row, col)))
                .map(|(row, col)| {
                    if (rows > 12 && row == 11) || (cols > 12 && col == 11) {
                        DisplayMath::text("…")
                    } else {
                        // Native result XML stores the columns consecutively.
                        cells[col * rows + row].clone()
                    }
                })
                .collect(),
        }
    } else {
        FormattedResult {
            rows: 1,
            cols: 1,
            brackets: false,
            cells: vec![cell(node, style)?],
        }
    };
    // Apply the same presentation limits to native and internal saved results.
    (!result.to_saved().is_empty()).then_some(result)
}

fn decimal_text(value: f64, style: &MathFormat) -> String {
    if !value.is_finite() {
        return if value.is_nan() {
            "NaN".into()
        } else if value.is_sign_negative() {
            "-∞".into()
        } else {
            "∞".into()
        };
    }
    let places = usize::from(style.precision.min(15));
    let text = format!("{value:.places$}");
    let mut text = if !style.show_trailing_zeros && text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        text
    };
    if text.starts_with('-') && text[1..].chars().all(|ch| ch == '0' || ch == '.') {
        text.remove(0);
    }
    text
}
fn is_zero_text(text: &str) -> bool {
    text.parse::<f64>().is_ok_and(|value| value == 0.)
}
fn is_zero_display(value: &DisplayMath) -> bool {
    matches!(value, DisplayMath::Text(text, _) if is_zero_text(text))
}
fn scientific(value: f64, style: &MathFormat, engineering: bool) -> DisplayMath {
    if value == 0. {
        return DisplayMath::text(decimal_text(0., style));
    }
    // Parsing Rust's decimal exponent avoids powi underflow for subnormal values.
    let source = format!("{value:e}");
    let (mantissa, exponent) = source.split_once('e').unwrap();
    let exponent: i32 = exponent.parse().unwrap();
    let exp = if engineering {
        exponent.div_euclid(3) * 3
    } else {
        exponent
    };
    let coefficient = mantissa.parse::<f64>().unwrap() * 10_f64.powi(exponent - exp);
    let coefficient = decimal_text(coefficient, style);
    // The original formatter selects its exponent before decimal formatting;
    // it does not renormalize a mantissa that rounds up to 10 or 1000.
    if is_zero_text(&coefficient) {
        return DisplayMath::text(coefficient);
    }
    if style.use_e_notation {
        // REA CreateExponentialENotationFormat: "+000;-000", uppercase E.
        return DisplayMath::text(format!("{}E{exp:+04}", coefficient.replace('-', "−")));
    }
    let coefficient = DisplayMath::text(coefficient.replace('-', "−"));
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
    let value = if value.abs() < 10_f64.powf(-f64::from(style.zero_threshold)) {
        0.
    } else {
        value
    };
    match style.result_format {
        ResultFormat::Scientific => scientific(value, style, false),
        ResultFormat::Engineering => scientific(value, style, true),
        ResultFormat::Decimal => DisplayMath::text(decimal_text(value, style).replace('-', "−")),
        ResultFormat::Percent => {
            let percent = decimal_text(value * 100., style);
            if is_zero_text(&percent) {
                DisplayMath::text(percent)
            } else {
                DisplayMath::Row(vec![
                    DisplayMath::text(percent.replace('-', "−")),
                    DisplayMath::text("%"),
                ])
            }
        }
        ResultFormat::General => {
            let threshold = i32::from(style.exponential_threshold.max(1));
            if value != 0.
                && (value.abs() >= 10_f64.powf(f64::from(threshold))
                    || value.abs() < 10_f64.powf(-f64::from(threshold - 1)))
            {
                // use_engineering_format is a preserved schema setting. The
                // inspected General renderer always requests exponent step 1.
                scientific(value, style, false)
            } else {
                DisplayMath::text(decimal_text(value, style).replace('-', "−"))
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
fn complex_numeric(mut re: f64, mut im: f64, style: &MathFormat) -> DisplayMath {
    let was_complex = im != 0.;
    if re != 0. && im != 0. {
        let ratio = 10_f64.powf(f64::from(style.complex_threshold));
        if (re / im).abs() > ratio {
            im = 0.;
        } else if (im / re).abs() > ratio {
            re = 0.;
        }
    }
    if was_complex
        && matches!(
            style.complex_format,
            ComplexFormat::PolarRadians
                | ComplexFormat::PolarPiRadians
                | ComplexFormat::PolarDegrees
        )
    {
        let radius = real(re.hypot(im), style);
        if is_zero_display(&radius) {
            return radius;
        }
        let theta = im.atan2(re);
        let angle = match style.complex_format {
            ComplexFormat::PolarRadians => real(theta, style),
            ComplexFormat::PolarPiRadians => DisplayMath::Row(vec![
                real(theta / std::f64::consts::PI, style),
                DisplayMath::text(" "),
                DisplayMath::text("π"),
            ]),
            ComplexFormat::PolarDegrees => DisplayMath::Row(vec![
                real(theta.to_degrees(), style),
                DisplayMath::text("°"),
            ]),
            _ => unreachable!(),
        };
        return DisplayMath::Row(vec![radius, DisplayMath::text(" ∠ "), angle]);
    }
    let real_part = real(re, style);
    let imaginary = real(im.abs(), style);
    if is_zero_display(&imaginary) {
        return real_part;
    }
    let imaginary_symbol = DisplayMath::text(if style.complex_format == ComplexFormat::J {
        "j"
    } else {
        "i"
    });
    let imaginary = if style.result_format == ResultFormat::Percent {
        // EquationEditor's percent imaginary formatter wraps PERCENT around
        // the coefficient-plus-symbol, so the order is 300i%, not 300%i.
        match imaginary {
            DisplayMath::Row(mut parts) if parts.len() == 2 => {
                parts.insert(1, imaginary_symbol);
                DisplayMath::Row(parts)
            }
            number => DisplayMath::Row(vec![number, imaginary_symbol]),
        }
    } else {
        DisplayMath::Row(vec![imaginary, imaginary_symbol])
    };
    if is_zero_display(&real_part) {
        if im < 0. {
            DisplayMath::Row(vec![DisplayMath::text("−"), imaginary])
        } else {
            imaginary
        }
    } else {
        DisplayMath::Row(vec![
            real_part,
            DisplayMath::text(if im < 0. { " − " } else { " + " }),
            imaginary,
        ])
    }
}

fn needs_numeric_group(numeric: &DisplayMath) -> bool {
    matches!(numeric, DisplayMath::Row(parts) if parts.iter().any(|part| matches!(part, DisplayMath::Text(text, _) if matches!(text.as_str(), " + " | " − " | " ∠ "))))
}

fn quantity(value: &Quantity, style: &MathFormat) -> DisplayMath {
    // These are the shipped SI unit system's named derived-unit candidates,
    // independently read from its static unit_system metadata. Only use an
    // exact dimension match; compound/fractional dimensions keep the base-unit
    // representation below. Explicit result-unit slots have already converted
    // their display payload to dimensionless values and do not enter this path.
    // See docs/analysis/0050-unit-regressions.md.
    const DERIVED: &[(&str, f64, [f64; 7])] = &[
        ("C", 1., [0., 0., 1., 1., 0., 0., 0.]),
        ("F", 1., [-2., -1., 4., 2., 0., 0., 0.]),
        ("H", 1., [2., 1., -2., -2., 0., 0., 0.]),
        ("J", 1., [2., 1., -2., 0., 0., 0., 0.]),
        ("L", 0.001, [3., 0., 0., 0., 0., 0., 0.]),
        ("lx", 1., [-2., 0., 0., 0., 0., 0., 1.]),
        ("N", 1., [1., 1., -2., 0., 0., 0., 0.]),
        ("Ω", 1., [2., 1., -3., -2., 0., 0., 0.]),
        ("Pa", 1., [-1., 1., -2., 0., 0., 0., 0.]),
        ("S", 1., [-2., -1., 3., 2., 0., 0., 0.]),
        ("T", 1., [0., 1., -2., -1., 0., 0., 0.]),
        ("W", 1., [2., 1., -3., 0., 0., 0., 0.]),
        ("Wb", 1., [2., 1., -2., -1., 0., 0., 0.]),
        ("V", 1., [2., 1., -3., -1., 0., 0., 0.]),
    ];
    if let Some((symbol, scale, _)) = DERIVED.iter().find(|(_, _, dimension)| {
        value
            .dimension
            .iter()
            .zip(dimension)
            .all(|(left, right)| (left - right).abs() < 1e-9)
    }) {
        let scaled = value.value / *scale;
        if scaled.re.is_finite() && scaled.im.is_finite() {
            let mut number = complex_numeric(scaled.re, scaled.im, style);
            if needs_numeric_group(&number) {
                number = DisplayMath::Group(Box::new(number));
            }
            return DisplayMath::Row(vec![
                number,
                DisplayMath::text(" "),
                DisplayMath::Text((*symbol).into(), MathStyle::Unit),
            ]);
        }
    }
    let mut numeric = complex_numeric(value.value.re, value.value.im, style);
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
                    mathfolio_core::Quantity::scalar(power.abs()).value,
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
    if needs_numeric_group(&numeric) {
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
        NodeKind::StringLiteral(value) => string_literal(value),
        NodeKind::Number(n) => DisplayMath::text(n),
        NodeKind::BasedNumber { literal, .. } => DisplayMath::text(literal),
        NodeKind::Imaginary(value) => DisplayMath::Row(vec![
            DisplayMath::text(value),
            DisplayMath::Text("i".into(), MathStyle::Constant),
        ]),
        NodeKind::ImaginarySymbol { value, symbol } => DisplayMath::Row(vec![
            DisplayMath::text(value),
            DisplayMath::Text(symbol.clone(), MathStyle::Constant),
        ]),
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
        NodeKind::LogicalNot => DisplayMath::Row(vec![DisplayMath::text("¬"), *unary()]),
        NodeKind::Factorial | NodeKind::Percent | NodeKind::Degree => {
            let child = if (tree.children[0].kind.precedence() < tree.kind.precedence()
                && !(tree.kind == NodeKind::Degree && tree.children[0].kind == NodeKind::Power))
                || (tree.kind == NodeKind::Percent
                    && matches!(tree.children[0].kind, NodeKind::Power | NodeKind::Root))
            {
                DisplayMath::Group(unary())
            } else {
                *unary()
            };
            let symbol = match tree.kind {
                NodeKind::Percent => "%",
                NodeKind::Degree => "°",
                _ => "!",
            };
            DisplayMath::Row(vec![child, DisplayMath::text(symbol)])
        }
        NodeKind::Transpose => {
            let child = if tree.children[0].kind.precedence() < tree.kind.precedence()
                || matches!(tree.children[0].kind, NodeKind::Row | NodeKind::Column)
            {
                DisplayMath::Group(unary())
            } else {
                *unary()
            };
            DisplayMath::Power(Box::new(child), Box::new(DisplayMath::text("T")))
        }
        NodeKind::Conjugate => DisplayMath::Overline(unary()),
        NodeKind::Vectorize => DisplayMath::OverArrow(unary()),
        NodeKind::Absolute => DisplayMath::Bars(unary(), false),
        NodeKind::Determinant => DisplayMath::Bars(unary(), true),
        NodeKind::Row | NodeKind::Column if children.len() == 2 => {
            let base = if tree.children[0].kind.precedence() < tree.kind.precedence()
                || matches!(
                    tree.children[0].kind,
                    NodeKind::Transpose | NodeKind::Row | NodeKind::Column
                ) {
                DisplayMath::Group(Box::new(children[0].clone()))
            } else {
                children[0].clone()
            };
            DisplayMath::Power(
                Box::new(base),
                Box::new(DisplayMath::AngleFence(
                    Box::new(children[1].clone()),
                    matches!(tree.kind, NodeKind::Row),
                )),
            )
        }
        NodeKind::Binary(op) if children.len() == 2 => {
            let symbol = match op {
                BinaryOp::Add => " + ",
                BinaryOp::Subtract => " − ",
                BinaryOp::Multiply => "·",
                BinaryOp::CrossProduct => " ⨯ ",
                BinaryOp::Polar => " ∠ ",
                BinaryOp::Scale => " ",
                BinaryOp::InlineDivide => "/",
                BinaryOp::Define => ":=",
                BinaryOp::GlobalDefine => "≡",
                BinaryOp::Evaluate => "=",
                BinaryOp::Equal => " ＝ ",
                BinaryOp::NotEqual => " ≠ ",
                BinaryOp::LessThan => " < ",
                BinaryOp::LessOrEqual => " ≤ ",
                BinaryOp::GreaterThan => " > ",
                BinaryOp::GreaterOrEqual => " ≥ ",
                BinaryOp::And => " ∧ ",
                BinaryOp::Or => " ∨ ",
                BinaryOp::Xor => " ⊕ ",
            };
            let precedence = tree.kind.precedence();
            let left = if tree.children[0].kind.precedence() < precedence
                || (*op == BinaryOp::CrossProduct
                    && matches!(
                        tree.children[0].kind,
                        NodeKind::Binary(BinaryOp::CrossProduct | BinaryOp::Polar)
                    )) {
                DisplayMath::Group(Box::new(children[0].clone()))
            } else {
                children[0].clone()
            };
            let right = if tree.children[1].kind.precedence() < precedence
                || (tree.children[1].kind.precedence() == precedence
                    && matches!(op, BinaryOp::Subtract | BinaryOp::InlineDivide))
                || (matches!(op, BinaryOp::CrossProduct | BinaryOp::Polar)
                    && tree.children[1].kind == NodeKind::Binary(BinaryOp::Polar))
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

fn string_literal(value: &str) -> DisplayMath {
    DisplayMath::Row(vec![
        DisplayMath::text("“"),
        DisplayMath::text(value),
        DisplayMath::text("”"),
    ])
}

pub(crate) fn format(value: &Value, style: &MathFormat) -> FormattedResult {
    let limit = 12;
    let scalar = |cell| FormattedResult {
        rows: 1,
        cols: 1,
        cells: vec![cell],
        brackets: false,
    };
    match value {
        Value::String { value } => scalar(string_literal(value)),
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
            let r = (*rows).min(limit);
            let c = (*cols).min(limit);
            FormattedResult {
                rows: r,
                cols: c,
                brackets: true,
                cells: (0..r)
                    .flat_map(|row| {
                        (0..c).map(move |col| {
                            if (*rows > limit && row + 1 == limit)
                                || (*cols > limit && col + 1 == limit)
                            {
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
                rows: (*count).min(limit),
                cols: 1,
                brackets: false,
                cells: (0..(*count).min(limit))
                    .map(|i| {
                        if *count > limit && i + 1 == limit {
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
                rows: elements.len().min(limit),
                cols: 1,
                brackets: false,
                cells: elements
                    .iter()
                    .take(limit)
                    .enumerate()
                    .map(|(i, q)| {
                        if elements.len() > limit && i + 1 == limit {
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
    fn saved_presentation_preserves_matrix_unit_styles_and_symbolic_structure() {
        let result = FormattedResult {
            rows: 1,
            cols: 2,
            brackets: true,
            cells: vec![
                quantity(
                    &Quantity {
                        dimension: [1., 1., -2., 0., 0., 0., 0.],
                        ..Quantity::scalar(2.5)
                    },
                    &MathFormat::default(),
                ),
                DisplayMath::Root(Box::new(DisplayMath::Subscript(
                    Box::new(DisplayMath::Text("x".into(), MathStyle::Variable)),
                    Box::new(DisplayMath::Text("c".into(), MathStyle::Constant)),
                ))),
            ],
        };
        assert_eq!(
            FormattedResult::from_saved(&result.to_saved()),
            Some(result)
        );
    }

    #[test]
    fn saved_presentation_keeps_overlines_and_vertical_fences_as_structure() {
        let variable = || DisplayMath::Text("z".into(), MathStyle::Variable);
        let result = FormattedResult {
            rows: 1,
            cols: 5,
            brackets: false,
            cells: vec![
                DisplayMath::Overline(Box::new(DisplayMath::Fraction(
                    Box::new(variable()),
                    Box::new(DisplayMath::text("2")),
                ))),
                DisplayMath::Bars(Box::new(variable()), false),
                DisplayMath::Bars(Box::new(variable()), true),
                DisplayMath::Power(
                    Box::new(variable()),
                    Box::new(DisplayMath::AngleFence(
                        Box::new(DisplayMath::text("1")),
                        false,
                    )),
                ),
                DisplayMath::Power(
                    Box::new(variable()),
                    Box::new(DisplayMath::AngleFence(
                        Box::new(DisplayMath::text("1")),
                        true,
                    )),
                ),
            ],
        };
        assert_eq!(
            FormattedResult::from_saved(&result.to_saved()),
            Some(result)
        );
        let invalid = serde_json::json!({"version":1,"rows":1,"cols":1,"brackets":false,"cells":[["overline",["text","z","variable"],"extra"]]});
        assert!(FormattedResult::from_saved(&invalid.to_string()).is_none());
        let mut node = serde_json::json!(["text", "z", "variable"]);
        for _ in 0..=MAX_DISPLAY_DEPTH {
            node = serde_json::json!(["overline", node]);
        }
        let deep =
            serde_json::json!({"version":1,"rows":1,"cols":1,"brackets":false,"cells":[node]});
        assert!(FormattedResult::from_saved(&deep.to_string()).is_none());
    }

    #[test]
    fn vectorization_arrow_is_a_bounded_saved_structure() {
        let result = FormattedResult {
            rows: 1,
            cols: 1,
            brackets: false,
            cells: vec![DisplayMath::OverArrow(Box::new(DisplayMath::Fraction(
                Box::new(DisplayMath::Text("a".into(), MathStyle::Variable)),
                Box::new(DisplayMath::Text("b".into(), MathStyle::Variable)),
            )))],
        };
        assert_eq!(
            FormattedResult::from_saved(&result.to_saved()),
            Some(result)
        );
        let invalid = serde_json::json!({"version":1,"rows":1,"cols":1,"brackets":false,"cells":[["over-arrow"]]});
        assert!(FormattedResult::from_saved(&invalid.to_string()).is_none());
    }

    #[test]
    fn native_symbolic_operator_results_retain_display_structure_without_evaluation() {
        for (opcode, operands, expected) in [
            ("factorial", "<real>5</real>", "5!"),
            ("percent", "<real>50</real>", "50%"),
            ("degree_op", "<real>90</real>", "90°"),
            (
                "polar",
                "<real>2</real><apply><degree_op/><real>90</real></apply>",
                "2 ∠ 90°",
            ),
            (
                "polar",
                "<id>a</id><apply><polar/><id>b</id><id>c</id></apply>",
                "a ∠ (b ∠ c)",
            ),
            ("transpose", "<id>M</id>", "M^T"),
            ("conjugate", "<id>z</id>", "overline(z)"),
            ("conjugate", "<imag>3</imag>", "overline(3i)"),
            ("absval", "<id>M</id>", "|M|"),
            ("determinant", "<id>M</id>", "‖M‖"),
            ("matcol", "<id>M</id><real>1</real>", "M^〈1〉"),
            ("matrow", "<id>M</id><real>1</real>", "M^vertical-angle(1)"),
            (
                "vectorize",
                "<apply><mult/><id>a</id><id>b</id></apply>",
                "vectorize(a·b)",
            ),
            ("crossProduct", "<id>a</id><id>b</id>", "a ⨯ b"),
            (
                "crossProduct",
                "<id>a</id><apply><crossProduct/><id>b</id><id>c</id></apply>",
                "a ⨯ b ⨯ c",
            ),
            (
                "crossProduct",
                "<apply><crossProduct/><id>a</id><id>b</id></apply><id>c</id>",
                "(a ⨯ b) ⨯ c",
            ),
            (
                "transpose",
                "<apply><matrow/><id>M</id><real>1</real></apply>",
                "(M^vertical-angle(1))^T",
            ),
            (
                "matrow",
                "<apply><transpose/><id>M</id></apply><real>1</real>",
                "(M^T)^vertical-angle(1)",
            ),
            (
                "matcol",
                "<apply><matcol/><id>M</id><real>1</real></apply><real>2</real>",
                "(M^〈1〉)^〈2〉",
            ),
        ] {
            let xml = format!(
                "<result xmlns=\"http://schemas.mathsoft.com/math50\"><symResult><apply><{opcode}/>{operands}</apply></symResult></result>"
            );
            let result =
                from_native_result(&xml, &MathFormat::default()).unwrap_or_else(|| panic!("{xml}"));
            assert_eq!(result.cells[0].plain(), expected);
            assert_eq!(
                FormattedResult::from_saved(&result.to_saved()),
                Some(result)
            );
        }
    }

    #[test]
    fn saved_presentation_rejects_malformed_or_excessive_payloads() {
        let base = serde_json::json!({"version":1,"rows":1,"cols":1,"brackets":false,"cells":[["text","3","operator"]]});
        for (key, value) in [
            ("version", serde_json::json!(2)),
            ("rows", serde_json::json!(13)),
            ("cols", serde_json::json!(0)),
            ("cells", serde_json::json!([])),
            ("brackets", serde_json::json!("false")),
        ] {
            let mut invalid = base.clone();
            invalid[key] = value;
            assert!(FormattedResult::from_saved(&invalid.to_string()).is_none());
        }
        let mut invalid = base.clone();
        invalid["cells"] = serde_json::json!([["text", "x", "execute"]]);
        assert!(FormattedResult::from_saved(&invalid.to_string()).is_none());
        invalid["cells"] =
            serde_json::json!([["text", "x".repeat(MAX_DISPLAY_TEXT + 1), "operator"]]);
        assert!(FormattedResult::from_saved(&invalid.to_string()).is_none());
        let mut deep = serde_json::json!(["text", "3", "operator"]);
        for _ in 0..=MAX_DISPLAY_DEPTH {
            deep = serde_json::json!(["group", deep]);
        }
        invalid["cells"] = serde_json::json!([deep]);
        assert!(FormattedResult::from_saved(&invalid.to_string()).is_none());
        invalid["cells"] = serde_json::json!([[
            "row",
            vec![serde_json::json!(["text", "x", "operator"]); MAX_DISPLAY_NODES]
        ]]);
        assert!(FormattedResult::from_saved(&invalid.to_string()).is_none());
        assert!(FormattedResult::from_saved(&" ".repeat(MAX_CACHE_BYTES + 1)).is_none());
    }

    #[test]
    fn native_saved_literals_show_without_evaluating_inputs() {
        let xml = "<ml:result xmlns:ml=\"http://schemas.mathsoft.com/math50\" xmlns:u=\"http://schemas.mathsoft.com/units10\"><ml:unitedValue><ml:complex><ml:real>2</ml:real><ml:imag>-3</ml:imag></ml:complex><u:unitMonomial><u:unitReference unit=\"volt\" power-numerator=\"1\" power-denominator=\"1\"/></u:unitMonomial></ml:unitedValue></ml:result>";
        let result = from_native_result(xml, &MathFormat::default()).unwrap();
        assert_eq!(result.cells[0].plain(), "(2 − 3i) V");
        assert_eq!(
            FormattedResult::from_saved(&result.to_saved()),
            Some(result)
        );
        for expression in [
            "<apply><plus/><real>2</real><real>3</real></apply>",
            "<apply><id>READPRN</id><str>file</str></apply>",
            "<real>NaN</real>",
            "<complex><real>2</real><imag>3</imag><real>4</real></complex>",
        ] {
            assert!(
                from_native_result(
                    &format!(
                        "<result xmlns=\"http://schemas.mathsoft.com/math50\">{expression}</result>"
                    ),
                    &MathFormat::default()
                )
                .is_none()
            );
        }
    }

    #[test]
    fn native_saved_matrix_validates_dimensions_and_retains_presentation() {
        let xml = "<result xmlns=\"http://schemas.mathsoft.com/math50\"><matrix rows=\"1\" cols=\"2\"><real>3</real><complex><real>4</real><imag>1</imag></complex></matrix></result>";
        let result = from_native_result(xml, &MathFormat::default()).unwrap();
        assert_eq!((result.rows, result.cols, result.brackets), (1, 2, true));
        assert_eq!(result.cells[1].plain(), "4 + 1i");
        assert!(
            from_native_result(
                &xml.replace("cols=\"2\"", "cols=\"3\""),
                &MathFormat::default()
            )
            .is_none()
        );
        let matrix = from_native_result("<matrix xmlns=\"http://schemas.mathsoft.com/math50\" rows=\"2\" cols=\"2\"><real>1</real><real>3</real><real>2</real><real>4</real></matrix>", &MathFormat::default()).unwrap();
        assert_eq!(
            matrix
                .cells
                .iter()
                .map(DisplayMath::plain)
                .collect::<Vec<_>>(),
            ["1", "2", "3", "4"]
        );
    }

    #[test]
    fn native_saved_symbolic_tree_is_displayed_without_calculation() {
        let xml = "<ml:symResult xmlns:ml=\"http://schemas.mathsoft.com/math50\"><ml:apply><ml:pow/><ml:id labels=\"VARIABLE\">x</ml:id><ml:real>2</ml:real></ml:apply></ml:symResult>";
        let result = from_native_result(xml, &MathFormat::default()).unwrap();
        assert_eq!(result.cells[0].plain(), "x^2");
        assert_eq!(
            FormattedResult::from_saved(&result.to_saved()),
            Some(result)
        );
        assert!(
            from_native_result(
                "<symResult xmlns=\"http://schemas.mathsoft.com/math50\"><unknown/></symResult>",
                &MathFormat::default()
            )
            .is_none()
        );
    }
    #[test]
    fn symbolic_result_wrapper_displays_the_computed_expression() {
        let result = Value::Symbolic { mathml:
            "<symResult xmlns=\"http://schemas.mathsoft.com/math50\"><id labels=\"VARIABLE\">x</id></symResult>".into() };
        assert_eq!(
            format(&result, &MathFormat::default()).cells[0].plain(),
            "x"
        );
    }
    #[test]
    fn units_are_typeset_as_fraction_and_powers_without_rounding_physical_values() {
        let mut value = Quantity::scalar(1.23456789);
        value.dimension = [1., 0., -2., 0., 0., 0., 0.];
        let before = value;
        let display = quantity(&value, &MathFormat::default()).plain();
        assert_eq!(display, "1.235 m/s^2");
        assert_eq!(value, before);
    }
    #[test]
    fn recalculated_si_quantities_keep_named_units_from_native_presentations() {
        for (name, expected) in [("V", "2 V"), ("Ω", "2 Ω"), ("W", "2 W")] {
            let mut value = mathfolio_core::unit(name).unwrap();
            value.value *= 2.;
            let before = value;
            assert_eq!(quantity(&value, &MathFormat::default()).plain(), expected);
            assert_eq!(value, before);
        }
        let mut voltage = mathfolio_core::unit("V").unwrap();
        voltage.value.re = 2.;
        voltage.value.im = 3.;
        assert_eq!(
            quantity(&voltage, &MathFormat::default()).plain(),
            "(2 + 3i) V"
        );
        let cubic_meter = Quantity {
            value: Quantity::scalar(0.002).value,
            dimension: [3., 0., 0., 0., 0., 0., 0.],
        };
        assert_eq!(
            quantity(&cubic_meter, &MathFormat::default()).plain(),
            "2 L"
        );
        assert_eq!(
            quantity(&mathfolio_core::unit("Hz").unwrap(), &MathFormat::default()).plain(),
            "1 1/s"
        );
    }
    #[test]
    fn exponent_is_selected_before_rounding_and_zero_threshold_hides_subnormal_values() {
        let mut style = MathFormat {
            result_format: ResultFormat::Engineering,
            ..Default::default()
        };
        assert_eq!(real(999999.99, &style).plain(), "1000 × 10^3");
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

    #[test]
    fn decimal_places_trailing_zeros_and_general_thresholds_are_independent_controls() {
        let mut style = MathFormat {
            result_format: ResultFormat::Decimal,
            show_trailing_zeros: true,
            ..Default::default()
        };
        assert_eq!(real(123.4, &style).plain(), "123.400");
        assert_eq!(real(-0.00001, &style).plain(), "0.000");
        style.precision = 0;
        assert_eq!(real(123.4, &style).plain(), "123");
        style.precision = 15;
        assert_eq!(real(0.125, &style).plain(), "0.125000000000000");
        style.precision = 3;
        style.show_trailing_zeros = false;
        style.result_format = ResultFormat::General;
        style.exponential_threshold = 4;
        assert_eq!(real(0.001, &style).plain(), "0.001");
        assert_eq!(real(0.0009, &style).plain(), "9 × 10^−4");
        assert_eq!(real(9999., &style).plain(), "9999");
        assert_eq!(real(10000., &style).plain(), "1 × 10^4");
        style.exponential_threshold = 0;
        assert_eq!(real(0.1, &style).plain(), "1 × 10^−1");
        assert_eq!(real(1., &style).plain(), "1");
        style.zero_threshold = 2;
        assert_eq!(real(0.009, &style).plain(), "0");
        assert_eq!(real(0.01, &style).plain(), "1 × 10^−2");
    }

    #[test]
    fn percent_live_and_inert_results_share_the_pipeline_without_changing_engine_values() {
        let style = MathFormat {
            result_format: ResultFormat::Percent,
            precision: 2,
            show_trailing_zeros: true,
            ..Default::default()
        };
        let value = Value::Scalar {
            quantity: Quantity::scalar(0.125),
        };
        let before = value.numeric_json();
        let live = format(&value, &style);
        let saved = from_native_result(
            "<real xmlns=\"http://schemas.mathsoft.com/math50\">0.125</real>",
            &style,
        )
        .unwrap();
        assert_eq!(live, saved);
        assert_eq!(live.cells[0].plain(), "12.50%");
        assert_eq!(value.numeric_json(), before);
        assert_eq!(FormattedResult::from_saved(&live.to_saved()), Some(live));
        assert_eq!(real(0., &style).plain(), "0.00");
        assert_eq!(real(-0.0000001, &style).plain(), "0.00");
        let threshold = MathFormat {
            zero_threshold: 2,
            ..style
        };
        assert_eq!(
            real(0.009, &threshold).plain(),
            "0.00",
            "Zero threshold precedes percentage scaling"
        );
        assert_eq!(real(0.01, &threshold).plain(), "1.00%");
    }

    #[test]
    fn scientific_engineering_and_e_notation_keep_the_recovered_exponent_shape() {
        let mut style = MathFormat {
            result_format: ResultFormat::Scientific,
            precision: 2,
            use_e_notation: true,
            show_trailing_zeros: true,
            ..Default::default()
        };
        assert_eq!(real(1200., &style).plain(), "1.20E+003");
        assert_eq!(real(-0.0012, &style).plain(), "−1.20E-003");
        assert_eq!(real(1.2, &style).plain(), "1.20E+000");
        assert_eq!(real(0., &style).plain(), "0.00");
        style.result_format = ResultFormat::Engineering;
        assert_eq!(real(12000., &style).plain(), "12.00E+003");
        assert_eq!(real(0.00012, &style).plain(), "120.00E-006");
        style.use_e_notation = false;
        assert_eq!(real(0.00012, &style).plain(), "120.00 × 10^−6");
        style.result_format = ResultFormat::General;
        style.use_engineering_format = true;
        assert_eq!(
            real(12000., &style).plain(),
            "1.20 × 10^4",
            "The preserved schema flag must not invent an unobserved General renderer"
        );
    }

    #[test]
    fn complex_threshold_is_relative_strict_and_never_changes_a_quantity() {
        let style = MathFormat {
            complex_threshold: 2,
            ..Default::default()
        };
        for (re, im, expected) in [
            (100., 1., "100 + 1i"),
            (101., 1., "101"),
            (1., 100., "1 + 100i"),
            (1., 101., "101i"),
        ] {
            let mut q = Quantity::scalar(re);
            q.value.im = im;
            let before = q;
            assert_eq!(quantity(&q, &style).plain(), expected);
            assert_eq!(q, before);
        }
        let mut q = Quantity::scalar(0.0001);
        q.value.im = -0.0002;
        let decimal = MathFormat {
            result_format: ResultFormat::Decimal,
            ..Default::default()
        };
        assert_eq!(
            quantity(&q, &decimal).plain(),
            "0",
            "Both components rounded to zero collapse to one zero"
        );
        let permissive = MathFormat {
            zero_threshold: 20,
            ..Default::default()
        };
        q.value.re = 0.;
        q.value.im = -1e-17;
        assert_eq!(quantity(&q, &permissive).plain(), "−1 × 10^−17i");
    }

    #[test]
    fn complex_i_j_polar_forms_match_native_matrix_cells_and_quadrants() {
        let mut q = Quantity::scalar(3.);
        q.value.im = 4.;
        for (complex_format, expected) in [
            (ComplexFormat::I, "3 + 4i"),
            (ComplexFormat::J, "3 + 4j"),
            (ComplexFormat::PolarRadians, "5 ∠ 0.927"),
            (ComplexFormat::PolarPiRadians, "5 ∠ 0.295 π"),
            (ComplexFormat::PolarDegrees, "5 ∠ 53.13°"),
        ] {
            let style = MathFormat {
                complex_format,
                ..Default::default()
            };
            let live = format(
                &Value::Matrix {
                    rows: 1,
                    cols: 2,
                    elements: vec![q, Quantity::scalar(0.125)],
                },
                &style,
            );
            let native = from_native_result("<matrix xmlns=\"http://schemas.mathsoft.com/math50\" rows=\"1\" cols=\"2\"><complex><real>3</real><imag>4</imag></complex><real>0.125</real></matrix>", &style).unwrap();
            assert_eq!(live, native);
            assert_eq!(live.cells[0].plain(), expected);
        }
        let degrees = MathFormat {
            complex_format: ComplexFormat::PolarDegrees,
            ..Default::default()
        };
        q.value.re = -3.;
        q.value.im = -4.;
        assert_eq!(quantity(&q, &degrees).plain(), "5 ∠ −126.87°");
        q.value.re = 0.;
        q.value.im = -2.;
        assert_eq!(quantity(&q, &degrees).plain(), "2 ∠ −90°");
        let tiny = MathFormat {
            zero_threshold: 2,
            ..degrees
        };
        q.value.im = 0.009;
        assert_eq!(
            quantity(&q, &tiny).plain(),
            "0",
            "A zero magnitude has no angle"
        );
        q.value.re = 0.008;
        q.value.im = 0.008;
        assert_ne!(
            quantity(&q, &tiny).plain(),
            "0",
            "Threshold applies to polar magnitude rather than separately zeroing both components"
        );
    }

    #[test]
    fn percent_complex_parts_keep_symbol_before_percent_and_share_unit_grouping() {
        let style = MathFormat {
            result_format: ResultFormat::Percent,
            complex_format: ComplexFormat::J,
            ..Default::default()
        };
        let mut q = Quantity::scalar(2.);
        q.value.im = -3.;
        q.dimension[0] = 1.;
        let before = q;
        let live = format(&Value::Scalar { quantity: q }, &style);
        let native = from_native_result("<unitedValue xmlns=\"http://schemas.mathsoft.com/math50\" xmlns:u=\"http://schemas.mathsoft.com/units10\"><complex><real>2</real><imag>-3</imag></complex><u:unitMonomial><u:unitReference unit=\"meter\" power-numerator=\"1\" power-denominator=\"1\"/></u:unitMonomial></unitedValue>", &style).unwrap();
        assert_eq!(live, native);
        assert_eq!(live.cells[0].plain(), "(200% − 300j%) m");
        assert_eq!(q, before);
        let polar = MathFormat {
            complex_format: ComplexFormat::PolarDegrees,
            ..style
        };
        q.dimension = [0.; 7];
        q.value.re = 0.;
        q.value.im = 1.;
        assert_eq!(
            quantity(&q, &polar).plain(),
            "100% ∠ 9000%°",
            "The original polar formatter passes angles through the same NumberStyle"
        );
    }
}
