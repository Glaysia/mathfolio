//! Native SCALE differs from explicit multiplication when its RHS is a function.
use mathcraft_compat::{RegionEvaluation, Value, editor_bridge::worksheet_xml};

fn id(name: &str) -> String {
    format!("<ml:id>{name}</ml:id>")
}
fn labelled(name: &str, label: &str) -> String {
    format!("<ml:id labels='{label}'>{name}</ml:id>")
}
fn real(value: f64) -> String {
    format!("<ml:real>{value}</ml:real>")
}
fn apply(operator: &str, args: &[String]) -> String {
    format!("<ml:apply><ml:{operator}/>{}</ml:apply>", args.join(""))
}
fn scale(left: String, right: String) -> String {
    apply("scale", &[left, right])
}
fn define(lhs: String, rhs: String) -> String {
    format!("<ml:define>{lhs}{rhs}</ml:define>")
}
fn declaration(name: &str, parameters: &[&str]) -> String {
    format!(
        "<ml:function>{}<ml:boundVars>{}</ml:boundVars></ml:function>",
        id(name),
        parameters.iter().map(|name| id(name)).collect::<String>()
    )
}
fn query(value: String) -> String {
    format!("<ml:eval>{value}<ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>")
}
fn run(lines: &[String]) -> Vec<RegionEvaluation> {
    let source = worksheet_xml(
        &lines
            .iter()
            .enumerate()
            .map(|(i, xml)| (i as u64 + 1, 0., i as f64 * 40., xml.clone()))
            .collect::<Vec<_>>(),
    )
    .unwrap();
    mathcraft_compat::evaluate_worksheet(&source).unwrap()
}
fn quantity(row: &RegionEvaluation) -> mathcraft_core::Quantity {
    assert!(row.error.is_none(), "{row:?}");
    let Some(Value::Scalar { quantity }) = &row.value else {
        panic!("{row:?}")
    };
    *quantity
}
fn scalar(row: &RegionEvaluation, expected: f64) {
    let value = quantity(row);
    assert!(
        (value.value.re - expected).abs() < 1e-12,
        "{value:?} versus {expected}"
    );
    assert_eq!(value.value.im, 0.);
}

#[test]
fn builtin_function_rhs_is_applied_while_explicit_multiplication_keeps_its_existing_error() {
    let rows = run(&[
        query(scale(real(0.), id("sin"))),
        query(scale(real(0.), id("cos"))),
        query(scale(real(9.), id("sqrt"))),
        query(apply("mult", &[real(0.), id("sin")])),
    ]);
    for (row, expected) in rows.iter().zip([0., 1., 3.]) {
        scalar(row, expected);
    }
    assert!(rows[3].error.is_some());
}

#[test]
fn named_function_capture_and_labelled_value_shadowing_are_resolved_before_dispatch() {
    let rows = run(&[
        define(id("a"), real(2.)),
        define(declaration("f", &["x"]), apply("plus", &[id("x"), id("a")])),
        define(id("a"), real(100.)),
        query(scale(real(5.), id("f"))),
        define(labelled("cos", "VARIABLE"), real(3.)),
        query(scale(real(5.), id("cos"))),
        query(scale(real(0.), labelled("cos", "FUNCTION"))),
        query(scale(real(5.), labelled("min", "UNIT"))),
    ]);
    scalar(&rows[3], 7.);
    scalar(&rows[5], 15.);
    scalar(&rows[6], 1.);
    scalar(&rows[7], 300.);
    assert_eq!(
        quantity(&rows[7]).dimension,
        mathcraft_core::unit("s").unwrap().dimension
    );
}

#[test]
fn function_definitions_validate_builtin_and_captured_function_scaling() {
    let rows = run(&[
        define(declaration("sine", &["x"]), scale(id("x"), id("sin"))),
        query(scale(real(0.), id("sine"))),
        define(
            declaration("f", &["x"]),
            apply("plus", &[id("x"), real(1.)]),
        ),
        define(declaration("g", &["x"]), scale(id("x"), id("f"))),
        query(scale(real(4.), id("g"))),
    ]);
    assert!(
        rows[0].error.is_none() && rows[2].error.is_none() && rows[3].error.is_none(),
        "{rows:?}"
    );
    scalar(&rows[1], 0.);
    scalar(&rows[4], 5.);
}

#[test]
fn function_rhs_uses_one_argument_and_does_not_hide_failed_function_bindings() {
    let rows = run(&[
        define(
            declaration("pair", &["x", "y"]),
            apply("plus", &[id("x"), id("y")]),
        ),
        query(scale(real(5.), id("pair"))),
        define(
            format!(
                "<ml:function>{}<ml:boundVars>{}</ml:boundVars></ml:function>",
                labelled("cos", "FUNCTION"),
                id("x")
            ),
            id("missing"),
        ),
        query(scale(real(0.), labelled("cos", "FUNCTION"))),
    ]);
    assert!(
        rows[1]
            .error
            .as_ref()
            .unwrap()
            .detail
            .contains("argument count")
    );
    assert!(rows[2].error.is_some());
    assert_eq!(
        rows[3].error.as_ref().unwrap().previous_region.as_deref(),
        Some("3")
    );
}

#[test]
fn arrays_can_be_scaled_by_values_or_passed_to_named_array_functions() {
    let matrix = format!(
        "<ml:matrix rows='2' cols='1'>{}{}</ml:matrix>",
        real(2.),
        real(3.)
    );
    let rows = run(&[
        query(scale(matrix.clone(), real(2.))),
        query(scale(matrix, id("rows"))),
        query(scale(real(3.), id("m"))),
    ]);
    let Some(Value::Matrix {
        rows: r,
        cols: c,
        elements,
    }) = &rows[0].value
    else {
        panic!("{rows:?}")
    };
    assert_eq!((*r, *c), (2, 1));
    assert_eq!(
        elements.iter().map(|q| q.value.re).collect::<Vec<_>>(),
        [4., 6.]
    );
    scalar(&rows[1], 2.);
    scalar(&rows[2], 3.);
    assert_eq!(
        quantity(&rows[2]).dimension,
        mathcraft_core::unit("m").unwrap().dimension
    );
}

#[test]
fn vectorized_function_scaling_matches_elementwise_named_calls() {
    let matrix = format!(
        "<ml:matrix rows='2' cols='1'>{}{}</ml:matrix>",
        real(0.),
        real(std::f64::consts::FRAC_PI_2)
    );
    let rows = run(&[
        query(apply("vectorize", &[scale(matrix.clone(), id("sin"))])),
        query(scale(matrix, id("sin"))),
    ]);
    assert!(rows[0].error.is_none(), "{rows:?}");
    let Some(Value::Matrix { elements, .. }) = &rows[0].value else {
        panic!("{rows:?}")
    };
    assert_eq!(elements[0].value.re, 0.);
    assert!((elements[1].value.re - 1.).abs() < 1e-12);
    assert!(rows[1].error.is_none(), "{rows:?}");
    assert_eq!(
        rows[0].value.as_ref().unwrap().numeric_json(),
        rows[1].value.as_ref().unwrap().numeric_json()
    );
}

#[test]
fn range_argument_scaling_uses_the_existing_range_iteration_contract() {
    let rows = run(&[
        define(
            id("k"),
            format!("<ml:range>{}{}</ml:range>", real(0.), real(2.)),
        ),
        query(scale(id("k"), id("cos"))),
    ]);
    assert!(rows[1].error.is_none(), "{rows:?}");
    let Some(Value::RangeValues { elements }) = &rows[1].value else {
        panic!("{rows:?}")
    };
    assert_eq!(elements.len(), 3);
    for (index, q) in elements.iter().enumerate() {
        assert!((q.value.re - (index as f64).cos()).abs() < 1e-12);
    }
}

#[test]
fn native_scale_roundtrip_does_not_reclassify_explicit_mult_or_result_unit_wrappers() {
    use mathcraft_compat::editor_bridge::{tree_from_mathml, tree_to_mathml};
    use mathcraft_editor::{BinaryOp, NodeKind};
    for (operator, kind) in [("scale", BinaryOp::Scale), ("mult", BinaryOp::Multiply)] {
        let xml = format!(
            "<ml:apply xmlns:ml='http://schemas.mathsoft.com/math50'><ml:{operator}/>{}{}</ml:apply>",
            real(3.),
            labelled("m", "UNIT")
        );
        let tree = tree_from_mathml(&xml).unwrap();
        assert_eq!(tree.kind, NodeKind::Binary(kind));
        let out = tree_to_mathml(&tree).unwrap();
        assert_eq!(tree_from_mathml(&out).unwrap(), tree);
        assert!(out.contains(&format!("<ml:{operator}/>")));
    }
}
