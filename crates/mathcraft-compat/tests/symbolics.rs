//! Independent exact-algebra fixtures, not captured vendor CAS outputs.
use mathcraft_compat::{Value, symbolics};
const ML: &str = "http://schemas.mathsoft.com/math50";
fn id(s: &str) -> String {
    format!("<id>{s}</id>")
}
fn real(s: &str) -> String {
    format!("<real>{s}</real>")
}
fn op(name: &str, a: &str, b: &str) -> String {
    format!("<apply><{name}/>{a}{b}</apply>")
}
fn symbolic(expression: &str, commands: &str) -> String {
    format!("<symEval xmlns=\"{ML}\">{expression}{commands}</symEval>")
}
fn command(name: &str) -> String {
    format!("<command>{}</command>", id(name))
}
fn mathml(value: Value) -> String {
    match value {
        Value::Symbolic { mathml } => mathml,
        _ => panic!("Expected symbolic result"),
    }
}
fn evaluate(expression: &str, commands: &str) -> String {
    mathml(symbolics::evaluate_mathml(&symbolic(expression, commands)).unwrap())
}

#[test]
fn rational_arithmetic_and_decimals_are_exact_without_binary_rounding() {
    let expression = op(
        "plus",
        &op("div", &real("1"), &real("3")),
        &op("div", &real("1"), &real("6")),
    );
    assert_eq!(
        evaluate(&expression, ""),
        format!(
            "<symResult xmlns=\"{ML}\"><apply><div/><real>1</real><real>2</real></apply></symResult>"
        )
    );
    let decimal = op("plus", &real("0.1"), &real("2e-1"));
    assert!(evaluate(&decimal, "").contains("<div/><real>3</real><real>10</real>"));
    let cancel = op("minus", &id("x"), &id("x"));
    assert!(evaluate(&cancel, &command("simplify")).contains("<real>0</real>"));
}

#[test]
fn expansion_combines_multivariate_terms_and_preserves_valid_xml() {
    let expression = op("pow", &op("plus", &id("x"), &real("1")), &real("3"));
    let result = evaluate(&expression, &command("expand"));
    assert!(result.contains("<pow/><id labels=\"VARIABLE\">x</id><real>3</real>"));
    assert_eq!(result.matches("<real>3</real>").count(), 3);
    let product = op(
        "mult",
        &op("plus", &id("x"), &id("y")),
        &op("minus", &id("x"), &id("y")),
    );
    let result = evaluate(&product, &command("simplify"));
    assert!(!result.contains("<mult/>"));
    let xml = op("mult", &id("a&amp;b"), &id("a&amp;b"));
    assert!(evaluate(&xml, "").contains("a&amp;b"));
    roxmltree::Document::parse(&result).unwrap();
}

#[test]
fn native_lambda_derivative_default_and_explicit_degree() {
    let body = op(
        "plus",
        &op("pow", &id("x"), &real("4")),
        &op("mult", &id("x"), &id("y")),
    );
    let derivative = |degree: &str| {
        format!(
            "<apply><derivative/><lambda><boundVars>{}</boundVars>{body}</lambda><degree>{degree}</degree></apply>",
            id("x")
        )
    };
    let first = evaluate(
        &derivative("<placeholder/>"),
        "<command><placeholder/></command>",
    );
    assert!(first.contains("<real>4</real>"));
    assert!(first.contains(">y</id>"));
    let second = evaluate(&derivative(&real("2")), "");
    assert!(second.contains("<real>12</real>"));
    assert!(!second.contains(">y</id>"));
}

#[test]
fn native_finite_polynomial_limits_are_exact_and_locally_bound() {
    let body = op(
        "plus",
        &op("pow", &id("x"), &real("2")),
        &op("plus", &op("mult", &real("3"), &id("x")), &real("1")),
    );
    let limit = |direction: &str, body: &str, point: &str| {
        format!(
            "<apply><limit{direction}/><lambda><boundVars>{}</boundVars>{body}</lambda>{point}</apply>",
            id("x")
        )
    };
    assert!(evaluate(&limit("", &body, &real("2")), "").contains("<real>11</real>"));
    for direction in [" direction=\"left\"", " direction=\"right\""] {
        assert!(evaluate(&limit(direction, &body, &real("2")), "").contains("<real>11</real>"));
    }

    // The lambda variable is local: a unit-bearing worksheet binding with the
    // same spelling must neither substitute into nor invalidate the limit.
    let outer = op("mult", &real("2"), "<id labels=\"UNIT\">m</id>");
    let rows = worksheet(&[
        define("x", &outer),
        symbolic(&limit("", &body, &real("2")), ""),
    ]);
    assert!(rows[0].error.is_none());
    assert!(rows[1].error.is_none(), "{:?}", rows[1].error);
    assert!(mathml(rows[1].value.clone().unwrap()).contains("<real>11</real>"));
}

#[test]
fn symbolic_limit_rejects_unknown_direction_infinity_and_removable_poles() {
    let lambda = |operator: &str, body: &str, point: &str| {
        format!(
            "<apply>{operator}<lambda><boundVars>{}</boundVars>{body}</lambda>{point}</apply>",
            id("x")
        )
    };
    let body = op("plus", &id("x"), &real("1"));
    assert!(
        symbolics::evaluate_mathml(&symbolic(
            &lambda("<limit direction=\"sideways\"/>", &body, &real("2")),
            ""
        ))
        .is_err()
    );
    assert!(
        symbolics::evaluate_mathml(&symbolic(
            &lambda("<limit/>", &body, "<id labels=\"CONSTANT\">∞</id>"),
            ""
        ))
        .is_err()
    );
    let removable = op(
        "div",
        &op("minus", &op("pow", &id("x"), &real("2")), &real("1")),
        &op("minus", &id("x"), &real("1")),
    );
    assert!(
        symbolics::evaluate_mathml(&symbolic(&lambda("<limit/>", &removable, &real("1")), ""))
            .is_err()
    );
}

#[test]
fn native_solve_handles_linear_and_quadratic_equations() {
    let solve = format!("<command>{}{}</command>", id("solve"), id("x"));
    let linear = op("equal", &op("mult", &real("3"), &id("x")), &real("2"));
    assert!(evaluate(&linear, &solve).contains("<div/><real>2</real><real>3</real>"));
    let quadratic = op("minus", &op("pow", &id("x"), &real("2")), &real("2"));
    let result = evaluate(&quadratic, &command("solve"));
    assert!(result.contains("<matrix rows=\"2\" cols=\"1\">"));
    assert_eq!(result.matches("<sqrt/>").count(), 2);
    let rational = op("minus", &op("pow", &id("x"), &real("2")), &real("9"));
    assert!(evaluate(&rational, &solve).contains("<real>-3</real><real>3</real>"));
    let repeated = op("pow", &op("minus", &id("x"), &real("3")), &real("2"));
    let result = evaluate(&repeated, &solve);
    assert!(!result.contains("<matrix"));
    assert!(result.contains("<real>3</real>"));
}

#[test]
fn cached_symresult_is_ignored_and_unsupported_commands_fail_explicitly() {
    let xml = symbolic(
        &op("plus", &real("2"), &real("3")),
        "<command><placeholder/></command><symResult><real>999</real></symResult>",
    );
    let result = mathml(symbolics::evaluate_mathml(&xml).unwrap());
    assert!(result.contains("<real>5</real>"));
    assert!(!result.contains("999"));
    for command in ["factor", "float", "assume", "differentiate"] {
        assert!(symbolics::evaluate_mathml(&symbolic(&id("x"), &self::command(command))).is_err());
    }
    assert!(symbolics::evaluate_mathml(&symbolic(&op("div", &id("x"), &id("x")), "")).is_err());
    assert!(symbolics::evaluate_mathml(&symbolic(&op("equal", &id("x"), &real("1")), "")).is_err());
}

fn worksheet(regions: &[String]) -> Vec<mathcraft_compat::RegionEvaluation> {
    let body = regions
        .iter()
        .enumerate()
        .map(|(i, m)| {
            format!(
                "<region region-id=\"{i}\" left=\"0\" top=\"{}\"><math>{m}</math></region>",
                i * 80
            )
        })
        .collect::<String>();
    mathcraft_compat::evaluate_worksheet(&format!("<worksheet xmlns=\"http://schemas.mathsoft.com/worksheet50\"><regions>{body}</regions></worksheet>")).unwrap()
}
fn define(name: &str, value: &str) -> String {
    format!("<define xmlns=\"{ML}\"><id labels=\"VARIABLE\">{name}</id>{value}</define>")
}

#[test]
fn real_worksheet_symeval_substitutes_only_supported_exact_bindings() {
    let expression = op("plus", &id("x"), &real("3"));
    let rows = worksheet(&[
        define("x", &real("2")),
        symbolic(&expression, &command("simplify")),
    ]);
    assert!(rows[1].error.is_none(), "{:?}", rows[1].error);
    assert!(mathml(rows[1].value.clone().unwrap()).contains("<real>5</real>"));
    let rows = worksheet(&[define("x", &real("0.5")), symbolic(&expression, "")]);
    assert!(rows[1].error.is_some());
    // Unsupported bindings still fail when algebra would cancel the name.
    let rows = worksheet(&[
        define("x", &real("0.5")),
        symbolic(&op("minus", &id("x"), &id("x")), ""),
    ]);
    assert!(rows[1].error.is_some());
    let unit_value = op("mult", &real("2"), "<id labels=\"UNIT\">m</id>");
    let rows = worksheet(&[define("x", &unit_value), symbolic(&expression, "")]);
    assert!(rows[0].error.is_none());
    assert!(rows[1].error.is_some());
    let rows = worksheet(&[
        define("x", &op("div", &real("1"), &real("0"))),
        symbolic(&expression, ""),
    ]);
    assert!(rows[0].error.is_some());
    assert!(rows[1].error.is_some());
}

#[test]
fn budgets_namespace_malformed_literals_and_overflow_are_bounded() {
    assert!(symbolics::evaluate_mathml("<symEval><real>1</real></symEval>").is_err());
    for literal in [
        "NaN",
        "--1",
        "1e-2147483648",
        "9999999999999999999999999999999999999999999999",
    ] {
        assert!(
            symbolics::evaluate_mathml(&symbolic(&real(literal), "")).is_err(),
            "{literal}"
        );
    }
    assert!(symbolics::evaluate_mathml(&symbolic(&op("pow", &id("x"), &real("33")), "")).is_err());
    assert!(symbolics::evaluate_mathml(&symbolic(&op("div", &real("1"), &real("0")), "")).is_err());
    assert!(symbolics::evaluate_mathml(&symbolic(&op("pow", &real("0"), &real("0")), "")).is_err());
    assert!(
        symbolics::evaluate_mathml(&symbolic(
            &op("minus", &op("pow", &id("x"), &real("3")), &real("1")),
            &command("solve")
        ))
        .is_err()
    );
}
