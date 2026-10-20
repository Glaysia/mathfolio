//! Allocate package identities only for independently authored Area math.
use super::*;

pub(super) fn new_math(
    child: Node<'_, '_>,
    source: &str,
    ids: &mut ResultIds,
) -> Result<Patches, String> {
    if !child.has_tag_name((WS, "region"))
        || child.attributes().any(|a| {
            a.namespace().is_some()
                || !matches!(
                    a.name(),
                    "region-id" | "left" | "top" | "actualWidth" | "actualHeight"
                )
        })
    {
        return Err("Unsupported new Area child metadata".into());
    }
    for name in ["left", "top"] {
        if !child
            .attribute(name)
            .and_then(|s| s.parse::<f64>().ok())
            .is_some_and(f64::is_finite)
        {
            return Err("Invalid new Area child coordinates".into());
        }
    }
    let content: Vec<_> = child.children().filter(Node::is_element).collect();
    if content.len() != 1 || !content[0].has_tag_name((WS, "math")) {
        return Err("Only new Area math children are supported".into());
    }
    let math = content[0];
    if math.attributes().any(|a| {
        a.namespace().is_some() || !matches!(a.name(), "resultRef" | "disable-calc" | "error")
    }) {
        return Err("Unsupported new Area math metadata".into());
    }
    if math
        .attribute("disable-calc")
        .is_some_and(|v| !matches!(v, "true" | "false" | "1" | "0"))
    {
        return Err("Invalid new Area disable-calc".into());
    }
    let expressions: Vec<_> = math
        .children()
        .filter(|n| n.is_element() && n.tag_name().namespace() == Some(ML))
        .collect();
    if expressions.len() != 1
        || math.children().filter(Node::is_element).any(|n| {
            n.tag_name().namespace() != Some(ML)
                && !n.has_tag_name((WS, "resultFormat"))
                && !n.has_tag_name((WS, "formattingOverride"))
        })
    {
        return Err("Unsupported new Area math structure".into());
    }
    tree_from_mathml(&crate::spec_table::standalone(expressions[0], source)?)?;
    let mut patches = attrs(
        child,
        source,
        &[
            ("region-id", Some(ids.allocate_region()?)),
            ("actualWidth", None),
            ("actualHeight", None),
        ],
    )?;
    patches.extend(attrs(
        math,
        source,
        &[
            ("resultRef", Some(ids.allocate()?.to_string())),
            ("error", None),
        ],
    )?);
    Ok(patches)
}

pub(super) fn payload(xml: &str, ids: &mut ResultIds) -> Result<String, String> {
    let doc = parse(xml)?;
    let owner = doc.root_element();
    if !owner.has_tag_name((WS, "region")) {
        return Err("New Area requires its owning region".into());
    }
    if owner.attributes().any(|attribute| {
        attribute.namespace().is_some()
            || !matches!(
                attribute.name(),
                "region-id" | "left" | "top" | "height" | "actualHeight"
            )
    }) {
        return Err("Unsupported new Area owning metadata".into());
    }
    let contents: Vec<_> = owner.children().filter(Node::is_element).collect();
    if contents.len() != 1 || !contents[0].has_tag_name((WS, "Area")) {
        return Err("New Area requires one Area payload".into());
    }
    let area = contents[0];
    if area.attributes().any(|a| {
        a.namespace().is_some()
            || !matches!(a.name(), "IsCollapsed" | "IsBorderVisible" | "AreaHeight")
    }) {
        return Err("Unsupported new Area metadata".into());
    }
    let metadata = crate::area::areas_from_xml(xml)?;
    if metadata.len() != 1 || metadata[0].protected || metadata[0].declared_disable_calc {
        return Err("Protected or nested new Area is unsupported".into());
    }
    let containers: Vec<_> = area.children().filter(Node::is_element).collect();
    if containers.len() != 1
        || !containers[0].has_tag_name((WS, "regions"))
        || containers[0].attributes().len() != 0
    {
        return Err("Unsupported new Area regions container".into());
    }
    let mut patches = vec![];
    let mut seen = HashSet::new();
    for child in containers[0].children().filter(Node::is_element) {
        let id = child
            .attribute("region-id")
            .ok_or("New Area child lacks identity")?;
        if !seen.insert(id) {
            return Err("Duplicate new Area child identity".into());
        }
        patches.extend(new_math(child, xml, ids)?);
    }
    let changed = apply(xml.to_owned(), patches)?;
    let parsed = parse(&changed)?;
    crate::spec_table::standalone(
        parsed
            .root_element()
            .children()
            .find(Node::is_element)
            .unwrap(),
        &changed,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn added(xml: &str) -> String {
        crate::area_authoring::append_math_xml(
            xml,
            [19.2, 19.2],
            &format!("<ml:real xmlns:ml=\"{ML}\">7</ml:real>"),
        )
        .unwrap()
        .xml
    }
    fn make() -> (CanvasDocument, NativePackage) {
        let mut canvas = CanvasDocument::default();
        let empty = crate::area_authoring::empty_area_xml(75.59055118110236).unwrap();
        canvas.add_opaque(20., 20., added(&empty), "Area");
        let package = minimal_package(&canvas).unwrap();
        (canvas, package)
    }
    #[test]
    fn new_areas_and_children_get_unique_package_ids_and_pending_results() {
        let (mut canvas, mut package) = make();
        let xml = if let CanvasContent::Opaque { xml, .. } = &canvas.regions[0].content {
            xml.clone()
        } else {
            unreachable!()
        };
        canvas.add_opaque(20., 150., xml, "Area");
        canvas.add_math(20., 300.);
        export_into(&canvas, &mut package).unwrap();
        let regions = package.regions().unwrap();
        let ids: HashSet<_> = regions
            .iter()
            .map(|r| r.region_id.clone().unwrap())
            .collect();
        assert_eq!(ids.len(), 5);
        assert!(ids.iter().all(|id| id.parse::<u64>().is_ok()));
        let references: HashSet<_> = regions
            .iter()
            .filter_map(|r| r.math.as_ref()?.result_ref.clone())
            .collect();
        assert_eq!(references.len(), 3);
        let areas = package.areas().unwrap();
        assert_eq!(areas.len(), 2);
        assert!(
            areas
                .iter()
                .all(|area| area.area_height == 0. && area.child_region_paths.len() == 1)
        );
        let worksheet = package.worksheet_xml().unwrap();
        assert!(!worksheet.contains("mathcraft-area-new-"));
        let parsed = parse(&worksheet).unwrap();
        for area in parsed
            .descendants()
            .filter(|node| node.has_tag_name((WS, "Area")))
        {
            assert_eq!(
                area.parent_element().unwrap().attribute("height"),
                Some("75.59055118110236")
            );
        }
        let results = package
            .original_xml_part("mathcad/result.xml")
            .unwrap()
            .unwrap();
        let results = parse(&results).unwrap();
        assert_eq!(
            results
                .descendants()
                .filter(|node| node.attribute("calculation-status") == Some("Pending"))
                .count(),
            3
        );
    }
    #[test]
    fn appended_child_allocates_global_identity_without_changing_existing_child_or_ref() {
        let (canvas, mut package) = make();
        export_into(&canvas, &mut package).unwrap();
        let worksheet = package.worksheet_xml().unwrap();
        let parsed = parse(&worksheet).unwrap();
        let area = parsed
            .descendants()
            .find(|node| node.has_tag_name((WS, "Area")))
            .unwrap()
            .parent_element()
            .unwrap();
        let owner_id = area.attribute("region-id").unwrap().to_owned();
        let original = crate::spec_table::standalone(area, &worksheet).unwrap();
        let old_child = crate::area::area_math_children_xml(&original, &[])
            .unwrap()
            .remove(0);
        let mut canvas = CanvasDocument::default();
        canvas.add_opaque(20., 20., added(&original), "Area");
        canvas.regions[0].source_region = Some(owner_id);
        canvas.add_math(20., 300.);
        export_into(&canvas, &mut package).unwrap();
        let regions = package.regions().unwrap();
        let ids: HashSet<_> = regions.iter().map(|row| row.region_id.clone()).collect();
        assert_eq!(ids.len(), 4);
        let original = regions
            .iter()
            .find(|row| row.region_id == old_child.region_id)
            .unwrap();
        assert_eq!(
            original.math.as_ref().unwrap().result_ref,
            old_child.result_ref
        );
        let areas = package.areas().unwrap();
        assert_eq!(areas.len(), 1);
        assert_eq!(areas[0].child_region_paths.len(), 2);
        assert!(
            !package
                .worksheet_xml()
                .unwrap()
                .contains("mathcraft-area-new-")
        );
    }
    #[test]
    fn new_or_appended_unknown_children_and_renamed_native_ids_are_rejected() {
        let (canvas, mut package) = make();
        export_into(&canvas, &mut package).unwrap();
        let original = package.worksheet_xml().unwrap();
        let parsed = parse(&original).unwrap();
        let owner = parsed
            .descendants()
            .find(|node| node.has_tag_name((WS, "Area")))
            .unwrap()
            .parent_element()
            .unwrap();
        let fragment = crate::spec_table::standalone(owner, &original).unwrap();
        let owner_id = owner.attribute("region-id").unwrap();
        let child_id = crate::area::area_math_children_xml(&fragment, &[]).unwrap()[0]
            .region_id
            .clone()
            .unwrap();
        for fragment in [
            fragment.replace(
                &format!("region-id=\"{child_id}\""),
                "region-id=\"unknown-id\"",
            ),
            added(&fragment).replace("<math>", "<math future=\"unknown\"> "),
        ] {
            let mut canvas = CanvasDocument::default();
            canvas.add_opaque(20., 20., fragment, "Area");
            canvas.regions[0].source_region = Some(owner_id.into());
            assert!(export_into(&canvas, &mut package).is_err());
            assert_eq!(package.worksheet_xml().unwrap(), original);
        }
    }
}
