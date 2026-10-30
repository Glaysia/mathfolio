//! Inert Area child presentation; never an evaluator environment or executable
//! MathML input. The outer string owns all nested presentation byte accounting.
use super::{
    CanvasContent, CanvasRegion, MAX_CANVAS_TREE_DEPTH, MAX_REGIONS, SavedCalculationRow,
    SavedCalculationStatus, SavedNumericSource,
};
use roxmltree::{Document, Node};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
const WS: &str = "http://schemas.mathsoft.com/worksheet50";
const ML: &str = "http://schemas.mathsoft.com/math50";
const MAX_SOURCE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedAreaPresentation {
    pub rows: Vec<SavedAreaChildResult>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedAreaChildResult {
    /// Element-child path relative to the owning WS region fragment. This
    /// identifies its direct Area math child region, not a top-level Canvas ID.
    pub region_path: Vec<usize>,
    pub row: SavedCalculationRow,
}
fn elements<'a, 'i>(node: Node<'a, 'i>) -> impl Iterator<Item = Node<'a, 'i>> {
    node.children().filter(Node::is_element)
}
fn contains_input_placeholder(root: Node<'_, '_>) -> Result<bool, String> {
    let mut stack = vec![(root, 1)];
    let mut incomplete = false;
    while let Some((node, depth)) = stack.pop() {
        if depth > MAX_CANVAS_TREE_DEPTH {
            return Err("Saved Area source expression depth exceeds limits".into());
        }
        if node.has_tag_name((ML, "unitOverride")) {
            continue;
        }
        incomplete |= node.has_tag_name((ML, "placeholder"));
        stack.extend(elements(node).map(|child| (child, depth + 1)));
    }
    Ok(incomplete)
}
fn child_paths(xml: &str) -> Result<HashMap<Vec<usize>, bool>, String> {
    if xml.len() > super::MAX_TEXT_BYTES {
        return Err("Saved Area source exceeds 8 MiB".into());
    }
    let doc = Document::parse_with_options(
        xml,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: 250_000,
            ..Default::default()
        },
    )
    .map_err(|_| "Invalid saved Area source XML")?;
    let owner = doc.root_element();
    if !owner.has_tag_name((WS, "region")) {
        return Err("Saved Area source must be its owning region fragment".into());
    }
    let roots: Vec<_> = elements(owner).collect();
    let [area] = roots.as_slice() else {
        return Err("Saved Area source has mixed content".into());
    };
    if !area.has_tag_name((WS, "Area")) {
        return Err("Saved Area source lacks Area".into());
    }
    if area
        .descendants()
        .skip(1)
        .any(|n| n.has_tag_name((WS, "Area")))
    {
        return Err("Nested saved Area source is unsupported".into());
    }
    let collections: Vec<_> = elements(*area)
        .enumerate()
        .filter(|(_, n)| n.has_tag_name((WS, "regions")))
        .collect();
    let [(collection_index, regions)] = collections.as_slice() else {
        return Err("Saved Area source requires one regions collection".into());
    };
    let mut paths = HashMap::new();
    for (region_index, region) in elements(*regions).enumerate() {
        if !region.has_tag_name((WS, "region")) {
            continue;
        }
        let maths: Vec<_> = elements(region)
            .filter(|n| n.has_tag_name((WS, "math")))
            .collect();
        let [math] = maths.as_slice() else {
            continue;
        };
        // Cache paths bind to actual direct math regions. No tree decoder or
        // engine is invoked, including for unsupported/read-only expressions.
        let mut incomplete = false;
        for expression in elements(*math).filter(|n| n.tag_name().namespace() == Some(ML)) {
            incomplete |= contains_input_placeholder(expression)?;
        }
        paths.insert(vec![0, *collection_index, region_index], incomplete);
    }
    Ok(paths)
}
impl SavedAreaPresentation {
    /// Parse and validate presentation against its owning Opaque Area source.
    /// A cache may contain a subset of direct children. This does not establish
    /// source-signature freshness; the app must still use the ordinary guard.
    pub fn parse_for_region(data: &str, owner: &CanvasRegion) -> Result<Self, String> {
        if data.len() > MAX_SOURCE_BYTES {
            return Err("Saved numeric source exceeds 4 MiB".into());
        }
        let CanvasContent::Opaque { kind, xml } = &owner.content else {
            return Err("Saved Area presentation requires an Opaque Area region".into());
        };
        if kind != "Area" {
            return Err("Saved Area presentation requires an Opaque Area region".into());
        }
        let payload: Self =
            serde_json::from_str(data).map_err(|_| "Invalid saved Area presentation JSON")?;
        if payload.rows.len() > MAX_REGIONS {
            return Err("Saved Area presentation row count exceeds limits".into());
        }
        let paths = child_paths(xml)?;
        let mut seen = HashSet::new();
        for child in &payload.rows {
            if child.region_path.is_empty()
                || child.region_path.len() > MAX_CANVAS_TREE_DEPTH
                || !seen.insert(child.region_path.clone())
            {
                return Err("Saved Area child paths must be unique and bounded".into());
            }
            let Some(incomplete) = paths.get(&child.region_path) else {
                return Err("Saved Area child path does not refer to direct math".into());
            };
            let row = &child.row;
            if row.region_id != owner.id {
                return Err("Saved Area child row must refer to its owning Canvas region".into());
            }
            if row.error.is_some()
                && (row.display.is_some()
                    || row.symbolic_mathml.is_some()
                    || row.numeric_source.is_some())
            {
                return Err(
                    "Saved calculation row cannot contain both a result and an error".into(),
                );
            }
            if row.warnings.len() > 1024 {
                return Err("Saved calculation warnings exceed limits".into());
            }
            if let Some(source) = &row.numeric_source {
                let data = match source {
                    SavedNumericSource::NativeMathml(data)
                    | SavedNumericSource::EvaluatedValue(data) => data,
                    SavedNumericSource::AreaPresentation(_) => {
                        return Err("Nested saved Area presentation is forbidden".into());
                    }
                };
                if data.len() > MAX_SOURCE_BYTES {
                    return Err("Saved numeric source exceeds 4 MiB".into());
                }
            }
            if *incomplete
                && row.status == SavedCalculationStatus::Synchronized
                && (row.display.is_some()
                    || row.symbolic_mathml.is_some()
                    || row.numeric_source.is_some())
            {
                return Err(
                    "Incomplete Area math cannot contain a synchronized saved result".into(),
                );
            }
        }
        Ok(payload)
    }
}
pub(super) fn validate(
    owner: &CanvasRegion,
    row: &SavedCalculationRow,
    data: &str,
) -> Result<(), String> {
    if row.display.is_some() || row.symbolic_mathml.is_some() {
        return Err("Saved Area parent cannot contain a direct result".into());
    }
    SavedAreaPresentation::parse_for_region(data, owner).map(|_| ())
}

#[cfg(test)]
#[path = "canvas_area_cache_tests.rs"]
mod tests;
