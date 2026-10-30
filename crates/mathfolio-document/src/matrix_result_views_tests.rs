use super::*;
use crate::canvas::{
    CanvasDocument, SavedCalculationCache, SavedCalculationRow, SavedCalculationStatus,
    SavedNumericSource,
};

fn viewport() -> MatrixResultViewport {
    MatrixResultViewport {
        size: Some(MatrixViewSize { rows: 3, cols: 7 }),
        offset: Some(MatrixViewOffset { row: 11, col: 2 }),
        show_indices: Some(false),
    }
}

#[test]
fn absent_wire_is_exact_legacy_and_old_documents_still_load() {
    // Independent pre-field serializer, including its original field order.
    #[derive(Serialize)]
    struct Legacy<'a> {
        version: u32,
        title: &'a str,
        width: f64,
        height: f64,
        grid: f64,
        next_id: u64,
        regions: &'a [CanvasRegion],
        page_sections: &'a crate::canvas::PageSections,
        #[serde(skip_serializing_if = "Option::is_none")]
        page_setup: Option<&'a crate::canvas::PageSetup>,
        #[serde(skip_serializing_if = "Option::is_none")]
        calculation_cache: Option<&'a SavedCalculationCache>,
        #[serde(skip_serializing_if = "Option::is_none")]
        bin_oct_hex_mode: Option<bool>,
        native_archive: Option<&'a Vec<u8>>,
    }
    let mut document = CanvasDocument::default();
    document.add_math(-0., 12.);
    document.add_text(1., 2., "한글😀");
    let old = Legacy {
        version: document.version,
        title: &document.title,
        width: document.width,
        height: document.height,
        grid: document.grid,
        next_id: document.next_id,
        regions: &document.regions,
        page_sections: &document.page_sections,
        page_setup: document.page_setup.as_ref(),
        calculation_cache: document.calculation_cache.as_ref(),
        bin_oct_hex_mode: document.bin_oct_hex_mode,
        native_archive: document.native_archive.as_ref(),
    };
    let bytes = serde_json::to_vec(&old).unwrap();
    assert_eq!(serde_json::to_vec(&document).unwrap(), bytes);
    let restored: CanvasDocument = serde_json::from_slice(&bytes).unwrap();
    assert!(restored.matrix_result_views.is_empty());
    restored.validate().unwrap();
    assert_eq!(serde_json::to_vec(&restored).unwrap(), bytes);
}

#[test]
fn view_changes_leave_source_signatures_and_complete_saved_numeric_bytes_unchanged() {
    let mut document = CanvasDocument::default();
    let id = document.add_math(0., 0.);
    document.regions[0].content = CanvasContent::Math(mathfolio_editor::PortableTree {
        kind: mathfolio_editor::NodeKind::Matrix {
            rows: 1,
            cols: 2,
            display: None,
        },
        children: ["123.456789", "7"]
            .into_iter()
            .map(|text| mathfolio_editor::PortableTree {
                kind: mathfolio_editor::NodeKind::Number(text.into()),
                children: vec![],
            })
            .collect(),
    });
    let signature = document.calculation_signature();
    let active = document.active_calculation_signature();
    document.calculation_cache = Some(SavedCalculationCache {
        source_signature: signature.clone(),
        rows: vec![SavedCalculationRow {
            region_id: id, status: SavedCalculationStatus::Synchronized,
            display: None, error: None, warnings: vec![], symbolic_mathml: None,
            numeric_source: Some(SavedNumericSource::NativeMathml(
                "<matrix xmlns=\"http://schemas.mathsoft.com/math50\" rows=\"1\" cols=\"2\"><real>123.456789</real><real>7</real></matrix>".into())),
        }],
    });
    let old_cache = serde_json::to_vec(&document.calculation_cache).unwrap();
    let old_regions = serde_json::to_vec(&document.regions).unwrap();
    document
        .matrix_result_views
        .set(MatrixResultTarget::math(id), viewport())
        .unwrap();
    document.validate().unwrap();
    assert_eq!(document.calculation_signature(), signature);
    assert_eq!(document.active_calculation_signature(), active);
    assert!(document.saved_calculation_cache().is_some());
    assert_eq!(serde_json::to_vec(&document.regions).unwrap(), old_regions);
    assert_eq!(
        serde_json::to_vec(&document.calculation_cache).unwrap(),
        old_cache
    );
    let restored: CanvasDocument =
        serde_json::from_slice(&serde_json::to_vec(&document).unwrap()).unwrap();
    restored.validate().unwrap();
    assert_eq!(restored.matrix_result_views, document.matrix_result_views);
    assert_eq!(
        serde_json::to_vec(&restored.calculation_cache).unwrap(),
        old_cache
    );
}

#[test]
fn optional_requests_keep_absence_distinct_from_explicit_false_and_row_column_order() {
    let mut views = MatrixResultViews::default();
    views
        .set(
            MatrixResultTarget::math(8),
            MatrixResultViewport {
                show_indices: Some(false),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        serde_json::to_string(&views).unwrap(),
        r#"[{"target":{"owner_id":8},"viewport":{"show_indices":false}}]"#
    );
    views.set(MatrixResultTarget::math(8), viewport()).unwrap();
    let wire = serde_json::to_string(&views).unwrap();
    assert!(wire.contains(r#""size":{"rows":3,"cols":7},"offset":{"row":11,"col":2}"#));
    assert!(!wire.contains("origin"));
    assert_eq!(
        serde_json::from_str::<MatrixResultViews>(&wire).unwrap(),
        views
    );
}

#[test]
fn duplicate_unknown_null_and_bad_shape_wire_are_rejected() {
    for wire in [
        "null",
        r#"[{"target":{"owner_id":1},"viewport":{"show_indices":true}},{"target":{"owner_id":1},"viewport":{"show_indices":false}}]"#,
        r#"[{"target":{"owner_id":1,"owner_id":2},"viewport":{"show_indices":true}}]"#,
        r#"[{"target":{"owner_id":1,"region_path":null},"viewport":{"show_indices":true}}]"#,
        r#"[{"target":{"owner_id":1,"region_path":[]},"viewport":{"show_indices":true}}]"#,
        r#"[{"target":{"owner_id":0},"viewport":{"show_indices":true}}]"#,
        r#"[{"target":{"owner_id":1},"viewport":{"show_indices":null}}]"#,
        r#"[{"target":{"owner_id":1},"viewport":{"show_indices":true,"show_indices":false}}]"#,
        r#"[{"target":{"owner_id":1},"viewport":{"size":{"rows":0,"cols":1}}}]"#,
        r#"[{"target":{"owner_id":1},"viewport":{"size":{"rows":65536,"cols":65536}}}]"#,
        r#"[{"target":{"owner_id":1},"viewport":{"offset":{"row":2147483648,"col":0}}}]"#,
        r#"[{"target":{"owner_id":1},"viewport":{"origin":0}}]"#,
        r#"[{"target":{"owner_id":1},"viewport":{}}]"#,
        r#"[{"target":{"owner_id":1},"viewport":{"show_indices":true},"value":0}]"#,
    ] {
        assert!(
            serde_json::from_str::<MatrixResultViews>(wire).is_err(),
            "{wire}"
        );
    }
    let mut wire = serde_json::to_value(CanvasDocument::default()).unwrap();
    wire["matrix_result_views"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<CanvasDocument>(wire).is_err());
}

#[test]
fn bounded_paths_and_collection_fail_without_changing_previous_state() {
    let mut views = MatrixResultViews::default();
    views.set(MatrixResultTarget::math(1), viewport()).unwrap();
    let before = views.clone();
    assert!(
        views
            .set(MatrixResultTarget::area_child(1, vec![0; 65]), viewport())
            .is_err()
    );
    assert!(
        views
            .set(
                MatrixResultTarget::math(1),
                MatrixResultViewport {
                    size: Some(MatrixViewSize {
                        rows: u32::MAX,
                        cols: 2
                    }),
                    ..Default::default()
                }
            )
            .is_err()
    );
    assert_eq!(views, before);
    let path = serde_json::json!([{"target":{"owner_id":1,"region_path":vec![0;65]},"viewport":{"show_indices":true}}]);
    assert!(serde_json::from_value::<MatrixResultViews>(path).is_err());
    for id in 2..=MAX_MATRIX_RESULT_VIEWS as u64 {
        views.set(MatrixResultTarget::math(id), viewport()).unwrap();
    }
    let before = views.clone();
    assert!(
        views
            .set(MatrixResultTarget::math(5000), viewport())
            .is_err()
    );
    assert_eq!(views, before);
    let mut values = serde_json::to_value(&views)
        .unwrap()
        .as_array()
        .unwrap()
        .clone();
    values.push(serde_json::json!({"target":{"owner_id":5000},"viewport":{"show_indices":true}}));
    assert!(serde_json::from_value::<MatrixResultViews>(serde_json::Value::Array(values)).is_err());
}

#[test]
fn region_target_admission_does_not_alias_missing_text_plot_or_area_owner() {
    let mut document = CanvasDocument::default();
    let math = document.add_math(0., 0.);
    let text = document.add_text(0., 20., "text");
    let plot = document.add_plot(0., 40.);
    for target in [
        MatrixResultTarget::math(999),
        MatrixResultTarget::math(text),
        MatrixResultTarget::math(plot),
        MatrixResultTarget::area_child(math, vec![0, 0, 0]),
    ] {
        let mut candidate = document.clone();
        candidate
            .matrix_result_views
            .set(target, viewport())
            .unwrap();
        assert!(candidate.validate().is_err());
    }
    document
        .matrix_result_views
        .set(MatrixResultTarget::math(math), viewport())
        .unwrap();
    document.validate().unwrap();
}

#[test]
fn area_direct_paths_are_distinct_even_with_duplicate_native_ids_and_nested_math_is_rejected() {
    let mut document = CanvasDocument::default();
    let xml = r#"<region xmlns="http://schemas.mathsoft.com/worksheet50" xmlns:ml="http://schemas.mathsoft.com/math50"><Area><regions><region region-id="7" left="0" top="0"><math><ml:real>1</ml:real></math></region><region region-id="7" left="0" top="20"><math><ml:real>2</ml:real></math></region><region region-id="8"><Area><regions><region left="0" top="0"><math><ml:real>3</ml:real></math></region></regions></Area></region></regions></Area></region>"#;
    let owner = document.add_opaque(0., 0., xml, "Area");
    let first = MatrixResultTarget::area_child(owner, vec![0, 0, 0]);
    let second = MatrixResultTarget::area_child(owner, vec![0, 0, 1]);
    document
        .matrix_result_views
        .set(first.clone(), viewport())
        .unwrap();
    let other = MatrixResultViewport {
        show_indices: Some(true),
        ..Default::default()
    };
    document
        .matrix_result_views
        .set(second.clone(), other)
        .unwrap();
    document.validate().unwrap();
    assert_eq!(document.matrix_result_views.get(&first), Some(&viewport()));
    assert_eq!(document.matrix_result_views.get(&second), Some(&other));
    for path in [vec![0, 0, 2], vec![0, 0, 2, 0, 0, 0], vec![0, 0, 99]] {
        let mut candidate = document.clone();
        candidate
            .matrix_result_views
            .set(MatrixResultTarget::area_child(owner, path), viewport())
            .unwrap();
        assert!(candidate.validate().is_err());
    }
    let mut candidate = document.clone();
    candidate
        .matrix_result_views
        .set(MatrixResultTarget::math(owner), viewport())
        .unwrap();
    assert!(candidate.validate().is_err());
}

#[test]
fn history_style_set_remove_is_exact_and_clones_are_independent_without_shape_clamping() {
    let mut views = MatrixResultViews::default();
    let one = MatrixResultTarget::math(1);
    let two = MatrixResultTarget::math(2);
    views.set(two.clone(), viewport()).unwrap();
    views.set(one.clone(), viewport()).unwrap();
    let before = serde_json::to_vec(&views).unwrap();
    let mut clone = views.clone();
    let next = MatrixResultViewport {
        offset: Some(MatrixViewOffset {
            row: 2_000_000,
            col: 9,
        }),
        ..viewport()
    };
    let old = clone.set(one.clone(), next).unwrap().unwrap();
    assert_eq!(clone.get(&one).unwrap().offset.unwrap().row, 2_000_000);
    assert_eq!(serde_json::to_vec(&views).unwrap(), before);
    clone.set(one.clone(), old).unwrap();
    assert_eq!(serde_json::to_vec(&clone).unwrap(), before);
    let old = clone.remove(&one).unwrap();
    clone.set(one, old).unwrap();
    assert_eq!(serde_json::to_vec(&clone).unwrap(), before);
    assert_eq!(clone.remove_owner(2), 1);
    assert!(clone.get(&two).is_none());
    assert_eq!(
        clone
            .set(MatrixResultTarget::math(1), Default::default())
            .unwrap(),
        Some(viewport())
    );
    assert!(clone.is_empty());
    assert_eq!(serde_json::to_string(&clone).unwrap(), "[]");
}
