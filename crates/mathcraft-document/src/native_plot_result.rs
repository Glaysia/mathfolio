//! Inert finite dimensionless XY samples, never executable engine input.
//! Native Core17f/180/181/182 writes Trace2dResult directly; current23c writes
//! flat JSON arrays despite the older nested-array example in Math50.xsd.
//! Caller owns dimensionlessness, source/status/context proof and resultRef IDs.
use serde::Deserializer;
use serde::de::{SeqAccess, Visitor};
use std::fmt;

const ML: &str = "http://schemas.mathsoft.com/math50";
const MAX_BYTES: usize = 4 * 1024 * 1024;
const MAX_POINTS: usize = 65_536;
const MAX_NODES: usize = 64;

#[derive(Clone, Debug, PartialEq)]
pub struct NativePlotResult {
    pub points: Vec<[f64; 2]>,
    pub range: [f64; 2],
    pub result: [f64; 2],
}

fn extrema(points: &[[f64; 2]], axis: usize) -> [f64; 2] {
    points
        .iter()
        .skip(1)
        .fold([points[0][axis]; 2], |[min, max], point| {
            [
                if point[axis].total_cmp(&min).is_lt() {
                    point[axis]
                } else {
                    min
                },
                if point[axis].total_cmp(&max).is_gt() {
                    point[axis]
                } else {
                    max
                },
            ]
        })
}

/// Return a complete namespace-bound ml:Trace2dResult, not ml:result wrapped.
/// Points must already be dimensionless; no dimensional inference is possible
/// from bare f64 values. Empty data is unsupported, not an invented empty cache.
pub fn finite_unitless_xml(points: &[[f64; 2]]) -> Result<Option<String>, String> {
    if points.is_empty() {
        return Ok(None);
    }
    if points.len() > MAX_POINTS {
        return Err("Native XY result exceeds point limit".into());
    }
    if points.iter().flatten().any(|value| !value.is_finite()) {
        return Err("Native XY result requires finite dimensionless points".into());
    }
    let range = extrema(points, 0);
    let result = extrema(points, 1);
    let x = serde_json::to_string(&points.iter().map(|p| p[0]).collect::<Vec<_>>())
        .map_err(|e| e.to_string())?;
    let y = serde_json::to_string(&points.iter().map(|p| p[1]).collect::<Vec<_>>())
        .map_err(|e| e.to_string())?;
    if x.len().saturating_add(y.len()).saturating_add(2048) > MAX_BYTES {
        return Err("Native XY result exceeds XML size limit".into());
    }
    Ok(Some(format!(
        "<ml:Trace2dResult xmlns:ml=\"{ML}\" TraceType=\"Vector\"><ml:RangeInfo Min=\"{}\" Max=\"{}\"/><ml:ResultInfo Min=\"{}\" Max=\"{}\"/><ml:Data><ml:RangePoints><ml:DataVectors VectorsType=\"Columns\" Count=\"1\" VectorLength=\"{}\" ValuesType=\"Numeric\">{x}</ml:DataVectors></ml:RangePoints><ml:ResultPoints><ml:DataVectors VectorsType=\"Columns\" Count=\"1\" VectorLength=\"{}\" ValuesType=\"Numeric\">{y}</ml:DataVectors></ml:ResultPoints></ml:Data></ml:Trace2dResult>",
        range[0],
        range[1],
        result[0],
        result[1],
        points.len(),
        points.len()
    )))
}

fn named(node: roxmltree::Node<'_, '_>, name: &str) -> bool {
    node.tag_name().namespace() == Some(ML) && node.tag_name().name() == name
}
fn fields(node: roxmltree::Node<'_, '_>, allowed: &[&str]) -> Result<(), String> {
    if node.attributes().len() != allowed.len()
        || node
            .attributes()
            .any(|a| a.namespace().is_some() || !allowed.contains(&a.name()))
    {
        return Err("Unsupported native XY result attributes".into());
    }
    Ok(())
}
fn children<'a, 'i>(
    node: roxmltree::Node<'a, 'i>,
    names: &[&str],
) -> Result<Vec<roxmltree::Node<'a, 'i>>, String> {
    if node
        .children()
        .any(|n| n.is_text() && !n.text().unwrap_or("").trim().is_empty())
    {
        return Err("Unexpected native XY structural text".into());
    }
    let elements: Vec<_> = node.children().filter(|n| n.is_element()).collect();
    if elements.len() != names.len() || elements.iter().zip(names).any(|(n, name)| !named(*n, name))
    {
        return Err("Unsupported native XY child order or structure".into());
    }
    Ok(elements)
}
fn axis_info(node: roxmltree::Node<'_, '_>) -> Result<Option<[f64; 2]>, String> {
    // Unitful axes are a valid but unsupported native presentation, not numeric
    // data to reinterpret silently as unitless.
    if node.children().any(|child| child.is_element()) {
        return Ok(None);
    }
    fields(node, &["Min", "Max"])?;
    children(node, &[])?;
    let parse = |key| {
        node.attribute(key)
            .unwrap()
            .parse::<f64>()
            .map_err(|_| "Invalid native XY axis value".to_string())
    };
    let range = [parse("Min")?, parse("Max")?];
    if range.iter().any(|n| !n.is_finite()) {
        return Ok(None);
    }
    if range[0] > range[1] {
        return Err("Invalid native XY axis interval".into());
    }
    Ok(Some(range))
}
struct FiniteVector {
    expected: usize,
}
impl<'de> Visitor<'de> for FiniteVector {
    type Value = Vec<f64>;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "a flat finite numeric vector of length {}",
            self.expected
        )
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut values = Vec::with_capacity(self.expected);
        while let Some(value) = seq.next_element::<f64>()? {
            if values.len() >= self.expected || !value.is_finite() {
                return Err(serde::de::Error::custom(
                    "Invalid native XY vector length or number",
                ));
            }
            values.push(value);
        }
        if values.len() != self.expected {
            return Err(serde::de::Error::custom("Native XY vector length mismatch"));
        }
        Ok(values)
    }
}
fn vectors(node: roxmltree::Node<'_, '_>) -> Result<Option<Vec<f64>>, String> {
    let nodes = children(node, &["DataVectors"])?;
    let data = nodes[0];
    fields(
        data,
        &["Count", "VectorLength", "VectorsType", "ValuesType"],
    )?;
    let parse = |key| {
        data.attribute(key)
            .unwrap()
            .parse::<usize>()
            .map_err(|_| "Invalid native XY vector count".to_string())
    };
    let count = parse("Count")?;
    let length = parse("VectorLength")?;
    if count == 0 || length == 0 {
        return Err("Native XY vector dimensions must be positive".into());
    }
    if count > MAX_POINTS || length > MAX_POINTS {
        return Err("Native XY result exceeds point limit".into());
    }
    if count != 1
        || data.attribute("VectorsType") != Some("Columns")
        || data.attribute("ValuesType") != Some("Numeric")
    {
        return Ok(None);
    }
    if data.children().any(|n| n.is_element()) {
        return Err("Native XY vector JSON cannot contain XML elements".into());
    }
    let text: String = data
        .children()
        .filter(|n| n.is_text())
        .map(|n| n.text().unwrap_or(""))
        .collect();
    let mut json = serde_json::Deserializer::from_str(&text);
    let values = json
        .deserialize_seq(FiniteVector { expected: length })
        .map_err(|e| e.to_string())?;
    json.end().map_err(|e| e.to_string())?;
    Ok(Some(values))
}

/// Read only the positively established, finite single-vector Columns shape.
/// Unknown trace types, units and multi-vector/string/Rows data return None.
/// Malformed structure/counts/JSON or budget violations return an error. The
/// caller must retain unsupported original source; this does not certify Sync.
pub fn read_result(node: roxmltree::Node<'_, '_>) -> Result<Option<NativePlotResult>, String> {
    if !named(node, "Trace2dResult") {
        return Ok(None);
    }
    if node.range().len() > MAX_BYTES || node.descendants().take(MAX_NODES + 1).count() > MAX_NODES
    {
        return Err("Native XY result exceeds XML size or node limit".into());
    }
    fields(node, &["TraceType"])?;
    if node.attribute("TraceType") != Some("Vector") {
        return Ok(None);
    }
    let nodes = children(node, &["RangeInfo", "ResultInfo", "Data"])?;
    let (Some(range), Some(result)) = (axis_info(nodes[0])?, axis_info(nodes[1])?) else {
        return Ok(None);
    };
    fields(nodes[2], &[])?;
    let data = children(nodes[2], &["RangePoints", "ResultPoints"])?;
    fields(data[0], &[])?;
    fields(data[1], &[])?;
    let (Some(x), Some(y)) = (vectors(data[0])?, vectors(data[1])?) else {
        return Ok(None);
    };
    if x.len() != y.len() {
        return Err("Native XY axes have different sample counts".into());
    }
    if x.iter().any(|x| *x < range[0] || *x > range[1])
        || y.iter().any(|y| *y < result[0] || *y > result[1])
    {
        return Err("Native XY points exceed saved axis bounds".into());
    }
    Ok(Some(NativePlotResult {
        points: x.into_iter().zip(y).map(|(x, y)| [x, y]).collect(),
        range,
        result,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn read(xml: &str) -> Result<Option<NativePlotResult>, String> {
        let doc = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
        read_result(doc.root_element())
    }
    fn fixture() -> String {
        finite_unitless_xml(&[[0., -2.], [1., 3.], [2., 7.]])
            .unwrap()
            .unwrap()
    }
    #[test]
    fn native_xy_single_columns_flat_json_roundtrips_bits_and_bounds() {
        let points = [
            [-0., -2.],
            [f64::from_bits(1), -0.],
            [1.2345678901234567, 3.],
        ];
        let xml = finite_unitless_xml(&points).unwrap().unwrap();
        assert!(xml.starts_with("<ml:Trace2dResult"));
        assert!(!xml.contains("<ml:result"));
        assert!(xml.contains("Count=\"1\""));
        assert!(xml.contains("ValuesType=\"Numeric\""));
        assert!(!xml.contains("[["));
        let actual = read(&xml).unwrap().unwrap();
        for (old, new) in points.iter().flatten().zip(actual.points.iter().flatten()) {
            assert_eq!(old.to_bits(), new.to_bits());
        }
        assert_eq!(actual.range, [-0., points[2][0]]);
        assert_eq!(actual.result, [-2., 3.]);
    }
    #[test]
    fn native_xy_literal_schema_shaped_golden_is_independent_of_writer() {
        let xml = r#"<ml:Trace2dResult xmlns:ml="http://schemas.mathsoft.com/math50" TraceType="Vector"><ml:RangeInfo Min="-1" Max="1"/><ml:ResultInfo Min="0" Max="2"/><ml:Data><ml:RangePoints><ml:DataVectors Count="1" VectorLength="3" ValuesType="Numeric" VectorsType="Columns">[-1,0,1]</ml:DataVectors></ml:RangePoints><ml:ResultPoints><ml:DataVectors Count="1" VectorLength="3" ValuesType="Numeric" VectorsType="Columns">[2,0,2]</ml:DataVectors></ml:ResultPoints></ml:Data></ml:Trace2dResult>"#;
        let value = read(xml).unwrap().unwrap();
        assert_eq!(value.points, [[-1., 2.], [0., 0.], [1., 2.]]);
        assert!(read(&format!("<result>{xml}</result>")).unwrap().is_none());
    }
    #[test]
    fn native_xy_unsupported_dimensions_types_and_units_are_not_reinterpreted() {
        for (from, to) in [
            ("TraceType=\"Vector\"", "TraceType=\"Parametric\""),
            ("VectorsType=\"Columns\"", "VectorsType=\"Rows\""),
            ("ValuesType=\"Numeric\"", "ValuesType=\"String\""),
            ("Count=\"1\"", "Count=\"2\""),
        ] {
            assert!(read(&fixture().replace(from, to)).unwrap().is_none());
        }
        let units=fixture().replace("<ml:RangeInfo Min=\"0\" Max=\"2\"/>","<ml:RangeInfo Min=\"0\" Max=\"2\"><u:unitMonomial xmlns:u=\"http://schemas.mathsoft.com/units10\"/></ml:RangeInfo>");
        assert!(read(&units).unwrap().is_none());
    }
    #[test]
    fn native_xy_bad_lengths_nested_json_extra_fields_and_nonfinite_fail() {
        for (from, to) in [
            ("[0.0,1.0,2.0]", "[[0,1,2]]"),
            ("[0.0,1.0,2.0]", "[0,1]"),
            ("[0.0,1.0,2.0]", "[0,1,2,3]"),
            ("[0.0,1.0,2.0]", "[0,1,null]"),
            ("[0.0,1.0,2.0]", "[0,1,NaN]"),
            ("Count=\"1\"", "Count=\"0\""),
            ("VectorLength=\"3\"", "VectorLength=\"65537\""),
            ("TraceType=\"Vector\"", "TraceType=\"Vector\" future=\"x\""),
        ] {
            assert!(read(&fixture().replace(from, to)).is_err(), "{from}→{to}");
        }
        assert!(finite_unitless_xml(&[[0., f64::NAN]]).is_err());
        assert!(finite_unitless_xml(&[[f64::INFINITY, 0.]]).is_err());
        assert_eq!(finite_unitless_xml(&[]).unwrap(), None);
        assert!(finite_unitless_xml(&vec![[0., 0.]; MAX_POINTS + 1]).is_err());
    }
    #[test]
    fn native_xy_axis_interval_sample_bounds_and_duplicate_children_are_guarded() {
        assert!(read(&fixture().replace("Min=\"0\" Max=\"2\"", "Min=\"2\" Max=\"0\"")).is_err());
        assert!(read(&fixture().replace("Min=\"0\" Max=\"2\"", "Min=\"0\" Max=\"1\"")).is_err());
        assert!(
            read(&fixture().replace("<ml:Data>", "<ml:RangeInfo Min=\"0\" Max=\"2\"/><ml:Data>"))
                .is_err()
        );
        let wider = read(&fixture().replace("Min=\"0\" Max=\"2\"", "Min=\"-1\" Max=\"4\""))
            .unwrap()
            .unwrap();
        assert_eq!(wider.range, [-1., 4.]);
    }
    #[test]
    #[ignore = "Writes bounded generated native XY result XML for offline XSD validation; no target runtime"]
    fn native_xy_result_schema_fixture() {
        let output = std::env::var_os("MATHCRAFT_NATIVE_XY_RESULT_OUTPUT")
            .map(std::path::PathBuf::from)
            .expect("MATHCRAFT_NATIVE_XY_RESULT_OUTPUT");
        std::fs::create_dir_all(&output).unwrap();
        for (name, points) in [
            ("ordinary", vec![[-1., 2.], [0., 0.], [1., 2.]]),
            (
                "float-bits",
                vec![
                    [-0., -0.],
                    [f64::from_bits(1), 1.2345678901234567],
                    [1., f64::MIN_POSITIVE],
                ],
            ),
            ("constant", vec![[1., 2.], [1., 2.]]),
        ] {
            let xml = finite_unitless_xml(&points).unwrap().unwrap();
            let actual = read(&xml).unwrap().unwrap();
            assert!(
                actual
                    .points
                    .iter()
                    .flatten()
                    .zip(points.iter().flatten())
                    .all(|(a, b)| a.to_bits() == b.to_bits())
            );
            let result = format!(
                "<r:resultsList xmlns:r=\"http://schemas.mathsoft.com/result10\"><r:resultData result-id=\"17\" calculation-status=\"Synchronized\">{xml}</r:resultData></r:resultsList>"
            );
            std::fs::write(output.join(format!("{name}.xml")), result).unwrap();
        }
        println!(
            "NATIVE_XY_RESULT_FIXTURE generated=3 directTraceRoot=true flatJson=true finiteColumnPair=true"
        );
    }
}
