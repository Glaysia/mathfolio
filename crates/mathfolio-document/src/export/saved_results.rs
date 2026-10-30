//! Bind validated inert app presentations to exported Math result identities.
//! Nothing here evaluates an expression or decodes a saved value as input code.
use super::*;
use crate::canvas::{
    SavedAreaPresentation, SavedCalculationRow, SavedCalculationStatus, SavedNumericSource,
};

fn presentation(
    row: &SavedCalculationRow,
    tree: &mathfolio_editor::PortableTree,
) -> Option<String> {
    if row.status != SavedCalculationStatus::Synchronized
        || row.error.is_some()
        || !row.warnings.is_empty()
        || row.symbolic_mathml.is_some()
    {
        return None;
    }
    if let Some(source) = &row.numeric_source {
        return crate::native_result::numeric_result_xml(source)
            .ok()
            .flatten();
    }
    // A synchronized definition can have no displayed value. Do not fabricate
    // an empty numeric result for an unevaluated query or incomplete input.
    if matches!(
        tree.kind,
        mathfolio_editor::NodeKind::Binary(
            mathfolio_editor::BinaryOp::Define | mathfolio_editor::BinaryOp::GlobalDefine
        )
    ) {
        let mut stack = vec![tree];
        while let Some(node) = stack.pop() {
            if matches!(node.kind, mathfolio_editor::NodeKind::Placeholder) {
                return None;
            }
            stack.extend(&node.children);
        }
        return Some(String::new());
    }
    None
}

pub(super) fn write(
    canvas: &CanvasDocument,
    package: &mut NativePackage,
    owners: &HashMap<String, u64>,
    ref_count: &HashMap<i32, usize>,
) -> Result<usize, String> {
    let Some(cache) = canvas.saved_calculation_cache() else {
        return Ok(0);
    };
    let source = package.worksheet_xml()?;
    let parsed = parse(&source)?;
    let canvas_by_id: HashMap<_, _> = canvas
        .regions
        .iter()
        .map(|region| (region.id, region))
        .collect();
    let rows_by_id: HashMap<_, _> = cache.rows.iter().map(|row| (row.region_id, row)).collect();
    let caches = package.caches()?;
    let mut original = HashMap::<i32, Vec<&crate::native::NativeCache>>::new();
    for row in &caches {
        if let Some(id) = row
            .result_id
            .as_deref()
            .and_then(|id| crate::native::parse_result_id(id).ok())
        {
            original.entry(id).or_default().push(row);
        }
    }
    let mut updates = HashMap::<i32, Option<String>>::new();
    for node in parsed
        .root_element()
        .children()
        .filter(|node| node.has_tag_name((WS, "regions")))
        .flat_map(|node| {
            node.children()
                .filter(|node| node.has_tag_name((WS, "region")))
        })
    {
        let Some(owner) = node
            .attribute("region-id")
            .and_then(|id| owners.get(id))
            .and_then(|id| canvas_by_id.get(id).copied())
        else {
            continue;
        };
        let Some(row) = rows_by_id.get(&owner.id).copied() else {
            continue;
        };
        if owner.calculation_disabled {
            continue;
        }
        let mut add = |reference: Option<&str>,
                       row: &SavedCalculationRow,
                       tree: &mathfolio_editor::PortableTree| {
            let Some(id) = reference
                .and_then(|reference| crate::native::parse_result_id(reference).ok())
                .filter(|id| ref_count.get(id) == Some(&1))
            else {
                return;
            };
            if let Some(value) = presentation(row, tree) {
                updates.insert(id, Some(value));
            } else if row.status == SavedCalculationStatus::Pending
                || matches!(
                    row.numeric_source,
                    Some(SavedNumericSource::EvaluatedValue(_))
                )
                || row.symbolic_mathml.is_some()
                || ((row.error.is_some() || !row.warnings.is_empty())
                    && !original
                        .get(&id)
                        .filter(|rows| rows.len() == 1)
                        .is_some_and(|rows| {
                            let native = rows[0];
                            let error = native.errors.first().map(|error| {
                                error
                                    .resource_string
                                    .clone()
                                    .or_else(|| error.code.clone())
                                    .unwrap_or_else(|| "저장된 계산 오류".into())
                            });
                            row.error == error
                                && row.warnings.len() == native.warnings.len()
                                && row
                                    .warnings
                                    .iter()
                                    .all(|warning| warning == "저장된 재정의 경고")
                        }))
            {
                // A fresh value we cannot encode must not leave an older
                // synchronized value in place, even for an unchanged program.
                updates.insert(id, None);
            }
        };
        match &owner.content {
            CanvasContent::Math(tree) => {
                if let Some(math) = node.children().find(|node| node.has_tag_name((WS, "math"))) {
                    add(math.attribute("resultRef"), row, tree);
                }
            }
            CanvasContent::Opaque { kind, .. } if kind == "Area" => {
                let Some(SavedNumericSource::AreaPresentation(data)) = &row.numeric_source else {
                    continue;
                };
                let Ok(bundle) = SavedAreaPresentation::parse_for_region(data, owner) else {
                    continue;
                };
                let by_path: HashMap<_, _> = bundle
                    .rows
                    .iter()
                    .map(|saved| (saved.region_path.as_slice(), &saved.row))
                    .collect();
                let fragment = crate::spec_table::standalone(node, &source)?;
                for child in crate::area::area_math_children_xml(&fragment, &[])? {
                    let Some(saved) = by_path.get(child.region_path.as_slice()).copied() else {
                        continue;
                    };
                    if child.calculation_disabled {
                        continue;
                    }
                    if let Ok(tree) = tree_from_mathml(&child.mathml) {
                        add(child.result_ref.as_deref(), saved, &tree);
                    }
                }
            }
            _ => {}
        }
    }
    upsert(package, updates)
}

/// All callers supply literal result XML or Pending; source identity and
/// reference multiplicity are checked before reaching this common part writer.
pub(super) fn upsert(
    package: &mut NativePackage,
    updates: HashMap<i32, Option<String>>,
) -> Result<usize, String> {
    upsert_updates(
        package,
        updates
            .into_iter()
            .map(|(id, value)| {
                (
                    id,
                    value.map_or(ResultUpdate::PendingKeepPayload, ResultUpdate::Synchronized),
                )
            })
            .collect(),
    )
}

/// An explicit empty Pending row prevents old curve replay on fresh open.
/// It does not synthesize an original engine error or force a live dispatch.
pub(super) enum ResultUpdate {
    Synchronized(String),
    PendingLiteral(String),
    PendingKeepPayload,
    PendingClearPayload,
}

pub(super) fn upsert_updates(
    package: &mut NativePackage,
    mut updates: HashMap<i32, ResultUpdate>,
) -> Result<usize, String> {
    if updates.is_empty() {
        return Ok(0);
    }
    let caches = package.caches()?;
    let mut locations = HashMap::<i32, Vec<String>>::new();
    for row in caches {
        if let Some(id) = row
            .result_id
            .as_deref()
            .and_then(|id| crate::native::parse_result_id(id).ok())
        {
            locations.entry(id).or_default().push(row.part);
        }
    }
    updates.retain(|id, _| {
        locations
            .get(id)
            .is_none_or(|locations| locations.len() == 1)
    });
    let missing: Vec<_> = updates
        .keys()
        .filter(|id| !locations.contains_key(id))
        .copied()
        .collect();
    append_pending_results(package, &missing)?;
    let mut grouped = HashMap::<String, HashMap<i32, ResultUpdate>>::new();
    for (id, value) in updates {
        let part = locations
            .get(&id)
            .map(|locations| locations[0].clone())
            .unwrap_or_else(|| "mathcad/result.xml".into());
        grouped.entry(part).or_default().insert(id, value);
    }
    let mut written = 0;
    for (part, updates) in grouped {
        let xml = xml_text(package.original_part(&part).ok_or("Missing result part")?)?;
        let doc = parse(&xml)?;
        let mut patches = vec![];
        for row in doc
            .root_element()
            .children()
            .filter(|node| node.has_tag_name((RESULTS, "resultData")))
        {
            if let Some(id) = row
                .attribute("result-id")
                .and_then(|id| crate::native::parse_result_id(id).ok())
                && let Some(value) = updates.get(&id)
            {
                match value {
                    ResultUpdate::Synchronized(value) => {
                        patches.push((row.range(),format!("<resultData xmlns=\"{RESULTS}\" result-id=\"{id}\" calculation-status=\"Synchronized\">{value}</resultData>")));
                        written += 1;
                    }
                    ResultUpdate::PendingLiteral(value) => {
                        patches.push((row.range(),format!("<resultData xmlns=\"{RESULTS}\" result-id=\"{id}\" calculation-status=\"Pending\">{value}</resultData>")));
                    }
                    ResultUpdate::PendingClearPayload => {
                        patches.push((row.range(),format!("<resultData xmlns=\"{RESULTS}\" result-id=\"{id}\" calculation-status=\"Pending\"/>")));
                    }
                    ResultUpdate::PendingKeepPayload => {
                        if row.attribute("calculation-status") != Some("Pending") {
                            patches.extend(attrs(
                                row,
                                &xml,
                                &[("calculation-status", Some("Pending".into()))],
                            )?);
                        }
                    }
                }
            }
        }
        if !patches.is_empty() {
            set_xml(package, &part, apply(xml, patches)?)?;
        }
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::SavedCalculationCache;
    fn fixture() -> (CanvasDocument, u64) {
        let mut canvas = CanvasDocument::default();
        let id = canvas.add_math(0., 0.);
        canvas.regions[0].content=CanvasContent::Math(tree_from_mathml(&format!("<ml:eval xmlns:ml=\"{ML}\"><ml:real>7</ml:real><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>")).unwrap());
        let value = mathfolio_compat::Value::Scalar {
            quantity: mathfolio_core::Quantity::scalar(7.),
        };
        canvas.calculation_cache = Some(SavedCalculationCache {
            source_signature: canvas.calculation_signature(),
            rows: vec![SavedCalculationRow {
                region_id: id,
                status: SavedCalculationStatus::Synchronized,
                display: Some("7".into()),
                error: None,
                warnings: vec![],
                symbolic_mathml: None,
                numeric_source: Some(SavedNumericSource::EvaluatedValue(
                    serde_json::to_string(&value).unwrap(),
                )),
            }],
        });
        (canvas, id)
    }
    fn output(canvas: &CanvasDocument) -> NativePackage {
        let mut package = minimal_package(canvas).unwrap();
        export_into(canvas, &mut package).unwrap();
        package
    }
    #[test]
    fn current_synchronized_literal_cache_writes_native_result_without_evaluating_source() {
        let (canvas, _) = fixture();
        let package = output(&canvas);
        let caches = package.caches().unwrap();
        assert_eq!(caches.len(), 1);
        assert_eq!(
            caches[0].calculation_status.as_deref(),
            Some("Synchronized")
        );
        let source = package
            .original_xml_part("mathcad/result.xml")
            .unwrap()
            .unwrap();
        let node = parse(&source).unwrap();
        assert!(
            node.descendants()
                .any(|node| node.has_tag_name((ML, "result")))
        );
        assert_eq!(
            node.descendants()
                .find(|node| node.has_tag_name((ML, "real")))
                .unwrap()
                .text(),
            Some("7")
        );
        assert_eq!(
            canvas.saved_calculation_cache().unwrap().rows[0]
                .display
                .as_deref(),
            Some("7")
        );
    }
    #[test]
    fn stale_pending_disabled_and_diagnostic_rows_are_never_promoted() {
        for case in 0..5 {
            let (mut canvas, _) = fixture();
            match case {
                0 => canvas
                    .calculation_cache
                    .as_mut()
                    .unwrap()
                    .source_signature
                    .push('x'),
                1 => {
                    canvas.calculation_cache.as_mut().unwrap().rows[0].status =
                        SavedCalculationStatus::Pending
                }
                2 => {
                    canvas.regions[0].calculation_disabled = true;
                    canvas.calculation_cache.as_mut().unwrap().source_signature =
                        canvas.calculation_signature();
                }
                3 => {
                    canvas.calculation_cache.as_mut().unwrap().rows[0].error =
                        Some("localized message without native code".into())
                }
                _ => canvas.calculation_cache.as_mut().unwrap().rows[0]
                    .warnings
                    .push("unknown native warning identity".into()),
            }
            let package = output(&canvas);
            assert!(
                package
                    .caches()
                    .unwrap()
                    .iter()
                    .all(|row| row.calculation_status.as_deref() == Some("Pending")),
                "case={case}"
            );
        }
    }
    #[test]
    fn unitful_or_malformed_cached_values_keep_pending_native_results() {
        let mut quantity = mathfolio_core::Quantity::scalar(7.);
        quantity.dimension[0] = 1.;
        let unitful = serde_json::to_string(&mathfolio_compat::Value::Scalar { quantity }).unwrap();
        for source in [unitful, "invalid-json".to_owned()] {
            let (mut canvas, _) = fixture();
            canvas.calculation_cache.as_mut().unwrap().rows[0].numeric_source =
                Some(SavedNumericSource::EvaluatedValue(source));
            let package = output(&canvas);
            assert!(
                package
                    .caches()
                    .unwrap()
                    .iter()
                    .all(|row| row.calculation_status.as_deref() == Some("Pending"))
            );
        }
    }
    #[test]
    fn missing_native_cache_row_is_created_for_an_existing_result_reference() {
        let (mut canvas, _) = fixture();
        let mut package = output(&canvas);
        package
            .set_part(
                "mathcad/result.xml",
                format!("<resultsList xmlns=\"{RESULTS}\"/>").into_bytes(),
            )
            .unwrap();
        canvas.regions[0].source_region = Some("1".into());
        canvas.calculation_cache.as_mut().unwrap().source_signature =
            canvas.calculation_signature();
        export_into(&canvas, &mut package).unwrap();
        assert_eq!(
            package.caches().unwrap()[0].calculation_status.as_deref(),
            Some("Synchronized")
        );
    }
    #[test]
    fn duplicated_result_identity_in_another_part_does_not_receive_a_fresh_value() {
        let (mut canvas, _) = fixture();
        let mut package = output(&canvas);
        let source = package
            .original_part("mathcad/result.xml")
            .unwrap()
            .to_vec();
        package
            .set_part("custom/duplicate-result.xml", source)
            .unwrap();
        canvas.regions[0].source_region = Some("1".into());
        canvas.calculation_cache.as_mut().unwrap().source_signature =
            canvas.calculation_signature();
        export_into(&canvas, &mut package).unwrap();
        assert_eq!(package.caches().unwrap().len(), 2);
        assert!(
            package
                .caches()
                .unwrap()
                .iter()
                .all(|row| row.calculation_status.as_deref() == Some("Pending"))
        );
    }

    fn imported() -> (CanvasDocument, NativePackage) {
        let (mut canvas, _) = fixture();
        let package = output(&canvas);
        canvas.regions[0].source_region = Some("1".into());
        canvas.calculation_cache = None;
        (canvas, package)
    }

    #[test]
    fn unchanged_program_preserves_raw_native_value_diagnostics_and_tag() {
        let (mut canvas, mut package) = imported();
        let raw = format!(
            "<resultsList xmlns=\"{RESULTS}\" xmlns:ml=\"{ML}\"><resultData result-id=\"1\" calculation-status=\"Synchronized\" tag=\"original tag\"><ml:result><ml:real>7</ml:real></ml:result><engineErrors><engineError error-id=\"2\" error-node-tag=\"1234\"><errorCode>source-owned-code</errorCode><errorArgument>x</errorArgument><resource-string>Original diagnostic</resource-string></engineError></engineErrors><Warnings><Warning nodeId=\"x\" oldType=\"Value\"/></Warnings></resultData></resultsList>"
        );
        package
            .set_part("mathcad/result.xml", raw.as_bytes().to_vec())
            .unwrap();
        canvas.regions[0].left += 10.;
        canvas.regions[0].top += 20.;
        export_into(&canvas, &mut package).unwrap();
        assert_eq!(
            package.original_part("mathcad/result.xml").unwrap(),
            raw.as_bytes()
        );
    }

    #[test]
    fn source_change_marks_all_cached_rows_pending_and_keeps_saved_payload() {
        let (mut canvas, mut package) = imported();
        let worksheet = package.worksheet_xml().unwrap();
        let extra = format!(
            "<region xmlns=\"{WS}\" region-id=\"2\" left=\"0\" top=\"50\"><math resultRef=\"2\"><ml:eval xmlns:ml=\"{ML}\"><ml:id>x</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval></math></region>"
        );
        let worksheet = worksheet.replace("</regions>", &format!("{extra}</regions>"));
        let part = package.worksheet_part_name().to_owned();
        set_xml(&mut package, &part, worksheet).unwrap();
        let row = CanvasRegion { id: 2, source_region: Some("2".into()), left: 0., top:50., content:CanvasContent::Math(tree_from_mathml(&format!("<ml:eval xmlns:ml=\"{ML}\"><ml:id>x</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>")).unwrap()), ..canvas.regions[0].clone() };
        canvas.regions.push(row);
        let raw = format!(
            "<resultsList xmlns=\"{RESULTS}\" xmlns:ml=\"{ML}\"><resultData result-id=\"1\" calculation-status=\"Synchronized\" tag=\"keep\"><ml:result><ml:real>7</ml:real></ml:result><Warnings><Warning nodeId=\"x\" oldType=\"Value\"/></Warnings></resultData><resultData result-id=\"2\" calculation-status=\"Synchronized\"><ml:result><ml:real>99</ml:real></ml:result></resultData></resultsList>"
        );
        package
            .set_part("mathcad/result.xml", raw.as_bytes().to_vec())
            .unwrap();
        canvas.regions[0].content = CanvasContent::Math(mathfolio_editor::PortableTree {
            kind: mathfolio_editor::NodeKind::Number("8".into()),
            children: vec![],
        });
        export_into(&canvas, &mut package).unwrap();
        let result = package
            .original_xml_part("mathcad/result.xml")
            .unwrap()
            .unwrap();
        let expected = raw.replace("Synchronized", "Pending");
        assert!(result.starts_with(expected.strip_suffix("</resultsList>").unwrap()));
        assert!(
            package
                .caches()
                .unwrap()
                .iter()
                .all(|row| row.calculation_status.as_deref() == Some("Pending"))
        );
        // Changed Math receives a new identity; untouched downstream Math must
        // also become Pending while its old displayed value remains available.
        let regions = package.regions().unwrap();
        assert_ne!(
            regions[0].math.as_ref().unwrap().result_ref.as_deref(),
            Some("1")
        );
        assert_eq!(
            regions[1].math.as_ref().unwrap().result_ref.as_deref(),
            Some("2")
        );
    }

    #[test]
    fn unsupported_fresh_value_demotes_an_older_synchronized_value() {
        let (mut canvas, mut package) = imported();
        let mut quantity = mathfolio_core::Quantity::scalar(8.);
        quantity.dimension[0] = 1.;
        canvas.calculation_cache = Some(SavedCalculationCache {
            source_signature: canvas.calculation_signature(),
            rows: vec![SavedCalculationRow {
                region_id: canvas.regions[0].id,
                status: SavedCalculationStatus::Synchronized,
                display: Some("8m".into()),
                error: None,
                warnings: vec![],
                symbolic_mathml: None,
                numeric_source: Some(SavedNumericSource::EvaluatedValue(
                    serde_json::to_string(&mathfolio_compat::Value::Scalar { quantity }).unwrap(),
                )),
            }],
        });
        export_into(&canvas, &mut package).unwrap();
        assert_eq!(
            package.caches().unwrap()[0].calculation_status.as_deref(),
            Some("Pending")
        );
        assert!(
            package
                .original_xml_part("mathcad/result.xml")
                .unwrap()
                .unwrap()
                .contains(">7<")
        );
    }

    #[test]
    fn display_only_page_break_insertion_preserves_saved_result_and_exports_native_payload() {
        use crate::page_break::{
            PageBreakBounds, PageBreakBoundsKind, PageBreakLayout, insert_page_break,
        };
        let (canvas, mut package) = imported();
        let raw = package
            .original_part("mathcad/result.xml")
            .unwrap()
            .to_vec();
        let changed = insert_page_break(
            &canvas,
            PageBreakLayout {
                body_height: 710.4,
                grid_height: 19.2,
            },
            [0., 100.],
            &[PageBreakBounds {
                region_id: canvas.regions[0].id,
                bottom: 22.,
                kind: PageBreakBoundsKind::Ordinary,
            }],
        )
        .unwrap();
        export_into(&changed.document, &mut package).unwrap();
        assert_eq!(package.original_part("mathcad/result.xml").unwrap(), raw);
        let regions = package.regions().unwrap();
        assert_eq!(regions.len(), 2);
        let added = regions
            .iter()
            .find(|region| region.kind == "pageBreak")
            .unwrap();
        assert_eq!(added.top, Some(710.4));
        assert_eq!(added.left, Some(0.));
        assert!(added.raw_xml.contains("height=\"2\""));
        assert!(!added.raw_xml.contains("NextPage"));
        assert_ne!(added.region_id.as_deref(), Some("1"));
    }

    #[test]
    fn new_page_break_does_not_discard_unknown_source_metadata() {
        let (mut canvas, mut package) = imported();
        let xml = format!(
            "<region xmlns=\"{WS}\" region-id=\"0\" left=\"0\" top=\"0\" height=\"2\" unknown=\"keep\"><pageBreak/></region>"
        );
        canvas.add_opaque(0., 710.4, xml, "pageBreak");
        assert!(export_into(&canvas, &mut package).is_err());
    }

    #[test]
    fn new_diagnostic_without_native_identity_cannot_leave_an_old_value_synchronized() {
        let (mut canvas, mut package) = imported();
        let raw = package
            .original_xml_part("mathcad/result.xml")
            .unwrap()
            .unwrap();
        canvas.calculation_cache = Some(SavedCalculationCache {
            source_signature: canvas.calculation_signature(),
            rows: vec![SavedCalculationRow {
                region_id: canvas.regions[0].id,
                status: SavedCalculationStatus::Synchronized,
                display: None,
                error: Some("new diagnostic without native node/code identity".into()),
                warnings: vec![],
                symbolic_mathml: None,
                numeric_source: None,
            }],
        });
        export_into(&canvas, &mut package).unwrap();
        assert_eq!(
            package
                .original_xml_part("mathcad/result.xml")
                .unwrap()
                .unwrap(),
            raw.replace("Synchronized", "Pending")
        );
    }

    #[test]
    fn table_column_references_without_saved_rows_reserve_the_package_result_id_space() {
        let (mut canvas, mut package) = imported();
        canvas.calculation_cache = None;
        let xml = format!(
            "<region xmlns=\"{WS}\" xmlns:ml=\"{ML}\" region-id=\"8\" left=\"0\" top=\"50\"><spec-table><math resultRef=\"40\"><ml:define><ml:id>x</ml:id><ml:matrix rows=\"1\" cols=\"1\"><ml:real>7</ml:real></ml:matrix></ml:define></math></spec-table></region>"
        );
        let source = package
            .worksheet_xml()
            .unwrap()
            .replace("</regions>", &format!("{xml}</regions>"));
        let part = package.worksheet_part_name().to_owned();
        set_xml(&mut package, &part, source).unwrap();
        let id = canvas.add_opaque(0., 50., xml, "spec-table");
        canvas
            .regions
            .iter_mut()
            .find(|region| region.id == id)
            .unwrap()
            .source_region = Some("8".into());
        let id = canvas.add_math(0., 100.);
        canvas
            .regions
            .iter_mut()
            .find(|region| region.id == id)
            .unwrap()
            .content = CanvasContent::Math(mathfolio_editor::PortableTree {
            kind: mathfolio_editor::NodeKind::Number("9".into()),
            children: vec![],
        });
        export_into(&canvas, &mut package).unwrap();
        let refs = result_references(&package).unwrap();
        assert_eq!(refs.get(&40), Some(&1));
        assert_eq!(refs.get(&41), Some(&1));
        assert!(refs.values().all(|count| *count == 1));
    }
}
