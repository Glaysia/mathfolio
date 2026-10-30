use mathfolio_compat::{
    Value,
    editor_bridge::{tree_from_mathml, tree_to_mathml, worksheet_xml},
    evaluate_worksheet,
};

fn worksheet(fragments: &[&str]) -> String {
    worksheet_xml(
        &fragments
            .iter()
            .enumerate()
            .map(|(i, xml)| (i as u64, 0., i as f64 * 100., xml.to_string()))
            .collect::<Vec<_>>(),
    )
    .unwrap()
}

fn eval(id: &str) -> String {
    format!("<ml:eval>{id}<ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>")
}

fn scalar(value: &Option<Value>) -> mathfolio_core::Quantity {
    let Some(Value::Scalar { quantity }) = value else {
        panic!("Expected scalar: {value:?}");
    };
    *quantity
}

#[test]
fn cached_unit_is_recomputed_but_explicit_unit_is_a_user_choice() {
    let contextual = "<ml:id labels=\"UNIT\" label-is-contextual=\"true\">m</ml:id>";
    let explicit = "<ml:id labels=\"UNIT\" label-is-contextual=\"false\">m</ml:id>";
    let source = tree_from_mathml(contextual).unwrap();
    let stored = tree_to_mathml(&source).unwrap();
    let xml = worksheet(&[
        "<ml:define><ml:id>m</ml:id><ml:real>7</ml:real></ml:define>",
        &eval(&stored),
        &eval(explicit),
    ]);
    let rows = evaluate_worksheet(&xml).unwrap();
    assert!(rows.iter().all(|r| r.error.is_none()), "{rows:?}");
    assert_eq!(scalar(&rows[1].value).value.re, 7.);
    assert!(scalar(&rows[1].value).is_scalar());
    assert_eq!(
        scalar(&rows[2].value).dimension,
        mathfolio_core::unit("m").unwrap().dimension
    );
    assert_eq!(
        tree_from_mathml(&tree_to_mathml(&source).unwrap()).unwrap(),
        source
    );
    assert!(stored.contains("label-is-contextual=\"true\""));
}

#[test]
fn unresolved_cached_variable_does_not_create_a_value() {
    let xml = worksheet(&[&eval(
        "<ml:id labels=\"VARIABLE\" label-is-contextual=\"1\">missing_name</ml:id>",
    )]);
    let rows = evaluate_worksheet(&xml).unwrap();
    assert!(rows[0].value.is_none());
    assert!(rows[0].error.is_some());
}

#[test]
fn direct_mathml_does_not_force_an_unresolved_cached_label() {
    for flag in ["true", "1"] {
        let xml = format!(
            "<worksheet xmlns=\"http://schemas.mathsoft.com/worksheet50\" xmlns:ml=\"http://schemas.mathsoft.com/math50\"><regions><region region-id=\"0\" left=\"0\" top=\"0\"><math resultRef=\"0\">{}</math></region></regions></worksheet>",
            eval(&format!(
                "<ml:id labels=\"UNIT\" label-is-contextual=\"{flag}\">m</ml:id>"
            )),
        );
        let rows = evaluate_worksheet(&xml).unwrap();
        assert!(rows[0].value.is_none());
        assert!(rows[0].error.is_some());
    }
}

#[test]
fn display_unit_context_overrides_cached_variable_and_visible_definition() {
    let xml = worksheet(&[
        "<ml:define><ml:id>cm</ml:id><ml:real>7</ml:real></ml:define>",
        "<ml:eval><ml:apply><ml:scale/><ml:real>2</ml:real><ml:id labels=\"UNIT\">m</ml:id></ml:apply><ml:unitOverride><ml:id labels=\"VARIABLE\" label-is-contextual=\"true\">cm</ml:id></ml:unitOverride></ml:eval>",
    ]);
    let rows = evaluate_worksheet(&xml).unwrap();
    assert!(rows.iter().all(|r| r.error.is_none()), "{rows:?}");
    assert_eq!(scalar(&rows[1].display_value).value.re, 200.);
}

#[test]
fn function_binders_are_variables_even_when_cached_as_units() {
    let xml = worksheet(&[
        "<ml:define><ml:function><ml:id>f</ml:id><ml:boundVars><ml:id labels=\"UNIT\" label-is-contextual=\"true\">m</ml:id></ml:boundVars></ml:function><ml:id labels=\"UNIT\" label-is-contextual=\"true\">m</ml:id></ml:define>",
        "<ml:eval><ml:apply><ml:id>f</ml:id><ml:real>9</ml:real></ml:apply><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
    ]);
    let rows = evaluate_worksheet(&xml).unwrap();
    assert!(rows.iter().all(|r| r.error.is_none()), "{rows:?}");
    assert_eq!(scalar(&rows[1].value).value.re, 9.);
}

#[test]
fn index_operand_does_not_inherit_the_display_unit_label() {
    let xml = worksheet(&[
        "<ml:define><ml:id>m</ml:id><ml:real>0</ml:real></ml:define>",
        "<ml:eval><ml:real>2</ml:real><ml:unitOverride><ml:apply><ml:indexer/><ml:id>u</ml:id><ml:apply><ml:plus/><ml:id labels=\"UNIT\" label-is-contextual=\"true\">m</ml:id><ml:real>1</ml:real></ml:apply></ml:apply></ml:unitOverride></ml:eval>",
    ]);
    let doc = roxmltree::Document::parse(&xml).unwrap();
    let ids: Vec<_> = doc
        .descendants()
        .filter(|n| n.tag_name().name() == "id")
        .collect();
    assert_eq!(ids[1].attribute("labels"), Some("UNIT"));
    assert_eq!(ids[2].attribute("labels"), Some("VARIABLE"));
}

#[test]
fn reserved_definition_names_are_system_but_explicit_variable_is_preserved() {
    for name in [
        "ORIGIN",
        "TOL",
        "CTOL",
        "PRNCOLWIDTH",
        "PRNPRECISION",
        "CWD",
        "Seed",
        "in0",
        "in9",
    ] {
        let definition = format!(
            "<ml:define><ml:id labels=\"VARIABLE\" label-is-contextual=\"true\">{name}</ml:id><ml:real>1</ml:real></ml:define>"
        );
        let xml = worksheet(&[&definition]);
        let doc = roxmltree::Document::parse(&xml).unwrap();
        let id = doc
            .descendants()
            .find(|n| n.tag_name().name() == "id")
            .unwrap();
        assert_eq!(id.attribute("labels"), Some("SYSTEM"), "{name}");
    }
    let xml = worksheet(&[
        "<ml:define><ml:id labels=\"VARIABLE\">TOL</ml:id><ml:real>1</ml:real></ml:define>",
    ]);
    assert!(xml.contains("labels=\"VARIABLE\">TOL"));
}

#[test]
fn contextual_subscript_labels_are_recomputed_without_changing_identity() {
    let rich = "<ml:id labels=\"UNIT\" label-is-contextual=\"true\"><p:Span xmlns:p=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\">Z<ptc:Subscript xmlns:ptc=\"clr-namespace:Ptc.Wpf;assembly=Ptc.Core\">in</ptc:Subscript></p:Span></ml:id>";
    let define = format!("<ml:define>{rich}<ml:real>8</ml:real></ml:define>");
    let rows = evaluate_worksheet(&worksheet(&[&define, &eval(rich)])).unwrap();
    assert!(rows.iter().all(|r| r.error.is_none()), "{rows:?}");
    assert_eq!(scalar(&rows[1].value).value.re, 8.);
}
