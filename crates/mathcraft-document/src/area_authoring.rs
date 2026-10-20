//! Source construction for empty Areas and newly authored direct Math children.
//! These are owning fragments, not flat Canvas child regions. Native export must
//! allocate package-global IDs/resultRefs; the returned child ID is source-local.
//! Original static contract: docs/analysis/0138-area-authoring-contract.md.
use crate::area::{area_math_children_xml, areas_from_xml};
use mathcraft_compat::editor_bridge::{tree_from_mathml, tree_to_mathml};
use roxmltree::{Document, Node};

const WS: &str = "http://schemas.mathsoft.com/worksheet50";
const MAX_XML: usize = 8 * 1024 * 1024;
/// Reserved for children not yet allocated a package-global native identity.
pub const NEW_CHILD_ID_PREFIX: &str = "mathcraft-area-new-";

#[derive(Clone, Debug, PartialEq)]
pub struct AddedAreaMath {
    pub xml: String,
    /// Element-child path relative to the authoritative owning region fragment.
    pub region_path: Vec<usize>,
    /// Unique within this owning source, not an allocated native package ID.
    pub region_id: String,
}

/// Create an empty Area at the supplied live major-grid height. `AreaHeight`
/// retains expanded height during collapse; a new expanded Area leaves it at
/// its original default zero. Canvas owns outer location and export owner ID.
pub fn empty_area_xml(height: f64) -> Result<String, String> {
    if !height.is_finite() || height <= 0. {
        return Err("Area height must be finite and positive".into());
    }
    let xml = format!(
        "<region xmlns=\"{WS}\" region-id=\"0\" left=\"0\" top=\"0\" height=\"{height}\"><Area IsCollapsed=\"false\" IsBorderVisible=\"true\"><regions/></Area></region>"
    );
    areas_from_xml(&xml)?;
    Ok(xml)
}

/// Current explicit owning-region height, then measured actualHeight, then
/// retained AreaHeight. Read once per source update, not per frame. A fresh
/// expanded Area has height=major-grid and retained height zero; no measured
/// actualHeight is fabricated before layout.
pub fn area_display_height_xml(xml: &str) -> Result<f64, String> {
    let metadata = areas_from_xml(xml)?;
    let area = metadata
        .iter()
        .find(|area| area.region_path.is_empty())
        .ok_or("Owning Area missing")?;
    let document = parse(xml)?;
    let current = document
        .root_element()
        .attribute("height")
        .or_else(|| document.root_element().attribute("actualHeight"))
        .map(|raw| raw.parse::<f64>().map_err(|_| "Invalid Area height"))
        .transpose()?;
    let height = current.unwrap_or(area.area_height);
    if !height.is_finite() || height < 0. {
        return Err("Invalid Area height".into());
    }
    Ok(height)
}

fn parse(xml: &str) -> Result<Document<'_>, String> {
    if xml.len() > MAX_XML {
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

fn append_to(xml: &str, node: Node<'_, '_>, child: &str) -> Result<String, String> {
    let range = node.range();
    let raw = &xml[range.clone()];
    let replacement = if let Some(open) = raw.strip_suffix("/>") {
        let name_end = raw[1..]
            .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
            .map(|end| end + 1)
            .ok_or("Missing Area XML element name")?;
        format!("{open}>{child}</{}>", &raw[1..name_end])
    } else {
        let close = raw.rfind("</").ok_or("Missing Area XML end tag")?;
        format!("{}{child}{}", &raw[..close], &raw[close..])
    };
    if xml.len() - raw.len() + replacement.len() > MAX_XML {
        return Err("Area XML exceeds size limit".into());
    }
    Ok(format!(
        "{}{}{}",
        &xml[..range.start],
        replacement,
        &xml[range.end..]
    ))
}

/// Append one supported expression, preserving existing child bytes and caches.
/// A newly typed child may be a placeholder: no saved result/ref is fabricated.
/// Position snapping belongs to the Area input controller, not this XML helper.
pub fn append_math_xml(xml: &str, local: [f64; 2], mathml: &str) -> Result<AddedAreaMath, String> {
    if !local.into_iter().all(f64::is_finite) {
        return Err("Area child location must be finite".into());
    }
    if mathml.len() > 1024 * 1024 {
        return Err("Area math exceeds size limit".into());
    }
    let tree = tree_from_mathml(mathml)?;
    let expression = crate::export::bind_math(&tree_to_mathml(&tree)?)?;
    let areas = areas_from_xml(xml)?;
    if areas.len() != 1 || !areas[0].region_path.is_empty() {
        return Err("Area authoring requires one unnested owning Area fragment".into());
    }
    if areas[0].protected {
        return Err("Protected Area child changes are unsupported".into());
    }
    if areas[0].is_collapsed {
        return Err("Collapsed Area children are not visible".into());
    }
    // Validate existing direct math before making any candidate source changes.
    area_math_children_xml(xml, &[])?;
    let document = parse(xml)?;
    let area = document
        .root_element()
        .children()
        .find(|n| n.has_tag_name((WS, "Area")))
        .ok_or("Owning Area missing")?;
    let collections = area
        .children()
        .filter(|n| n.has_tag_name((WS, "regions")))
        .collect::<Vec<_>>();
    if collections.len() > 1 {
        return Err("Area contains multiple child collections".into());
    }
    let mut seen = std::collections::HashSet::new();
    let mut next = 1_u64;
    if let Some(collection) = collections.first() {
        for region in collection
            .children()
            .filter(|n| n.has_tag_name((WS, "region")))
        {
            let id = region
                .attribute("region-id")
                .ok_or("Existing Area child has no source ID")?;
            if !seen.insert(id) {
                return Err("Duplicate Area child source ID".into());
            }
            if let Some(suffix) = id.strip_prefix(NEW_CHILD_ID_PREFIX) {
                let suffix = suffix
                    .parse::<u64>()
                    .map_err(|_| "Invalid new Area source ID")?;
                if suffix == 0 {
                    return Err("Invalid new Area source ID".into());
                }
                next = next.max(
                    suffix
                        .checked_add(1)
                        .ok_or("Area source ID space exhausted")?,
                );
            }
        }
    }
    let region_id = format!("{NEW_CHILD_ID_PREFIX}{next}");
    let child = format!(
        "<region xmlns=\"{WS}\" region-id=\"{region_id}\" left=\"{}\" top=\"{}\"><math>{expression}</math></region>",
        local[0], local[1]
    );
    let candidate = if let Some(collection) = collections.first() {
        append_to(xml, *collection, &child)?
    } else {
        append_to(
            xml,
            area,
            &format!("<regions xmlns=\"{WS}\">{child}</regions>"),
        )?
    };
    let created = area_math_children_xml(&candidate, &[])?
        .into_iter()
        .find(|source| source.region_id.as_deref() == Some(region_id.as_str()))
        .ok_or("New Area child was not preserved by source validation")?;
    Ok(AddedAreaMath {
        xml: candidate,
        region_path: created.region_path,
        region_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const REAL: &str = "<ml:real xmlns:ml='http://schemas.mathsoft.com/math50'>12</ml:real>";
    const PH: &str = "<ml:placeholder xmlns:ml='http://schemas.mathsoft.com/math50'/>";

    #[test]
    fn empty_area_has_no_child_or_copied_math_and_append_uses_local_source_identity() {
        let source = empty_area_xml(2. * 96. / 2.54).unwrap();
        let area = areas_from_xml(&source).unwrap().remove(0);
        assert!(!area.is_collapsed && area.is_border_visible && !area.protected);
        assert_eq!(area.area_height, 0.);
        assert_eq!(
            parse(&source)
                .unwrap()
                .root_element()
                .attribute("height")
                .unwrap()
                .parse::<f64>()
                .unwrap(),
            2. * 96. / 2.54
        );
        assert!(area.child_region_paths.is_empty());
        let added = append_math_xml(&source, [19.2, 19.2], PH).unwrap();
        assert_eq!(added.region_path, [0, 0, 0]);
        assert_eq!(added.region_id, "mathcraft-area-new-1");
        let child = area_math_children_xml(&added.xml, &[]).unwrap().remove(0);
        assert_eq!([child.left, child.top], [19.2, 19.2]);
        assert_eq!(child.result_ref, None);
        assert!(child.mathml.contains("placeholder"));
        assert!(area_math_children_xml(&source, &[]).unwrap().is_empty());
    }

    #[test]
    fn append_preserves_unknown_siblings_original_identity_caches_and_area_state() {
        let old = "<w:region region-id='27' left='9.6' top='38.4'><w:math resultRef='8' disable-calc='true'><ml:real>7</ml:real></w:math></w:region>";
        let source = format!(
            "<w:region xmlns:w='{WS}' xmlns:ml='http://schemas.mathsoft.com/math50' xmlns:f='urn:future' region-id='owner' left='100' top='200'><w:Area AreaHeight='96' IsBorderVisible='false' f:keep='yes'><w:regions><!--keep-->{old}<w:region region-id='future' left='0' top='0'><f:future exact='yes'/></w:region></w:regions></w:Area></w:region>"
        );
        let added = append_math_xml(&source, [38.4, 57.6], REAL).unwrap();
        assert!(added.xml.contains(old));
        assert!(added.xml.contains("<!--keep-->"));
        assert!(added.xml.contains("<f:future exact='yes'/>"));
        assert!(added.xml.contains("IsBorderVisible='false' f:keep='yes'"));
        assert_eq!(added.region_id, "mathcraft-area-new-1");
        assert_eq!(added.region_path, [0, 0, 2]);
        assert_eq!(area_math_children_xml(&added.xml, &[]).unwrap().len(), 2);
    }

    #[test]
    fn missing_or_self_closing_collections_accept_append_without_namespace_assumptions() {
        for body in ["<a:Area/>", "<a:Area><a:regions /></a:Area>"] {
            let source = format!("<a:region xmlns:a='{WS}'>{body}</a:region>");
            let added = append_math_xml(&source, [0., 9.6], REAL).unwrap();
            assert_eq!(added.region_path, [0, 0, 0]);
            assert_eq!(area_math_children_xml(&added.xml, &[]).unwrap().len(), 1);
        }
    }

    #[test]
    fn protected_collapsed_nested_and_ambiguous_sources_reject_without_mutation() {
        for body in [
            "<Area><ProtectionData/></Area>",
            "<Area IsCollapsed='true'/>",
            "<Area><regions><region region-id='1'><Area/></region></regions></Area>",
            "<Area><regions/><regions/></Area>",
        ] {
            let source = format!("<region xmlns='{WS}'>{body}</region>");
            let before = source.clone();
            assert!(append_math_xml(&source, [19.2, 19.2], REAL).is_err());
            assert_eq!(source, before);
        }
    }

    #[test]
    fn invalid_height_coordinates_math_and_source_ids_are_atomic_errors() {
        for height in [0., -1., f64::NAN, f64::INFINITY] {
            assert!(empty_area_xml(height).is_err());
        }
        let source = empty_area_xml(96.).unwrap();
        assert!(append_math_xml(&source, [f64::NAN, 19.2], REAL).is_err());
        assert!(append_math_xml(&source, [19.2, 19.2], "<alien/>").is_err());
        for ids in [
            vec!["1", "1"],
            vec!["mathcraft-area-new-18446744073709551615"],
            vec![""],
        ] {
            let children = ids
                .iter()
                .map(|id| {
                    format!(
                        "<region {} left='0' top='1'><unknown/></region>",
                        if id.is_empty() {
                            String::new()
                        } else {
                            format!("region-id='{id}'")
                        }
                    )
                })
                .collect::<String>();
            let invalid =
                format!("<region xmlns='{WS}'><Area><regions>{children}</regions></Area></region>");
            assert!(append_math_xml(&invalid, [0., 1.], REAL).is_err());
        }
    }

    #[test]
    fn appended_markers_are_distinct_and_never_repurpose_original_numeric_ids() {
        let source = format!(
            "<region xmlns='{WS}' height='96' actualHeight='90'><Area AreaHeight='120'><regions><region region-id='18446744073709551615' left='0' top='9.6'><unknown/></region></regions></Area></region>"
        );
        assert_eq!(area_display_height_xml(&source).unwrap(), 96.);
        let first = append_math_xml(&source, [19.2, 19.2], REAL).unwrap();
        let second = append_math_xml(&first.xml, [19.2, 38.4], PH).unwrap();
        assert_eq!(first.region_id, "mathcraft-area-new-1");
        assert_eq!(second.region_id, "mathcraft-area-new-2");
        assert_eq!(second.region_path, [0, 0, 2]);
        assert!(second.xml.contains("region-id='18446744073709551615'"));
        let measured_only = source.replace(" height='96'", "");
        assert_eq!(area_display_height_xml(&measured_only).unwrap(), 90.);
        let retained_only = measured_only.replace(" actualHeight='90'", "");
        assert_eq!(area_display_height_xml(&retained_only).unwrap(), 120.);
    }
}
