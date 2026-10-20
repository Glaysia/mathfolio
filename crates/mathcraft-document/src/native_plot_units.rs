//! Inert residual unit metadata for finite native XY results (0170).
//! These units are not authored scaling expressions or an automatic SI policy.
use crate::native_plot_result::{self, NativePlotResult};
use serde::{Deserialize, Serialize};

const ML: &str = "http://schemas.mathsoft.com/math50";
const U: &str = "http://schemas.mathsoft.com/units10";
const MAX_BYTES: usize = 4 * 1024 * 1024;
const MAX_NODES: usize = 384;
const MAX_REFERENCES: usize = 32;
const MAX_LABEL_BYTES: usize = 1024;
const MAX_POWER: i32 = 1_000_000;
pub const MAX_NATIVE_UNIT_METADATA_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "UnitReferenceWire")]
pub struct NativeUnitReference {
    pub unit: String,
    pub power_numerator: Option<i32>,
    pub power_denominator: Option<i32>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "UnitMonomialWire")]
pub struct NativeUnitMonomial {
    pub system: Option<String>,
    pub references: Vec<NativeUnitReference>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UnitReferenceWire {
    unit: String,
    power_numerator: Option<i32>,
    power_denominator: Option<i32>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UnitMonomialWire {
    system: Option<String>,
    #[serde(deserialize_with = "bounded_references")]
    references: Vec<NativeUnitReference>,
}
fn bounded_references<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Vec<NativeUnitReference>, D::Error> {
    struct References;
    impl<'de> serde::de::Visitor<'de> for References {
        type Value = Vec<NativeUnitReference>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("at most 32 native unit references")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> Result<Self::Value, A::Error> {
            let mut out = Vec::new();
            while let Some(value) = seq.next_element()? {
                if out.len() == MAX_REFERENCES {
                    return Err(serde::de::Error::custom("Native unit reference budget"));
                }
                out.push(value);
            }
            Ok(out)
        }
    }
    d.deserialize_seq(References)
}
impl TryFrom<UnitReferenceWire> for NativeUnitReference {
    type Error = String;
    fn try_from(w: UnitReferenceWire) -> Result<Self, Self::Error> {
        let value = Self {
            unit: w.unit,
            power_numerator: w.power_numerator,
            power_denominator: w.power_denominator,
        };
        value.validate()?;
        Ok(value)
    }
}
impl TryFrom<UnitMonomialWire> for NativeUnitMonomial {
    type Error = String;
    fn try_from(w: UnitMonomialWire) -> Result<Self, Self::Error> {
        let value = Self {
            system: w.system,
            references: w.references,
        };
        value.validate()?;
        Ok(value)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct NativePlotUnitResult {
    pub curve: NativePlotResult,
    pub range_units: Option<NativeUnitMonomial>,
    pub result_units: Option<NativeUnitMonomial>,
}

fn legal(text: &str) -> bool {
    text.len() <= MAX_LABEL_BYTES
        && text.chars().all(|c| matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}'))
}
impl NativeUnitReference {
    pub fn validate(&self) -> Result<(), String> {
        let reference = self;
        if reference.unit.is_empty() || !legal(&reference.unit) {
            return Err("Invalid or oversized native unit name".into());
        }
        for value in [reference.power_numerator, reference.power_denominator]
            .into_iter()
            .flatten()
        {
            if !(-MAX_POWER..=MAX_POWER).contains(&value) {
                return Err("Native unit power exceeds supported integer range".into());
            }
        }
        if reference.power_denominator == Some(0) {
            return Err("Native unit denominator cannot be zero".into());
        }
        Ok(())
    }
}
impl NativeUnitMonomial {
    pub fn validate(&self) -> Result<(), String> {
        let units = self;
        if units.references.is_empty() || units.references.len() > MAX_REFERENCES {
            return Err("Native unit monomial requires 1–32 references".into());
        }
        if units.system.as_ref().is_some_and(|s| !legal(s)) {
            return Err("Invalid or oversized native unit system".into());
        }
        for reference in &units.references {
            reference.validate()?;
        }
        if units.system.as_ref().map_or(0, String::len)
            + units
                .references
                .iter()
                .map(|r| r.unit.len() + 32)
                .sum::<usize>()
            > MAX_NATIVE_UNIT_METADATA_BYTES
        {
            return Err("Native unit metadata exceeds byte budget".into());
        }
        Ok(())
    }
}
fn escaped(value: &str) -> String {
    let mut out = String::new();
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' => out.push_str("&#9;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            _ => out.push(c),
        }
    }
    out
}
fn xml(units: &NativeUnitMonomial) -> Result<String, String> {
    units.validate()?;
    let mut out = format!("<u:unitMonomial xmlns:u=\"{U}\"");
    if let Some(system) = &units.system {
        out.push_str(&format!(" system=\"{}\"", escaped(system)));
    }
    out.push('>');
    for reference in &units.references {
        out.push_str(&format!(
            "<u:unitReference unit=\"{}\"",
            escaped(&reference.unit)
        ));
        for (name, value) in [
            ("power-numerator", reference.power_numerator),
            ("power-denominator", reference.power_denominator),
        ] {
            if let Some(value) = value {
                out.push_str(&format!(" {name}=\"{value}\""));
            }
        }
        out.push_str("/>");
    }
    out.push_str("</u:unitMonomial>");
    Ok(out)
}

/// Preserve order and absent versus explicit powers. Does not simplify units.
/// Limits are independent policy: 32 references/axis, 1024 UTF-8 bytes/name or
/// system, powers within ±1,000,000 and nonzero denominator. Empty monomials
/// are rejected (the shipped XSD requires at least one reference).
pub fn finite_with_units_xml(
    points: &[[f64; 2]],
    range_units: &Option<NativeUnitMonomial>,
    result_units: &Option<NativeUnitMonomial>,
) -> Result<Option<String>, String> {
    let units = [range_units, result_units]
        .into_iter()
        .map(|value| value.as_ref().map(xml).transpose())
        .collect::<Result<Vec<_>, _>>()?;
    let Some(mut source) = native_plot_result::finite_unitless_xml(points)? else {
        return Ok(None);
    };
    for (name, units) in ["RangeInfo", "ResultInfo"].into_iter().zip(units) {
        if let Some(units) = units {
            let start = source.find(&format!("<ml:{name} ")).unwrap();
            let end = start + source[start..].find("/>").unwrap();
            source.replace_range(end..end + 2, &format!(">{units}</ml:{name}>"));
        }
    }
    if source.len() > MAX_BYTES {
        return Err("Native unit-aware XY result exceeds XML budget".into());
    }
    Ok(Some(source))
}

fn attributes(node: roxmltree::Node<'_, '_>, allowed: &[&str]) -> Result<(), String> {
    if node
        .attributes()
        .any(|a| a.namespace().is_some() || !allowed.contains(&a.name()))
    {
        return Err("Unsupported native unit attributes".into());
    }
    Ok(())
}
fn structural_text(node: roxmltree::Node<'_, '_>) -> Result<(), String> {
    if node
        .children()
        .any(|n| n.is_text() && !n.text().unwrap_or("").trim().is_empty())
    {
        return Err("Unexpected native unit structural text".into());
    }
    Ok(())
}
fn monomial(node: roxmltree::Node<'_, '_>) -> Result<NativeUnitMonomial, String> {
    if !node.has_tag_name((U, "unitMonomial")) {
        return Err("Unsupported native unit namespace or element".into());
    }
    attributes(node, &["system"])?;
    structural_text(node)?;
    let mut references = Vec::new();
    for child in node.children().filter(|n| n.is_element()) {
        if !child.has_tag_name((U, "unitReference")) || child.children().any(|n| n.is_element()) {
            return Err("Unsupported native unit reference structure".into());
        }
        attributes(child, &["unit", "power-numerator", "power-denominator"])?;
        structural_text(child)?;
        let power = |name| {
            child
                .attribute(name)
                .map(|s| {
                    s.trim()
                        .parse::<i32>()
                        .map_err(|_| "Invalid native unit power".to_string())
                })
                .transpose()
        };
        references.push(NativeUnitReference {
            unit: child
                .attribute("unit")
                .ok_or("Missing native unit name")?
                .into(),
            power_numerator: power("power-numerator")?,
            power_denominator: power("power-denominator")?,
        });
        if references.len() > MAX_REFERENCES {
            return Err("Native unit reference count exceeds budget".into());
        }
    }
    let units = NativeUnitMonomial {
        system: node.attribute("system").map(str::to_owned),
        references,
    };
    units.validate()?;
    Ok(units)
}

/// Read inert residual metadata. Unsupported numeric layouts return None;
/// malformed/unknown units and budgets return Err. No name/unit interpretation.
pub fn read_result_with_units(
    node: roxmltree::Node<'_, '_>,
) -> Result<Option<NativePlotUnitResult>, String> {
    if !node.has_tag_name((ML, "Trace2dResult")) {
        return Ok(None);
    }
    if node.range().len() > MAX_BYTES || node.descendants().take(MAX_NODES + 1).count() > MAX_NODES
    {
        return Err("Native unit-aware XY result exceeds XML budget".into());
    }
    if node.descendants().any(|n| n.is_comment() || n.is_pi()) {
        return Err(
            "Native unit-aware XY comments and processing instructions are unsupported".into(),
        );
    }
    let mut units = [None, None];
    let mut remove = Vec::new();
    for (i, name) in ["RangeInfo", "ResultInfo"].into_iter().enumerate() {
        for axis in node.children().filter(|n| n.has_tag_name((ML, name))) {
            structural_text(axis)?;
            let children: Vec<_> = axis.children().filter(|n| n.is_element()).collect();
            if children.len() > 1 {
                return Err("Duplicate native unit monomials".into());
            }
            if let Some(child) = children.first() {
                units[i] = Some(monomial(*child)?);
                remove.push(child.range());
            }
        }
    }
    // Strip only validated unit elements, then reuse every existing curve check.
    // Restore inherited namespaces for fragments inside a larger package XML.
    let mut source = node.document().input_text()[node.range()].to_owned();
    remove.sort_by_key(|r| r.start);
    for range in remove.into_iter().rev() {
        source.replace_range(
            range.start - node.range().start..range.end - node.range().start,
            "",
        );
    }
    let mut wrapper = String::from("<wrapper");
    if node.namespaces().len() > 32
        || node
            .namespaces()
            .map(|n| n.uri().len() + n.name().map_or(0, str::len))
            .sum::<usize>()
            > 8192
    {
        return Err("Native XY inherited namespaces exceed budget".into());
    }
    for namespace in node.namespaces() {
        wrapper.push_str(" xmlns");
        if let Some(name) = namespace.name() {
            wrapper.push(':');
            wrapper.push_str(name);
        }
        wrapper.push_str(&format!("=\"{}\"", escaped(namespace.uri())));
    }
    wrapper.push('>');
    wrapper.push_str(&source);
    wrapper.push_str("</wrapper>");
    let doc = roxmltree::Document::parse_with_options(
        &wrapper,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: MAX_NODES as u32,
            ..Default::default()
        },
    )
    .map_err(|e| e.to_string())?;
    let root = doc
        .root_element()
        .first_element_child()
        .ok_or("Missing native XY result")?;
    let Some(curve) = native_plot_result::read_result(root)? else {
        return Ok(None);
    };
    let [range_units, result_units] = units;
    Ok(Some(NativePlotUnitResult {
        curve,
        range_units,
        result_units,
    }))
}

pub fn read_mathml_with_units(source: &str) -> Result<Option<NativePlotUnitResult>, String> {
    if source.len() > MAX_BYTES {
        return Err("Native unit-aware XY result exceeds XML budget".into());
    }
    let doc = roxmltree::Document::parse_with_options(
        source,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: MAX_NODES as u32,
            ..Default::default()
        },
    )
    .map_err(|e| e.to_string())?;
    if doc.descendants().any(|n| n.is_comment() || n.is_pi()) {
        return Err("Native XY comments and processing instructions are unsupported".into());
    }
    read_result_with_units(doc.root_element())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn units() -> NativeUnitMonomial {
        NativeUnitMonomial {
            system: Some("SI & \"단위\"\t\n\r😀".into()),
            references: vec![
                NativeUnitReference {
                    unit: "meter".into(),
                    power_numerator: None,
                    power_denominator: None,
                },
                NativeUnitReference {
                    unit: "kg<&\"😀".into(),
                    power_numerator: Some(-2),
                    power_denominator: Some(-3),
                },
                NativeUnitReference {
                    unit: "second".into(),
                    power_numerator: Some(1),
                    power_denominator: Some(1),
                },
            ],
        }
    }
    fn fixture() -> String {
        finite_with_units_xml(
            &[
                [-0., -2.],
                [f64::from_bits(1), -0.],
                [1.2345678901234567, 3.],
            ],
            &Some(units()),
            &None,
        )
        .unwrap()
        .unwrap()
    }
    #[test]
    fn units_preserve_order_optional_powers_unicode_escapes_and_float_bits() {
        let xml = fixture();
        let value = read_mathml_with_units(&xml).unwrap().unwrap();
        assert_eq!(value.range_units, Some(units()));
        assert_eq!(value.result_units, None);
        for (a, b) in [
            [-0., -2.],
            [f64::from_bits(1), -0.],
            [1.2345678901234567, 3.],
        ]
        .iter()
        .flatten()
        .zip(value.curve.points.iter().flatten())
        {
            assert_eq!(a.to_bits(), b.to_bits());
        }
        let doc = roxmltree::Document::parse(&xml).unwrap();
        assert!(
            native_plot_result::read_result(doc.root_element())
                .unwrap()
                .is_none()
        );
        let plain = native_plot_result::finite_unitless_xml(&[[1., 2.]])
            .unwrap()
            .unwrap();
        assert_eq!(
            finite_with_units_xml(&[[1., 2.]], &None, &None)
                .unwrap()
                .unwrap(),
            plain
        );
        assert_eq!(
            read_mathml_with_units(&plain).unwrap().unwrap().range_units,
            None
        );
    }
    #[test]
    fn serde_is_strict_bounded_and_preserves_optional_metadata() {
        let value = units();
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(
            serde_json::from_str::<NativeUnitMonomial>(&json).unwrap(),
            value
        );
        for bad in [
            r#"{"references":[{"unit":"meter","unit":"second"}]}"#.to_owned(),
            r#"{"references":[{"unit":"meter","future":1}]}"#.to_owned(),
            r#"{"references":[{"unit":"meter","power_denominator":0}]}"#.to_owned(),
            r#"{"references":[]}"#.to_owned(),
            r#"{"references":[{"power_numerator":1}]}"#.to_owned(),
            r#"{"system":"SI","system":"SI","references":[{"unit":"m"}]}"#.to_owned(),
            serde_json::json!({"references":vec![serde_json::json!({"unit":"meter"});33]})
                .to_string(),
            serde_json::json!({"references":vec![serde_json::json!({"unit":"a".repeat(1024)});32]})
                .to_string(),
        ] {
            assert!(
                serde_json::from_str::<NativeUnitMonomial>(&bad).is_err(),
                "{bad}"
            );
        }
        let nulls:NativeUnitMonomial=serde_json::from_str(r#"{"system":null,"references":[{"unit":"meter","power_numerator":null,"power_denominator":1}]}"#).unwrap();
        assert_eq!(nulls.system, None);
        assert_eq!(nulls.references[0].power_numerator, None);
        assert_eq!(nulls.references[0].power_denominator, Some(1));
    }
    #[test]
    fn literal_units_and_inherited_namespaces_preserve_wider_intervals() {
        let plain = native_plot_result::finite_unitless_xml(&[[0., 2.], [1., 3.]])
            .unwrap()
            .unwrap();
        let literal=plain.replace("xmlns:ml=\"http://schemas.mathsoft.com/math50\"", "").replace("<ml:RangeInfo Min=\"0\" Max=\"1\"/>", "<ml:RangeInfo Min=\"-1\" Max=\"4\"><u:unitMonomial system=\"\"><u:unitReference unit=\"meter\" power-numerator=\"0\" power-denominator=\"-2\"/></u:unitMonomial></ml:RangeInfo>");
        let package = format!("<package xmlns:ml=\"{ML}\" xmlns:u=\"{U}\">{literal}</package>");
        let doc = roxmltree::Document::parse(&package).unwrap();
        let value = read_result_with_units(doc.root_element().first_element_child().unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(value.curve.range, [-1., 4.]);
        assert_eq!(
            value.range_units.unwrap().references[0].power_denominator,
            Some(-2)
        );
    }
    #[test]
    fn malformed_unknown_units_and_lexical_injections_are_rejected() {
        let base = fixture();
        for (from, to) in [
            ("unit=\"meter\"", "unit=\"\""),
            ("power-denominator=\"-3\"", "power-denominator=\"0\""),
            ("power-numerator=\"-2\"", "power-numerator=\"1000001\""),
            ("power-numerator=\"-2\"", "power-numerator=\"1.5\""),
            ("unit=\"meter\"", "unit=\"meter\" future=\"x\""),
            (U, "urn:unknown"),
            (
                "<u:unitReference unit=\"meter\"/>",
                "<u:unitReference unit=\"meter\">text</u:unitReference>",
            ),
            ("</u:unitMonomial>", "</u:unitMonomial><u:unitMonomial/>"),
            ("[-0.0,5e-324,1.2345678901234567]", "[0,1,2,3]"),
            (
                "unit=\"meter\"",
                "unit=\"meter\" xmlns:z=\"urn:z\" z:unit=\"x\"",
            ),
        ] {
            assert_ne!(base.replace(from, to), base, "test must mutate {from}");
            assert!(
                read_mathml_with_units(&base.replace(from, to)).is_err(),
                "{from}"
            );
        }
        for bad in [
            base.replace("<u:unitReference unit=\"meter\"/>", "<!-- hidden -->"),
            base.replace("[-0.0,5e-324,1.2345678901234567]", "[0,1<?split?>,2]"),
            format!("<?outside x?>{base}"),
        ] {
            assert!(read_mathml_with_units(&bad).is_err());
        }
        let empty = base.replace(
            &xml(&units()).unwrap(),
            &format!("<u:unitMonomial xmlns:u=\"{U}\"/>"),
        );
        assert!(read_mathml_with_units(&empty).is_err());
        assert!(
            read_mathml_with_units(&base.replace("Max=\"1.2345678901234567\"", "Max=\"0\""))
                .is_err()
        );
    }
    #[test]
    fn limits_and_unsupported_vector_shapes_remain_explicit() {
        assert_eq!(finite_with_units_xml(&[], &None, &None).unwrap(), None);
        let mut u = units();
        u.references = vec![u.references[0].clone(); 33];
        assert!(finite_with_units_xml(&[[1., 2.]], &Some(u), &None).is_err());
        let mut u = units();
        u.system = Some("x".repeat(1025));
        assert!(finite_with_units_xml(&[[1., 2.]], &Some(u), &None).is_err());
        let mut u = units();
        u.references[0].unit = "\u{ffff}".into();
        assert!(finite_with_units_xml(&[[1., 2.]], &Some(u), &None).is_err());
        assert!(finite_with_units_xml(&vec![[0., 0.]; 65_537], &None, &None).is_err());
        assert!(finite_with_units_xml(&[[0., f64::NAN]], &None, &None).is_err());
        assert!(read_mathml_with_units(&" ".repeat(MAX_BYTES + 1)).is_err());
        for (from, to) in [
            ("TraceType=\"Vector\"", "TraceType=\"Parametric\""),
            ("VectorsType=\"Columns\"", "VectorsType=\"Rows\""),
            ("Count=\"1\"", "Count=\"2\""),
        ] {
            assert!(
                read_mathml_with_units(&fixture().replace(from, to))
                    .unwrap()
                    .is_none()
            );
        }
    }
    #[test]
    fn utf16_native_result_part_keeps_units_inert_and_original_bytes() {
        use std::io::Write;
        let result = format!(
            "<?xml version=\"1.0\" encoding=\"utf-16\"?><resultsList xmlns=\"http://schemas.mathsoft.com/result10\"><resultData result-id=\"1\" calculation-status=\"Synchronized\">{}</resultData></resultsList>",
            fixture()
        );
        let bytes: Vec<u8> = [0xff, 0xfe]
            .into_iter()
            .chain(result.encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for (name,data) in [("mathcad/worksheet.xml",b"<worksheet xmlns='http://schemas.mathsoft.com/worksheet50'><regions/></worksheet>".as_slice()),("mathcad/result.xml",bytes.as_slice())] {
            zip.start_file(name,zip::write::SimpleFileOptions::default()).unwrap();zip.write_all(data).unwrap();
        }
        let package =
            crate::native::NativePackage::from_bytes(&zip.finish().unwrap().into_inner()).unwrap();
        assert_eq!(
            package.original_part("mathcad/result.xml"),
            Some(bytes.as_slice())
        );
        let rows = package.caches().unwrap();
        assert_eq!(rows.len(), 1);
        let restored = read_mathml_with_units(rows[0].result_mathml.as_deref().unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(restored.range_units, Some(units()));
    }
    #[test]
    #[ignore = "Write bounded unit-aware native XML for offline XSD/UTF16 package verification"]
    fn native_xy_units_schema_fixture() {
        let output = std::path::PathBuf::from(
            std::env::var_os("MATHCRAFT_NATIVE_XY_UNITS_OUTPUT")
                .expect("MATHCRAFT_NATIVE_XY_UNITS_OUTPUT"),
        );
        std::fs::create_dir_all(&output).unwrap();
        for (name, units) in [
            (
                "si",
                NativeUnitMonomial {
                    system: Some("SI".into()),
                    references: vec![NativeUnitReference {
                        unit: "meter".into(),
                        power_numerator: None,
                        power_denominator: None,
                    }],
                },
            ),
            ("escaped-rational", units()),
        ] {
            let xml =
                finite_with_units_xml(&[[0., -0.], [1., 2.]], &Some(units.clone()), &Some(units))
                    .unwrap()
                    .unwrap();
            std::fs::write(output.join(format!("{name}.xml")),format!("<resultsList xmlns=\"http://schemas.mathsoft.com/result10\"><resultData result-id=\"1\" calculation-status=\"Synchronized\">{xml}</resultData></resultsList>")).unwrap();
        }
    }
}
