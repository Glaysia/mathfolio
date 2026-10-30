//! Self-authored regression fixtures for the bounded None-label resolver.
//! Original evidence establishes None metadata/fallback, not complete wildcard lookup.
use mathfolio_compat::{
    RegionEvaluation, Value,
    editor_bridge::{WorksheetXmlCache, tree_from_mathml, tree_to_mathml, worksheet_xml},
    evaluate_worksheet,
};

const ML: &str = "http://schemas.mathsoft.com/math50";
const WS: &str = "http://schemas.mathsoft.com/worksheet50";
type Region = (u64, f64, f64, String);
fn id(name: &str) -> String {
    format!("<ml:id labels=\"*\">{name}</ml:id>")
}
fn eval(expression: &str) -> String {
    format!("<ml:eval>{expression}<ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>")
}
fn define(name: &str, body: &str) -> String {
    format!("<ml:define>{}{body}</ml:define>", id(name))
}
fn regions(fragments: &[String]) -> Vec<Region> {
    fragments
        .iter()
        .enumerate()
        .map(|(i, source)| (i as u64, 0., i as f64 * 100., source.clone()))
        .collect()
}
fn run(fragments: &[String]) -> Vec<RegionEvaluation> {
    let rows = evaluate_worksheet(&worksheet_xml(&regions(fragments)).unwrap()).unwrap();
    assert!(rows.iter().all(|row| row.error.is_none()), "{rows:?}");
    rows
}
fn scalar(row: &RegionEvaluation) -> mathfolio_core::Quantity {
    let Some(Value::Scalar { quantity }) = row.value else {
        panic!("{row:?}")
    };
    quantity
}
fn labels(xml: &str) -> Vec<(String, String)> {
    roxmltree::Document::parse(xml)
        .unwrap()
        .descendants()
        .filter(|node| node.has_tag_name((ML, "id")))
        .map(|node| {
            (
                node.text().unwrap_or("").into(),
                node.attribute("labels").unwrap_or("").into(),
            )
        })
        .collect()
}
fn oracle(cache: &mut WorksheetXmlCache, source: &[Region]) -> String {
    let expected = worksheet_xml(source).unwrap();
    assert_eq!(cache.build(source).unwrap(), expected);
    assert_eq!(
        cache.build_prepared(source).unwrap().into_parts().0,
        expected
    );
    expected
}

#[test]
fn none_metadata_and_literal_suffix_roundtrip_without_becoming_contextual() {
    for contextual in [
        "",
        " label-is-contextual=\"false\"",
        " label-is-contextual=\"true\"",
    ] {
        for content in [
            "m",
            "<p:Span xmlns:p=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\">ORIGIN<ptc:Subscript xmlns:ptc=\"clr-namespace:Ptc.Wpf;assembly=Ptc.Core\">in</ptc:Subscript></p:Span>",
        ] {
            let raw = format!("<ml:id labels=\"*\"{contextual}>{content}</ml:id>");
            let tree = tree_from_mathml(&raw).unwrap();
            let stored = tree_to_mathml(&tree).unwrap();
            assert!(stored.contains("labels=\"*\""));
            assert_eq!(
                stored.contains("label-is-contextual="),
                !contextual.is_empty()
            );
            assert_eq!(tree_from_mathml(&stored).unwrap(), tree);
            let input = regions(&[eval(&raw)]);
            let saved = input.clone();
            let calculated = worksheet_xml(&input).unwrap();
            assert!(!calculated.contains("labels=\"*\""));
            assert_eq!(input, saved);
            if content != "m" {
                assert!(calculated.contains("labels=\"VARIABLE\""));
            }
        }
    }
}

#[test]
fn none_uses_visible_definitions_but_explicit_six_labels_remain_exact() {
    let rows = run(&[
        eval(&id("m")),
        define("m", "<ml:real>7</ml:real>"),
        eval(&id("m")),
    ]);
    assert_eq!(
        scalar(&rows[0]).dimension,
        mathfolio_core::unit("m").unwrap().dimension
    );
    assert!(scalar(&rows[2]).is_scalar());
    assert_eq!(scalar(&rows[2]).value.re, 7.);
    for label in [
        "VARIABLE", "UNIT", "CONSTANT", "FUNCTION", "SYSTEM", "KEYWORD",
    ] {
        for flag in ["", " label-is-contextual=\"false\""] {
            let raw = format!("<ml:id labels=\"{label}\"{flag}>m</ml:id>");
            let output =
                worksheet_xml(&regions(&[define("m", "<ml:real>7</ml:real>"), eval(&raw)]))
                    .unwrap();
            assert_eq!(labels(&output).last().unwrap().1, label);
            assert_eq!(
                tree_from_mathml(&tree_to_mathml(&tree_from_mathml(&raw).unwrap()).unwrap())
                    .unwrap(),
                tree_from_mathml(&raw).unwrap()
            );
        }
    }
}

#[test]
fn cached_none_dependencies_follow_new_local_and_future_global_declarations() {
    let mut cache = WorksheetXmlCache::default();
    let mut source = vec![(5, 0., 100., eval(&id("m")))];
    assert_eq!(labels(&oracle(&mut cache, &source))[0].1, "UNIT");
    source.insert(0, (4, 0., 0., define("m", "<ml:real>7</ml:real>")));
    assert_eq!(
        labels(&oracle(&mut cache, &source)).last().unwrap().1,
        "VARIABLE"
    );
    source[0].3 =
        "<ml:define><ml:id labels=\"UNIT\">m</ml:id><ml:real>7</ml:real></ml:define>".into();
    assert_eq!(
        labels(&oracle(&mut cache, &source)).last().unwrap().1,
        "UNIT"
    );
    source.remove(0);
    source.push((
        6,
        0.,
        200.,
        format!(
            "<ml:globalDefine>{}<ml:real>9</ml:real></ml:globalDefine>",
            id("m")
        ),
    ));
    let output = oracle(&mut cache, &source);
    assert!(labels(&output).iter().all(|(_, label)| label == "VARIABLE"));
    assert_eq!(
        scalar(&evaluate_worksheet(&output).unwrap()[0]).value.re,
        9.
    );
    source.pop();
    assert_eq!(labels(&oracle(&mut cache, &source))[0].1, "UNIT");
}

#[test]
fn function_and_calculus_binders_shadow_units_only_within_their_scope() {
    let function = format!(
        "<ml:define><ml:function>{}<ml:boundVars>{}</ml:boundVars></ml:function>{}</ml:define>",
        id("f"),
        id("m"),
        id("m")
    );
    let call = format!("<ml:apply>{}<ml:real>9</ml:real></ml:apply>", id("f"));
    let derivative = format!(
        "<ml:apply><ml:derivative/><ml:lambda><ml:boundVars>{}</ml:boundVars><ml:apply><ml:pow/>{}<ml:real>2</ml:real></ml:apply></ml:lambda><ml:degree><ml:real>1</ml:real></ml:degree></ml:apply>",
        id("m"),
        id("m")
    );
    let rows = run(&[
        function,
        eval(&call),
        define("m", "<ml:real>3</ml:real>"),
        eval(&derivative),
        eval(&id("m")),
        eval(&format!(
            "<ml:apply>{}<ml:real>0</ml:real></ml:apply>",
            id("sin")
        )),
    ]);
    assert_eq!(scalar(&rows[1]).value.re, 9.);
    assert!((scalar(&rows[3]).value.re - 6.).abs() < 1e-5);
    assert_eq!(scalar(&rows[4]).value.re, 3.);
    assert_eq!(scalar(&rows[5]).value.re, 0.);
    let output = worksheet_xml(&regions(&[eval(&id("m"))])).unwrap();
    assert_eq!(labels(&output)[0].1, "UNIT");
}

#[test]
fn program_none_names_and_nested_function_formals_have_local_scope() {
    let body = format!(
        "<ml:program><ml:localDefine>{}<ml:real>4</ml:real></ml:localDefine><ml:localDefine><ml:function>{}<ml:boundVars>{}</ml:boundVars></ml:function><ml:apply><ml:plus/>{}{}</ml:apply></ml:localDefine><ml:apply>{}<ml:real>3</ml:real></ml:apply></ml:program>",
        id("m"),
        id("f"),
        id("s"),
        id("m"),
        id("s"),
        id("f")
    );
    let rows = run(&[eval(&body), eval(&id("m")), eval(&id("s"))]);
    assert_eq!(scalar(&rows[0]).value.re, 7.);
    assert_eq!(
        scalar(&rows[1]).dimension,
        mathfolio_core::unit("m").unwrap().dimension
    );
    assert_eq!(
        scalar(&rows[2]).dimension,
        mathfolio_core::unit("s").unwrap().dimension
    );
}

#[test]
fn display_unit_and_system_definitions_apply_the_same_none_fallback() {
    let display = format!(
        "<ml:eval><ml:apply><ml:scale/><ml:real>2</ml:real><ml:id labels=\"UNIT\">m</ml:id></ml:apply><ml:unitOverride>{}</ml:unitOverride></ml:eval>",
        id("cm")
    );
    let rows = run(&[
        define("cm", "<ml:real>7</ml:real>"),
        display,
        define("ORIGIN", "<ml:real>1</ml:real>"),
        eval(&id("ORIGIN")),
    ]);
    let Some(Value::Scalar { quantity }) = rows[1].display_value else {
        panic!("{rows:?}")
    };
    assert_eq!(quantity.value.re, 200.);
    assert_eq!(scalar(&rows[3]).value.re, 1.);
    let output = worksheet_xml(&regions(&[define("ORIGIN", "<ml:real>1</ml:real>")])).unwrap();
    assert_eq!(labels(&output)[0].1, "SYSTEM");
}

#[test]
fn spec_table_none_header_data_and_unit_use_the_common_resolver() {
    let table = format!(
        "<spec-table xmlns=\"{WS}\"><math><ml:define>{}<ml:apply><ml:scale/><ml:matrix rows=\"1\" cols=\"1\">{}</ml:matrix>{}</ml:apply></ml:define></math></spec-table>",
        id("cm"),
        id("n"),
        id("m")
    );
    let mut cache = WorksheetXmlCache::default();
    let mut source = regions(&[define("n", "<ml:real>3</ml:real>"), table, eval(&id("cm"))]);
    let output = oracle(&mut cache, &source);
    let rows = evaluate_worksheet(&output).unwrap();
    assert!(rows.iter().all(|r| r.error.is_none()), "{rows:?}");
    assert_eq!(scalar(&rows[2]).value.re, 3.);
    assert_eq!(
        scalar(&rows[2]).dimension,
        mathfolio_core::unit("m").unwrap().dimension
    );
    source[0].3 = define("n", "<ml:real>5</ml:real>");
    let output = oracle(&mut cache, &source);
    assert_eq!(
        scalar(&evaluate_worksheet(&output).unwrap()[2]).value.re,
        5.
    );
}

#[test]
fn direct_numeric_none_requires_a_resolver_instead_of_creating_a_star_namespace() {
    for flag in [
        "",
        " label-is-contextual=\"false\"",
        " label-is-contextual=\"true\"",
    ] {
        let raw = format!(
            "<worksheet xmlns=\"{WS}\" xmlns:ml=\"{ML}\"><regions><region region-id=\"0\" left=\"0\" top=\"0\"><math>{}</math></region></regions></worksheet>",
            eval(&format!("<ml:id labels=\"*\"{flag}>m</ml:id>"))
        );
        let rows = evaluate_worksheet(&raw).unwrap();
        assert!(rows[0].value.is_none());
        let error = rows[0].error.as_ref().unwrap();
        assert!(
            error.detail.contains("Automatic identifier labelling"),
            "{error:?}"
        );
    }
    let raw = format!(
        "<program xmlns=\"{ML}\"><apply><derivative/><lambda><boundVars><id labels=\"*\">m</id></boundVars><real>1</real></lambda><degree><real>1</real></degree></apply></program>"
    );
    assert!(mathfolio_compat::program::from_mathml(&raw).is_err());
}

#[test]
fn none_for_binder_is_local_but_its_iteration_source_uses_the_outer_definition() {
    let program = format!(
        "<ml:program><ml:localDefine>{}<ml:real>0</ml:real></ml:localDefine><ml:for>{}<ml:range><ml:real>1</ml:real>{}</ml:range><ml:program><ml:localDefine>{}<ml:apply><ml:plus/>{}{}</ml:apply></ml:localDefine></ml:program></ml:for>{}</ml:program>",
        id("s"),
        id("k"),
        id("k"),
        id("s"),
        id("s"),
        id("k"),
        id("s")
    );
    let rows = run(&[
        define("k", "<ml:real>4</ml:real>"),
        eval(&program),
        eval(&id("k")),
        eval(&id("s")),
    ]);
    assert_eq!(scalar(&rows[1]).value.re, 10.);
    assert_eq!(scalar(&rows[2]).value.re, 4.);
    assert_eq!(
        scalar(&rows[3]).dimension,
        mathfolio_core::unit("s").unwrap().dimension
    );
    // Without an outer declaration the loop source keeps catalog lookup, while
    // its binder and body references still use VARIABLE.
    let xml = worksheet_xml(&regions(&[eval(&program)])).unwrap();
    let k_labels: Vec<_> = labels(&xml)
        .into_iter()
        .filter(|(name, _)| name == "k")
        .map(|(_, label)| label)
        .collect();
    assert_eq!(k_labels, ["VARIABLE", "CONSTANT", "VARIABLE"]);
}

#[test]
fn cached_builtin_callee_changes_when_a_user_function_declaration_appears() {
    let mut cache = WorksheetXmlCache::default();
    let query = eval(&format!(
        "<ml:apply>{}<ml:real>0</ml:real></ml:apply>",
        id("sin")
    ));
    let mut source = vec![(2, 0., 100., query)];
    assert_eq!(labels(&oracle(&mut cache, &source))[0].1, "FUNCTION");
    let function = format!(
        "<ml:define><ml:function>{}<ml:boundVars>{}</ml:boundVars></ml:function><ml:real>42</ml:real></ml:define>",
        id("sin"),
        id("x")
    );
    source.insert(0, (1, 0., 0., function));
    let output = oracle(&mut cache, &source);
    assert_eq!(labels(&output).last().unwrap().1, "VARIABLE");
    assert_eq!(
        scalar(&evaluate_worksheet(&output).unwrap()[1]).value.re,
        42.
    );
    source.remove(0);
    assert_eq!(labels(&oracle(&mut cache, &source))[0].1, "FUNCTION");
}

#[test]
fn none_literal_suffix_is_a_distinct_variable_and_index_operand_keeps_lookup() {
    let suffix = "<ml:id labels=\"*\"><p:Span xmlns:p=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\">ORIGIN<ptc:Subscript xmlns:ptc=\"clr-namespace:Ptc.Wpf;assembly=Ptc.Core\">in</ptc:Subscript></p:Span></ml:id>";
    let definition = format!("<ml:define>{suffix}<ml:real>8</ml:real></ml:define>");
    let rows = run(&[definition, eval(suffix), eval(&id("ORIGIN"))]);
    assert_eq!(scalar(&rows[1]).value.re, 8.);
    assert_eq!(scalar(&rows[2]).value.re, 0.);
    let index_unit = format!(
        "<ml:eval><ml:real>2</ml:real><ml:unitOverride><ml:apply><ml:indexer/>{}<ml:apply><ml:plus/>{}<ml:real>1</ml:real></ml:apply></ml:apply></ml:unitOverride></ml:eval>",
        id("u"),
        id("m")
    );
    let xml = worksheet_xml(&regions(&[define("m", "<ml:real>0</ml:real>"), index_unit])).unwrap();
    assert_eq!(
        labels(&xml),
        [
            ("m".into(), "VARIABLE".into()),
            ("u".into(), "UNIT".into()),
            ("m".into(), "VARIABLE".into())
        ]
    );
}

#[test]
fn uppercase_none_alias_preserves_raw_metadata_and_uses_the_same_bounded_resolver() {
    let raw = "<ml:id labels=\"NONE\" label-is-contextual=\"false\">m</ml:id>";
    let tree = tree_from_mathml(raw).unwrap();
    let stored = tree_to_mathml(&tree).unwrap();
    assert!(stored.contains("labels=\"NONE\""));
    assert!(stored.contains("label-is-contextual=\"false\""));
    assert_eq!(tree_from_mathml(&stored).unwrap(), tree);
    let mut cache = WorksheetXmlCache::default();
    let mut source = vec![(2, 0., 100., eval(raw))];
    assert_eq!(labels(&oracle(&mut cache, &source))[0].1, "UNIT");
    source.insert(
        0,
        (
            1,
            0.,
            0.,
            define("m", "<ml:real>7</ml:real>").replace("labels=\"*\"", "labels=\"NONE\""),
        ),
    );
    let output = oracle(&mut cache, &source);
    assert!(labels(&output).iter().all(|(_, label)| label == "VARIABLE"));
    assert_eq!(
        scalar(&evaluate_worksheet(&output).unwrap()[1]).value.re,
        7.
    );
    source.remove(0);
    assert_eq!(labels(&oracle(&mut cache, &source))[0].1, "UNIT");
    // The raw evaluator has no resolver for either spelling of None.
    let direct = format!(
        "<worksheet xmlns=\"{WS}\" xmlns:ml=\"{ML}\"><regions><region region-id=\"0\" left=\"0\" top=\"0\"><math>{}</math></region></regions></worksheet>",
        eval(raw)
    );
    let rows = evaluate_worksheet(&direct).unwrap();
    assert!(rows[0].value.is_none());
    assert!(
        rows[0]
            .error
            .as_ref()
            .unwrap()
            .detail
            .contains("Automatic identifier labelling")
    );
    let binder = format!(
        "<program xmlns=\"{ML}\"><apply><derivative/><lambda><boundVars><id labels=\"NONE\">m</id></boundVars><real>1</real></lambda><degree><real>1</real></degree></apply></program>"
    );
    assert!(
        mathfolio_compat::program::from_mathml(&binder)
            .unwrap_err()
            .detail
            .contains("Automatic identifier labelling")
    );
    // No unobserved case folding of arbitrary labels.
    let output = worksheet_xml(&regions(&[eval("<ml:id labels=\"none\">m</ml:id>")])).unwrap();
    assert_eq!(labels(&output)[0].1, "none");
}
