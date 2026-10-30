use super::*;
use crate::canvas::{CanvasDocument, SavedCalculationCache, SavedCalculationStatus};
fn source(expression: &str) -> String {
    format!(
        "<ws:region xmlns:ws='{WS}' xmlns:ml='{ML}' region-id='native-owner'><ws:Area><ws:regions><ws:region region-id='native-child' left='10' top='20'><ws:math resultRef='saved-result'>{expression}</ws:math></ws:region><ws:region region-id='text'><ws:text/></ws:region></ws:regions></ws:Area></ws:region>"
    )
}
fn document() -> CanvasDocument {
    let mut d = CanvasDocument::default();
    d.add_opaque(0.,0.,source("<ml:eval><ml:real>7</ml:real><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>"),"Area");
    d
}
fn row(id: u64) -> SavedCalculationRow {
    SavedCalculationRow {
        region_id: id,
        status: SavedCalculationStatus::Synchronized,
        display: Some("7".into()),
        error: None,
        warnings: vec![],
        symbolic_mathml: None,
        numeric_source: Some(SavedNumericSource::EvaluatedValue(
            "inert numeric value, not parsed by document validation".into(),
        )),
    }
}
fn payload(owner: u64) -> SavedAreaPresentation {
    SavedAreaPresentation {
        rows: vec![SavedAreaChildResult {
            region_path: vec![0, 0, 0],
            row: row(owner),
        }],
    }
}
fn install(d: &mut CanvasDocument, payload: &SavedAreaPresentation) {
    let id = d.regions[0].id;
    d.calculation_cache = Some(SavedCalculationCache {
        source_signature: d.calculation_signature(),
        rows: vec![SavedCalculationRow {
            region_id: id,
            status: SavedCalculationStatus::Synchronized,
            display: None,
            error: None,
            warnings: vec![],
            symbolic_mathml: None,
            numeric_source: Some(SavedNumericSource::AreaPresentation(
                serde_json::to_string(payload).unwrap(),
            )),
        }],
    });
}
fn check(
    payload: &SavedAreaPresentation,
    d: &CanvasDocument,
) -> Result<SavedAreaPresentation, String> {
    SavedAreaPresentation::parse_for_region(&serde_json::to_string(payload).unwrap(), &d.regions[0])
}
#[test]
fn inert_bundle_file_roundtrip_preserves_paths_results_and_both_source_signatures() {
    let mut d = document();
    let signature = d.calculation_signature();
    let active = d.active_calculation_signature();
    let p = payload(d.regions[0].id);
    install(&mut d, &p);
    d.validate().unwrap();
    assert_eq!(d.calculation_signature(), signature);
    assert_eq!(d.active_calculation_signature(), active);
    assert!(d.saved_calculation_cache().is_some());
    let encoded = serde_json::to_string(&d).unwrap();
    let restored: CanvasDocument = serde_json::from_str(&encoded).unwrap();
    restored.validate().unwrap();
    assert_eq!(restored, d);
    let path = std::env::temp_dir().join(format!(
        "mathfolio-area-inert-cache-{}-{}.mathfolio",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    d.save(&path).unwrap();
    let loaded = CanvasDocument::load(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    assert_eq!(loaded, d);
    assert_eq!(check(&p, &d).unwrap(), p);
}
#[test]
fn old_row_json_and_empty_bundle_have_stable_shapes() {
    let old = r#"{"region_id":1,"status":"Pending","display":null,"error":null,"warnings":[],"symbolic_mathml":null}"#;
    let row: SavedCalculationRow = serde_json::from_str(old).unwrap();
    assert_eq!(serde_json::to_string(&row).unwrap(), old);
    let empty = SavedAreaPresentation { rows: vec![] };
    assert_eq!(serde_json::to_string(&empty).unwrap(), "{\"rows\":[]}");
    let mut d = document();
    install(&mut d, &empty);
    d.validate().unwrap();
}
#[test]
fn paths_bind_to_unique_actual_direct_math_children_and_canvas_owner() {
    let d = document();
    let p = payload(d.regions[0].id);
    assert!(check(&p, &d).is_ok());
    for path in [
        vec![],
        vec![0, 0, 1],
        vec![0, 0, 0, 0],
        vec![1, 0, 0],
        vec![0; MAX_CANVAS_TREE_DEPTH + 1],
    ] {
        let mut invalid = p.clone();
        invalid.rows[0].region_path = path;
        assert!(check(&invalid, &d).is_err());
    }
    let mut duplicate = p.clone();
    duplicate.rows.push(p.rows[0].clone());
    assert!(check(&duplicate, &d).unwrap_err().contains("unique"));
    let mut owner = p;
    owner.rows[0].row.region_id += 1;
    assert!(check(&owner, &d).unwrap_err().contains("owning Canvas"));
}
#[test]
fn bundle_rejects_wrong_canvas_kind_and_malformed_or_nested_source() {
    let p = payload(1);
    let mut d = document();
    for kind in ["math", "area", "spec-table"] {
        if let CanvasContent::Opaque { kind: k, .. } = &mut d.regions[0].content {
            *k = kind.into();
        }
        assert!(check(&p, &d).is_err());
    }
    d.regions[0].content = CanvasContent::Text("text".into());
    assert!(check(&p, &d).is_err());
    for xml in [
        "<broken".into(),
        format!("<ws:Area xmlns:ws='{WS}'><ws:regions/></ws:Area>"),
        source("<ml:real>7</ml:real>").replace(
            "</ws:regions>",
            "<ws:region><ws:Area><ws:regions/></ws:Area></ws:region></ws:regions>",
        ),
        source("<ml:real>7</ml:real>").replace("</ws:Area>", "<ws:regions/></ws:Area>"),
    ] {
        d.regions[0].content = CanvasContent::Opaque {
            kind: "Area".into(),
            xml,
        };
        assert!(check(&p, &d).is_err());
    }
}
#[test]
fn unknown_json_fields_and_recursive_bundle_variants_are_rejected_without_decoding() {
    let d = document();
    let p = payload(1);
    let value = serde_json::to_value(&p).unwrap();
    for path in [0, 1, 2] {
        let mut bad = value.clone();
        match path {
            0 => bad["extra"] = true.into(),
            1 => bad["rows"][0]["extra"] = true.into(),
            _ => bad["rows"][0]["row"]["extra"] = true.into(),
        };
        assert!(SavedAreaPresentation::parse_for_region(&bad.to_string(), &d.regions[0]).is_err());
    }
    let mut nested = p;
    nested.rows[0].row.numeric_source = Some(SavedNumericSource::AreaPresentation(
        "this must never be recursively parsed".into(),
    ));
    assert!(check(&nested, &d).unwrap_err().contains("Nested"));
    let deep = "[".repeat(140) + &"]".repeat(140);
    assert!(SavedAreaPresentation::parse_for_region(&deep, &d.regions[0]).is_err());
}
#[test]
fn error_result_warning_and_parent_conflicts_preserve_old_error_precedence() {
    let mut d = document();
    let mut p = payload(1);
    p.rows[0].row.error = Some("saved error".into());
    assert_eq!(
        check(&p, &d).unwrap_err(),
        "Saved calculation row cannot contain both a result and an error"
    );
    p.rows[0].row.display = None;
    p.rows[0].row.numeric_source = None;
    assert!(check(&p, &d).is_ok());
    p.rows[0].row.warnings = vec![String::new(); 1025];
    assert_eq!(
        check(&p, &d).unwrap_err(),
        "Saved calculation warnings exceed limits"
    );
    install(&mut d, &payload(1));
    d.calculation_cache.as_mut().unwrap().rows[0].error = Some("parent error".into());
    assert_eq!(
        d.validate().unwrap_err(),
        "Saved calculation row cannot contain both a result and an error"
    );
    d.calculation_cache.as_mut().unwrap().rows[0].error = None;
    d.calculation_cache.as_mut().unwrap().rows[0].display = Some("7".into());
    assert!(d.validate().unwrap_err().contains("parent"));
}
#[test]
fn placeholders_reject_fresh_results_but_keep_pending_previous_payload_and_errors() {
    for expr in [
        "<ml:placeholder/>",
        "<ml:eval><ml:placeholder/><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
        "<ml:define><ml:id>x</ml:id><ml:placeholder/></ml:define>",
    ] {
        let mut d = document();
        d.regions[0].content = CanvasContent::Opaque {
            kind: "Area".into(),
            xml: source(expr),
        };
        let mut p = payload(1);
        assert!(check(&p, &d).unwrap_err().contains("Incomplete"));
        p.rows[0].row.status = SavedCalculationStatus::Pending;
        assert!(
            check(&p, &d).is_ok(),
            "Pending previous numeric/display is inert"
        );
        install(&mut d, &p);
        d.validate().unwrap();
        let restored: CanvasDocument =
            serde_json::from_str(&serde_json::to_string(&d).unwrap()).unwrap();
        restored.validate().unwrap();
        assert_eq!(restored.calculation_cache, d.calculation_cache);
        p.rows[0].row.display = None;
        p.rows[0].row.numeric_source = None;
        p.rows[0].row.status = SavedCalculationStatus::Pending;
        assert!(check(&p, &d).is_ok());
        p.rows[0].row.status = SavedCalculationStatus::Synchronized;
        assert!(check(&p, &d).is_ok());
        p.rows[0].row.error = Some("incomplete statement".into());
        assert!(check(&p, &d).is_ok());
    }
    // The normal empty unit slot does not make a complete evaluation incomplete.
    assert!(check(&payload(1), &document()).is_ok());
}
#[test]
fn byte_limits_count_the_outer_json_once_and_signature_freshness_stays_separate() {
    let mut d = document();
    let mut p = payload(1);
    p.rows[0].row.numeric_source = None;
    p.rows[0].row.display = Some("x".repeat(3 * 1024 * 1024));
    install(&mut d, &p);
    d.validate().unwrap();
    // If nested text bytes were counted again this plus the bundle would exceed8MiB.
    let extra = d.add_math(10., 10.);
    d.calculation_cache
        .as_mut()
        .unwrap()
        .rows
        .push(SavedCalculationRow {
            numeric_source: Some(SavedNumericSource::EvaluatedValue(
                "x".repeat(4 * 1024 * 1024),
            )),
            display: None,
            ..row(extra)
        });
    d.validate().unwrap();
    d.calculation_cache.as_mut().unwrap().rows[1].numeric_source = Some(
        SavedNumericSource::EvaluatedValue("x".repeat(4 * 1024 * 1024 + 1)),
    );
    assert_eq!(
        d.validate().unwrap_err(),
        "Saved numeric source exceeds 4 MiB"
    );
    let mut huge = payload(1);
    huge.rows[0].row.display = Some("x".repeat(4 * 1024 * 1024));
    assert!(check(&huge, &document()).unwrap_err().contains("4 MiB"));
    let mut stale = document();
    install(&mut stale, &payload(1));
    stale.regions[0].left = 10.;
    stale.validate().unwrap();
    assert!(stale.saved_calculation_cache().is_none());
}
#[test]
fn row_count_and_source_depth_are_bounded_without_recursive_tree_execution() {
    let d = document();
    let mut p = payload(1);
    p.rows = vec![p.rows[0].clone(); MAX_REGIONS + 1];
    assert!(check(&p, &d).unwrap_err().contains("row count"));
    let expr = "<ml:parens>".repeat(MAX_CANVAS_TREE_DEPTH + 1)
        + "<ml:real>7</ml:real>"
        + &"</ml:parens>".repeat(MAX_CANVAS_TREE_DEPTH + 1);
    let mut deep = document();
    deep.regions[0].content = CanvasContent::Opaque {
        kind: "Area".into(),
        xml: source(&expr),
    };
    assert!(check(&payload(1), &deep).unwrap_err().contains("depth"));
}

#[test]
fn total_presentation_budget_and_ordinary_row_error_order_remain_unchanged() {
    let mut d = document();
    let mut p = payload(1);
    p.rows[0].row.numeric_source = None;
    p.rows[0].row.display = Some("x".repeat(3 * 1024 * 1024));
    install(&mut d, &p);
    for bytes in [4 * 1024 * 1024, 2 * 1024 * 1024] {
        let id = d.add_math(0., 10.);
        d.calculation_cache
            .as_mut()
            .unwrap()
            .rows
            .push(SavedCalculationRow {
                numeric_source: Some(SavedNumericSource::EvaluatedValue("x".repeat(bytes))),
                display: None,
                ..row(id)
            });
    }
    assert_eq!(
        d.validate().unwrap_err(),
        "Saved calculation presentation exceeds 8 MiB"
    );
    let mut old = CanvasDocument::default();
    let id = old.add_math(0., 0.);
    let mut r = row(id);
    r.error = Some("error".into());
    r.warnings = vec![String::new(); 1025];
    r.numeric_source = Some(SavedNumericSource::EvaluatedValue(
        "x".repeat(4 * 1024 * 1024 + 1),
    ));
    old.calculation_cache = Some(SavedCalculationCache {
        source_signature: old.calculation_signature(),
        rows: vec![r],
    });
    assert_eq!(
        old.validate().unwrap_err(),
        "Saved calculation row cannot contain both a result and an error"
    );
    let r = &mut old.calculation_cache.as_mut().unwrap().rows[0];
    r.error = None;
    assert_eq!(
        old.validate().unwrap_err(),
        "Saved calculation warnings exceed limits"
    );
    let r = &mut old.calculation_cache.as_mut().unwrap().rows[0];
    r.warnings.clear();
    assert_eq!(
        old.validate().unwrap_err(),
        "Saved numeric source exceeds 4 MiB"
    );
    let mut huge = document();
    huge.regions[0].content = CanvasContent::Opaque {
        kind: "Area".into(),
        xml: "x".repeat(8 * 1024 * 1024 + 1),
    };
    assert!(check(&payload(1), &huge).unwrap_err().contains("8 MiB"));
}
