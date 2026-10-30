//! Independent source/order oracles for the static Area contract (analysis/0133).
use mathfolio_compat::{
    CalculationRequest, CalculationUpdate, PlacementGrid, Value, WorksheetCalculator,
    evaluate_worksheet,
};
const WS: &str = "http://schemas.mathsoft.com/worksheet50";
const ML: &str = "http://schemas.mathsoft.com/math50";
fn sheet(body: &str) -> String {
    format!("<worksheet xmlns='{WS}' xmlns:ml='{ML}'><regions>{body}</regions></worksheet>")
}
fn region(id: &str, left: f64, top: f64, body: &str) -> String {
    format!("<region region-id='{id}' left='{left}' top='{top}'>{body}</region>")
}
fn math(id: &str, left: f64, top: f64, expr: &str) -> String {
    region(
        id,
        left,
        top,
        &format!("<math resultRef='cache-{id}'>{expr}</math>"),
    )
}
fn define(value: &str) -> String {
    format!("<ml:define><ml:id labels='VARIABLE'>x</ml:id><ml:real>{value}</ml:real></ml:define>")
}
fn query() -> String {
    "<ml:eval><ml:id labels='VARIABLE'>x</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>".into()
}
fn area(id: &str, top: f64, attrs: &str, body: &str) -> String {
    region(
        id,
        0.,
        top,
        &format!("<Area {attrs}><regions>{body}</regions></Area>"),
    )
}
fn number(update: &CalculationUpdate, id: &str) -> f64 {
    let row = update.rows.iter().find(|r| r.region_id == id).unwrap();
    assert!(row.error.is_none(), "{:?}", row.error);
    let Some(Value::Scalar { quantity }) = &row.value else {
        panic!("no scalar {id}");
    };
    quantity.value.re
}
fn calculated(xml: &str) -> CalculationUpdate {
    let mut c = WorksheetCalculator::default();
    let open = c.update(xml, None).unwrap();
    assert!(open.executed_region_ids.is_empty());
    assert!(open.rows.iter().all(|r| r.value.is_none()));
    c.calculate(CalculationRequest::Worksheet).unwrap()
}

#[test]
fn owning_slot_is_a_block_and_rows_keep_local_geometry_ids_and_results() {
    let children = math("inner-query", 0., 2000., &query())
        + &math("inner-definition", 0., 1000., &define("2"));
    // Intentionally unsorted XML; globally adding Area top to child top would
    // execute the outer query too early and produce 1 instead of 2.
    let xml = sheet(
        &(math("outer-query", 0., 30., &query())
            + &area("container", 20., "IsCollapsed='true'", &children)
            + &math("initial", 0., 0., &define("1"))),
    );
    let update = calculated(&xml);
    assert_eq!(
        update.executed_region_ids,
        ["initial", "inner-definition", "inner-query", "outer-query"]
    );
    assert_eq!(number(&update, "inner-query"), 2.);
    assert_eq!(number(&update, "outer-query"), 2.);
    assert_eq!(update.rows[1].top, 1000.);
    assert_eq!(update.rows[1].left, 0.);
    assert_eq!(
        update.rows[1].result_ref.as_deref(),
        Some("cache-inner-definition")
    );
    assert!(update.rows.iter().all(|r| r.region_id != "container"));
}
#[test]
fn siblings_sort_local_y_then_x_and_multiple_areas_keep_their_slots() {
    let first = math("q1", 30., 10., &query())
        + &math("d2", 20., 10., &define("2"))
        + &math("d1", 0., 0., &define("1"));
    let second = math("q2", 0., 1., &query()) + &math("d3", 0., 0., &define("3"));
    let result = calculated(&sheet(
        &(area("b", 200., "", &second) + &area("a", 100., "", &first)),
    ));
    assert_eq!(result.executed_region_ids, ["d1", "d2", "q1", "d3", "q2"]);
    assert_eq!(number(&result, "q1"), 2.);
    assert_eq!(number(&result, "q2"), 3.);
}
#[test]
fn collapse_border_height_changes_neither_bindings_nor_pending_cache() {
    let xml = sheet(&area(
        "a",
        20.,
        "IsCollapsed='false' IsBorderVisible='true' AreaHeight='300'",
        &(math("d", 0., 50., &define("7")) + &math("q", 0., 100., &query())),
    ));
    let mut c = WorksheetCalculator::default();
    c.update(&xml, None).unwrap();
    c.calculate(CalculationRequest::Worksheet).unwrap();
    let changed = xml
        .replace("IsCollapsed='false'", "IsCollapsed='true'")
        .replace("IsBorderVisible='true'", "IsBorderVisible='false'")
        .replace("AreaHeight='300'", "AreaHeight='20'");
    let update = c.update(&changed, None).unwrap();
    assert!(update.pending_region_ids.is_empty());
    assert_eq!(number(&update, "q"), 7.);
    assert!(
        c.calculate(CalculationRequest::ThroughRegion("q".into()))
            .unwrap()
            .executed_region_ids
            .is_empty()
    );
}
#[test]
fn child_disable_is_independent_of_collapse_and_does_not_publish_a_binding() {
    let disabled = region(
        "disabled",
        0.,
        0.,
        &format!("<math disable-calc='true'>{}</math>", define("99")),
    );
    let enabled = region(
        "enabled",
        0.,
        20.,
        &format!("<math disable-calc='0' resultRef='r'>{}</math>", query()),
    );
    let xml = sheet(
        &(math("before", 0., 0., &define("3"))
            + &area("a", 20., "IsCollapsed='true'", &(disabled + &enabled))),
    );
    let update = calculated(&xml);
    assert_eq!(update.executed_region_ids, ["before", "enabled"]);
    assert_eq!(number(&update, "enabled"), 3.);
    // The existing direct top-level metadata diagnostic remains unchanged.
    assert!(
        evaluate_worksheet(&sheet(&region(
            "top",
            0.,
            0.,
            &format!("<math disable-calc='true'>{}</math>", define("99"))
        )))
        .unwrap()[0]
            .error
            .is_some()
    );
}
#[test]
fn unknown_parent_flags_nested_and_protected_content_fail_atomically() {
    for contents in [
        "<Area disable-calc='true'><regions>{children}</regions></Area>",
        "<Area disable-calc='maybe'><regions>{children}</regions></Area>",
        "<Area future='x'><regions>{children}</regions></Area>",
        "<Area><regions>{children}</regions></Area><math><ml:define><ml:id labels='VARIABLE'>x</ml:id><ml:real>99</ml:real></ml:define></math>",
        "<Area IsCollapsed='maybe'><regions>{children}</regions></Area>",
        "<Area AreaHeight='NaN'><regions>{children}</regions></Area>",
        "<Area><regions>{children}<region region-id='nested' left='0' top='10'><Area><regions/></Area></region></regions></Area>",
        "<Area><regions>{children}</regions><ProtectionData ProtectionLevel='DisplayState'/></Area>",
    ] {
        let body = contents.replace(
            "{children}",
            &math("must-not-execute", 0., 0., &define("99")),
        );
        let xml = sheet(
            &(math("before", 0., 0., &define("3"))
                + &region("a", 0., 20., &body)
                + &math("after", 0., 30., &query())),
        );
        let update = calculated(&xml);
        assert_eq!(update.executed_region_ids, ["before", "a", "after"]);
        assert!(update.rows[1].error.is_some(), "{contents}");
        assert_eq!(number(&update, "after"), 3.);
    }
}
#[test]
fn child_edits_preserve_old_payload_and_calculation_commands_use_hierarchical_prefix() {
    let xml = sheet(&area(
        "a",
        100.,
        "",
        &(math("d", 0., 1000., &define("2"))
            + &math("q1", 0., 2000., &query())
            + &math("q2", 0., 3000., &query())),
    ));
    let mut c = WorksheetCalculator::default();
    c.update(&xml, None).unwrap();
    c.calculate(CalculationRequest::Worksheet).unwrap();
    let changed = c
        .update(
            &xml.replace("<ml:real>2</ml:real>", "<ml:real>8</ml:real>"),
            None,
        )
        .unwrap();
    assert_eq!(changed.pending_region_ids, ["d", "q1", "q2"]);
    assert_eq!(number(&changed, "q1"), 2.);
    let prefix = c
        .calculate(CalculationRequest::ThroughRegion("q1".into()))
        .unwrap();
    assert_eq!(prefix.executed_region_ids, ["d", "q1"]);
    assert_eq!(prefix.pending_region_ids, ["q2"]);
    assert_eq!(number(&prefix, "q1"), 8.);
    let active = c
        .calculate(CalculationRequest::ActiveRegion("q1".into()))
        .unwrap();
    assert_eq!(active.executed_region_ids, ["q1"]);
    assert_eq!(active.pending_region_ids, ["q2"]);
}
#[test]
fn coordinate_based_external_context_respects_container_slot_not_child_local_y() {
    let xml = sheet(
        &(math("d0", 0., 0., &define("1"))
            + &area("a", 20., "", &math("d1", 0., 1000., &define("2")))
            + &math("d2", 0., 40., &define("3"))),
    );
    let mut c = WorksheetCalculator::default();
    c.update(&xml, None).unwrap();
    c.calculate(CalculationRequest::ThroughRegion("d1".into()))
        .unwrap();
    for (top, expected) in [(10., 1.), (30., 2.)] {
        let context = c.context_before(0., top).unwrap();
        let Value::Scalar { quantity } = context.evaluate_mathml(&query()).unwrap() else {
            panic!()
        };
        assert_eq!(quantity.value.re, expected);
    }
    assert!(c.context_before(0., 50.).is_err());
}
#[test]
fn malformed_child_coordinates_abort_update_without_destroying_previous_cache() {
    let xml = sheet(&area(
        "a",
        100.,
        "",
        &math(
            "q",
            0.,
            0.,
            "<ml:eval><ml:real>7</ml:real><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
        ),
    ));
    let mut c = WorksheetCalculator::default();
    c.update(&xml, None).unwrap();
    c.calculate(CalculationRequest::Worksheet).unwrap();
    assert_eq!(
        c.update(&xml.replace("left='0' top='0'", "left='NaN' top='0'"), None)
            .unwrap_err(),
        "Nonfinite left"
    );
    let old = c.update(&xml, None).unwrap();
    assert!(old.pending_region_ids.is_empty());
    assert_eq!(number(&old, "q"), 7.);
}
#[test]
fn empty_area_is_no_calculation_row_and_child_grid_snap_keeps_local_coordinates() {
    let xml = sheet(
        &(area("empty", 0., "", "")
            + &area(
                "a",
                10.,
                "disable-calc='false'",
                &(math("d", 0., 0., &define("7")) + &math("q", 9.7, 19.3, &query())),
            )),
    );
    let mut c = WorksheetCalculator::default();
    let opened=c.update(&xml,Some(PlacementGrid::from_presentation("<p:presentation xmlns:p='http://schemas.ptc.com/mathcad/settings/presentation10'><p:pageModel paper-code='Letter' grid-size='Fine'/></p:presentation>").unwrap())).unwrap();
    assert_eq!(opened.rows.len(), 2);
    assert!((opened.rows[1].left - 9.6).abs() < 1e-12);
    assert!((opened.rows[1].top - 19.2).abs() < 1e-12);
    assert_eq!(
        number(&c.calculate(CalculationRequest::Worksheet).unwrap(), "q"),
        7.
    );
}

#[test]
fn area_globals_are_prerequisites_without_publishing_future_local_rows() {
    let global =
        "<ml:globalDefine><ml:id labels='VARIABLE'>g</ml:id><ml:real>7</ml:real></ml:globalDefine>";
    let gquery = query().replace(">x<", ">g<");
    let xml = sheet(
        &(math("early", 0., 0., &gquery)
            + &area(
                "a",
                100.,
                "IsCollapsed='true'",
                &(math("global", 0., 2000., global) + &math("future", 0., 3000., &gquery)),
            )),
    );
    let mut c = WorksheetCalculator::default();
    let open = c.update(&xml, None).unwrap();
    assert_eq!(open.pending_region_ids, ["early", "global", "future"]);
    let result = c
        .calculate(CalculationRequest::ActiveRegion("early".into()))
        .unwrap();
    assert_eq!(result.executed_region_ids, ["global", "early"]);
    assert_eq!(number(&result, "early"), 7.);
    assert_eq!(result.pending_region_ids, ["global", "future"]);
    assert!(result.rows[2].value.is_none());
}
#[test]
fn child_disable_transition_invalidates_downstream_and_reenabling_restores_original_identity() {
    let make = |disabled| {
        sheet(
            &(math("outer-definition", 0., 0., &define("3"))
                + &area(
                    "a",
                    20.,
                    "",
                    &region(
                        "inner-definition",
                        0.,
                        0.,
                        &format!(
                            "<math disable-calc='{disabled}' resultRef='child-result'>{}</math>",
                            define("99")
                        ),
                    ),
                )
                + &math("q", 0., 30., &query())),
        )
    };
    let mut c = WorksheetCalculator::default();
    c.update(&make("false"), None).unwrap();
    assert_eq!(
        number(&c.calculate(CalculationRequest::Worksheet).unwrap(), "q"),
        99.
    );
    let pending = c.update(&make("true"), None).unwrap();
    assert_eq!(pending.pending_region_ids, ["q"]);
    assert_eq!(number(&pending, "q"), 99.);
    assert_eq!(
        number(
            &c.calculate(CalculationRequest::ThroughRegion("q".into()))
                .unwrap(),
            "q"
        ),
        3.
    );
    let reenabled = c.update(&make("0"), None).unwrap();
    assert_eq!(reenabled.pending_region_ids, ["inner-definition", "q"]);
    assert_eq!(
        reenabled.rows[1].result_ref.as_deref(),
        Some("child-result")
    );
    assert_eq!(
        number(
            &c.calculate(CalculationRequest::ThroughRegion("q".into()))
                .unwrap(),
            "q"
        ),
        99.
    );
}
#[test]
fn expanded_region_budget_is_bounded_and_parse_error_is_atomic() {
    let initial = sheet(&math(
        "q",
        0.,
        0.,
        "<ml:eval><ml:real>7</ml:real><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
    ));
    let mut c = WorksheetCalculator::default();
    c.update(&initial, None).unwrap();
    c.calculate(CalculationRequest::Worksheet).unwrap();
    let body = "<region left='0' top='0'><math><ml:real>1</ml:real></math></region>".repeat(10000);
    assert_eq!(
        c.update(&sheet(&area("a", 0., "", &body)), None)
            .unwrap_err(),
        "Region budget exceeded"
    );
    let old = c.update(&initial, None).unwrap();
    assert!(old.pending_region_ids.is_empty());
    assert_eq!(number(&old, "q"), 7.);
}
