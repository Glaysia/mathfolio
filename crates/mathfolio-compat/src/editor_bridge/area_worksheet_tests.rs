use super::*;
use crate::{CalculationRequest, Value, WorksheetCalculator};
fn define(name: &str, value: &str) -> String {
    format!("<ml:define><ml:id>{name}</ml:id><ml:real>{value}</ml:real></ml:define>")
}
fn query(name: &str) -> String {
    format!(
        "<ml:eval><ml:id>{name}</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>"
    )
}
fn child(id: &str, left: f64, top: f64, math: &str) -> String {
    format!(
        "<region region-id='{id}' left='{left}' top='{top}'><math resultRef='cache-{id}'>{math}</math></region>"
    )
}
fn area(attrs: &str, body: &str) -> String {
    format!(
        "<region region-id='50' left='0' top='20'><Area {attrs}><regions>{body}</regions></Area></region>"
    )
}
fn rows(source: &str) -> Vec<crate::RegionEvaluation> {
    crate::evaluate_worksheet(source).unwrap()
}
fn scalar(row: &crate::RegionEvaluation) -> f64 {
    assert!(row.error.is_none(), "{row:?}");
    let Some(Value::Scalar { quantity }) = &row.value else {
        panic!("{row:?}")
    };
    assert!(quantity.is_scalar());
    quantity.value.re
}
#[test]
fn area_variables_label_following_raw_math_before_unit_inference_and_keep_native_bytes() {
    let fragment = area(
        "IsCollapsed='true'",
        &child("1", 0., 1000., &define("m", "7")),
    );
    let raw = [(100, 0., 30., query("m"))];
    let source = worksheet_xml_with_areas(&raw, std::slice::from_ref(&fragment)).unwrap();
    let result = rows(&source);
    assert_eq!(
        result
            .iter()
            .map(|r| r.region_id.as_str())
            .collect::<Vec<_>>(),
        ["1", "100"]
    );
    assert_eq!(scalar(&result[1]), 7.);
    assert!(source.contains("left='0' top='1000'"));
    assert!(source.contains("resultRef='cache-1'"));
    assert!(source.contains("IsCollapsed='true'"));
    // Independent explicit-label source oracle, composed without our new builder.
    let expected = format!(
        "<worksheet xmlns='{WS}' xmlns:ml='{ML}'><regions>{}<region region-id='100' left='0' top='30'><math resultRef='100'>{}</math></region></regions></worksheet>",
        fragment.replace("<ml:id>m</ml:id>", "<ml:id labels='VARIABLE'>m</ml:id>"),
        query("m").replace("<ml:id>m</ml:id>", "<ml:id labels='VARIABLE'>m</ml:id>")
    );
    assert_eq!(
        serde_json::to_value(result).unwrap(),
        serde_json::to_value(rows(&expected)).unwrap()
    );
}
#[test]
fn local_y_x_order_and_previous_outer_definitions_share_one_label_environment() {
    let fragment = area(
        "",
        &(child("q-local", 20., 0., &query("m")) + &child("d-local", 0., 0., &define("m", "4"))),
    );
    let raw = [
        (100, 0., 30., query("m")),
        (101, 0., 0., define("before", "2")),
    ];
    let source = worksheet_xml_with_areas(&raw, &[fragment]).unwrap();
    let result = rows(&source);
    assert_eq!(
        result
            .iter()
            .map(|r| r.region_id.as_str())
            .collect::<Vec<_>>(),
        ["101", "d-local", "q-local", "100"]
    );
    assert_eq!(scalar(&result[2]), 4.);
    assert_eq!(scalar(&result[3]), 4.);
}
#[test]
fn active_globals_inside_area_precede_early_queries_but_disabled_and_unknown_area_do_not_leak() {
    let global = define("m", "9").replace("ml:define", "ml:globalDefine");
    let raw = [(100, 0., 0., query("m"))];
    let source = worksheet_xml_with_areas(&raw, &[area("", &child("g", 0., 0., &global))]).unwrap();
    assert_eq!(scalar(&rows(&source)[0]), 9.);
    for flags in ["disable-calc='true'", "future='unknown'"] {
        let source =
            worksheet_xml_with_areas(&raw, &[area(flags, &child("g", 0., 0., &global))]).unwrap();
        let r = rows(&source);
        assert_eq!(r.len(), 2);
        assert!(r[1].error.is_some());
        let Some(Value::Scalar { quantity }) = &r[0].value else {
            panic!()
        };
        assert_eq!(
            quantity.dimension,
            mathfolio_core::unit("m").unwrap().dimension
        );
        assert_eq!(quantity.value.re, 1.);
    }
    let disabled = child("g", 0., 0., &global).replace("<math ", "<math disable-calc='true' ");
    let source = worksheet_xml_with_areas(&raw, &[area("IsCollapsed='true'", &disabled)]).unwrap();
    let r = rows(&source);
    assert_eq!(r.len(), 1);
    let Some(Value::Scalar { quantity }) = &r[0].value else {
        panic!()
    };
    assert_eq!(
        quantity.dimension,
        mathfolio_core::unit("m").unwrap().dimension
    );
}
#[test]
fn inherited_namespace_aliases_and_math_format_siblings_survive_label_patches() {
    let native = format!(
        "<w:region xmlns:w='{WS}' xmlns:q='{ML}' xmlns:u='urn:future&amp;kept' region-id='50' left='0' top='20'><w:Area IsCollapsed='false'><w:regions><!--original--><w:region region-id='1' left='3' top='4'><w:math resultRef='17'><q:define><q:id>m</q:id><q:real>6</q:real></q:define><w:resultFormat u:keep='yes'/><w:formattingOverride/></w:math></w:region></w:regions></w:Area></w:region>"
    );
    let source = worksheet_xml_with_areas(&[(100, 0., 30., query("m"))], &[native]).unwrap();
    assert_eq!(scalar(&rows(&source)[1]), 6.);
    assert!(source.contains("<!--original-->"));
    assert!(source.contains("<w:resultFormat u:keep='yes'/><w:formattingOverride/>"));
    assert!(source.contains("resultRef='17'"));
    assert!(source.contains("xmlns:u='urn:future&amp;kept'"));
}
#[test]
fn explicit_unit_labels_and_literal_subscripts_are_not_overwritten_by_area_definitions() {
    let xaml = "http://schemas.microsoft.com/winfx/2006/xaml/presentation";
    let ptc = "clr-namespace:Ptc.Wpf;assembly=Ptc.Core";
    let named = format!(
        "<ml:id xmlns:p='{xaml}' xmlns:t='{ptc}' labels='*' label-is-contextual='true'><p:Span>V<t:Subscript>dc</t:Subscript></p:Span></ml:id>"
    );
    let expr = format!("<ml:define>{named}<ml:real>8</ml:real></ml:define>");
    let querysub =
        format!("<ml:eval>{named}<ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>");
    let raw = [
        (100, 0., 30., querysub),
        (
            101,
            0.,
            40.,
            query("m").replace("<ml:id>m</ml:id>", "<ml:id labels='UNIT'>m</ml:id>"),
        ),
    ];
    let source = worksheet_xml_with_areas(
        &raw,
        &[area(
            "",
            &(child("d", 0., 0., &expr) + &child("m", 0., 10., &define("m", "99"))),
        )],
    )
    .unwrap();
    let r = rows(&source);
    assert_eq!(scalar(&r[2]), 8.);
    let Some(Value::Scalar { quantity }) = &r[3].value else {
        panic!()
    };
    assert_eq!(quantity.value.re, 1.);
    assert_eq!(
        quantity.dimension,
        mathfolio_core::unit("m").unwrap().dimension
    );
    assert!(!source.contains("label-is-contextual='true'"));
    let doc = roxmltree::Document::parse(&source).unwrap();
    assert_eq!(
        doc.descendants()
            .filter(|n| n.has_tag_name((PTC, "Subscript")) && n.text() == Some("dc"))
            .count(),
        2
    );
}
#[test]
fn table_labels_are_accumulated_and_solveblock_raw_source_is_not_wrapped_as_math() {
    let table = "<spec-table><math><ml:define><ml:id>m</ml:id><ml:apply><ml:scale/><ml:matrix rows='1' cols='1'><ml:real>11</ml:real></ml:matrix><ml:placeholder/></ml:apply></ml:define></math></spec-table>";
    let native = area(
        "",
        &format!("<region region-id='t' left='0' top='0'>{table}</region>"),
    );
    let solve="<region region-id='solve' left='0' top='100'><solveblock future='guard'><regions/></solveblock></region>".to_string();
    let source =
        worksheet_xml_with_areas(&[(100, 0., 30., query("m"))], &[native, solve.clone()]).unwrap();
    let r = rows(&source);
    assert_eq!(scalar(&r[1]), 11.);
    assert!(r[2].error.is_some());
    assert!(source.contains(&solve));
}
#[test]
fn complete_region_guards_reject_id_collisions_fragment_escape_and_namespace_conflicts() {
    let a = area("", &child("1", 0., 0., &define("x", "2")));
    assert!(
        worksheet_xml_with_areas(&[(50, 0., 0., query("x"))], std::slice::from_ref(&a))
            .unwrap_err()
            .contains("Duplicate")
    );
    assert!(
        worksheet_xml_with_areas(&[(1, 0., 0., query("x"))], std::slice::from_ref(&a))
            .unwrap_err()
            .contains("Duplicate")
    );
    let missing = a.replace("region-id='1'", "");
    assert!(worksheet_xml_with_areas(&[], &[missing]).is_err());
    let conflict = a
        .replace("<Area ", "<Area xmlns:ml='urn:wrong' ")
        .replace("<ml:", "<q:")
        .replace("</ml:", "</q:")
        .replace("<Area ", &format!("<Area xmlns:q='{ML}' "));
    assert!(worksheet_xml_with_areas(&[], &[conflict]).is_err());
    for invalid in [
        "<math/>".to_string(),
        a.clone() + &a,
        a.replace("left='0' top='20'", "left='NaN' top='20'"),
    ] {
        assert!(worksheet_xml_with_areas(&[], &[invalid]).is_err());
    }
    assert!(
        worksheet_xml_with_areas(
            &[(
                2,
                0.,
                0.,
                "</context></region><region region-id='9' left='0' top='0'><context>".into()
            )],
            &[a]
        )
        .is_err()
    );
}
#[test]
fn normal_builder_is_byte_identical_and_size_budgets_remain_bounded() {
    let raw = [(10, 0., 0., define("x", "3")), (20, 0., 10., query("x"))];
    assert_eq!(
        worksheet_xml_with_areas(&raw, &[]).unwrap(),
        worksheet_xml(&raw).unwrap()
    );
    let a = area("", "");
    assert!(
        worksheet_xml_with_areas(
            &[(1, 0., 0., "x".repeat(MAX_XML + 1))],
            std::slice::from_ref(&a)
        )
        .is_err()
    );
    assert!(worksheet_xml_with_areas(&[], &["x".repeat(MAX_XML + 1)]).is_err());
    let source = worksheet_xml_with_areas(&raw, &[a]).unwrap();
    let mut calc = WorksheetCalculator::default();
    let initial = calc.update(&source, None).unwrap();
    assert!(initial.executed_region_ids.is_empty());
    let done = calc.calculate(CalculationRequest::Worksheet).unwrap();
    assert_eq!(scalar(&done.rows[1]), 3.);
}

#[test]
fn a_conflicting_descendant_ml_scope_cannot_rebind_generated_identifier_patches() {
    let expression = format!(
        "<q:define xmlns:q='{ML}'><q:id xmlns:ml='urn:not-math'>x</q:id><q:real>3</q:real></q:define>"
    );
    let native = area("", &child("conflict", 0., 0., &expression));
    assert!(
        worksheet_xml_with_areas(&[], &[native])
            .unwrap_err()
            .contains("Conflicting ml")
    );
    assert!(
        worksheet_xml_with_areas(&[(100, 0., 0., expression)], &[area("", "")])
            .unwrap_err()
            .contains("Conflicting ml")
    );
}
