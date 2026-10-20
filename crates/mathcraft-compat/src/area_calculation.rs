//! Area children execute at their owning container's sibling slot, never by
//! globally adding local coordinates. Static contract: analysis/0132 and 0133.
use super::{Fault, PlacementGrid, WS, children};
use roxmltree::Node;

pub(super) struct PositionedRegion<'a, 'i> {
    pub region: Node<'a, 'i>,
    pub left: f64,
    pub top: f64,
    /// Top-level slot for coordinate-based external queries (e.g. plot context).
    pub context_position: (f64, f64),
    pub in_area: bool,
    pub fault: Option<Fault>,
}

fn coordinate(
    region: Node<'_, '_>,
    name: &str,
    grid: Option<PlacementGrid>,
) -> Result<f64, String> {
    let value: f64 = region
        .attribute(name)
        .ok_or_else(|| format!("Missing {name}"))?
        .parse()
        .map_err(|_| format!("Invalid {name}"))?;
    if !value.is_finite() {
        return Err(format!("Nonfinite {name}"));
    }
    grid.map_or(Ok(value), |grid| grid.snap(value))
}

fn area_children<'a, 'i>(area: Node<'a, 'i>) -> Result<Node<'a, 'i>, Fault> {
    for attr in area.attributes() {
        if attr.namespace().is_some() {
            return Err(Fault::unsupported("Area execution metadata"));
        }
        match attr.name() {
            "disable-calc" => {
                if !matches!(attr.value(), "false" | "0") {
                    return Err(Fault::unsupported("Area parent calculation flag semantics"));
                }
            }
            "IsCollapsed" | "IsBorderVisible" => {
                if !matches!(attr.value(), "true" | "1" | "false" | "0") {
                    return Err(Fault::unsupported("Invalid Area display flag"));
                }
            }
            "AreaHeight" => {
                if !attr
                    .value()
                    .parse::<f64>()
                    .is_ok_and(|v| v.is_finite() && v >= 0.)
                {
                    return Err(Fault::unsupported("Invalid Area height"));
                }
            }
            _ => return Err(Fault::unsupported("Area execution metadata")),
        }
    }
    let content = children(area);
    let [regions] = content.as_slice() else {
        return Err(Fault::unsupported("Protected or unknown Area content"));
    };
    if !regions.has_tag_name((WS, "regions"))
        || regions.attributes().len() != 0
        || regions
            .children()
            .filter(Node::is_element)
            .any(|n| !n.has_tag_name((WS, "region")))
    {
        return Err(Fault::unsupported("Invalid Area regions"));
    }
    if regions.descendants().any(|n| n.has_tag_name((WS, "Area"))) {
        return Err(Fault::unsupported("Nested Areas are unsupported"));
    }
    Ok(*regions)
}

pub(super) fn ordered_regions<'a, 'i>(
    regions: Node<'a, 'i>,
    grid: Option<PlacementGrid>,
    hierarchical: bool,
) -> Result<Vec<PositionedRegion<'a, 'i>>, String> {
    fn collect<'a, 'i>(
        regions: Node<'a, 'i>,
        grid: Option<PlacementGrid>,
        anchor: Option<(f64, f64)>,
        out: &mut Vec<PositionedRegion<'a, 'i>>,
        visited: &mut usize,
        hierarchical: bool,
    ) -> Result<(), String> {
        let mut siblings = Vec::new();
        for region in regions.children().filter(Node::is_element) {
            let content = children(region);
            if !content.is_empty()
                && content
                    .iter()
                    .all(|n| n.has_tag_name((WS, "text")) || n.has_tag_name((WS, "image")))
            {
                continue;
            }
            if content.len() == 1
                && matches!(content[0].attribute("disable-calc"), Some("true" | "1"))
                && (content[0].has_tag_name((WS, "spec-table"))
                    || (anchor.is_some()
                        && matches!(
                            content[0].tag_name().name(),
                            "math"
                                | "solveblock"
                                | "plot"
                                | "include"
                                | "mathBoard"
                                | "excelComponent"
                        )
                        && content[0].tag_name().namespace() == Some(WS)))
            {
                continue;
            }
            // Containers count toward the bounded traversal too, including empty Areas.
            if *visited >= 10000 {
                return Err("Region budget exceeded".into());
            }
            *visited += 1;
            let left = coordinate(region, "left", grid)?;
            let top = coordinate(region, "top", grid)?;
            siblings.push(PositionedRegion {
                region,
                left,
                top,
                context_position: anchor.unwrap_or((left, top)),
                in_area: anchor.is_some(),
                fault: None,
            });
        }
        // Stable overlap order remains our explicit independent policy.
        if hierarchical {
            siblings.sort_by(|a, b| a.top.total_cmp(&b.top).then(a.left.total_cmp(&b.left)));
        }
        for mut positioned in siblings {
            let content = children(positioned.region);
            if content.iter().any(|node| node.has_tag_name((WS, "Area"))) {
                if content.len() != 1 {
                    positioned.fault = Some(Fault::unsupported("Area region has mixed content"));
                } else {
                    match area_children(content[0]) {
                        Ok(local) => {
                            collect(
                                local,
                                grid,
                                Some(positioned.context_position),
                                out,
                                visited,
                                true,
                            )?;
                            continue;
                        }
                        Err(fault) => positioned.fault = Some(fault),
                    }
                }
            }
            if out.len() >= 10000 {
                return Err("Region budget exceeded".into());
            }
            out.push(positioned);
        }
        Ok(())
    }
    let mut out = Vec::new();
    collect(regions, grid, None, &mut out, &mut 0, hierarchical)?;
    Ok(out)
}
