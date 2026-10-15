//! Independent fixtures. These are behavioral tests of our interpreter, not
//! captured original outputs or copied vendor worksheet examples.
use mathcraft_compat::{Value, evaluate_worksheet, program};

const ML: &str = "http://schemas.mathsoft.com/math50";
fn id(name: &str) -> String {
    format!("<id labels=\"VARIABLE\">{name}</id>")
}
fn real(value: i64) -> String {
    format!("<real>{value}</real>")
}
fn apply(op: &str, a: &str, b: &str) -> String {
    format!("<apply><{op}/>{a}{b}</apply>")
}
fn assign(name: &str, value: &str) -> String {
    format!("<localDefine>{}{value}</localDefine>", id(name))
}
fn program_xml(lines: &str) -> String {
    format!("<program xmlns=\"{ML}\">{lines}</program>")
}
fn branch(test: &str, then: &str, otherwise: &str) -> String {
    format!("<if><test>{test}</test><then><program>{then}</program></then>{otherwise}</if>")
}
fn executed(lines: &str) -> Value {
    program::execute(
        &program::from_mathml(&program_xml(lines)).unwrap(),
        &[],
        10_000,
    )
    .unwrap()
}
fn scalar(value: Value) -> f64 {
    value.numeric_json()["real"].as_f64().unwrap()
}
fn evaluated(regions: &[String]) -> Vec<mathcraft_compat::RegionEvaluation> {
    let mut xml =
        String::from("<worksheet xmlns=\"http://schemas.mathsoft.com/worksheet50\"><regions>");
    for (index, math) in regions.iter().enumerate() {
        xml.push_str(&format!(
            "<region region-id=\"{index}\" left=\"0\" top=\"{}\"><math>{math}</math></region>",
            index * 80
        ));
    }
    xml.push_str("</regions></worksheet>");
    evaluate_worksheet(&xml).unwrap()
}
fn evaluation(expression: &str) -> String {
    format!("<eval xmlns=\"{ML}\">{expression}<unitOverride><placeholder/></unitOverride></eval>")
}

#[test]
fn local_assignment_for_and_final_line_return_value() {
    let sum = assign("s", &apply("plus", &id("s"), &id("k")));
    let lines = format!(
        "{}<for>{}<range>{}{}</range><program>{sum}</program></for>{}",
        assign("s", &real(0)),
        id("k"),
        real(1),
        real(6),
        id("s")
    );
    assert_eq!(scalar(executed(&lines)), 21.0);
}

#[test]
fn while_continue_break_and_return_propagate_to_the_correct_level() {
    let increment = assign("i", &apply("plus", &id("i"), &real(1)));
    let skip = branch(&apply("equal", &id("i"), &real(2)), "<continue/>", "");
    let stop = branch(&apply("equal", &id("i"), &real(5)), "<break/>", "");
    let sum = assign("s", &apply("plus", &id("s"), &id("i")));
    let lines = format!(
        "{}{}<while>{}<program>{increment}{skip}{stop}{sum}</program></while><return>{}</return>{}",
        assign("i", &real(0)),
        assign("s", &real(0)),
        apply("lessThan", &id("i"), &real(9)),
        id("s"),
        real(999)
    );
    assert_eq!(scalar(executed(&lines)), 8.0);
    let inner_return = branch(&real(1), &format!("<return>{}</return>", real(73)), "");
    let nested = format!(
        "<for>{}<range>{}{}</range><program><while>{}<program>{inner_return}</program></while></program></for>{}",
        id("n"),
        real(1),
        real(2),
        real(1),
        real(999)
    );
    assert_eq!(scalar(executed(&nested)), 73.0);
}

#[test]
fn elseif_is_lazy_but_alsoif_checks_all_tests_and_else_requires_no_match() {
    let error = apply("div", &real(1), &real(0));
    let else_if = format!(
        "<elseif><test>{error}</test><then><program>{}</program></then></elseif>",
        real(999)
    );
    let first = branch(&real(1), &real(17), &else_if);
    assert_eq!(scalar(executed(&first)), 17.0);
    let also = format!(
        "<alsoif><test>{}</test><then><program>{}</program></then></alsoif><else><program>{error}</program></else>",
        real(1),
        assign("s", &real(31))
    );
    let lines = format!(
        "{}{}{}",
        assign("s", &real(0)),
        branch(&real(1), &assign("s", &real(19)), &also),
        id("s")
    );
    assert_eq!(scalar(executed(&lines)), 31.0);
    let lazy_else = branch(
        &real(0),
        &error,
        &format!("<else><program>{}</program></else>", real(23)),
    );
    assert_eq!(scalar(executed(&lazy_else)), 23.0);
}

#[test]
fn program_scope_shadow_initializes_from_worksheet_then_stays_local() {
    let define = format!("<define xmlns=\"{ML}\">{}{}</define>", id("x"), real(9));
    let body = format!(
        "{}{}{}",
        assign("x", &apply("plus", &id("x"), &real(1))),
        assign("x", &apply("plus", &id("x"), &real(1))),
        id("x")
    );
    let regions = evaluated(&[
        define,
        evaluation(&program_xml(&body)),
        evaluation(&id("x")),
    ]);
    assert!(regions[1].error.is_none(), "{:?}", regions[1].error);
    assert_eq!(scalar(regions[1].value.clone().unwrap()), 11.0);
    assert_eq!(scalar(regions[2].value.clone().unwrap()), 9.0);
}

#[test]
fn program_function_body_resolves_parameters_and_local_variables() {
    let body = format!(
        "{}<for>{}<range>{}{}</range><program>{}</program></for>{}",
        assign("sum", &real(0)),
        id("k"),
        real(1),
        id("n"),
        assign("sum", &apply("plus", &id("sum"), &id("k"))),
        id("sum")
    );
    let define = format!(
        "<define xmlns=\"{ML}\"><function><id labels=\"FUNCTION\">accumulate</id><boundVars>{}</boundVars></function>{}</define>",
        id("n"),
        program_xml(&body)
    );
    let call = "<apply><id labels=\"FUNCTION\">accumulate</id><real>6</real></apply>";
    let regions = evaluated(&[define, evaluation(call)]);
    assert!(regions[0].error.is_none(), "{:?}", regions[0].error);
    assert!(regions[1].error.is_none(), "{:?}", regions[1].error);
    assert_eq!(scalar(regions[1].value.clone().unwrap()), 21.0);
}

#[test]
fn complex_truth_comparison_and_explicit_budget_are_bounded() {
    assert_eq!(scalar(executed("<apply><not/><imag>2</imag></apply>")), 0.0);
    assert_eq!(
        scalar(executed(
            "<apply><xor/><imag>2</imag><real>0</real></apply>"
        )),
        1.0
    );
    assert_eq!(
        scalar(executed(&apply("greaterOrEqual", &real(4), &real(4)))),
        1.0
    );
    let forever = program::from_mathml(&program_xml(
        "<while><real>1</real><program><real>1</real></program></while>",
    ))
    .unwrap();
    assert_eq!(
        program::execute(&forever, &[], 8).unwrap_err().code,
        "program_budget"
    );
    let outside = program::from_mathml(&program_xml("<break/>")).unwrap();
    assert_eq!(
        program::execute(&outside, &[], 8).unwrap_err().code,
        "program_control"
    );
}

#[test]
fn malformed_and_unimplemented_program_forms_are_explicit_errors() {
    assert!(program::from_mathml(&program_xml("<symEval><real>3</real></symEval>")).is_err());
    assert!(
        program::from_mathml(&program_xml(&assign(
            "x",
            "<symEval><real>3</real></symEval>"
        )))
        .is_err()
    );
    assert!(
        program::from_mathml(&program_xml(
            "<localDefine><id labels=\"VARIABLE\">x</id></localDefine>"
        ))
        .is_err()
    );
    assert!(
        program::from_mathml(&program_xml(
            "<for><real>1</real><real>2</real><real>3</real></for>"
        ))
        .is_err()
    );
    assert!(program::from_mathml(&program_xml("<program/>")).is_err());
    let mixed = "<if><test><real>1</real></test><then><program><real>2</real></program></then><elseif><test><real>1</real></test><then><program><real>3</real></program></then></elseif><alsoif><test><real>1</real></test><then><program><real>4</real></program></then></alsoif></if>";
    assert!(program::from_mathml(&program_xml(mixed)).is_err());
}

#[test]
fn automatic_local_references_follow_explicit_declarations_without_global_leaks() {
    let automatic = |name: &str| format!("<id labels=\"*\">{name}</id>");
    let body = format!(
        "{}<for>{}<range>{}{}</range><program>{}</program></for>{}",
        assign("s", &real(0)),
        id("k"),
        real(1),
        real(4),
        assign("s", &apply("plus", &automatic("s"), &automatic("k"))),
        automatic("s")
    );
    assert_eq!(scalar(executed(&body)), 10.0);
    let define = format!("<define xmlns=\"{ML}\">{}{}</define>", id("x"), real(9));
    let shadow = format!(
        "{}{}{}",
        assign("x", &apply("plus", &automatic("x"), &real(1))),
        assign("x", &apply("plus", &automatic("x"), &real(1))),
        automatic("x")
    );
    let regions = evaluated(&[
        define,
        evaluation(&program_xml(&shadow)),
        evaluation(&id("x")),
    ]);
    assert_eq!(scalar(regions[1].value.clone().unwrap()), 11.0);
    assert_eq!(scalar(regions[2].value.clone().unwrap()), 9.0);
}
