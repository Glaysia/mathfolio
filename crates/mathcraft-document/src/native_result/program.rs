//! Conservative proof for preserving original saved results as-is. This does
//! not prove rows independently and never rebuilds or executes an environment.
use crate::native::{NativePackage, xml_text};
use roxmltree::{Document, Node};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};

const WS: &str = "http://schemas.mathsoft.com/worksheet50";
const RESULTS: &str = "http://schemas.mathsoft.com/result10";
const MAX_XML: usize = 8 * 1024 * 1024;
const MAX_NODES: u32 = 250_000;
const MAX_DEPTH: usize = 64;

/// Entire package context except its worksheet and actual saved-result parts.
/// Settings, relationships, flow documents and binaries are all hash-bound;
/// changing even a plain text part is intentionally conservative for now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextSnapshot {
    worksheet_part: String,
    parts: BTreeMap<String, (usize, [u8; 32])>,
}

impl ContextSnapshot {
    pub fn capture(package: &NativePackage) -> Result<Self, String> {
        let mut parts = BTreeMap::new();
        for part in package.parts() {
            if part.name == package.worksheet_part_name() || is_results_list(part.bytes()) {
                continue;
            }
            parts.insert(
                part.name.clone(),
                (part.bytes().len(), Sha256::digest(part.bytes()).into()),
            );
        }
        Ok(Self {
            worksheet_part: package.worksheet_part_name().to_owned(),
            parts,
        })
    }

    pub fn matches(&self, package: &NativePackage) -> Result<bool, String> {
        Ok(*self == Self::capture(package)?)
    }
}

fn parse(xml: &str) -> Result<Document<'_>, String> {
    if xml.len() > MAX_XML {
        return Err("Saved-result source proof exceeds XML byte limit".into());
    }
    Document::parse_with_options(
        xml,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: MAX_NODES,
            ..Default::default()
        },
    )
    .map_err(|error| error.to_string())
}

fn is_results_list(bytes: &[u8]) -> bool {
    // Do not decode/copy arbitrary large binary parts merely to classify them.
    // A parse failure is not an exclusion: those exact bytes remain hash-bound.
    let bytes_without_bom = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    let looks_xml = bytes.starts_with(&[0xff, 0xfe])
        || bytes.starts_with(&[0xfe, 0xff])
        || bytes.starts_with(&[0, b'<'])
        || bytes_without_bom
            .iter()
            .find(|byte| !byte.is_ascii_whitespace())
            == Some(&b'<');
    if !looks_xml {
        return false;
    }
    xml_text(bytes)
        .ok()
        .and_then(|xml| {
            parse(&xml)
                .ok()
                .map(|doc| doc.root_element().has_tag_name((RESULTS, "resultsList")))
        })
        .unwrap_or(false)
}

fn token(hash: &mut Sha256, text: &str) {
    hash.update((text.len() as u64).to_le_bytes());
    hash.update(text.as_bytes());
}

fn attributes(hash: &mut Sha256, node: Node<'_, '_>, ignored: &[&str]) {
    let mut attrs = node
        .attributes()
        .filter(|attribute| attribute.namespace().is_some() || !ignored.contains(&attribute.name()))
        .map(|attribute| {
            (
                attribute.namespace().unwrap_or(""),
                attribute.name(),
                attribute.value(),
            )
        })
        .collect::<Vec<_>>();
    attrs.sort_unstable();
    hash.update((attrs.len() as u64).to_le_bytes());
    for (namespace, name, value) in attrs {
        token(hash, namespace);
        token(hash, name);
        token(hash, value);
    }
}

fn leaf_format(node: Node<'_, '_>, attrs: &[&str]) -> bool {
    node.children().all(|child| {
        child.is_comment() || (child.is_text() && child.text().unwrap_or("").trim().is_empty())
    }) && node
        .attributes()
        .all(|attribute| attribute.namespace().is_none() && attrs.contains(&attribute.name()))
}

fn display_only_math_child(node: Node<'_, '_>) -> bool {
    if node.has_tag_name((WS, "formattingOverride")) {
        return leaf_format(
            node,
            &[
                "FontFamily",
                "FontSize",
                "Bold",
                "Italic",
                "Underline",
                "ForegroundColor",
                "BackgroundColor",
            ],
        );
    }
    if !node.has_tag_name((WS, "resultFormat")) || node.attributes().len() != 0 {
        return false;
    }
    node.children().all(|child| {
        if child.is_comment() || (child.is_text() && child.text().unwrap_or("").trim().is_empty()) {
            return true;
        }
        child.tag_name().namespace() == Some(WS)
            && matches!(
                child.tag_name().name(),
                "general" | "decimal" | "scientific" | "engineering" | "fraction" | "percent"
            )
            && leaf_format(
                child,
                &[
                    "imaginary-value",
                    "precision",
                    "complex-threshold",
                    "zero-threshold",
                    "radix",
                    "show-trailing-zeros",
                    "exponential-threshold",
                    "use-e-notation",
                    "use-engineering-format",
                    "use-mixed-numbers",
                ],
            )
    })
}

fn canonical(
    hash: &mut Sha256,
    node: Node<'_, '_>,
    depth: usize,
    math_root: bool,
) -> Result<(), String> {
    if depth > MAX_DEPTH {
        return Err("Saved-result source proof exceeds depth limit".into());
    }
    if node.is_element() {
        token(hash, "element");
        token(hash, node.tag_name().namespace().unwrap_or(""));
        token(hash, node.tag_name().name());
        attributes(
            hash,
            node,
            if math_root {
                &["resultRef", "error"]
            } else {
                &[]
            },
        );
        for child in node.children() {
            if math_root && display_only_math_child(child) {
                continue;
            }
            // Only indentation directly in the known math wrapper is ignored.
            // Unknown nested mixed content/whitespace remains source-bound.
            if math_root && child.is_text() && child.text().unwrap_or("").trim().is_empty() {
                continue;
            }
            canonical(hash, child, depth + 1, false)?;
        }
        token(hash, "end");
    } else if node.is_text() {
        token(hash, "text");
        token(hash, node.text().unwrap_or(""));
    } else if node.is_comment() {
        token(hash, "comment");
        token(hash, node.text().unwrap_or(""));
    } else if let Some(pi) = node.pi() {
        token(hash, "pi");
        token(hash, pi.target);
        token(hash, pi.value.unwrap_or(""));
    }
    Ok(())
}

const GEOMETRY: &[&str] = &[
    "left",
    "top",
    "width",
    "height",
    "actualWidth",
    "actualHeight",
    "z-order",
];

fn coordinate(region: Node<'_, '_>, name: &str) -> Result<f64, String> {
    let value: f64 = region
        .attribute(name)
        .ok_or("Missing calculation-order coordinate")?
        .parse()
        .map_err(|_| "Invalid calculation-order coordinate")?;
    if !value.is_finite() {
        return Err("Nonfinite calculation-order coordinate".into());
    }
    Ok(value)
}

fn ordered_children<'a, 'i>(regions: Node<'a, 'i>) -> Result<Vec<Node<'a, 'i>>, String> {
    let mut order = vec![];
    for region in regions
        .children()
        .filter(|node| node.has_tag_name((WS, "region")))
    {
        order.push((
            coordinate(region, "top")?,
            coordinate(region, "left")?,
            region,
        ));
    }
    // Stable equal-coordinate order matches the existing bounded compat policy.
    order.sort_by(|left, right| left.0.total_cmp(&right.0).then(left.1.total_cmp(&right.1)));
    Ok(order.into_iter().map(|(_, _, region)| region).collect())
}

fn nonembedded_display(region: Node<'_, '_>) -> bool {
    if region.attributes().any(|attribute| {
        attribute.namespace().is_some()
            || !(GEOMETRY.contains(&attribute.name()) || attribute.name() == "region-id")
    }) {
        return false;
    }
    let payloads = region
        .children()
        .filter(Node::is_element)
        .collect::<Vec<_>>();
    if payloads.len() != 1 {
        return false;
    }
    let payload = payloads[0];
    if !(payload.has_tag_name((WS, "text")) || payload.has_tag_name((WS, "pageBreak"))) {
        return false;
    }
    if payload.attributes().any(|attribute| {
        attribute.namespace().is_some()
            || !(payload.has_tag_name((WS, "text")) && attribute.name() == "item-idref")
    }) {
        return false;
    }
    // An external flow document is protected by ContextSnapshot; any embedded
    // region/unknown child in this source remains a conservative source record.
    !payload.descendants().skip(1).any(|node| node.is_element())
}

fn collection_context(
    hash: &mut Sha256,
    collection: Node<'_, '_>,
    depth: usize,
) -> Result<(), String> {
    attributes(hash, collection, &[]);
    for child in collection
        .children()
        .filter(|node| !node.has_tag_name((WS, "region")))
    {
        canonical(hash, child, depth, false)?;
    }
    Ok(())
}

fn insert_id(ids: &mut HashSet<String>, region: Node<'_, '_>) -> Result<(), String> {
    let id = region
        .attribute("region-id")
        .ok_or("Missing source region ID")?;
    if id.is_empty() || id.len() > 128 || !id.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("Unsupported source region ID for saved-result proof".into());
    }
    let normalized = id.trim_start_matches('0');
    if !ids.insert(
        if normalized.is_empty() {
            "0"
        } else {
            normalized
        }
        .to_owned(),
    ) {
        return Err("Duplicate source region ID for saved-result proof".into());
    }
    Ok(())
}

fn region_record(
    region: Node<'_, '_>,
    ids: &mut HashSet<String>,
    depth: usize,
) -> Result<Option<[u8; 32]>, String> {
    if depth > MAX_DEPTH {
        return Err("Saved-result region hierarchy exceeds depth limit".into());
    }
    insert_id(ids, region)?;
    if nonembedded_display(region) {
        return Ok(None);
    }
    let payloads = region
        .children()
        .filter(Node::is_element)
        .collect::<Vec<_>>();
    let area = (payloads.len() == 1 && payloads[0].has_tag_name((WS, "Area"))).then(|| payloads[0]);
    let mut hash = Sha256::new();
    token(&mut hash, "region");
    attributes(&mut hash, region, GEOMETRY);
    if let Some(area) = area {
        let collections = area
            .children()
            .filter(|node| node.has_tag_name((WS, "regions")))
            .collect::<Vec<_>>();
        if collections.len() > 1 {
            return Err("Ambiguous Area child collections in saved-result proof".into());
        }
        let mut children = vec![];
        if let Some(regions) = collections.first() {
            for child in ordered_children(*regions)? {
                if let Some(record) = region_record(child, ids, depth + 1)? {
                    children.push(record);
                }
            }
        }
        if children.is_empty()
            && region.attributes().all(|attribute| {
                attribute.namespace().is_none()
                    && (GEOMETRY.contains(&attribute.name()) || attribute.name() == "region-id")
            })
            && collections.first().is_none_or(|collection| {
                collection.attributes().len() == 0
                    && collection
                        .children()
                        .all(|node| !node.is_element() || node.has_tag_name((WS, "region")))
            })
            && area.attributes().all(|attribute| {
                attribute.namespace().is_none()
                    && ["IsCollapsed", "IsBorderVisible", "AreaHeight"].contains(&attribute.name())
            })
            && area
                .children()
                .all(|child| child.has_tag_name((WS, "regions")) || !child.is_element())
        {
            return Ok(None);
        }
        token(&mut hash, "Area");
        attributes(
            &mut hash,
            area,
            &["IsCollapsed", "IsBorderVisible", "AreaHeight"],
        );
        if let Some(collection) = collections.first() {
            collection_context(&mut hash, *collection, depth + 1)?;
        }
        for child in area
            .children()
            .filter(|node| !node.has_tag_name((WS, "regions")))
        {
            canonical(&mut hash, child, depth + 1, false)?;
        }
        for record in children {
            hash.update(record);
        }
    } else {
        for child in region.children() {
            canonical(
                &mut hash,
                child,
                depth + 1,
                child.has_tag_name((WS, "math")),
            )?;
        }
    }
    Ok(Some(hash.finalize().into()))
}

fn fingerprint(xml: &str) -> Result<[u8; 32], String> {
    let document = parse(xml)?;
    let root = document.root_element();
    if !root.has_tag_name((WS, "worksheet")) {
        return Err("Saved-result source proof requires a Worksheet50 root".into());
    }
    let collections = root
        .children()
        .filter(|node| node.has_tag_name((WS, "regions")))
        .collect::<Vec<_>>();
    if collections.len() != 1 {
        return Err("Saved-result source proof requires one root region collection".into());
    }
    let mut hash = Sha256::new();
    let mut all_ids = HashSet::new();
    for region in root
        .descendants()
        .filter(|node| node.has_tag_name((WS, "region")))
    {
        insert_id(&mut all_ids, region)?;
    }
    attributes(&mut hash, root, &[]);
    collection_context(&mut hash, collections[0], 0)?;
    for child in root
        .children()
        .filter(|child| !child.has_tag_name((WS, "regions")))
    {
        canonical(&mut hash, child, 0, false)?;
    }
    let mut ids = HashSet::new();
    for region in ordered_children(collections[0])? {
        if let Some(record) = region_record(region, &mut ids, 0)? {
            hash.update(record);
        }
    }
    Ok(hash.finalize().into())
}

/// Proves source identity only for the complete ordered calculation program.
/// Caller MUST also require ContextSnapshot::matches for all package context.
/// False or error requires Pending/fresh-result fallback, never a partial Sync.
pub fn same_calculation_program(before: &str, after: &str) -> Result<bool, String> {
    Ok(fingerprint(before)? == fingerprint(after)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use zip::{ZipWriter, write::SimpleFileOptions};

    fn worksheet(regions: &str) -> String {
        format!(
            "<worksheet xmlns='{WS}' xmlns:ml='http://schemas.mathsoft.com/math50'><regions>{regions}</regions></worksheet>"
        )
    }
    fn math(id: &str, y: f64, value: &str) -> String {
        format!(
            "<region region-id='{id}' left='0' top='{y}'><math resultRef='{id}'><ml:real>{value}</ml:real></math></region>"
        )
    }
    fn package(worksheet: &str, extra: &[(&str, &[u8])]) -> NativePackage {
        let mut zip = ZipWriter::new(Cursor::new(vec![]));
        for (name, bytes) in std::iter::once(("mathcad/worksheet.xml", worksheet.as_bytes()))
            .chain(extra.iter().copied())
        {
            zip.start_file(name, SimpleFileOptions::default()).unwrap();
            std::io::Write::write_all(&mut zip, bytes).unwrap();
        }
        NativePackage::from_bytes(&zip.finish().unwrap().into_inner()).unwrap()
    }

    #[test]
    fn whole_program_changes_never_preserve_an_unchanged_downstream_result_as_sync() {
        let before = worksheet(&(math("1", 0., "2") + &math("2", 50., "3")));
        assert!(same_calculation_program(&before, &before).unwrap());
        assert!(!same_calculation_program(&before, &before.replacen(">2<", ">7<", 1)).unwrap());
        assert!(
            !same_calculation_program(&before, &before.replace("region-id='1'", "region-id='9'"))
                .unwrap()
        );
        assert!(
            !same_calculation_program(
                &before,
                &before.replace(
                    "<math resultRef='1'>",
                    "<math resultRef='1' disable-calc='true'>"
                )
            )
            .unwrap()
        );
        let moved = worksheet(&(math("1", 70., "2") + &math("2", 50., "3")));
        assert!(!same_calculation_program(&before, &moved).unwrap());
        let same_order = worksheet(&(math("1", 10., "2") + &math("2", 60., "3")));
        assert!(same_calculation_program(&before, &same_order).unwrap());
    }

    #[test]
    fn area_children_are_local_sorted_blocks_not_a_global_absolute_coordinate_merge() {
        let area = |child1: f64, child2: f64| {
            format!(
                "<region region-id='10' left='0' top='0'><Area IsCollapsed='false'><regions>{}{}</regions></Area></region>",
                math("11", child1, "1"),
                math("12", child2, "2")
            )
        };
        let before = worksheet(&(area(0., 500.) + &math("20", 50., "3")));
        let collapsed = before.replace(
            "IsCollapsed='false'",
            "IsCollapsed='true' IsBorderVisible='false' AreaHeight='600'",
        );
        assert!(same_calculation_program(&before, &collapsed).unwrap());
        assert!(
            !same_calculation_program(
                &before,
                &worksheet(&(area(600., 500.) + &math("20", 50., "3")))
            )
            .unwrap()
        );
        let empty =
            "<region region-id='99' left='0' top='25'><Area height='96'><regions/></Area></region>";
        // Unknown Area attributes are retained, not guessed to be display-only.
        assert!(
            !same_calculation_program(
                &before,
                &worksheet(&(area(0., 500.) + empty + &math("20", 50., "3")))
            )
            .unwrap()
        );
        assert!(
            same_calculation_program(
                &before,
                &worksheet(
                    &(area(0., 500.) + &empty.replace(" height='96'", "") + &math("20", 50., "3"))
                )
            )
            .unwrap()
        );
    }

    #[test]
    fn known_display_formats_and_plain_text_breaks_are_excluded_but_unknown_fields_are_not() {
        let before = worksheet(&math("1", 0., "2"));
        let formatted=before.replace("resultRef='1'","resultRef='8' error='old'").replace("</math>","<resultFormat><decimal precision='8'/></resultFormat><formattingOverride FontSize='18'/></math>");
        assert!(same_calculation_program(&before, &formatted).unwrap());
        for extra in [
            "<region region-id='2' left='0' top='-1'><text item-idref='rId7'/></region>",
            "<region region-id='2' left='0' top='-1'><pageBreak/></region>",
        ] {
            assert!(
                same_calculation_program(
                    &before,
                    &worksheet(&(extra.to_owned() + &math("1", 0., "2")))
                )
                .unwrap()
            );
        }
        assert!(
            !same_calculation_program(
                &before,
                &formatted.replace("FontSize='18'", "FontSize='18' future='semantic'")
            )
            .unwrap()
        );
        let unknown = before.replace("<ml:real>", "<ml:real xmlns:f='urn:future' f:mode='a'>");
        assert!(
            !same_calculation_program(&unknown, &unknown.replace("f:mode='a'", "f:mode='b'"))
                .unwrap()
        );
        let embedded = "<region region-id='2' left='0' top='-1'><text><region region-id='3' left='0' top='0'><math><ml:real>4</ml:real></math></region></text></region>";
        assert!(
            !same_calculation_program(
                &before,
                &worksheet(&(embedded.to_owned() + &math("1", 0., "2")))
            )
            .unwrap()
        );
    }

    #[test]
    fn namespace_prefix_and_attribute_order_are_canonical_but_unknown_values_remain_bound() {
        let before = worksheet(&math("1", 0., "2")).replace(
            "<math resultRef='1'>",
            "<math xmlns:f='urn:future' resultRef='1' f:mode='a'>",
        );
        let after = before
            .replace("xmlns:ml=", "xmlns:q=")
            .replace("ml:", "q:")
            .replace("resultRef='1' f:mode='a'", "f:mode='a' resultRef='1'");
        assert!(same_calculation_program(&before, &after).unwrap());
        assert!(
            !same_calculation_program(&before, &after.replace("f:mode='a'", "f:mode='b'")).unwrap()
        );
    }

    #[test]
    fn context_hashes_all_parts_and_excludes_only_actual_result_roots_not_names() {
        let xml = worksheet(&math("1", 0., "2"));
        let results = format!("<resultsList xmlns='{RESULTS}'/>");
        let a = package(
            &xml,
            &[
                ("mathcad/result.xml", results.as_bytes()),
                ("calc/settings.xml", b"<settings origin='0'/>"),
                ("text/flow.xml", b"<FlowDocument/>"),
                ("custom/data.bin", b"\x00\x01\x02"),
                ("custom/result.xml", b"<other/>"),
                ("custom/results.data", results.as_bytes()),
            ],
        );
        let snap = ContextSnapshot::capture(&a).unwrap();
        assert!(snap.matches(&a).unwrap());
        let mut result_changed = NativePackage::from_bytes(&a.to_bytes().unwrap()).unwrap();
        result_changed
            .set_part(
                "mathcad/result.xml",
                format!("<resultsList xmlns='{RESULTS}'><resultData result-id='1'/></resultsList>")
                    .into_bytes(),
            )
            .unwrap();
        assert!(snap.matches(&result_changed).unwrap());
        for (name, bytes) in [
            ("calc/settings.xml", b"<settings origin='1'/>".as_slice()),
            (
                "text/flow.xml",
                b"<FlowDocument><math/></FlowDocument>".as_slice(),
            ),
            ("custom/data.bin", b"\x00\x01\x03".as_slice()),
            ("custom/result.xml", b"<changed/>".as_slice()),
        ] {
            let mut b = NativePackage::from_bytes(&a.to_bytes().unwrap()).unwrap();
            b.set_part(name, bytes.to_vec()).unwrap();
            assert!(!snap.matches(&b).unwrap(), "{name}");
        }
        assert_eq!(snap.parts.len(), 4);
    }

    #[test]
    fn malformed_duplicate_ids_and_depth_limits_cannot_produce_a_preservation_proof() {
        let valid = worksheet(&math("1", 0., "2"));
        for invalid in [
            "<worksheet>".into(),
            worksheet(&(math("1", 0., "2") + &math("01", 2., "3"))),
            worksheet(&math("mathcraft-area-new-1", 0., "2")),
            valid.replace("top='0'", "top='NaN'"),
        ] {
            assert!(same_calculation_program(&invalid, &invalid).is_err());
        }
        let deeply_nested = format!(
            "{}<ml:real>2</ml:real>{}",
            "<ml:apply>".repeat(MAX_DEPTH + 1),
            "</ml:apply>".repeat(MAX_DEPTH + 1)
        );
        let deep = valid.replace("<ml:real>2</ml:real>", &deeply_nested);
        assert!(same_calculation_program(&deep, &deep).is_err());
        assert!(same_calculation_program(&" ".repeat(MAX_XML + 1), &valid).is_err());
    }

    #[test]
    fn unknown_region_collection_and_empty_area_metadata_are_never_silently_omitted() {
        let before = worksheet(&math("1", 0., "2"));
        let collection = before.replace(
            "<regions>",
            "<regions xmlns:f='urn:future' f:mode='a'><f:calculation/>",
        );
        assert!(!same_calculation_program(&before, &collection).unwrap());
        assert!(
            !same_calculation_program(&collection, &collection.replace("f:mode='a'", "f:mode='b'"))
                .unwrap()
        );
        let future_text = "<region xmlns:f='urn:future' region-id='2' left='0' top='1' f:calculation='a'><text item-idref='rId7'/></region>";
        assert!(
            !same_calculation_program(&before, &worksheet(&(math("1", 0., "2") + future_text)))
                .unwrap()
        );
        let future_area = "<region xmlns:f='urn:future' region-id='2' left='0' top='1' f:calculation='a'><Area><regions/></Area></region>";
        assert!(
            !same_calculation_program(&before, &worksheet(&(math("1", 0., "2") + future_area)))
                .unwrap()
        );
    }

    #[test]
    fn context_classifies_utf16_result_roots_without_ignoring_new_unrelated_parts() {
        let worksheet = worksheet(&math("1", 0., "2"));
        let result = format!("<resultsList xmlns='{RESULTS}'/>");
        let mut utf16 = vec![0xff, 0xfe];
        utf16.extend(result.encode_utf16().flat_map(u16::to_le_bytes));
        let mut package = package(
            &worksheet,
            &[
                ("custom/result.data", &utf16),
                ("custom/blob.bin", b"unchanged"),
            ],
        );
        let snapshot = ContextSnapshot::capture(&package).unwrap();
        assert_eq!(snapshot.parts.len(), 1);
        package
            .set_part(
                "custom/result.data",
                format!("<resultsList xmlns='{RESULTS}'><resultData result-id='1'/></resultsList>")
                    .into_bytes(),
            )
            .unwrap();
        assert!(snapshot.matches(&package).unwrap());
        package.set_part("custom/new.bin", b"new".to_vec()).unwrap();
        assert!(!snapshot.matches(&package).unwrap());
    }
}
