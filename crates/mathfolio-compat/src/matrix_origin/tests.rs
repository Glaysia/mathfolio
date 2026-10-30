//! Proposed compat tests; not executed by artifact author.
use super::*;

fn worksheet(expressions: &[String]) -> String {
    editor_bridge::worksheet_xml(
        &expressions
            .iter()
            .enumerate()
            .map(|(i, e)| ((i + 1) as u64, 0., i as f64 * 30., e.clone()))
            .collect::<Vec<_>>(),
    )
    .unwrap()
}
fn real(n: &str) -> String {
    format!("<ml:real>{n}</ml:real>")
}
fn origin(n: &str, label: &str, global: bool) -> String {
    let tag = if global { "globalDefine" } else { "define" };
    format!(
        "<ml:{tag}><ml:id labels=\"{label}\">ORIGIN</ml:id>{}</ml:{tag}>",
        real(n)
    )
}
fn matrix() -> String {
    "<ml:eval><ml:matrix rows=\"2\" cols=\"2\"><ml:real>1</ml:real><ml:real>3</ml:real><ml:real>2</ml:real><ml:real>4</ml:real></ml:matrix><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>".into()
}
fn canonical(row: &RegionEvaluation) -> serde_json::Value {
    row.value.as_ref().unwrap().numeric_json()
}
fn pair(second: &str) -> String {
    worksheet(&[
        origin("-2", "SYSTEM", false),
        matrix(),
        origin(second, "SYSTEM", false),
        matrix(),
    ])
}

#[test]
fn identical_matrix_values_capture_different_actual_system_origins_by_execution_position() {
    let mut calc = WorksheetCalculator::default();
    let opened = calc.update(&pair("1"), None).unwrap();
    assert!(opened.executed_region_ids.is_empty());
    assert!(opened.rows.iter().all(|r| r.matrix_origin.is_none()));
    let out = calc.calculate(CalculationRequest::Worksheet).unwrap();
    assert_eq!(out.executed_region_ids, ["1", "2", "3", "4"]);
    assert_eq!(out.rows[1].matrix_origin, Some(-2));
    assert_eq!(out.rows[3].matrix_origin, Some(1));
    assert_eq!(canonical(&out.rows[1]), canonical(&out.rows[3]));
    assert_eq!(out.rows[0].matrix_origin, None);
    assert_eq!(out.rows[2].matrix_origin, None);
}

#[test]
fn cold_unexecuted_is_none_but_an_actually_executed_default_scope_is_known_zero() {
    let mut calc = WorksheetCalculator::default();
    let opened = calc.update(&worksheet(&[matrix()]), None).unwrap();
    assert!(
        serde_json::to_value(&opened.rows[0])
            .unwrap()
            .get("matrix_origin")
            .is_none()
    );
    let prerequisites = calc.calculate(CalculationRequest::Prerequisites).unwrap();
    assert!(prerequisites.executed_region_ids.is_empty());
    assert_eq!(prerequisites.rows[0].matrix_origin, None);
    let live = calc
        .calculate(CalculationRequest::ThroughRegion("1".into()))
        .unwrap();
    assert_eq!(live.rows[0].matrix_origin, Some(0));
    assert_eq!(
        serde_json::to_value(&live.rows[0]).unwrap()["matrix_origin"],
        0
    );
    let replay = calc
        .calculate(CalculationRequest::ThroughRegion("1".into()))
        .unwrap();
    assert!(replay.executed_region_ids.is_empty());
    assert_eq!(replay.rows[0].matrix_origin, Some(0));
}

#[test]
fn explicit_variable_origin_cannot_change_system_origin_or_equal_matrix_values() {
    let rows = evaluate_worksheet(&worksheet(&[
        origin("7", "VARIABLE", false),
        matrix(),
        origin("-3", "SYSTEM", false),
        matrix(),
    ]))
    .unwrap();
    assert_eq!(rows[1].matrix_origin, Some(0));
    assert_eq!(rows[3].matrix_origin, Some(-3));
    assert_eq!(canonical(&rows[1]), canonical(&rows[3]));
}

#[test]
fn pending_rows_hide_origin_while_retaining_old_values_and_clean_prefix_keeps_its_snapshot() {
    let mut calc = WorksheetCalculator::default();
    calc.update(&pair("1"), None).unwrap();
    let old = calc.calculate(CalculationRequest::Worksheet).unwrap();
    let pending = calc.update(&pair("5"), None).unwrap();
    assert!(pending.executed_region_ids.is_empty());
    assert_eq!(pending.pending_region_ids, ["3", "4"]);
    assert_eq!(pending.rows[1].matrix_origin, Some(-2));
    assert_eq!(pending.rows[3].matrix_origin, None);
    assert_eq!(canonical(&pending.rows[3]), canonical(&old.rows[3]));
    let prefix = calc
        .calculate(CalculationRequest::ThroughRegion("2".into()))
        .unwrap();
    assert!(prefix.executed_region_ids.is_empty());
    assert_eq!(prefix.rows[1].matrix_origin, Some(-2));
    assert_eq!(prefix.rows[3].matrix_origin, None);
    let final_rows = calc
        .calculate(CalculationRequest::ThroughRegion("4".into()))
        .unwrap();
    assert_eq!(final_rows.executed_region_ids, ["3", "4"]);
    assert_eq!(final_rows.rows[1].matrix_origin, Some(-2));
    assert_eq!(final_rows.rows[3].matrix_origin, Some(5));
    assert_eq!(canonical(&final_rows.rows[3]), canonical(&old.rows[3]));
}

#[test]
fn source_reparse_with_same_program_retains_clean_metadata_without_execution() {
    let mut calc = WorksheetCalculator::default();
    let xml = worksheet(&[matrix()]);
    calc.update(&xml, None).unwrap();
    let first = calc.calculate(CalculationRequest::Worksheet).unwrap();
    // Comments alter container bytes but are excluded from the same source program.
    let changed = xml.replace("</worksheet>", "<!-- inert -->\n</worksheet>");
    assert_ne!(changed, xml);
    let clean = calc.update(&changed, None).unwrap();
    assert!(clean.executed_region_ids.is_empty() && clean.pending_region_ids.is_empty());
    assert_eq!(clean.rows[0].matrix_origin, Some(0));
    assert_eq!(canonical(&clean.rows[0]), canonical(&first.rows[0]));
}

#[test]
fn unsupported_system_origin_never_poison_successful_literal_matrix_or_invents_i32() {
    for n in ["1.5", "2147483648", "-2147483649"] {
        let rows = evaluate_worksheet(&worksheet(&[origin(n, "SYSTEM", false), matrix()])).unwrap();
        assert!(rows[1].error.is_none() && matches!(rows[1].value, Some(Value::Matrix { .. })));
        assert_eq!(rows[1].matrix_origin, None);
    }
    for n in ["-2147483648", "2147483647"] {
        let rows = evaluate_worksheet(&worksheet(&[origin(n, "SYSTEM", false), matrix()])).unwrap();
        assert_eq!(rows[1].matrix_origin, n.parse::<i32>().ok());
    }
}

#[test]
fn failed_complex_unitful_and_nonscalar_origin_only_hide_metadata() {
    let bodies = [
        "<ml:id labels=\"VARIABLE\">missing</ml:id>".to_owned(),
        "<ml:imag>2</ml:imag>".into(),
        "<ml:apply><ml:scale/><ml:real>1</ml:real><ml:id labels=\"UNIT\">m</ml:id></ml:apply>"
            .into(),
        "<ml:matrix rows=\"1\" cols=\"1\"><ml:real>1</ml:real></ml:matrix>".into(),
    ];
    for body in bodies {
        let def = format!("<ml:define><ml:id labels=\"SYSTEM\">ORIGIN</ml:id>{body}</ml:define>");
        let rows = evaluate_worksheet(&worksheet(&[def, matrix()])).unwrap();
        assert!(rows[1].error.is_none());
        assert!(matches!(rows[1].value, Some(Value::Matrix { .. })));
        assert_eq!(rows[1].matrix_origin, None);
    }
}

#[test]
fn error_and_cancel_publications_cannot_retain_a_known_old_origin() {
    let mut calc = WorksheetCalculator::default();
    let xml = worksheet(&[matrix()]);
    calc.update(&xml, None).unwrap();
    let good = calc.calculate(CalculationRequest::Worksheet).unwrap();
    assert_eq!(good.rows[0].matrix_origin, Some(0));
    let stop = CalculationControl::default();
    stop.cancel();
    let cancelled = calc
        .calculate_with_control(CalculationRequest::Worksheet, &stop)
        .unwrap();
    assert!(cancelled.cancelled && cancelled.executed_region_ids.is_empty());
    assert_eq!(cancelled.rows[0].matrix_origin, None);
    assert_eq!(canonical(&cancelled.rows[0]), canonical(&good.rows[0]));
    let bad=worksheet(&["<ml:eval><ml:id labels=\"VARIABLE\">missing</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>".into()]);
    calc.update(&bad, None).unwrap();
    let failed = calc.calculate(CalculationRequest::Worksheet).unwrap();
    assert!(failed.rows[0].error.is_some() && failed.rows[0].value.is_none());
    assert_eq!(failed.rows[0].matrix_origin, None);
}

#[test]
fn later_global_origin_is_captured_from_actual_rust_prerequisite_scope() {
    let mut calc = WorksheetCalculator::default();
    calc.update(&worksheet(&[matrix(), origin("3", "SYSTEM", true)]), None)
        .unwrap();
    let prefix = calc
        .calculate(CalculationRequest::ThroughRegion("1".into()))
        .unwrap();
    assert_eq!(prefix.executed_region_ids, ["2", "1"]);
    assert_eq!(prefix.rows[0].matrix_origin, Some(3));
    assert_eq!(prefix.pending_region_ids, ["2"]);
    assert_eq!(prefix.rows[1].matrix_origin, None);
}
