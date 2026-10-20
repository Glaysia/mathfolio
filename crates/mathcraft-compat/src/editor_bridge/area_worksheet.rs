//! Calculation-copy composition for Area documents. Raw plain statements are
//! labelled only after the complete hierarchical environment is available.
use super::*;
use std::ops::Range;

fn statement<'a, 'i>(math: Node<'a, 'i>, in_area: bool) -> Option<Node<'a, 'i>> {
    if math.attributes().any(|a| {
        a.namespace().is_some()
            || (a.name() != "resultRef"
                && !(in_area && a.name() == "disable-calc" && matches!(a.value(), "false" | "0")))
    }) {
        return None;
    }
    let all: Vec<_> = math.children().filter(Node::is_element).collect();
    if all.iter().any(|n| {
        n.tag_name().namespace() != Some(ML)
            && !n.has_tag_name((WS, "resultFormat"))
            && !n.has_tag_name((WS, "formattingOverride"))
    }) {
        return None;
    }
    let roots: Vec<_> = all
        .into_iter()
        .filter(|n| n.tag_name().namespace() == Some(ML))
        .collect();
    (roots.len() == 1).then(|| roots[0])
}
fn check_ml_scope(node: Node<'_, '_>) -> Result<(), String> {
    if node.descendants().filter(Node::is_element).any(|node| {
        node.lookup_namespace_uri(Some("ml"))
            .is_some_and(|uri| uri != ML)
    }) {
        return Err("Conflicting ml namespace in Area calculation source".into());
    }
    Ok(())
}
fn native_label(
    node: Node<'_, '_>,
    source: &str,
    definitions: &mut HashMap<String, String>,
) -> Result<String, String> {
    check_ml_scope(node)?;
    let mut wrapped = format!("<context xmlns:ml=\"{ML}\"");
    for ns in node.namespaces() {
        if matches!(ns.name(), Some("ml" | "xml")) {
            continue;
        }
        wrapped.push_str(" xmlns");
        if let Some(name) = ns.name() {
            wrapped.push(':');
            wrapped.push_str(name);
        }
        wrapped.push_str("=\"");
        wrapped.push_str(&escape(ns.uri()));
        wrapped.push('"');
    }
    wrapped.push('>');
    wrapped.push_str(&source[node.range()]);
    wrapped.push_str("</context>");
    let doc = roxmltree::Document::parse(&wrapped).map_err(|e| e.to_string())?;
    label_parsed_fragment(doc.root_element(), &wrapped, definitions, None)
}
fn declare(global: Node<'_, '_>, definitions: &mut HashMap<String, String>) -> Result<(), String> {
    if let Some(id) = global.first_element_child().and_then(definition_identifier) {
        let (name, _, _, subscript) = identifier_parts(id)?;
        definitions.insert(
            identifier_identity(&name, subscript.as_deref()),
            declaration_label(id)?,
        );
    }
    Ok(())
}

/// Build a calculation-only worksheet from raw statements plus complete native
/// region fragments (Areas and solve blocks). Native source is never rewritten
/// in the document. The regular cached no-Area builder is unchanged.
pub fn worksheet_xml_with_areas(
    math_regions: &[(u64, f64, f64, String)],
    native_regions: &[String],
) -> Result<String, String> {
    if native_regions.is_empty() {
        return worksheet_xml(math_regions);
    }
    worksheet_region_order(math_regions)?;
    if math_regions.len().saturating_add(native_regions.len()) > 10_000 {
        return Err("Region budget exceeded".into());
    }
    let mut source = format!("<worksheet xmlns=\"{WS}\" xmlns:ml=\"{ML}\"><regions>");
    let mut records: Vec<(Range<usize>, Option<Range<usize>>)> = Vec::new();
    for (id, left, top, xml) in math_regions {
        use std::fmt::Write;
        let start = source.len();
        write!(
            source,
            "<region region-id=\"{id}\" left=\"{left}\" top=\"{top}\">"
        )
        .unwrap();
        let context_start = source.len();
        source.push_str("<context>");
        source.push_str(xml);
        source.push_str("</context>");
        let context_end = source.len();
        source.push_str("</region>");
        records.push((start..source.len(), Some(context_start..context_end)));
        if source.len() > MAX_XML {
            return Err("Worksheet exceeds XML budget".into());
        }
    }
    for native in native_regions {
        if native.len() > MAX_XML {
            return Err("Invalid native region XML budget".into());
        }
        let start = source.len();
        source.push_str(native.trim());
        records.push((start..source.len(), None));
        if source.len() > MAX_XML {
            return Err("Worksheet exceeds XML budget".into());
        }
    }
    source.push_str("</regions></worksheet>");
    if source.len() > MAX_XML {
        return Err("Worksheet exceeds XML budget".into());
    }
    let doc = roxmltree::Document::parse(&source).map_err(|e| e.to_string())?;
    let regions = doc
        .root_element()
        .first_element_child()
        .ok_or("Missing regions")?;
    let nodes: Vec<_> = regions.children().filter(Node::is_element).collect();
    if nodes.len() != records.len() {
        return Err("Invalid native region boundaries".into());
    }
    let mut raw = HashMap::new();
    let mut ids = HashSet::new();
    for (node, (range, context)) in nodes.iter().zip(&records) {
        if !node.has_tag_name((WS, "region")) || node.range() != *range {
            return Err("Expected complete worksheet region".into());
        }
        let id = node
            .attribute("region-id")
            .ok_or("Native region ID missing")?;
        if !ids.insert(id.to_owned()) {
            return Err("Duplicate region ID".into());
        }
        if let Some(range) = context {
            let children = elements(*node)?;
            let [context] = children.as_slice() else {
                return Err("Invalid raw statement region boundary".into());
            };
            if !context.has_tag_name((WS, "context")) || context.range() != *range {
                return Err("Invalid raw statement context boundary".into());
            }
            raw.insert(node.id(), *context);
        }
    }
    let ordered = crate::area_calculation::ordered_regions(regions, None, true)?;
    let mut active_ids = HashSet::new();
    for positioned in &ordered {
        let id = positioned
            .region
            .attribute("region-id")
            .ok_or("Area calculation region ID missing")?;
        if !active_ids.insert(id.to_owned()) {
            return Err("Duplicate calculation region ID".into());
        }
    }
    let mut definitions = HashMap::new();
    // Only active leaf calculations participate. Unsupported Area containers
    // remain atomic and do not leak hidden declarations into any outer row.
    for positioned in &ordered {
        if positioned.fault.is_some() {
            continue;
        }
        if let Some(context) = raw.get(&positioned.region.id()) {
            let roots = elements(*context)?;
            if roots.len() == 1 && math(roots[0], "globalDefine") {
                declare(roots[0], &mut definitions)?;
            }
        } else if let Some(math) = positioned
            .region
            .children()
            .find(|n| n.has_tag_name((WS, "math")))
            && let Some(root) = statement(math, positioned.in_area)
            && super::math(root, "globalDefine")
        {
            declare(root, &mut definitions)?;
        }
    }
    let mut patches = Vec::new();
    for positioned in ordered {
        if positioned.fault.is_some() {
            continue;
        }
        let region = positioned.region;
        if let Some(context) = raw.get(&region.id()) {
            let roots = elements(*context)?;
            check_ml_scope(*context)?;
            let labelled = label_parsed_fragment(*context, &source, &mut definitions, None)?;
            let replacement = if roots.len() == 1 && roots[0].has_tag_name((WS, "spec-table")) {
                labelled
            } else {
                format!(
                    "<math resultRef=\"{}\">{labelled}</math>",
                    region.attribute("region-id").unwrap()
                )
            };
            patches.push((context.range(), replacement));
        } else if let Some(math) = region.children().find(|n| n.has_tag_name((WS, "math"))) {
            if let Some(root) = statement(math, positioned.in_area) {
                patches.push((root.range(), native_label(root, &source, &mut definitions)?));
            }
        } else {
            let children: Vec<_> = region.children().filter(Node::is_element).collect();
            if children.len() == 1 && children[0].has_tag_name((WS, "spec-table")) {
                patches.push((
                    children[0].range(),
                    native_label(children[0], &source, &mut definitions)?,
                ));
            }
            // Solve blocks keep their independently validated raw source.
        }
    }
    patches.sort_by_key(|(range, _)| range.start);
    let mut output_len = source.len();
    let mut end = 0;
    for (range, text) in &patches {
        if range.start < end {
            return Err("Overlapping Area label patches".into());
        }
        end = range.end;
        output_len = output_len
            .checked_sub(range.len())
            .and_then(|n| n.checked_add(text.len()))
            .ok_or("Worksheet exceeds XML budget")?;
    }
    if output_len > MAX_XML {
        return Err("Worksheet exceeds XML budget".into());
    }
    let mut output = source;
    for (range, text) in patches.into_iter().rev() {
        output.replace_range(range, &text);
    }
    // Generated patches must retain a complete, namespace-valid document.
    roxmltree::Document::parse(&output).map_err(|e| e.to_string())?;
    Ok(output)
}

#[cfg(test)]
#[path = "area_worksheet_tests.rs"]
mod tests;
