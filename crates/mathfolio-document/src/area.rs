//! Source-bound Prime collapsible areas. See analysis/0132. Display state and
//! per-child calculation flags are independent; no formula or cache is run here.
use crate::native::NativePackage;
use roxmltree::{Document, Node};
use serde::Serialize;
use std::ops::Range;
const WS: &str = "http://schemas.mathsoft.com/worksheet50";

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NativeArea {
    /// Element-child path to the enclosing worksheet region, as in NativeRegion.
    pub region_path: Vec<usize>,
    pub region_id: Option<String>,
    pub is_collapsed: bool,
    pub is_border_visible: bool,
    pub area_height: f64,
    /// Raw inherited schema flag. The UI bulk command aggregates child flags;
    /// this value is preserved rather than silently treated as a masking scope.
    pub declared_disable_calc: bool,
    pub protected: bool,
    /// Direct region children, with their original local coordinates in regions().
    pub child_region_paths: Vec<Vec<usize>>,
}

/// A direct math child of an Area. Paths are relative to the XML input root;
/// coordinates remain Area-local and source IDs are never reallocated here.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NativeAreaMath {
    pub region_path: Vec<usize>,
    pub region_id: Option<String>,
    pub left: f64,
    pub top: f64,
    pub result_ref: Option<String>,
    pub calculation_disabled: bool,
    pub mathml: String,
    pub math_xml: String,
    pub region_xml: String,
}

fn math_expression<'a, 'i>(math: Node<'a, 'i>) -> Result<Node<'a, 'i>, String> {
    let mut expressions = math.children().filter(|n| {
        n.is_element() && n.tag_name().namespace() == Some("http://schemas.mathsoft.com/math50")
    });
    let expression = expressions
        .next()
        .ok_or("Area child math has no expression")?;
    if expressions.next().is_some() {
        return Err("Area child math has multiple expression roots".into());
    }
    Ok(expression)
}

/// Read only direct math children, preserving unknown non-math siblings intact.
pub fn area_math_children_xml(
    xml: &str,
    area_path: &[usize],
) -> Result<Vec<NativeAreaMath>, String> {
    let doc = parse(xml)?;
    let area = area_at(&doc, area_path)?;
    let mut result = vec![];
    for region in children(area) {
        let Some(math) = region.children().find(|n| is(*n, "math")) else {
            continue;
        };
        let expression = math_expression(math)?;
        let coordinate = |name| -> Result<f64, String> {
            let n: f64 = region
                .attribute(name)
                .ok_or_else(|| format!("Missing Area child {name}"))?
                .parse()
                .map_err(|_| format!("Invalid Area child {name}"))?;
            if !n.is_finite() {
                return Err(format!("Nonfinite Area child {name}"));
            }
            Ok(n)
        };
        result.push(NativeAreaMath {
            region_path: path(region),
            region_id: region.attribute("region-id").map(str::to_owned),
            left: coordinate("left")?,
            top: coordinate("top")?,
            result_ref: math.attribute("resultRef").map(str::to_owned),
            calculation_disabled: boolean(math, "disable-calc", false)?,
            mathml: crate::spec_table::standalone(expression, xml)?,
            math_xml: crate::spec_table::standalone(math, xml)?,
            region_xml: crate::spec_table::standalone(region, xml)?,
        });
    }
    Ok(result)
}

/// Replace one direct math child's supported expression. The intermediate XML
/// deliberately drops stale resultRef; native export supplies a fresh required
/// resultRef and Pending resultData through the shared package allocator.
pub fn replace_area_math_xml(
    xml: &str,
    area_path: &[usize],
    child_path: &[usize],
    mathml: &str,
) -> Result<Option<String>, String> {
    use mathfolio_compat::editor_bridge::{tree_from_mathml, tree_to_mathml};
    if mathml.len() > 1024 * 1024 {
        return Err("Area math exceeds size limit".into());
    }
    let tree = tree_from_mathml(mathml)?;
    let serialized = crate::export::bind_math(&tree_to_mathml(&tree)?)?;
    let serialized_doc = parse(&serialized)?;
    if serialized_doc.root_element().tag_name().namespace()
        != Some("http://schemas.mathsoft.com/math50")
    {
        return Err("Area replacement must be namespace-complete MathML".into());
    }
    let doc = parse(xml)?;
    let area = area_at(&doc, area_path)?;
    if protected(area) {
        return Err("Protected Area child changes are unsupported".into());
    }
    let region = children(area)
        .into_iter()
        .find(|n| path(*n) == child_path)
        .ok_or("Math path is not a direct child of this Area")?;
    let math = region
        .children()
        .find(|n| is(*n, "math"))
        .ok_or("Area child is not math")?;
    let expression = math_expression(math)?;
    let before = crate::spec_table::standalone(expression, xml)?;
    if tree_from_mathml(&before).as_ref().ok() == Some(&tree) {
        return Ok(None);
    }
    let mut patches = vec![(expression.range(), serialized)];
    for (node, names) in [
        (math, &["resultRef", "error"][..]),
        (region, &["actualWidth", "actualHeight"][..]),
    ] {
        for name in names {
            if let Some(attribute) = node.attribute_node(*name) {
                patches.push((attribute.range(), String::new()));
            }
        }
    }
    apply(xml, patches).map(Some)
}

fn parse(xml: &str) -> Result<Document<'_>, String> {
    if xml.len() > 8 * 1024 * 1024 {
        return Err("Area XML exceeds size limit".into());
    }
    Document::parse_with_options(
        xml,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: 250_000,
            ..Default::default()
        },
    )
    .map_err(|e| e.to_string())
}
fn is(node: Node<'_, '_>, name: &str) -> bool {
    node.has_tag_name((WS, name))
}
fn boolean(node: Node<'_, '_>, name: &str, default: bool) -> Result<bool, String> {
    match node.attribute(name) {
        None => Ok(default),
        Some("true" | "1") => Ok(true),
        Some("false" | "0") => Ok(false),
        _ => Err(format!("Invalid Area boolean {name}")),
    }
}
fn path(mut node: Node<'_, '_>) -> Vec<usize> {
    let mut result = vec![];
    while let Some(parent) = node.parent_element() {
        result.push(
            parent
                .children()
                .filter(Node::is_element)
                .position(|child| child == node)
                .unwrap(),
        );
        node = parent;
    }
    result.reverse();
    result
}
fn area_at<'a, 'i>(doc: &'a Document<'i>, region_path: &[usize]) -> Result<Node<'a, 'i>, String> {
    let mut node = doc.root_element();
    for &slot in region_path {
        node = node
            .children()
            .filter(Node::is_element)
            .nth(slot)
            .ok_or("Area region path not found")?;
    }
    if !is(node, "region") {
        return Err("Area path does not identify a worksheet region".into());
    }
    node.children()
        .find(|n| is(*n, "Area"))
        .ok_or("Region is not an Area".into())
}
fn protected(area: Node<'_, '_>) -> bool {
    area.children()
        .any(|n| is(n, "ProtectionData") || is(n, "ProtectedItem"))
}
fn children<'a, 'i>(area: Node<'a, 'i>) -> Vec<Node<'a, 'i>> {
    area.children()
        .find(|n| is(*n, "regions"))
        .map(|n| n.children().filter(|r| is(*r, "region")).collect())
        .unwrap_or_default()
}
fn calculatable<'a, 'i>(region: Node<'a, 'i>) -> Option<Node<'a, 'i>> {
    region.children().find(|n| {
        n.tag_name().namespace() == Some(WS)
            && matches!(
                n.tag_name().name(),
                "math"
                    | "plot"
                    | "include"
                    | "mathBoard"
                    | "solveblock"
                    | "spec-table"
                    | "specTable"
                    | "excelComponent"
            )
    })
}
fn patch_attribute(
    xml: &str,
    node: Node<'_, '_>,
    name: &str,
    value: &str,
    patches: &mut Vec<(Range<usize>, String)>,
) -> Result<(), String> {
    if let Some(attribute) = node.attribute_node(name) {
        patches.push((attribute.range_value(), value.into()));
        return Ok(());
    }
    let start = node.range().start;
    let bytes = xml.as_bytes();
    let mut quote = 0;
    let mut end = start + 1;
    while end < bytes.len() {
        let byte = bytes[end];
        if quote != 0 {
            if byte == quote {
                quote = 0;
            }
        } else if byte == b'\'' || byte == b'"' {
            quote = byte;
        } else if byte == b'>' {
            break;
        }
        end += 1;
    }
    if end == bytes.len() {
        return Err("Unterminated Area XML start tag".into());
    }
    let insert = if bytes[end - 1] == b'/' { end - 1 } else { end };
    patches.push((insert..insert, format!(" {name}=\"{value}\"")));
    Ok(())
}
fn apply(xml: &str, mut patches: Vec<(Range<usize>, String)>) -> Result<String, String> {
    // Repeated zero-width insertions are allowed; stable ordering preserves all.
    patches.sort_by_key(|(range, _)| (range.start, range.end));
    let mut out = xml.to_owned();
    let mut previous = xml.len();
    for (range, value) in patches.into_iter().rev() {
        if range.end > previous {
            return Err("Overlapping Area XML patches".into());
        }
        previous = range.start;
        out.replace_range(range, &value);
    }
    parse(&out)?;
    Ok(out)
}

/// Read source Area metadata from a worksheet or namespace-complete owning
/// region fragment. Coordinates remain local; descendants are never flattened.
pub fn areas_from_xml(xml: &str) -> Result<Vec<NativeArea>, String> {
    let doc = parse(xml)?;
    let mut result = vec![];
    for area in doc.descendants().filter(|n| is(*n, "Area")) {
        let region = area
            .parent_element()
            .filter(|n| is(*n, "region"))
            .ok_or("Area lacks worksheet region owner")?;
        let height = area
            .attribute("AreaHeight")
            .unwrap_or("0")
            .parse::<f64>()
            .map_err(|_| "Invalid AreaHeight")?;
        if !height.is_finite() || height < 0. {
            return Err("Invalid AreaHeight".into());
        }
        result.push(NativeArea {
            region_path: path(region),
            region_id: region.attribute("region-id").map(str::to_owned),
            is_collapsed: boolean(area, "IsCollapsed", false)?,
            is_border_visible: boolean(area, "IsBorderVisible", true)?,
            area_height: height,
            declared_disable_calc: boolean(area, "disable-calc", false)?,
            protected: protected(area),
            child_region_paths: children(area).into_iter().map(path).collect(),
        });
    }
    Ok(result)
}

/// Patch a namespace-complete worksheet or owning-region XML fragment. A region
/// fragment uses an empty path; worksheet paths match `NativeRegion::path`.
/// `None` means byte-identical no-op. Returned UTF-8 XML retains declarations,
/// unknown content, children, and result references; the caller owns encoding.
pub fn set_area_display_xml(
    xml: &str,
    region_path: &[usize],
    collapsed: Option<bool>,
    border_visible: Option<bool>,
) -> Result<Option<String>, String> {
    let doc = parse(xml)?;
    let area = area_at(&doc, region_path)?;
    if protected(area) {
        return Err("Protected Area display changes are unsupported".into());
    }
    let mut patches = vec![];
    for (name, value, default) in [
        ("IsCollapsed", collapsed, false),
        ("IsBorderVisible", border_visible, true),
    ] {
        if let Some(value) = value
            && boolean(area, name, default)? != value
        {
            patch_attribute(
                xml,
                area,
                name,
                if value { "true" } else { "false" },
                &mut patches,
            )?;
        }
    }
    if patches.is_empty() {
        Ok(None)
    } else {
        apply(xml, patches).map(Some)
    }
}

/// Source-only bulk disable command on a worksheet or namespace-complete owning
/// region fragment. This is independent of collapsed state and never evaluates.
pub fn toggle_area_calculation_xml(
    xml: &str,
    region_path: &[usize],
) -> Result<Option<String>, String> {
    let doc = parse(xml)?;
    let area = area_at(&doc, region_path)?;
    if protected(area) {
        return Err("Protected Area calculation changes are unsupported".into());
    }
    let nodes: Vec<_> = children(area)
        .into_iter()
        .filter_map(calculatable)
        .collect();
    let disabled: Vec<_> = nodes
        .iter()
        .map(|n| boolean(*n, "disable-calc", false))
        .collect::<Result<_, _>>()?;
    let target = disabled.iter().any(|value| !*value);
    let mut patches = vec![];
    for (node, old) in nodes.into_iter().zip(disabled) {
        if old != target {
            patch_attribute(
                xml,
                node,
                "disable-calc",
                if target { "true" } else { "false" },
                &mut patches,
            )?;
        }
    }
    if patches.is_empty() {
        Ok(None)
    } else {
        apply(xml, patches).map(Some)
    }
}

impl NativePackage {
    /// Enumerate Areas without flattening descendants or reading saved data as code.
    pub fn areas(&self) -> Result<Vec<NativeArea>, String> {
        areas_from_xml(&self.worksheet_xml()?)
    }

    /// Patch only source display attributes. Child XML and saved cache parts are
    /// untouched. Protected display states remain unsupported rather than unlocked.
    pub fn set_area_display(
        &mut self,
        region_path: &[usize],
        collapsed: Option<bool>,
        border_visible: Option<bool>,
    ) -> Result<bool, String> {
        let xml = self.worksheet_xml()?;
        let Some(changed) = set_area_display_xml(&xml, region_path, collapsed, border_visible)?
        else {
            return Ok(false);
        };
        let part = self.worksheet_part_name().to_owned();
        crate::export::set_xml(self, &part, changed)?;
        Ok(true)
    }

    /// Original bulk command: if any calculatable child is enabled, disable its
    /// enabled children; otherwise enable all. This changes child flags, not a
    /// reversible parent mask. The caller owns one document Undo transaction.
    pub fn toggle_area_calculation(&mut self, region_path: &[usize]) -> Result<bool, String> {
        let xml = self.worksheet_xml()?;
        let Some(changed) = toggle_area_calculation_xml(&xml, region_path)? else {
            return Ok(false);
        };
        let part = self.worksheet_part_name().to_owned();
        crate::export::set_xml(self, &part, changed)?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};
    use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};
    fn package(xml: &str) -> NativePackage {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, data) in [
            ("mathcad/worksheet.xml", xml),
            (
                "mathcad/result.xml",
                r#"<resultsList xmlns="http://schemas.mathsoft.com/result10"/>"#,
            ),
            ("custom/keep.bin", "untouched"),
        ] {
            zip.start_file(
                name,
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
            )
            .unwrap();
            zip.write_all(data.as_bytes()).unwrap();
        }
        NativePackage::from_bytes(&zip.finish().unwrap().into_inner()).unwrap()
    }
    fn xml(extra: &str) -> String {
        format!(
            r#"<ws:worksheet xmlns:ws="{WS}" xmlns:ml="http://schemas.mathsoft.com/math50" xmlns:u="urn:future"><ws:regions><ws:region region-id="2" left="4" top="50"><ws:Area AreaHeight="120" u:keep="yes" {extra}><!--kept--><ws:regions><ws:region region-id="3" left="19.2" top="9.6"><ws:math resultRef="7"><ml:real>12</ml:real></ws:math></ws:region><ws:region region-id="4" left="40" top="9.6"><ws:math disable-calc="true" resultRef="8"><ml:real>34</ml:real></ws:math></ws:region><ws:region region-id="5"><ws:text/></ws:region></ws:regions></ws:Area></ws:region></ws:regions></ws:worksheet>"#
        )
    }
    #[test]
    fn child_math_edit_keeps_local_identity_unknowns_and_other_saved_reference() {
        let source = xml("");
        let children = area_math_children_xml(&source, &[0, 0]).unwrap();
        assert_eq!(children.len(), 2);
        let first = &children[0];
        assert_eq!((first.left, first.top), (19.2, 9.6));
        assert_eq!(first.region_id.as_deref(), Some("3"));
        assert!(
            replace_area_math_xml(&source, &[0, 0], &first.region_path, &first.mathml)
                .unwrap()
                .is_none()
        );
        let changed = replace_area_math_xml(
            &source,
            &[0, 0],
            &first.region_path,
            r#"<ml:real xmlns:ml="http://schemas.mathsoft.com/math50">99</ml:real>"#,
        )
        .unwrap()
        .unwrap();
        let after = area_math_children_xml(&changed, &[0, 0]).unwrap();
        assert!(after[0].result_ref.is_none());
        assert_eq!(after[1], children[1]);
        assert_eq!(after[0].region_path, first.region_path);
        assert_eq!((after[0].left, after[0].top), (first.left, first.top));
        assert!(changed.contains("<!--kept-->"));
        assert!(changed.contains("u:keep=\"yes\""));
        assert!(replace_area_math_xml(&source, &[0, 0], &[9], &first.mathml).is_err());
        assert!(replace_area_math_xml(&source, &[0, 0], &first.region_path, "<bogus/>").is_err());
    }
    #[test]
    fn owning_region_fragment_helpers_need_no_package_or_flattening() {
        let xml = format!(
            r#"<ws:region xmlns:ws="{WS}" region-id="9"><ws:Area><ws:regions><ws:region region-id="10"><ws:math resultRef="3"/></ws:region></ws:regions></ws:Area></ws:region>"#
        );
        let info = areas_from_xml(&xml).unwrap().remove(0);
        assert!(info.region_path.is_empty());
        assert_eq!(info.child_region_paths, vec![vec![0, 0, 0]]);
        let changed = set_area_display_xml(&xml, &[], Some(true), Some(false))
            .unwrap()
            .unwrap();
        let changed = toggle_area_calculation_xml(&changed, &[]).unwrap().unwrap();
        assert!(changed.contains("resultRef=\"3\""));
        assert!(changed.contains("disable-calc=\"true\""));
        let info = areas_from_xml(&changed).unwrap().remove(0);
        assert!(info.is_collapsed && !info.is_border_visible);
    }
    #[test]
    fn display_patch_preserves_children_local_coordinates_unknowns_and_saved_results() {
        let mut p = package(&xml(""));
        let before = p.regions().unwrap();
        let area = p.areas().unwrap().remove(0);
        assert_eq!(area.child_region_paths.len(), 3);
        assert!(!area.is_collapsed && area.is_border_visible);
        assert_eq!(area.area_height, 120.);
        let cache = p.original_part("mathcad/result.xml").unwrap().to_vec();
        assert!(
            p.set_area_display(&area.region_path, Some(true), Some(false))
                .unwrap()
        );
        assert!(
            !p.set_area_display(&area.region_path, Some(true), Some(false))
                .unwrap()
        );
        let after = p.regions().unwrap();
        for (a, b) in before.iter().skip(1).zip(after.iter().skip(1)) {
            assert_eq!(a.raw_xml, b.raw_xml);
            assert_eq!(a.path, b.path);
            assert_eq!(a.parent_path, b.parent_path);
        }
        assert_eq!(p.original_part("mathcad/result.xml").unwrap(), cache);
        let text = p.worksheet_xml().unwrap();
        assert!(text.contains("<!--kept-->") && text.contains("u:keep=\"yes\""));
        let info = p.areas().unwrap().remove(0);
        assert!(info.is_collapsed && !info.is_border_visible);
    }
    #[test]
    fn bulk_disable_mutates_child_flags_and_enables_all_on_second_toggle() {
        let mut p = package(&xml("IsCollapsed=\"true\""));
        let path = p.areas().unwrap()[0].region_path.clone();
        assert!(p.toggle_area_calculation(&path).unwrap());
        let s = p.worksheet_xml().unwrap();
        assert_eq!(s.matches("disable-calc=\"true\"").count(), 2);
        assert!(p.areas().unwrap()[0].is_collapsed);
        assert!(p.toggle_area_calculation(&path).unwrap());
        let s = p.worksheet_xml().unwrap();
        assert!(!s.contains("disable-calc=\"true\""));
        assert_eq!(s.matches("disable-calc=\"false\"").count(), 2);
        assert!(s.contains("resultRef=\"7\"") && s.contains("resultRef=\"8\""));
        assert_eq!(p.original_part("custom/keep.bin").unwrap(), b"untouched");
    }
    #[test]
    fn utf16_area_patch_repackages_without_changing_declaration_or_cache_bytes() {
        let text = format!("<?xml version=\"1.0\" encoding=\"utf-16\"?>{}", xml(""));
        let mut p = package(&xml(""));
        let bytes: Vec<_> = [0xff, 0xfe]
            .into_iter()
            .chain(text.encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        p.set_part("mathcad/worksheet.xml", bytes).unwrap();
        let path = p.areas().unwrap()[0].region_path.clone();
        let cache = p.original_part("mathcad/result.xml").unwrap().to_vec();
        assert!(p.set_area_display(&path, Some(true), None).unwrap());
        assert!(
            p.original_part("mathcad/worksheet.xml")
                .unwrap()
                .starts_with(&[0xff, 0xfe])
        );
        let roundtrip = NativePackage::from_bytes(&p.to_bytes().unwrap()).unwrap();
        assert!(roundtrip.areas().unwrap()[0].is_collapsed);
        assert!(
            roundtrip
                .worksheet_xml()
                .unwrap()
                .contains("encoding=\"utf-16\"")
        );
        assert_eq!(
            roundtrip.original_part("mathcad/result.xml").unwrap(),
            cache
        );
    }

    #[test]
    fn rejected_path_protection_and_invalid_state_are_atomic() {
        let mut p = package(&xml(""));
        let before = p.worksheet_xml().unwrap();
        assert!(p.set_area_display(&[99], Some(true), None).is_err());
        assert_eq!(p.worksheet_xml().unwrap(), before);
        let mut protected_xml = xml("");
        protected_xml = protected_xml.replace(
            "</ws:Area>",
            "<ws:ProtectionData ProtectionLevel=\"LockDisplayState\"/></ws:Area>",
        );
        let mut p = package(&protected_xml);
        let path = p.areas().unwrap()[0].region_path.clone();
        assert!(p.set_area_display(&path, Some(true), None).is_err());
        assert!(p.toggle_area_calculation(&path).is_err());
        assert_eq!(p.worksheet_xml().unwrap(), protected_xml);
        assert!(package(&xml("IsCollapsed=\"maybe\"")).areas().is_err());
    }
}
