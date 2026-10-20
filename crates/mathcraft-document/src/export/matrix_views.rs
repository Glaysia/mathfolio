//! Privately constructed, source-bound result viewport patches after proof.
//! Never writes numeric results, calculation state, or a shared proof whitelist.
use super::{ML, Patches, WS, apply, attrs, context, insert_child, parse, set_xml};
use crate::{
    canvas::{CanvasContent, CanvasDocument, MatrixResultViewport},
    native::NativePackage,
};
use roxmltree::Node;
use std::collections::HashMap;

const MAX_XML: usize = 8 * 1024 * 1024;

fn owner_number(id: &str) -> Option<i128> {
    id.trim_matches([' ', '\t', '\r', '\n']).parse().ok()
}

#[derive(Default)]
pub(super) struct DeferredMatrixViews(Option<WorksheetReceipt>);

// Constructor is private; neither public wire nor a UI flag is a receipt.
struct WorksheetReceipt {
    part: String,
    source: String,
    updated: String,
}

fn one<'a, 'i>(
    mut nodes: impl Iterator<Item = Node<'a, 'i>>,
    message: &str,
) -> Result<Node<'a, 'i>, String> {
    let node = nodes.next().ok_or(message)?;
    if nodes.next().is_some() {
        return Err(message.into());
    }
    Ok(node)
}

fn selected_values(
    request: &MatrixResultViewport,
    local: &MatrixResultViewport,
) -> Vec<(&'static str, Option<String>)> {
    let mut values = Vec::new();
    if let Some(size) = request.size.filter(|value| Some(*value) != local.size) {
        values.push(("size", Some(format!("{},{}", size.rows, size.cols))));
    }
    if let Some(offset) = request.offset.filter(|value| Some(*value) != local.offset) {
        values.push(("offset", Some(format!("{},{}", offset.row, offset.col))));
    }
    if let Some(show) = request
        .show_indices
        .filter(|value| Some(*value) != local.show_indices)
    {
        values.push(("show-indices", Some(show.to_string())));
    }
    values
}

fn patches_for(
    math: Node<'_, '_>,
    source: &str,
    request: &MatrixResultViewport,
) -> Result<Patches, String> {
    // The shared reader rejects duplicate/unknown/unsupported nested matrices.
    // None is absence, not removal or an inferred effective/default value.
    let local = crate::matrix_result_format::read_local(math)?;
    let values = selected_values(request, &local);
    if values.is_empty() {
        return Ok(Vec::new());
    }
    let format = math
        .children()
        .find(|n| n.has_tag_name((WS, "resultFormat")));
    if let Some(matrix) = format.and_then(|n| n.children().find(|n| n.has_tag_name((WS, "matrix"))))
    {
        return attrs(matrix, source, &values);
    }
    let attributes = values
        .iter()
        .map(|(name, value)| {
            // All values above are generated bounded integer pairs or booleans.
            format!(" {name}=\"{}\"", value.as_deref().unwrap_or_default())
        })
        .collect::<String>();
    let matrix = format!("<matrix xmlns=\"{WS}\"{attributes}/>");
    if let Some(format) = format {
        return Ok(vec![(
            format.range(),
            insert_child(&source[format.range()], &matrix)?,
        )]);
    }
    let child = format!("<resultFormat xmlns=\"{WS}\">{matrix}</resultFormat>");
    Ok(vec![(
        math.range(),
        insert_child(&source[math.range()], &child)?,
    )])
}

/// Called after the existing project_source proof, before any cache writes.
/// The mapping/references come from that same original exporter/ID allocator.
pub(super) fn prepare(
    canvas: &CanvasDocument,
    package: &NativePackage,
    owners: &HashMap<String, u64>,
    references: &HashMap<i32, usize>,
) -> Result<DeferredMatrixViews, String> {
    if canvas.matrix_result_views.is_empty() {
        return Ok(DeferredMatrixViews::default());
    }
    canvas
        .matrix_result_views
        .validate_for_regions(&canvas.regions)?;
    if canvas
        .matrix_result_views
        .iter()
        .any(|view| view.target.region_path.is_some())
    {
        return Err("Native export of Area matrix view overrides is not implemented".into());
    }
    let source = package.worksheet_xml()?;
    if source.len() > MAX_XML {
        return Err("Matrix view source exceeds 8 MiB".into());
    }
    let doc = parse(&source)?;
    let root = doc.root_element();
    if !root.has_tag_name((WS, "worksheet")) {
        return Err("Matrix view source must be the actual worksheet".into());
    }
    let regions = one(
        root.children().filter(|n| n.has_tag_name((WS, "regions"))),
        "Matrix view source needs exactly one regions collection",
    )?;
    let regions = regions
        .children()
        .filter(|n| n.has_tag_name((WS, "region")))
        .collect::<Vec<_>>();
    let mut emitted = HashMap::<u64, Option<&str>>::new();
    for (native_id, owner) in owners {
        emitted
            .entry(*owner)
            .and_modify(|id| *id = None)
            .or_insert(Some(native_id.as_str()));
    }
    let mut native_by_id = HashMap::<&str, Option<Node<'_, '_>>>::new();
    let mut numeric_counts = HashMap::<i128, usize>::new();
    for region in &regions {
        if let Some(id) = region.attribute("region-id") {
            native_by_id
                .entry(id)
                .and_modify(|node| *node = None)
                .or_insert(Some(*region));
            if let Some(id) = owner_number(id) {
                *numeric_counts.entry(id).or_default() += 1;
            }
        }
    }
    let canvas_by_id = canvas
        .regions
        .iter()
        .map(|r| (r.id, r))
        .collect::<HashMap<_, _>>();
    let mut changes = Vec::new();
    for view in canvas.matrix_result_views.iter() {
        let native_id = emitted
            .get(&view.target.owner_id)
            .copied()
            .flatten()
            .ok_or("Missing or ambiguous emitted matrix view owner")?;
        if owner_number(native_id).is_some_and(|id| numeric_counts.get(&id) != Some(&1)) {
            return Err("Ambiguous native matrix view owner ID".into());
        }
        let region = native_by_id
            .get(native_id)
            .copied()
            .flatten()
            .ok_or("Matrix view owner does not identify one emitted region")?;
        let math = one(
            region.children().filter(Node::is_element),
            "Matrix view owner must contain exactly one Math",
        )?;
        if !math.has_tag_name((WS, "math")) {
            return Err("Matrix view owner is not Math".into());
        }
        let result_ref = crate::native::parse_result_id(
            math.attribute("resultRef")
                .ok_or("Matrix view Math has no resultRef")?,
        )?;
        if references.get(&result_ref) != Some(&1) {
            return Err("Ambiguous matrix view result reference".into());
        }
        let expression = one(
            math.children()
                .filter(|n| n.is_element() && n.tag_name().namespace() == Some(ML)),
            "Matrix view Math needs exactly one expression",
        )?;
        let (bound, _) = context(expression, &source[expression.range()]);
        let actual = mathcraft_compat::editor_bridge::tree_from_mathml(&bound)?;
        let owner = canvas_by_id
            .get(&view.target.owner_id)
            .ok_or("Matrix view canvas owner disappeared")?;
        if !matches!(&owner.content, CanvasContent::Math(tree) if tree == &actual) {
            return Err("Matrix view source does not match its canvas owner".into());
        }
        changes.extend(patches_for(math, &source, &view.viewport)?);
    }
    drop(native_by_id);
    drop(regions);
    drop(doc);
    if changes.is_empty() {
        return Ok(DeferredMatrixViews::default());
    }
    // Hold one original and one prospective worksheet, not one copy per owner.
    let updated = apply(source.clone(), changes)?;
    if updated.len() > MAX_XML {
        return Err("Matrix view output exceeds 8 MiB".into());
    }
    parse(&updated)?;
    Ok(DeferredMatrixViews(Some(WorksheetReceipt {
        part: package.worksheet_part_name().to_owned(),
        source,
        updated,
    })))
}

/// No mutation on stale/foreign source. Existing set_xml preserves UTF encoding.
pub(super) fn apply_deferred(
    package: &mut NativePackage,
    edits: DeferredMatrixViews,
) -> Result<(), String> {
    let Some(receipt) = edits.0 else {
        return Ok(());
    };
    if package.worksheet_part_name() != receipt.part || package.worksheet_xml()? != receipt.source {
        return Err("Deferred matrix view source changed after admission".into());
    }
    set_xml(package, &receipt.part, receipt.updated)
}

#[cfg(test)]
#[path = "matrix_views_tests.rs"]
mod tests;
