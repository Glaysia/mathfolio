//! Inert native result literals, never worksheet expressions or evaluator input.
//! Original ResultSerializer BB6 wraps numeric values in ml:result; Calculation
//! Core 16f/170 writes round-trip doubles and 171 serializes matrix columns first.
//! Caller owns source/signature/status checks and resultRef/result-id allocation.
use crate::canvas::SavedNumericSource;
use mathfolio_compat::Value;
use mathfolio_core::Quantity;

pub mod program;

const ML: &str = "http://schemas.mathsoft.com/math50";
const MAX_BYTES: usize = 4 * 1024 * 1024;
const MAX_CELLS: usize = 65_536;
const MAX_JSON_NODES: usize = 1_500_000;
const MAX_XML_NODES: usize = 250_000;
const MAX_JSON_DEPTH: usize = 16;

fn check_json(value: &serde_json::Value) -> Result<(), String> {
    let mut pending = vec![(value, 0_usize)];
    let mut count = 0;
    while let Some((node, depth)) = pending.pop() {
        count += 1;
        if count > MAX_JSON_NODES || depth > MAX_JSON_DEPTH {
            return Err("Native numeric result JSON exceeds structural limits".into());
        }
        match node {
            serde_json::Value::Array(children) => {
                pending.extend(children.iter().map(|child| (child, depth + 1)));
            }
            serde_json::Value::Object(children) => {
                pending.extend(children.values().map(|child| (child, depth + 1)));
            }
            _ => {}
        }
    }
    Ok(())
}

fn exact_fields(value: &serde_json::Value, allowed: &[&str]) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or("Numeric result must be an object")?;
    if object.len() != allowed.len() || object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err("Unsupported numeric result fields".into());
    }
    Ok(())
}

fn check_quantity_json(value: &serde_json::Value) -> Result<(), String> {
    exact_fields(value, &["value", "dimension"])
}

fn dimensionless(quantity: &Quantity) -> Result<bool, String> {
    if !quantity.value.re.is_finite()
        || !quantity.value.im.is_finite()
        || !quantity.dimension.iter().all(|value| value.is_finite())
    {
        return Err("Native numeric result must contain finite values".into());
    }
    Ok(quantity.dimension.iter().all(|value| *value == 0.))
}

struct LiteralWriter {
    xml: String,
    nodes: usize,
}

impl LiteralWriter {
    fn string(&mut self, value: &str) -> Result<(), String> {
        if !value.chars().all(|c| {
            matches!(c, '\t' | '\n' | '\r') || c >= ' ' && !matches!(c, '\u{fffe}' | '\u{ffff}')
        }) {
            return Err("Native string result contains invalid XML text".into());
        }
        self.nodes(1)?;
        // Original Visit(IStringResultNode) writes str and WriteString(value),
        // without the source editor's xml:space attribute or visual quotes.
        self.push("<ml:str>")?;
        let escaped = value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('\r', "&#13;");
        self.push(&escaped)?;
        self.push("</ml:str>")
    }
    fn push(&mut self, text: &str) -> Result<(), String> {
        if self.xml.len().saturating_add(text.len()) > MAX_BYTES {
            return Err("Native numeric result XML exceeds size limit".into());
        }
        self.xml.push_str(text);
        Ok(())
    }

    fn nodes(&mut self, count: usize) -> Result<(), String> {
        self.nodes = self.nodes.saturating_add(count);
        if self.nodes > MAX_XML_NODES {
            return Err("Native numeric result XML exceeds node limit".into());
        }
        Ok(())
    }

    fn quantity(&mut self, quantity: &Quantity) -> Result<(), String> {
        if quantity.value.im == 0. {
            self.nodes(1)?;
            self.push("<ml:real>")?;
            self.push(&quantity.value.re.to_string())?;
            self.push("</ml:real>")
        } else {
            self.nodes(3)?;
            self.push("<ml:complex><ml:real>")?;
            self.push(&quantity.value.re.to_string())?;
            self.push("</ml:real><ml:imag>")?;
            self.push(&quantity.value.im.to_string())?;
            self.push("</ml:imag></ml:complex>")
        }
    }
}

/// Serialize precision-preserving *display* values into a numeric result root.
/// Scalar/complex and rectangular matrices must be finite and exactly unitless.
/// None means a known unsupported carrier/value (units, ranges, symbolic or raw
/// native MathML); malformed or over-budget evaluated data returns an error.
/// No application code, function call, symbol, unit or saved MathML is executed.
pub fn numeric_result_xml(source: &SavedNumericSource) -> Result<Option<String>, String> {
    let SavedNumericSource::EvaluatedValue(source) = source else {
        return Ok(None);
    };
    if source.len() > MAX_BYTES {
        return Err("Native numeric result JSON exceeds size limit".into());
    }
    let json: serde_json::Value =
        serde_json::from_str(source).map_err(|error| error.to_string())?;
    check_json(&json)?;
    match json.get("kind").and_then(serde_json::Value::as_str) {
        Some("string") => exact_fields(&json, &["kind", "value"])?,
        Some("scalar") => {
            exact_fields(&json, &["kind", "quantity"])?;
            check_quantity_json(&json["quantity"])?;
        }
        Some("matrix") => {
            exact_fields(&json, &["kind", "rows", "cols", "elements"])?;
            for raw in json["elements"]
                .as_array()
                .ok_or("Invalid matrix elements")?
            {
                check_quantity_json(raw)?;
            }
        }
        _ => {}
    }
    let value: Value = serde_json::from_value(json).map_err(|error| error.to_string())?;
    match &value {
        Value::String { .. } => {}
        Value::Scalar { quantity } => {
            if !dimensionless(quantity)? {
                return Ok(None);
            }
        }
        Value::Matrix {
            rows,
            cols,
            elements,
        } => {
            if *rows == 0 || *cols == 0 || rows.checked_mul(*cols) != Some(elements.len()) {
                return Err("Invalid numeric result matrix dimensions".into());
            }
            if elements.len() > MAX_CELLS {
                return Err("Native numeric result matrix exceeds cell limit".into());
            }
            for quantity in elements {
                if !dimensionless(quantity)? {
                    return Ok(None);
                }
            }
        }
        Value::Range { .. } | Value::RangeValues { .. } | Value::Symbolic { .. } => {
            return Ok(None);
        }
    }
    let mut output = LiteralWriter {
        xml: String::new(),
        nodes: 1,
    };
    output.push(&format!("<ml:result xmlns:ml=\"{ML}\">"))?;
    match &value {
        Value::String { value } => output.string(value)?,
        Value::Scalar { quantity } => output.quantity(quantity)?,
        Value::Matrix {
            rows,
            cols,
            elements,
        } => {
            output.nodes(1)?;
            output.push(&format!("<ml:matrix rows=\"{rows}\" cols=\"{cols}\">"))?;
            // Internal values are row-major. Original WriteMatrixXml 0171
            // enumerates column first, then row, calling get_Item(row,column).
            for column in 0..*cols {
                for row in 0..*rows {
                    output.quantity(&elements[row * cols + column])?;
                }
            }
            output.push("</ml:matrix>")?;
        }
        _ => unreachable!("unsupported values returned above"),
    }
    output.push("</ml:result>")?;
    Ok(Some(output.xml))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cached(value: Value) -> SavedNumericSource {
        SavedNumericSource::EvaluatedValue(serde_json::to_string(&value).unwrap())
    }

    fn scalar(value: f64) -> SavedNumericSource {
        cached(Value::Scalar {
            quantity: Quantity::scalar(value),
        })
    }

    #[test]
    fn finite_real_values_keep_exact_roundtrip_precision_and_signed_zero() {
        for value in [
            1.2345678901234567,
            -0.,
            f64::MAX,
            f64::MIN_POSITIVE,
            f64::from_bits(1),
        ] {
            let xml = numeric_result_xml(&scalar(value)).unwrap().unwrap();
            let doc = roxmltree::Document::parse(&xml).unwrap();
            assert!(doc.root_element().has_tag_name((ML, "result")));
            let real = doc.root_element().first_element_child().unwrap();
            assert!(real.has_tag_name((ML, "real")));
            assert_eq!(
                real.text().unwrap().parse::<f64>().unwrap().to_bits(),
                value.to_bits()
            );
        }
    }

    #[test]
    fn complex_result_has_original_ordered_real_and_imaginary_literals() {
        let mut quantity = Quantity::scalar(-1.2345678901234567);
        quantity.value.im = 9.87654321098765;
        let xml = numeric_result_xml(&cached(Value::Scalar { quantity }))
            .unwrap()
            .unwrap();
        let doc = roxmltree::Document::parse(&xml).unwrap();
        let complex = doc.root_element().first_element_child().unwrap();
        assert!(complex.has_tag_name((ML, "complex")));
        let parts = complex
            .children()
            .filter(Node::is_element)
            .collect::<Vec<_>>();
        assert!(parts[0].has_tag_name((ML, "real")) && parts[1].has_tag_name((ML, "imag")));
        assert_eq!(
            parts[0].text().unwrap().parse::<f64>().unwrap().to_bits(),
            quantity.value.re.to_bits()
        );
        assert_eq!(
            parts[1].text().unwrap().parse::<f64>().unwrap().to_bits(),
            quantity.value.im.to_bits()
        );
    }

    use roxmltree::Node;

    #[test]
    fn non_square_matrix_transposes_row_major_input_to_native_column_major_xml() {
        let value = Value::Matrix {
            rows: 2,
            cols: 3,
            elements: (1..=6)
                .map(|value| Quantity::scalar(f64::from(value)))
                .collect(),
        };
        let xml = numeric_result_xml(&cached(value)).unwrap().unwrap();
        let doc = roxmltree::Document::parse(&xml).unwrap();
        let matrix = doc.root_element().first_element_child().unwrap();
        assert!(matrix.has_tag_name((ML, "matrix")));
        assert_eq!(matrix.attribute("rows"), Some("2"));
        assert_eq!(matrix.attribute("cols"), Some("3"));
        let values = matrix
            .children()
            .filter(Node::is_element)
            .map(|n| n.text().unwrap().parse::<u32>().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(values, [1, 4, 2, 5, 3, 6]);
    }

    #[test]
    fn unsupported_units_ranges_symbolic_and_raw_mathml_are_not_promoted_to_numeric_results() {
        let mut unit = Quantity::scalar(2.);
        unit.dimension[0] = 1.;
        for source in [
            cached(Value::Scalar { quantity: unit }),
            cached(Value::Matrix {
                rows: 1,
                cols: 1,
                elements: vec![unit],
            }),
            cached(Value::RangeValues {
                elements: vec![Quantity::scalar(1.)],
            }),
            cached(Value::Range {
                start: Quantity::scalar(1.),
                step: Quantity::scalar(1.),
                end: Quantity::scalar(2.),
                count: 2,
            }),
            cached(Value::Symbolic {
                mathml: "<ml:apply><ml:id>read</ml:id></ml:apply>".into(),
            }),
            SavedNumericSource::NativeMathml("<ml:apply><ml:id>read</ml:id></ml:apply>".into()),
            SavedNumericSource::AreaPresentation("{}".into()),
        ] {
            assert_eq!(numeric_result_xml(&source).unwrap(), None);
        }
    }

    #[test]
    fn malformed_unknown_metadata_and_invalid_matrix_dimensions_are_errors() {
        let valid = match scalar(1.) {
            SavedNumericSource::EvaluatedValue(json) => json,
            _ => unreachable!(),
        };
        let mut extra: serde_json::Value = serde_json::from_str(&valid).unwrap();
        extra["quantity"]["future"] = true.into();
        for raw in [
            "not JSON".into(),
            "null".into(),
            "{\"kind\":\"alien\"}".into(),
            serde_json::to_string(&extra).unwrap(),
            valid.replace("1.0", "1e999"),
        ] {
            assert!(numeric_result_xml(&SavedNumericSource::EvaluatedValue(raw)).is_err());
        }
        for (rows, cols, len) in [(0, 1, 0), (2, 3, 5), (usize::MAX, 2, 0)] {
            assert!(
                numeric_result_xml(&cached(Value::Matrix {
                    rows,
                    cols,
                    elements: vec![Quantity::scalar(1.); len]
                }))
                .is_err()
            );
        }
    }

    #[test]
    fn byte_depth_and_cell_budgets_reject_atomically() {
        assert!(
            numeric_result_xml(&SavedNumericSource::EvaluatedValue(
                " ".repeat(MAX_BYTES + 1)
            ))
            .is_err()
        );
        let deep = format!(
            "{}0{}",
            "[".repeat(MAX_JSON_DEPTH + 1),
            "]".repeat(MAX_JSON_DEPTH + 1)
        );
        assert!(numeric_result_xml(&SavedNumericSource::EvaluatedValue(deep)).is_err());
        assert!(
            numeric_result_xml(&cached(Value::Matrix {
                rows: 1,
                cols: MAX_CELLS + 1,
                elements: vec![Quantity::scalar(1.); MAX_CELLS + 1]
            }))
            .is_err()
        );
        let source = scalar(7.);
        let before = source.clone();
        assert!(numeric_result_xml(&source).unwrap().is_some());
        assert_eq!(source, before);
    }
}
#[cfg(test)]
#[path = "native_result/string_tests.rs"]
mod string_tests;
