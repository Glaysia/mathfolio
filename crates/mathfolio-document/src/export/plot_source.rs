//! Source-bound XY export. Never simplify an unsupported native plot.
use super::*;
use crate::canvas::PlotSpec;
fn traces<'a, 'input>(doc: &'a Document<'input>) -> Vec<Node<'a, 'input>> {
    doc.descendants()
        .filter(|node| node.has_tag_name((WS, "trace")))
        .collect()
}
fn spec(region: &CanvasRegion) -> Result<&PlotSpec, String> {
    match &region.content {
        CanvasContent::Plot(spec) => Ok(spec),
        _ => Err("Expected editable plot source".into()),
    }
}
pub(super) fn fresh(region: &CanvasRegion, ids: &mut ResultIds) -> Result<String, String> {
    let payload = crate::native_plot::payload(spec(region)?)?;
    let parsed = parse(&payload)?;
    let mut changes = attrs(
        parsed.root_element(),
        &payload,
        &[(
            "disable-calc",
            region.calculation_disabled.then(|| "true".into()),
        )],
    )?;
    let mut references = Vec::new();
    for trace in traces(&parsed) {
        let id = ids.allocate()?;
        references.push(id);
        changes.extend(attrs(
            trace,
            &payload,
            &[("resultRef", Some(id.to_string()))],
        )?);
    }
    if references.is_empty() {
        return Err("Native plot has no supported trace".into());
    }
    ids.plots.insert(region.id, references);
    apply(payload, changes)
}
pub(super) fn edit(
    node: Node<'_, '_>,
    source: &str,
    region: &CanvasRegion,
    ids: &mut ResultIds,
) -> Result<Patches, String> {
    let original = crate::spec_table::standalone(node, source)?;
    let previous = crate::native_plot::parse(&original)?;
    let plot = node
        .children()
        .find(|node| node.has_tag_name((WS, "plot")))
        .ok_or("Original native plot missing")?;
    if !crate::native_plot::matches_retained_source(spec(region)?, &previous)? {
        let mut patches = vec![(plot.range(), fresh(region, ids)?)];
        if spec(region)?.native_geometry.is_some() {
            patches.extend(geometry_patches(node, source, spec(region)?)?);
        } else {
            patches.extend(attrs(
                node,
                source,
                &[("actualWidth", None), ("actualHeight", None)],
            )?);
        }

        return Ok(patches);
    }
    let mut patches = Vec::new();
    let mut references = Vec::new();
    for trace in plot
        .descendants()
        .filter(|node| node.has_tag_name((WS, "trace")))
    {
        let reference = match trace.attribute("resultRef") {
            Some(reference) => crate::native::parse_result_id(reference)?,
            None => {
                let reference = ids.allocate()?;
                patches.extend(attrs(
                    trace,
                    source,
                    &[("resultRef", Some(reference.to_string()))],
                )?);
                reference
            }
        };
        references.push(reference);
    }
    if matches!(plot.attribute("disable-calc"), Some("true" | "1")) != region.calculation_disabled {
        patches.extend(attrs(
            plot,
            source,
            &[(
                "disable-calc",
                Some(region.calculation_disabled.to_string()),
            )],
        )?);
    }
    ids.plots.insert(region.id, references);
    Ok(patches)
}
pub(super) fn geometry_attributes(spec: &PlotSpec) -> Vec<(&'static str, Option<f64>)> {
    spec.native_geometry
        .as_ref()
        .map(|g| {
            vec![
                ("width", g.requested_width),
                ("height", g.requested_height),
                ("actualWidth", g.actual_width),
                ("actualHeight", g.actual_height),
            ]
        })
        .unwrap_or_default()
}
fn geometry_patches(node: Node<'_, '_>, source: &str, spec: &PlotSpec) -> Result<Patches, String> {
    let values: Vec<_> = geometry_attributes(spec)
        .into_iter()
        .filter(|(name, wanted)| {
            let previous = node.attribute(*name).and_then(|v| v.parse::<f64>().ok());
            previous.map(f64::to_bits) != wanted.map(f64::to_bits)
        })
        .map(|(name, value)| (name, value.map(|v| v.to_string())))
        .collect();
    attrs(node, source, &values)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_native_plot_has_real_axis_expressions_and_reserved_trace_identity() {
        let mut canvas = CanvasDocument::default();
        canvas.add_plot(20., 20.);
        let mut package = minimal_package(&canvas).unwrap();
        export_into(&canvas, &mut package).unwrap();
        let source = package.worksheet_xml().unwrap();
        let parsed = parse(&source).unwrap();
        let region = parsed
            .descendants()
            .find(|node| node.has_tag_name((WS, "region")))
            .unwrap();
        let raw = crate::spec_table::standalone(region, &source).unwrap();
        let plot = crate::native_plot::parse(&raw).unwrap();
        let pair = plot.paired_sources().next().unwrap();
        assert!(
            matches!(pair.x.expression.as_tree().unwrap().kind,mathfolio_editor::NodeKind::Identifier(ref name) if name=="x")
        );
        let y = pair.y.expression.as_tree().unwrap();
        assert!(matches!(
            y.kind,
            mathfolio_editor::NodeKind::Call { argument_count: 1 }
        ));
        assert!(
            matches!(&y.children[0].kind,mathfolio_editor::NodeKind::Identifier(name) if name=="sin")
        );
        let refs = crate::native_plot::trace_result_refs(&raw).unwrap();
        assert_eq!(refs.len(), 1);
        assert!(refs[0].is_some());
        assert_eq!(
            result_references(&package).unwrap().get(&refs[0].unwrap()),
            Some(&1)
        );
        assert_eq!(
            package.caches().unwrap()[0].calculation_status.as_deref(),
            Some("Pending")
        );
    }
    #[test]
    fn unchanged_native_plot_keeps_reference_and_changed_source_gets_a_fresh_one() {
        let mut canvas = CanvasDocument::default();
        let id = canvas.add_plot(20., 20.);
        let mut package = minimal_package(&canvas).unwrap();
        export_into(&canvas, &mut package).unwrap();
        canvas.regions[0].source_region = Some("1".into());
        let reference = package.caches().unwrap()[0].result_id.clone();
        let original = package.worksheet_xml().unwrap();
        export_into(&canvas, &mut package).unwrap();
        assert_eq!(package.worksheet_xml().unwrap(), original);
        let CanvasContent::Plot(plot) = &mut canvas.regions[0].content else {
            panic!()
        };
        *plot.primary_y_expression_mut().unwrap() = "cos(x)".into();
        export_into(&canvas, &mut package).unwrap();
        let source = package.worksheet_xml().unwrap();
        let parsed = parse(&source).unwrap();
        let owner = parsed
            .descendants()
            .find(|node| node.has_tag_name((WS, "region")))
            .unwrap();
        let raw = crate::spec_table::standalone(owner, &source).unwrap();
        let refs = crate::native_plot::trace_result_refs(&raw).unwrap();
        assert_ne!(refs[0].unwrap().to_string(), reference.unwrap());
        assert_eq!(
            crate::native_plot::parse(&raw)
                .unwrap()
                .paired_sources()
                .next()
                .unwrap()
                .y
                .expression
                .as_tree()
                .unwrap()
                .children[0]
                .kind,
            mathfolio_editor::NodeKind::Identifier("cos".into())
        );
        assert_eq!(canvas.regions[0].id, id);
    }
}
