//! Inert, retained flat-matrix authority with bounded window presentation.
//! New profile only: the legacy v1 preview parser and wire are unchanged.
use super::*;
use std::sync::Arc;
const ML: &str = "http://schemas.mathsoft.com/math50";
const UNITS: &str = "http://schemas.mathsoft.com/units10";
const MAX_SOURCE_BYTES: usize = 1024 * 1024;
const MAX_SOURCE_TEXT: usize = 256 * 1024;
const MAX_SOURCE_NODES: u32 = 262_144;
const MAX_CELLS: usize = 65_536;
const MAX_SOURCE_DEPTH: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MatrixWindow {
    pub row_offset: usize,
    pub col_offset: usize,
    pub rows: usize,
    pub cols: usize,
}
impl MatrixWindow {
    fn validate(self) -> Result<(), String> {
        if self.rows == 0
            || self.cols == 0
            || self.rows > MAX_CELLS
            || self.cols > MAX_CELLS
            || self
                .rows
                .checked_mul(self.cols)
                .is_none_or(|n| n > MAX_CELLS)
        {
            return Err("Matrix window exceeds bounded dimensions".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MatrixProjection {
    pub full_rows: usize,
    pub full_cols: usize,
    pub row_offset: usize,
    pub col_offset: usize,
    pub rows: usize,
    pub cols: usize,
    pub cells: Vec<DisplayMath>,
    pub hidden_above: bool,
    pub hidden_below: bool,
    pub hidden_left: bool,
    pub hidden_right: bool,
}
// Native cells are deliberately not compat evaluator Values. Keep complete raw
// carrier too; this validated index is not a replacement serialization authority.
#[derive(Clone, Debug)]
enum Number {
    Real(f64),
    Complex(f64, f64),
}
#[derive(Clone, Debug)]
struct UnitReference {
    name: String,
    numerator: i32,
    denominator: u32,
}
#[derive(Clone, Debug)]
struct Units {
    _system: Option<String>,
    references: Vec<UnitReference>,
}
#[derive(Clone, Debug)]
enum NativeCell {
    Number(Number),
    String(String),
    United(Number, Units),
}
#[derive(Clone, Debug)]
pub(crate) struct PreparedNativeMatrix {
    // Retain the exact carrier even though projection reads the validated index.
    _raw: Arc<str>,
    rows: usize,
    cols: usize,
    cells: Arc<[NativeCell]>, // native column-major, all cells validated
}
#[derive(Clone, Debug)]
pub(crate) struct PreparedValueMatrix {
    value: Arc<Value>,
    rows: usize,
    cols: usize,
}

fn attrs(node: roxmltree::Node<'_, '_>, allowed: &[&str]) -> Result<(), String> {
    if node
        .attributes()
        .any(|a| a.namespace().is_some() || !allowed.contains(&a.name()))
    {
        return Err("Unsupported native matrix metadata".into());
    }
    Ok(())
}
fn children<'a, 'i>(node: roxmltree::Node<'a, 'i>) -> Result<Vec<roxmltree::Node<'a, 'i>>, String> {
    if node
        .children()
        .any(|n| n.is_text() && !n.text().unwrap_or_default().trim().is_empty())
    {
        return Err("Unexpected native matrix structural text".into());
    }
    Ok(node
        .children()
        .filter(roxmltree::Node::is_element)
        .collect())
}
fn shape(rows: usize, cols: usize, count: usize) -> Result<(), String> {
    if rows == 0
        || cols == 0
        || rows > MAX_CELLS
        || cols > MAX_CELLS
        || rows.checked_mul(cols) != Some(count)
        || count > MAX_CELLS
    {
        return Err("Invalid or over-budget matrix shape".into());
    }
    Ok(())
}
fn number(node: roxmltree::Node<'_, '_>) -> Result<Number, String> {
    if node.has_tag_name((ML, "real")) {
        return numeric_literal::coefficient(node, false)
            .map(Number::Real)
            .ok_or_else(|| "Unsupported native matrix real coefficient".into());
    }
    if node.has_tag_name((ML, "complex")) {
        attrs(node, &[])?;
        let parts = children(node)?;
        if parts.len() == 2
            && parts[0].has_tag_name((ML, "real"))
            && parts[1].has_tag_name((ML, "imag"))
        {
            let re = numeric_literal::coefficient(parts[0], false)
                .ok_or("Unsupported native matrix real coefficient")?;
            let im = numeric_literal::coefficient(parts[1], true)
                .ok_or("Unsupported native matrix imaginary coefficient")?;
            return Ok(Number::Complex(re, im));
        }
    }
    Err("Unsupported native matrix numeric cell".into())
}
fn units(node: roxmltree::Node<'_, '_>) -> Result<Units, String> {
    if !node.has_tag_name((UNITS, "unitMonomial")) {
        return Err("Unsupported native matrix units".into());
    }
    attrs(node, &["system"])?;
    let system = node.attribute("system").map(str::to_owned);
    if system.as_ref().is_some_and(|s| s.len() > 1024) {
        return Err("Native matrix unit system exceeds limit".into());
    }
    let nodes = children(node)?;
    if nodes.len() > 64 {
        return Err("Native matrix unit references exceed limit".into());
    }
    let mut references = Vec::with_capacity(nodes.len());
    for unit in nodes {
        if !unit.has_tag_name((UNITS, "unitReference")) {
            return Err("Unsupported native matrix unit reference".into());
        }
        attrs(unit, &["unit", "power-numerator", "power-denominator"])?;
        if !children(unit)?.is_empty() {
            return Err("Unsupported native matrix unit content".into());
        }
        let name = unit
            .attribute("unit")
            .ok_or("Missing native matrix unit name")?;
        let numerator: i32 = unit
            .attribute("power-numerator")
            .ok_or("Missing unit power")?
            .parse()
            .map_err(|_| "Invalid unit power")?;
        let denominator: u32 = unit
            .attribute("power-denominator")
            .ok_or("Missing unit power")?
            .parse()
            .map_err(|_| "Invalid unit power")?;
        if name.is_empty() || name.len() > 256 || numerator == i32::MIN || denominator == 0 {
            return Err("Invalid native matrix unit reference".into());
        }
        references.push(UnitReference {
            name: name.into(),
            numerator,
            denominator,
        });
    }
    Ok(Units {
        _system: system,
        references,
    })
}
fn cell(node: roxmltree::Node<'_, '_>) -> Result<NativeCell, String> {
    if node.has_tag_name((ML, "str")) {
        if node.attributes().any(|a| {
            a.namespace() != Some("http://www.w3.org/XML/1998/namespace")
                || a.name() != "space"
                || a.value() != "preserve"
        }) || node.children().any(|n| !n.is_text())
        {
            return Err("Unsupported native matrix string metadata".into());
        }
        return Ok(NativeCell::String(
            node.children().filter_map(|n| n.text()).collect(),
        ));
    }
    if node.has_tag_name((ML, "unitedValue")) {
        attrs(node, &[])?;
        let parts = children(node)?;
        if parts.len() != 2 {
            return Err("Invalid native matrix united value".into());
        }
        return Ok(NativeCell::United(number(parts[0])?, units(parts[1])?));
    }
    number(node).map(NativeCell::Number)
}
impl PreparedNativeMatrix {
    pub(crate) fn parse(xml: &str) -> Result<Self, String> {
        if xml.len() > MAX_SOURCE_BYTES {
            return Err("Native matrix source exceeds byte limit".into());
        }
        let doc = roxmltree::Document::parse_with_options(
            xml,
            roxmltree::ParsingOptions {
                allow_dtd: false,
                nodes_limit: MAX_SOURCE_NODES,
                ..Default::default()
            },
        )
        .map_err(|e| e.to_string())?;
        let mut text = 0usize;
        for n in doc.descendants() {
            if n.is_comment() || n.is_pi() || n.ancestors().count() > MAX_SOURCE_DEPTH {
                return Err("Unsupported native matrix XML structure".into());
            }
            if n.is_text() {
                text = text
                    .checked_add(n.text().unwrap_or_default().len())
                    .ok_or("Native matrix text overflow")?;
            }
            if text > MAX_SOURCE_TEXT {
                return Err("Native matrix source text exceeds limit".into());
            }
        }
        let mut matrix = doc.root_element();
        if matrix.has_tag_name((ML, "result")) {
            attrs(matrix, &[])?;
            let parts = children(matrix)?;
            if parts.len() != 1 {
                return Err("Invalid native matrix result wrapper".into());
            }
            matrix = parts[0];
        }
        if !matrix.has_tag_name((ML, "matrix")) {
            return Err("Native result is not a supported flat matrix".into());
        }
        attrs(matrix, &["rows", "cols"])?;
        let rows = matrix
            .attribute("rows")
            .ok_or("Missing matrix rows")?
            .parse()
            .map_err(|_| "Invalid matrix rows")?;
        let cols = matrix
            .attribute("cols")
            .ok_or("Missing matrix columns")?
            .parse()
            .map_err(|_| "Invalid matrix columns")?;
        let nodes = children(matrix)?;
        shape(rows, cols, nodes.len())?;
        let cells = nodes.into_iter().map(cell).collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            _raw: Arc::from(xml),
            rows,
            cols,
            cells: cells.into(),
        })
    }
    #[cfg(test)]
    pub(crate) fn raw_xml(&self) -> &str {
        &self._raw
    }
    pub(crate) fn shape(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
    pub(crate) fn project(
        &self,
        window: MatrixWindow,
        style: &MathFormat,
    ) -> Result<MatrixProjection, String> {
        project(self.rows, self.cols, window, |r, c| {
            render(&self.cells[c * self.rows + r], style)
        })
    }
}
impl PreparedValueMatrix {
    pub(crate) fn new(value: Arc<Value>) -> Result<Self, String> {
        let Value::Matrix {
            rows,
            cols,
            elements,
        } = value.as_ref()
        else {
            return Err("Value is not a matrix".into());
        };
        shape(*rows, *cols, elements.len())?;
        if elements.iter().any(|q| {
            !q.value.re.is_finite()
                || !q.value.im.is_finite()
                || q.dimension.iter().any(|d| !d.is_finite())
        }) {
            return Err("Matrix value contains non-finite quantities".into());
        }
        let (rows, cols) = (*rows, *cols);
        Ok(Self { value, rows, cols })
    }
    #[cfg(test)]
    pub(crate) fn value(&self) -> &Arc<Value> {
        &self.value
    }
    pub(crate) fn shape(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
    pub(crate) fn project(
        &self,
        window: MatrixWindow,
        style: &MathFormat,
    ) -> Result<MatrixProjection, String> {
        let Value::Matrix { elements, .. } = self.value.as_ref() else {
            unreachable!("validated matrix")
        };
        project(self.rows, self.cols, window, |r, c| {
            quantity(&elements[r * self.cols + c], style)
        })
    }
}
fn number_display(value: &Number, style: &MathFormat) -> DisplayMath {
    match value {
        Number::Real(n) => real(*n, style),
        Number::Complex(re, im) => {
            let mut q = Quantity::scalar(*re);
            q.value.im = *im;
            quantity(&q, style)
        }
    }
}
fn unit_display(units: &Units) -> DisplayMath {
    let mut top = Vec::new();
    let mut bottom = Vec::new();
    for unit in &units.references {
        if unit.numerator == 0 {
            continue;
        }
        let name = match unit.name.as_str() {
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
        let power = unit.numerator.unsigned_abs();
        let factor = if power == 1 && unit.denominator == 1 {
            base
        } else {
            let exponent = if unit.denominator == 1 {
                DisplayMath::text(power.to_string())
            } else {
                DisplayMath::Fraction(
                    Box::new(DisplayMath::text(power.to_string())),
                    Box::new(DisplayMath::text(unit.denominator.to_string())),
                )
            };
            DisplayMath::Power(Box::new(base), Box::new(exponent))
        };
        if unit.numerator > 0 {
            top.push(factor)
        } else {
            bottom.push(factor)
        }
    }
    if bottom.is_empty() {
        product(top)
    } else {
        DisplayMath::Fraction(Box::new(product(top)), Box::new(product(bottom)))
    }
}
fn render(cell: &NativeCell, style: &MathFormat) -> DisplayMath {
    match cell {
        NativeCell::Number(n) => number_display(n, style),
        NativeCell::String(s) => string_literal(s),
        NativeCell::United(n, u) => {
            let n = number_display(n, style);
            let n = if needs_numeric_group(&n) {
                DisplayMath::Group(Box::new(n))
            } else {
                n
            };
            DisplayMath::Row(vec![n, DisplayMath::text(" "), unit_display(u)])
        }
    }
}
fn presentation_budget(root: &DisplayMath, budget: &mut DisplayBudget) -> Result<(), String> {
    let mut pending = vec![(root, 0)];
    while let Some((node, depth)) = pending.pop() {
        let text = if let DisplayMath::Text(s, _) = node {
            s.len()
        } else {
            0
        };
        budget
            .take(depth, text)
            .ok_or("Matrix window presentation exceeds display budget")?;
        match node {
            DisplayMath::Text(..) => {}
            DisplayMath::Row(parts) => pending.extend(parts.iter().map(|p| (p, depth + 1))),
            DisplayMath::Power(a, b)
            | DisplayMath::Subscript(a, b)
            | DisplayMath::Fraction(a, b) => {
                pending.push((a, depth + 1));
                pending.push((b, depth + 1));
            }
            DisplayMath::Group(a)
            | DisplayMath::Root(a)
            | DisplayMath::Overline(a)
            | DisplayMath::OverArrow(a)
            | DisplayMath::Bars(a, _)
            | DisplayMath::AngleFence(a, _) => pending.push((a, depth + 1)),
        }
    }
    Ok(())
}
fn project(
    rows: usize,
    cols: usize,
    requested: MatrixWindow,
    mut format: impl FnMut(usize, usize) -> DisplayMath,
) -> Result<MatrixProjection, String> {
    requested.validate()?;
    // Original GetFormatAdjusted preserves requested size at the lower/right
    // boundary by moving the offset back; shrink only to full matrix shape.
    let nr = requested.rows.min(rows);
    let nc = requested.cols.min(cols);
    let row_offset = requested.row_offset.min(rows - nr);
    let col_offset = requested.col_offset.min(cols - nc);
    let mut cells = Vec::with_capacity(nr * nc);
    let mut budget = DisplayBudget::default();
    for r in row_offset..row_offset + nr {
        for c in col_offset..col_offset + nc {
            #[cfg(test)]
            record_matrix_cell();
            let cell = format(r, c);
            presentation_budget(&cell, &mut budget)?;
            cells.push(cell);
        }
    }
    Ok(MatrixProjection {
        full_rows: rows,
        full_cols: cols,
        row_offset,
        col_offset,
        rows: nr,
        cols: nc,
        cells,
        hidden_above: row_offset > 0,
        hidden_below: row_offset + nr < rows,
        hidden_left: col_offset > 0,
        hidden_right: col_offset + nc < cols,
    })
}
#[cfg(test)]
thread_local! { static FORMATTED_CELLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
#[cfg(test)]
pub(super) fn record_matrix_cell() {
    FORMATTED_CELLS.with(|n| n.set(n.get() + 1));
}
#[cfg(test)]
pub(crate) fn take_formatted_matrix_cells() -> usize {
    FORMATTED_CELLS.with(|n| n.replace(0))
}
#[cfg(test)]
mod tests;
