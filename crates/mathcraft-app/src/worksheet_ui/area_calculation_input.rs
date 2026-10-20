//! Private IDs join an opaque Area's local rows to their owning canvas region.
//! Persisted/native XML keeps original IDs, paths and local coordinates.
use super::*;
const WS: &str = "http://schemas.mathsoft.com/worksheet50";

#[derive(Clone, Debug)]
pub(super) struct ChildAddress {
    pub owner: u64,
    pub key: String,
}
#[derive(Clone, Debug, Default)]
pub(super) struct AreaIndex {
    pub children: HashMap<String, ChildAddress>,
    pub tails: HashMap<u64, String>,
}
impl AreaIndex {
    pub fn owner(&self, id: &str) -> Option<u64> {
        self.children.get(id).map(|address| address.owner)
    }
    pub fn target(&self, owner: u64) -> String {
        self.tails
            .get(&owner)
            .cloned()
            .unwrap_or_else(|| owner.to_string())
    }
    pub fn child_target(&self, owner: u64, key: &str) -> Option<String> {
        self.children
            .iter()
            .find(|(_, address)| address.owner == owner && address.key == key)
            .map(|(id, _)| id.clone())
    }
    pub fn append(&mut self, other: Self) {
        self.children.extend(other.children);
        self.tails.extend(other.tails);
    }
}
fn path(mut node: roxmltree::Node<'_, '_>) -> Vec<usize> {
    let mut path = vec![];
    while let Some(parent) = node.parent_element() {
        path.push(
            parent
                .children()
                .filter(roxmltree::Node::is_element)
                .position(|child| child == node)
                .unwrap(),
        );
        node = parent;
    }
    path.reverse();
    path
}
fn known_area(area: roxmltree::Node<'_, '_>) -> bool {
    area.attributes().all(|attr| {
        attr.namespace().is_none()
            && match attr.name() {
                "disable-calc" => matches!(attr.value(), "false" | "0"),
                "IsCollapsed" | "IsBorderVisible" => {
                    matches!(attr.value(), "true" | "false" | "0" | "1")
                }
                "AreaHeight" => attr
                    .value()
                    .parse::<f64>()
                    .is_ok_and(|v| v.is_finite() && v >= 0.),
                _ => false,
            }
    }) && {
        let children: Vec<_> = area
            .children()
            .filter(roxmltree::Node::is_element)
            .collect();
        children.len() == 1
            && children[0].has_tag_name((WS, "regions"))
            && children[0].attributes().len() == 0
            && children[0]
                .children()
                .filter(roxmltree::Node::is_element)
                .all(|node| node.has_tag_name((WS, "region")))
            && !children[0]
                .descendants()
                .any(|node| node.has_tag_name((WS, "Area")))
    }
}

/// Generate only the calculator copy. Unsupported Area wrappers remain one
/// original owner row; the compatibility evaluator supplies its exact fault.
pub(super) fn calculation_region(
    xml: &str,
    owner: u64,
    left: f64,
    top: f64,
) -> Result<(String, AreaIndex), String> {
    if xml.len() > 8 * 1024 * 1024 || !left.is_finite() || !top.is_finite() {
        return Err("Invalid Area calculation input".into());
    }
    let document = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
    let region = document.root_element();
    if !region.has_tag_name((WS, "region")) {
        return Err("Expected owning Area region".into());
    }
    let content: Vec<_> = region
        .children()
        .filter(roxmltree::Node::is_element)
        .collect();
    let area = content
        .iter()
        .copied()
        .find(|node| node.has_tag_name((WS, "Area")))
        .ok_or("Missing Area")?;
    let mut index = AreaIndex::default();
    index.tails.insert(owner, owner.to_string());
    let mut patches = vec![];
    if content.len() == 1 && known_area(area) {
        let regions = area.first_element_child().unwrap();
        let mut order = vec![];
        for child in regions.children().filter(roxmltree::Node::is_element) {
            let payload: Vec<_> = child
                .children()
                .filter(roxmltree::Node::is_element)
                .collect();
            if !payload.is_empty()
                && payload
                    .iter()
                    .all(|n| n.has_tag_name((WS, "text")) || n.has_tag_name((WS, "image")))
            {
                continue;
            }
            if payload.len() == 1
                && matches!(payload[0].attribute("disable-calc"), Some("true" | "1"))
                && payload[0].tag_name().namespace() == Some(WS)
                && matches!(
                    payload[0].tag_name().name(),
                    "math"
                        | "spec-table"
                        | "solveblock"
                        | "plot"
                        | "include"
                        | "mathBoard"
                        | "excelComponent"
                )
            {
                continue;
            }
            let coordinate = |name| -> Result<f64, String> {
                let value = child
                    .attribute(name)
                    .ok_or_else(|| format!("Missing Area child {name}"))?
                    .parse::<f64>()
                    .map_err(|_| format!("Invalid Area child {name}"))?;
                value
                    .is_finite()
                    .then_some(value)
                    .ok_or_else(|| format!("Nonfinite Area child {name}"))
            };
            let local_left = coordinate("left")?;
            let local_top = coordinate("top")?;
            let child_path = path(child);
            let key = serde_json::to_string(&child_path).unwrap();
            let id = format!(
                "area-{owner}-{}",
                child_path
                    .iter()
                    .map(usize::to_string)
                    .collect::<Vec<_>>()
                    .join("-")
            );
            let attribute = child
                .attribute_node("region-id")
                .ok_or("Area child region ID missing")?;
            patches.push((attribute.range_value(), id.clone()));
            index
                .children
                .insert(id.clone(), ChildAddress { owner, key });
            order.push((local_top, local_left, id));
            if order.len() > 10_000 {
                return Err("Area child budget exceeded".into());
            }
        }
        order.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        if let Some((_, _, id)) = order.last() {
            index.tails.insert(owner, id.clone());
        } else {
            index.tails.remove(&owner);
        }
    }
    let mut source = xml.to_owned();
    for (range, id) in patches.into_iter().rev() {
        source.replace_range(range, &id);
    }
    let changed = roxmltree::Document::parse(&source).map_err(|e| e.to_string())?;
    let mut payload = String::new();
    // Preserve mixed/unknown siblings too, so a rejected wrapper cannot become
    // a valid partial program simply by being copied to the evaluator.
    for node in changed
        .root_element()
        .children()
        .filter(roxmltree::Node::is_element)
    {
        payload.push_str(&standalone(node, &source));
    }
    Ok((
        format!("<region region-id=\"{owner}\" left=\"{left}\" top=\"{top}\">{payload}</region>"),
        index,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source(flags: &str) -> String {
        format!(
            r#"<region xmlns="{WS}" xmlns:m="http://schemas.mathsoft.com/math50" region-id="7" left="1" top="2"><Area {flags}><regions><region region-id="1" left="0" top="500"><math resultRef="9"><m:define><m:id>x</m:id><m:real>3</m:real></m:define></math></region><region region-id="2" left="0" top="0"><math resultRef="10"><m:define><m:id>y</m:id><m:real>2</m:real></m:define></math></region></regions></Area></region>"#
        )
    }
    #[test]
    fn private_ids_keep_local_order_and_do_not_modify_native_source() {
        let original = source("IsCollapsed=\"true\"");
        let (region, index) = calculation_region(&original, 42, 0., 0.).unwrap();
        assert_eq!(index.children.len(), 2);
        assert!(original.contains("region-id=\"1\""));
        assert!(region.contains("top=\"500\""));
        let worksheet = worksheet_math::worksheet_xml_with_areas(&[], &[region]).unwrap();
        let rows = mathcraft_compat::evaluate_worksheet(&worksheet).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(
            rows.iter().all(|row| row.error.is_none()),
            "{rows:?}\n{worksheet}"
        );
        assert_eq!(index.target(42), rows[1].region_id);
        assert!(
            rows.iter()
                .all(|row| index.owner(&row.region_id) == Some(42))
        );
        assert_eq!(index.children[&rows[0].region_id].key, "[0,0,1]");
    }
    #[test]
    fn unknown_parent_flag_remains_one_owner_fault_without_executing_children() {
        let (region, index) =
            calculation_region(&source("disable-calc=\"true\""), 42, 0., 0.).unwrap();
        assert!(index.children.is_empty());
        assert_eq!(index.target(42), "42");
        let worksheet = worksheet_math::worksheet_xml_with_areas(&[], &[region]).unwrap();
        let rows = mathcraft_compat::evaluate_worksheet(&worksheet).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].region_id, "42");
        assert!(rows[0].error.is_some());
    }
    #[test]
    fn actual_app_area_children_supply_bindings_before_the_following_canvas_query() {
        let mut app = WorksheetApp::default();
        let area = app.document.add_math(0., 0.);
        app.document.regions.last_mut().unwrap().content = CanvasContent::Opaque {
            kind: "Area".into(),
            xml: source("IsCollapsed=\"true\""),
        };
        let query = app.document.add_math(0., 50.);
        let tree = PortableTree {
            kind: NodeKind::Binary(BinaryOp::Evaluate),
            children: vec![
                PortableTree {
                    kind: NodeKind::Binary(BinaryOp::Add),
                    children: vec![
                        PortableTree {
                            kind: NodeKind::Identifier("x".into()),
                            children: vec![],
                        },
                        PortableTree {
                            kind: NodeKind::Identifier("y".into()),
                            children: vec![],
                        },
                    ],
                },
                PortableTree {
                    kind: NodeKind::Placeholder,
                    children: vec![],
                },
            ],
        };
        app.document.regions.last_mut().unwrap().content = CanvasContent::Math(tree);
        app.recalculate();
        assert!(
            app.calculation_error.is_none(),
            "{:?}",
            app.calculation_error
        );
        let result = &app.results[&query];
        assert!(result.error.is_none(), "{result:?}");
        let Some(Value::Scalar { quantity }) = &result.value else {
            panic!("{result:?}")
        };
        assert_eq!(quantity.value.re, 5.);
        assert_eq!(app.area_results[&area].len(), 2);
        assert!(!app.pending_regions.contains(&area));
        // Presentation toggles cannot execute the child definitions again.
        if let CanvasContent::Opaque { xml, .. } = &mut app.document.regions[0].content {
            *xml = xml.replace("IsCollapsed=\"true\"", "IsCollapsed=\"false\"");
        }
        app.last_executed_regions.clear();
        app.calculate_command(CalculationCommand::Automatic);
        assert!(app.last_executed_regions.is_empty());
        assert_eq!(app.area_results[&area].len(), 2);
    }
}
