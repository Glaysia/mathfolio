//! Open/commit-time Area presentation. Data is source-bound and inert: these
//! helpers never run the calculator, change XML, or allocate Canvas children.
use super::area_ui::child_key;
use super::*;
use mathfolio_document::{
    area::{self, NativeAreaMath},
    canvas::{SavedAreaChildResult, SavedAreaPresentation},
    native::NativeCache,
    native_visual::{AreaChildVisualEntry, area_child_visual_entries},
};

pub(super) type SavedChildren = HashMap<u64, HashMap<String, SavedCalculationRow>>;
pub(super) type AreaVisuals = HashMap<u64, Vec<AreaChildVisualEntry>>;

fn area_xml(region: &CanvasRegion) -> Option<&str> {
    match &region.content {
        CanvasContent::Opaque { kind, xml } if kind == "Area" => Some(xml),
        _ => None,
    }
}
fn pending_row(owner: u64) -> SavedCalculationRow {
    SavedCalculationRow {
        region_id: owner,
        status: SavedCalculationStatus::Pending,
        display: None,
        error: None,
        warnings: vec![],
        symbolic_mathml: None,
        numeric_source: None,
    }
}
fn child_format(child: &NativeAreaMath, defaults: MathFormat) -> Result<MathFormat, String> {
    let doc = roxmltree::Document::parse(&child.math_xml).map_err(|e| e.to_string())?;
    Ok(
        mathfolio_document::formatting::read_math_format_with_defaults(
            doc.root_element(),
            defaults,
        ),
    )
}
fn expects_display(tree: &PortableTree) -> bool {
    match tree.kind {
        NodeKind::Binary(BinaryOp::Evaluate) | NodeKind::SymbolicEvaluation { .. } => true,
        NodeKind::Binary(BinaryOp::Define | BinaryOp::GlobalDefine) => tree
            .children
            .get(1)
            .is_some_and(|t| matches!(t.kind, NodeKind::Binary(BinaryOp::Evaluate))),
        _ => false,
    }
}

/// Suppress a chosen output unit only on the actual top-level value.
/// A matrix may contain unitedValue cells; finding a descendant would collapse
/// the entire saved matrix to its first scalar. Full matrix cells stay inert.
pub(super) fn explicit_unit_numeric(xml: &str) -> Option<String> {
    const ML: &str = "http://schemas.mathsoft.com/math50";
    let doc = roxmltree::Document::parse(xml).ok()?;
    let mut value = doc.root_element();
    if value.has_tag_name((ML, "result")) {
        let mut children = value.children().filter(roxmltree::Node::is_element);
        value = children.next()?;
        if children.next().is_some() {
            return None;
        }
    }
    if !value.has_tag_name((ML, "unitedValue")) {
        return None;
    }
    let mut children = value.children().filter(roxmltree::Node::is_element);
    let number = children.next()?;
    // Retain the existing numeric display validator as the authority. This
    // helper selects a carrier; it does not execute, recursively strip or
    // rewrite units, or interpret a native unit as a calculation Quantity.
    let units = children.next()?;
    if !units.has_tag_name(("http://schemas.mathsoft.com/units10", "unitMonomial"))
        || children.next().is_some()
    {
        return None;
    }
    Some(standalone(number, xml))
}

/// Shared original native-cache reader. The ordinary worksheet reader can use
/// this exact function too; chosen units are drawn by source, never duplicated.
pub(super) fn native_saved_row(
    owner: u64,
    tree: Option<&PortableTree>,
    format: &MathFormat,
    expects_display: bool,
    cache: &NativeCache,
) -> SavedCalculationRow {
    let mut numeric_source = None;
    let display = cache
        .result_mathml
        .as_deref()
        .and_then(|xml| {
            let evaluation = tree.and_then(|tree| {
                if matches!(
                    tree.kind,
                    NodeKind::Binary(BinaryOp::Define | BinaryOp::GlobalDefine)
                ) {
                    tree.children.get(1)
                } else {
                    Some(tree)
                }
            });
            let explicit_unit = evaluation.is_some_and(|tree| {
                matches!(tree.kind, NodeKind::Binary(BinaryOp::Evaluate))
                    && tree
                        .children
                        .get(1)
                        .is_some_and(|unit| unit.kind != NodeKind::Placeholder)
            });
            let numeric = explicit_unit.then(|| explicit_unit_numeric(xml)).flatten();
            let source = numeric.as_deref().unwrap_or(xml);
            let display = crate::result_display::from_cached_native_result(source, format);
            if display.is_some() || inert_string_result::validate_native(source).is_some() {
                numeric_source = Some(SavedNumericSource::NativeMathml(source.to_owned()));
            }
            display
        })
        .map(|display| display.to_saved());
    let error = cache.errors.first().map(|error| {
        error
            .resource_string
            .clone()
            .or_else(|| error.code.clone())
            .unwrap_or_else(|| "저장된 계산 오류".into())
    });
    let status = if cache.calculation_status.as_deref() == Some("Synchronized")
        && (display.is_some() || numeric_source.is_some() || error.is_some() || !expects_display)
    {
        SavedCalculationStatus::Synchronized
    } else {
        SavedCalculationStatus::Pending
    };
    SavedCalculationRow {
        region_id: owner,
        status,
        display: if error.is_none() { display } else { None },
        numeric_source: if error.is_none() {
            numeric_source
        } else {
            None
        },
        error,
        warnings: cache
            .warnings
            .iter()
            .map(|_| "저장된 재정의 경고".into())
            .collect(),
        symbolic_mathml: None,
    }
}

pub(super) fn restore_saved(
    document: &CanvasDocument,
    parents: &HashMap<u64, SavedCalculationRow>,
) -> SavedChildren {
    document
        .regions
        .iter()
        .filter_map(|region| {
            area_xml(region)?;
            let parent = parents.get(&region.id)?;
            if parent.region_id != region.id || parent.error.is_some() {
                return None;
            }
            let Some(SavedNumericSource::AreaPresentation(data)) = &parent.numeric_source else {
                return None;
            };
            let bundle = SavedAreaPresentation::parse_for_region(data, region).ok()?;
            Some((
                region.id,
                bundle
                    .rows
                    .into_iter()
                    .map(|child| (child_key(&child.region_path), child.row))
                    .collect(),
            ))
        })
        .collect()
}

pub(super) fn restore_native(
    document: &CanvasDocument,
    package: &NativePackage,
) -> Result<(SavedChildren, AreaVisuals), String> {
    let regions = package.regions().map_err(|e| e.to_string())?;
    let caches = package.caches().map_err(|e| e.to_string())?;
    let worksheet = package.worksheet_xml().map_err(|e| e.to_string())?;
    let mut saved = HashMap::new();
    let mut visuals = HashMap::new();
    for owner in &document.regions {
        let Some(xml) = area_xml(owner) else {
            continue;
        };
        let Some(source_id) = owner.source_region.as_ref() else {
            continue;
        };
        let mut matches = regions.iter().filter(|r| {
            r.part == package.worksheet_part_name()
                && r.parent_path.is_none()
                && r.kind == "Area"
                && r.region_id.as_ref() == Some(source_id)
        });
        let Some(native) = matches.next() else {
            continue;
        };
        if matches.next().is_some() {
            continue;
        }
        if let Ok(entries) = area_child_visual_entries(package, &native.path) {
            visuals.insert(owner.id, entries);
        }
        let Ok(current) = area::area_math_children_xml(xml, &[]) else {
            continue;
        };
        let Ok(original) = area::area_math_children_xml(&worksheet, &native.path) else {
            continue;
        };
        let protected = area::areas_from_xml(xml)
            .ok()
            .and_then(|a| a.into_iter().find(|a| a.region_path.is_empty()))
            .is_none_or(|a| a.protected);
        let mut rows = HashMap::new();
        for child in current {
            let mut row = pending_row(owner.id);
            let tree = worksheet_math::tree_from_mathml(&child.mathml).ok();
            let same = original.iter().find(|old| {
                old.region_path.strip_prefix(native.path.as_slice())
                    == Some(child.region_path.as_slice())
                    && old.region_id == child.region_id
                    && old.result_ref == child.result_ref
            });
            if let (Some(tree), Some(old), Some(reference)) =
                (&tree, same, child.result_ref.as_ref())
                && worksheet_math::tree_from_mathml(&old.mathml).ok().as_ref() == Some(tree)
                && let Some(cache) = caches
                    .iter()
                    .find(|cache| cache.matches_result_ref(reference))
            {
                let format = child_format(&child, owner.format)?;
                row = native_saved_row(owner.id, Some(tree), &format, expects_display(tree), cache);
                if protected || child.calculation_disabled {
                    row.status = SavedCalculationStatus::Pending;
                }
            }
            rows.insert(child_key(&child.region_path), row);
        }
        saved.insert(owner.id, rows);
    }
    Ok((saved, visuals))
}

fn input_placeholder(xml: &str) -> bool {
    let Ok(doc) = roxmltree::Document::parse(xml) else {
        return true;
    };
    let mut nodes = vec![doc.root_element()];
    while let Some(node) = nodes.pop() {
        if node.has_tag_name(("http://schemas.mathsoft.com/math50", "unitOverride")) {
            continue;
        }
        if node.has_tag_name(("http://schemas.mathsoft.com/math50", "placeholder")) {
            return true;
        }
        nodes.extend(node.children().filter(roxmltree::Node::is_element));
    }
    false
}

pub(super) fn save_parent(
    region: &CanvasRegion,
    live: &HashMap<String, RegionEvaluation>,
    saved: &HashMap<String, SavedCalculationRow>,
    pending: &HashSet<(u64, String)>,
    all_drafts: bool,
) -> Result<SavedCalculationRow, String> {
    let xml = area_xml(region).ok_or("Area cache requires its Opaque owner")?;
    let mut rows = vec![];
    for child in area::area_math_children_xml(xml, &[])? {
        let key = child_key(&child.region_path);
        let format = child_format(&child, region.format)?;
        let mut row = match saved.get(&key) {
            Some(row) if row.region_id == region.id => row.clone(),
            Some(_) => return Err("Saved Area child belongs to another owner".into()),
            None => pending_row(region.id),
        };
        let mut fresh = false;
        if let Some(result) = live.get(&key) {
            if let Some(error) = &result.error {
                fresh = true;
                row.error = Some(error.detail.clone());
                row.display = None;
                row.numeric_source = None;
                row.symbolic_mathml = None;
            } else if let Some(value) = result.display_value.as_ref().or(result.value.as_ref()) {
                fresh = true;
                row.numeric_source = numeric_cache::source(value);
                row.display = Some(crate::result_display::format(value, &format).to_saved());
                row.error = None;
                row.symbolic_mathml = None;
            } else if worksheet_math::tree_from_mathml(&child.mathml)
                .is_ok_and(|tree| !expects_display(&tree))
            {
                fresh = true;
                row.error = None;
                row.display = None;
                row.numeric_source = None;
                row.symbolic_mathml = None;
            }
            if fresh || result.redefinition_warning {
                row.warnings = if result.redefinition_warning {
                    vec!["이 이름은 앞선 정의를 다시 정의합니다.".into()]
                } else {
                    vec![]
                };
            }
        }
        if !fresh && let Some(display) = numeric_cache::display(&row, &format) {
            row.display = Some(display.to_saved());
        }
        if all_drafts
            || child.calculation_disabled
            || pending.contains(&(region.id, key.clone()))
            || input_placeholder(&child.mathml)
        {
            row.status = SavedCalculationStatus::Pending;
        } else if fresh {
            row.status = SavedCalculationStatus::Synchronized;
        }
        rows.push(SavedAreaChildResult {
            region_path: child.region_path,
            row,
        });
    }
    let status = if all_drafts
        || rows
            .iter()
            .any(|r| r.row.status == SavedCalculationStatus::Pending)
    {
        SavedCalculationStatus::Pending
    } else {
        SavedCalculationStatus::Synchronized
    };
    let data = serde_json::to_string(&SavedAreaPresentation { rows }).map_err(|e| e.to_string())?;
    SavedAreaPresentation::parse_for_region(&data, region)?;
    Ok(SavedCalculationRow {
        region_id: region.id,
        status,
        display: None,
        error: None,
        warnings: vec![],
        symbolic_mathml: None,
        numeric_source: Some(SavedNumericSource::AreaPresentation(data)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};
    const WS: &str = "http://schemas.mathsoft.com/worksheet50";
    const ML: &str = "http://schemas.mathsoft.com/math50";
    const FIRST: &str = "[0,0,0]";
    const SECOND: &str = "[0,0,1]";
    const THIRD: &str = "[0,0,2]";
    fn source() -> String {
        format!(
            r#"<ws:region xmlns:ws="{WS}" xmlns:ml="{ML}" region-id="owner" left="5" top="100"><ws:Area AreaHeight="150" IsCollapsed="false" IsBorderVisible="true"><ws:regions><ws:region region-id="a" left="10" top="20"><ws:math resultRef="10"><ml:eval><ml:real>1</ml:real><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval></ws:math></ws:region><ws:region region-id="b" left="10" top="50"><ws:math resultRef="11"><ml:eval><ml:real>2</ml:real><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval></ws:math></ws:region><ws:region region-id="c" left="10" top="80"><ws:math resultRef="12"><ml:eval><ml:real>3</ml:real><ml:unitOverride><ml:id labels="UNIT">mm</ml:id></ml:unitOverride></ml:eval></ws:math></ws:region><ws:region region-id="text" left="10" top="110"><ws:text/></ws:region></ws:regions></ws:Area></ws:region>"#
        )
    }
    fn package() -> NativePackage {
        let worksheet = format!(
            "<ws:worksheet xmlns:ws=\"{WS}\" xmlns:ml=\"{ML}\"><ws:regions>{}</ws:regions></ws:worksheet>",
            source()
        );
        let results = format!(
            r#"<resultsList xmlns="http://schemas.mathsoft.com/result10" xmlns:ml="{ML}" xmlns:u="http://schemas.mathsoft.com/units10"><resultData result-id="10" calculation-status="Synchronized"><ml:real>12.3456</ml:real><Warning nodeId="old"/></resultData><resultData result-id="11" calculation-status="Synchronized"><engineError><errorCode>saved-error-code</errorCode><resource-string>saved failure</resource-string></engineError></resultData><resultData result-id="12" calculation-status="Pending"><ml:unitedValue><ml:real>1.2346</ml:real><u:unitMonomial><u:unitReference unit="meter" power-numerator="1" power-denominator="1"/></u:unitMonomial></ml:unitedValue></resultData></resultsList>"#
        );
        let mut zip = zip::ZipWriter::new(Cursor::new(vec![]));
        for (name, data) in [
            ("mathcad/worksheet.xml", worksheet.as_str()),
            ("mathcad/result.xml", results.as_str()),
            ("custom/keep.bin", "untouched"),
        ] {
            zip.start_file(
                name,
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
            zip.write_all(data.as_bytes()).unwrap();
        }
        NativePackage::from_bytes(&zip.finish().unwrap().into_inner()).unwrap()
    }
    fn document() -> CanvasDocument {
        let mut d = CanvasDocument::default();
        d.add_opaque(5., 100., source(), "Area");
        d.regions[0].source_region = Some("owner".into());
        d
    }
    fn plain(row: &SavedCalculationRow) -> Option<String> {
        Some(
            crate::result_display::FormattedResult::from_saved(row.display.as_deref()?)?
                .cells
                .iter()
                .map(|c| c.plain())
                .collect(),
        )
    }
    fn live(n: f64) -> RegionEvaluation {
        RegionEvaluation {
            region_id: "private-child".into(),
            result_ref: None,
            left: 10.,
            top: 20.,
            value: Some(Value::Scalar {
                quantity: mathfolio_core::Quantity::scalar(n),
            }),
            display_value: None,
            error: None,
            redefinition_warning: false,
            matrix_origin: None,
        }
    }
    #[test]
    fn native_cache_preserves_status_errors_warnings_and_explicit_unit_payload_without_execution_or_source_change()
     {
        let package = package();
        let worksheet = package.worksheet_xml().unwrap();
        let document = document();
        let before = document.clone();
        let (saved, visuals) = restore_native(&document, &package).unwrap();
        let owner = document.regions[0].id;
        assert_eq!(saved[&owner].len(), 3);
        assert_eq!(plain(&saved[&owner][FIRST]).as_deref(), Some("12.346"));
        assert_eq!(
            saved[&owner][FIRST].status,
            SavedCalculationStatus::Synchronized
        );
        assert_eq!(saved[&owner][FIRST].warnings, vec!["저장된 재정의 경고"]);
        assert_eq!(
            saved[&owner][SECOND].error.as_deref(),
            Some("saved failure")
        );
        assert!(
            saved[&owner][SECOND].display.is_none()
                && saved[&owner][SECOND].numeric_source.is_none()
        );
        assert_eq!(saved[&owner][THIRD].status, SavedCalculationStatus::Pending);
        assert_eq!(plain(&saved[&owner][THIRD]).as_deref(), Some("1.235"));
        assert!(
            matches!(&saved[&owner][THIRD].numeric_source,Some(SavedNumericSource::NativeMathml(xml)) if !xml.contains("unitedValue") && !xml.contains("unitReference"))
        );
        assert_eq!(visuals[&owner].len(), 4);
        assert_eq!(document, before);
        assert_eq!(package.worksheet_xml().unwrap(), worksheet);
    }
    #[test]
    fn native_child_cache_survives_collapse_border_move_disable_and_current_format_changes() {
        let package = package();
        let mut d = document();
        let owner = d.regions[0].id;
        d.regions[0].left = 500.;
        d.regions[0].top = 900.;
        let CanvasContent::Opaque { xml, .. } = &mut d.regions[0].content else {
            panic!()
        };
        *xml = area::set_area_display_xml(xml, &[], Some(true), Some(false))
            .unwrap()
            .unwrap();
        *xml = area::toggle_area_calculation_xml(xml, &[])
            .unwrap()
            .unwrap();
        *xml = xml.replace(
            "</ws:math>",
            "<ws:resultFormat><ws:decimal precision=\"2\"/></ws:resultFormat></ws:math>",
        );
        let (saved, _) = restore_native(&d, &package).unwrap();
        assert_eq!(plain(&saved[&owner][FIRST]).as_deref(), Some("12.35"));
        assert_eq!(saved[&owner][FIRST].status, SavedCalculationStatus::Pending);
        assert_eq!(
            saved[&owner][SECOND].error.as_deref(),
            Some("saved failure")
        );
        assert_eq!(plain(&saved[&owner][THIRD]).as_deref(), Some("1.23"));
    }
    #[test]
    fn native_child_identity_reference_or_expression_change_does_not_reuse_another_sources_result()
    {
        let package = package();
        for (from, to) in [
            ("<ml:real>1</ml:real>", "<ml:real>9</ml:real>"),
            ("resultRef=\"10\"", "resultRef=\"11\""),
            ("region-id=\"a\"", "region-id=\"other\""),
        ] {
            let mut d = document();
            let owner = d.regions[0].id;
            let CanvasContent::Opaque { xml, .. } = &mut d.regions[0].content else {
                panic!()
            };
            *xml = xml.replace(from, to);
            let (saved, _) = restore_native(&d, &package).unwrap();
            assert_eq!(saved[&owner][FIRST], pending_row(owner));
            assert_eq!(
                saved[&owner][SECOND].error.as_deref(),
                Some("saved failure")
            );
        }
    }
    #[test]
    fn own_bundle_file_cache_roundtrip_keeps_pending_precision_and_child_errors_and_filters_removed_paths()
     {
        let mut d = document();
        let owner = d.regions[0].id;
        let (saved, _) = restore_native(&d, &package()).unwrap();
        let parent = save_parent(
            &d.regions[0],
            &HashMap::new(),
            &saved[&owner],
            &HashSet::new(),
            false,
        )
        .unwrap();
        assert!(
            parent.display.is_none() && parent.error.is_none() && parent.symbolic_mathml.is_none()
        );
        assert_eq!(parent.status, SavedCalculationStatus::Pending);
        d.calculation_cache = Some(SavedCalculationCache {
            source_signature: d.calculation_signature(),
            rows: vec![parent.clone()],
        });
        let path = std::env::temp_dir().join(format!(
            "mathfolio-area-app-cache-{}-{}.mathfolio",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        d.save(&path).unwrap();
        let loaded = CanvasDocument::load(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        let rows = restore_saved(&loaded, &HashMap::from([(owner, parent)]));
        assert_eq!(rows, saved);
        let mut saved = saved[&owner].clone();
        saved.insert("[0,0,99]".into(), pending_row(owner));
        let filtered = save_parent(
            &d.regions[0],
            &HashMap::new(),
            &saved,
            &HashSet::new(),
            false,
        )
        .unwrap();
        assert_eq!(
            restore_saved(&d, &HashMap::from([(owner, filtered)]))[&owner].len(),
            3
        );
    }
    #[test]
    fn invalid_or_foreign_bundle_and_wrapper_error_do_not_escape_the_source_guard() {
        let d = document();
        let owner = d.regions[0].id;
        let (saved, _) = restore_native(&d, &package()).unwrap();
        let parent = save_parent(
            &d.regions[0],
            &HashMap::new(),
            &saved[&owner],
            &HashSet::new(),
            false,
        )
        .unwrap();
        for corrupted in ["not JSON".to_owned(),r#"{"rows":[{"region_path":[0,0,99],"row":{"region_id":1,"status":"Pending","display":null,"error":null,"warnings":[],"symbolic_mathml":null}}]}"#.into()] {
            let mut row=parent.clone();row.numeric_source=Some(SavedNumericSource::AreaPresentation(corrupted));
            assert!(restore_saved(&d,&HashMap::from([(owner,row)])).is_empty());
        }
        let mut foreign = parent.clone();
        foreign.region_id = owner + 1;
        assert!(restore_saved(&d, &HashMap::from([(owner, foreign)])).is_empty());
        let mut error = parent;
        error.error = Some("unsupported wrapper".into());
        assert!(restore_saved(&d, &HashMap::from([(owner, error)])).is_empty());
        let mut foreign_children = saved[&owner].clone();
        foreign_children.get_mut(FIRST).unwrap().region_id = owner + 1;
        assert!(
            save_parent(
                &d.regions[0],
                &HashMap::new(),
                &foreign_children,
                &HashSet::new(),
                false
            )
            .is_err()
        );
    }

    #[test]
    fn delayed_live_row_keeps_pending_warning_payload_and_explicit_pending_marks_fresh_values() {
        let d = document();
        let owner = d.regions[0].id;
        let (mut saved, _) = restore_native(&d, &package()).unwrap();
        let original = saved.get_mut(&owner).unwrap().get_mut(FIRST).unwrap();
        original.status = SavedCalculationStatus::Pending;
        assert!(
            matches!(&original.numeric_source,Some(SavedNumericSource::NativeMathml(raw)) if raw.contains("12.3456"))
        );
        let mut waiting = live(1.);
        waiting.value = None;
        let parent = save_parent(
            &d.regions[0],
            &HashMap::from([(FIRST.into(), waiting)]),
            &saved[&owner],
            &HashSet::new(),
            false,
        )
        .unwrap();
        let restored = restore_saved(&d, &HashMap::from([(owner, parent)]));
        assert_eq!(restored[&owner][FIRST], saved[&owner][FIRST]);
        let parent = save_parent(
            &d.regions[0],
            &HashMap::from([(FIRST.into(), live(7.))]),
            &saved[&owner],
            &HashSet::from([(owner, FIRST.into())]),
            false,
        )
        .unwrap();
        let restored = restore_saved(&d, &HashMap::from([(owner, parent)]));
        assert_eq!(
            restored[&owner][FIRST].status,
            SavedCalculationStatus::Pending
        );
        assert_eq!(plain(&restored[&owner][FIRST]).as_deref(), Some("7"));
        assert!(restored[&owner][FIRST].warnings.is_empty());
    }

    #[test]
    fn live_success_error_pending_and_incomplete_source_are_saved_without_promoting_stale_cache() {
        let mut d = document();
        let owner = d.regions[0].id;
        let (saved, _) = restore_native(&d, &package()).unwrap();
        let mut error = live(9.);
        error.value = None;
        error.error = Some(mathfolio_compat::Fault {
            code: "test".into(),
            detail: "fresh failure".into(),
            previous_region: None,
            unsupported: false,
        });
        let live_rows = HashMap::from([(FIRST.into(), live(7.)), (SECOND.into(), error)]);
        let parent = save_parent(
            &d.regions[0],
            &live_rows,
            &saved[&owner],
            &HashSet::new(),
            false,
        )
        .unwrap();
        let restored = restore_saved(&d, &HashMap::from([(owner, parent)]));
        assert_eq!(plain(&restored[&owner][FIRST]).as_deref(), Some("7"));
        assert_eq!(
            restored[&owner][FIRST].status,
            SavedCalculationStatus::Synchronized
        );
        assert!(restored[&owner][FIRST].warnings.is_empty());
        assert_eq!(
            restored[&owner][SECOND].error.as_deref(),
            Some("fresh failure")
        );
        assert!(
            restored[&owner][SECOND].display.is_none()
                && restored[&owner][SECOND].numeric_source.is_none()
        );
        let CanvasContent::Opaque { xml, .. } = &mut d.regions[0].content else {
            panic!()
        };
        *xml = xml.replace("<ml:real>1</ml:real>", "<ml:placeholder/>");
        let parent = save_parent(
            &d.regions[0],
            &HashMap::new(),
            &saved[&owner],
            &HashSet::new(),
            true,
        )
        .unwrap();
        let restored = restore_saved(&d, &HashMap::from([(owner, parent)]));
        assert_eq!(
            restored[&owner][FIRST].status,
            SavedCalculationStatus::Pending
        );
        assert_eq!(plain(&restored[&owner][FIRST]).as_deref(), Some("12.346"));
    }
}
