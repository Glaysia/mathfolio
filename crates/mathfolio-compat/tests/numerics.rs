//! Self-authored Math50 examples; numerical assertions are not Prime oracles.
use mathfolio_compat::{RegionEvaluation, Value, editor_bridge::worksheet_xml, evaluate_worksheet};
use mathfolio_core::Quantity;

fn real(x: f64) -> String {
    format!("<ml:real>{x}</ml:real>")
}
fn id(name: &str) -> String {
    format!("<ml:id labels=\"VARIABLE\">{name}</ml:id>")
}
fn binary(op: &str, a: &str, b: &str) -> String {
    format!("<ml:apply><ml:{op}/>{a}{b}</ml:apply>")
}
fn eval(x: &str) -> String {
    format!("<ml:eval>{x}<ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>")
}
fn define(name: &str, x: &str) -> String {
    format!("<ml:define>{}{x}</ml:define>", id(name))
}
fn run(fragments: &[String]) -> Vec<RegionEvaluation> {
    let rows: Vec<_> = fragments
        .iter()
        .enumerate()
        .map(|(i, s)| (i as u64, 0., i as f64 * 100., s.clone()))
        .collect();
    evaluate_worksheet(&worksheet_xml(&rows).unwrap()).unwrap()
}
fn scalar(row: &RegionEvaluation) -> Quantity {
    assert!(row.error.is_none(), "{:?}", row.error);
    match row.value.as_ref().unwrap() {
        Value::Scalar { quantity } => *quantity,
        x => panic!("{x:?}"),
    }
}
fn root(body: &str, variable: &str, bounds: Option<(&str, &str)>) -> String {
    let extra = bounds.map(|(a, b)| format!("{a}{b}")).unwrap_or_default();
    format!(
        "<ml:apply><ml:id labels=\"KEYWORD\">root</ml:id><ml:sequence>{body}{}{extra}</ml:sequence></ml:apply>",
        id(variable)
    )
}
fn calculus(op: &str, body: &str, variable: &str, tail: &str) -> String {
    format!(
        "<ml:apply><ml:{op}/><ml:lambda><ml:boundVars>{}</ml:boundVars>{body}</ml:lambda>{tail}</ml:apply>",
        id(variable)
    )
}
fn integral(body: &str, variable: &str, a: &str, b: &str) -> String {
    calculus(
        "integral",
        body,
        variable,
        &format!("<ml:lowerBound>{a}</ml:lowerBound><ml:upperBound>{b}</ml:upperBound>"),
    )
}
fn derivative(body: &str, variable: &str, n: &str) -> String {
    calculus(
        "derivative",
        body,
        variable,
        &format!("<ml:degree>{n}</ml:degree>"),
    )
}
fn metre(x: f64) -> String {
    binary("scale", &real(x), "<ml:id labels=\"UNIT\">m</ml:id>")
}
fn near(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{actual} != {expected} within {tolerance}"
    );
}

#[test]
fn root_brackets_bind_an_undefined_variable_and_leave_outer_bindings_unchanged() {
    let f = binary("minus", &binary("pow", &id("x"), &real(2.)), &real(2.));
    let r = run(&[
        eval(&root(&f, "x", Some((&real(0.), &real(2.))))),
        define("x", &real(42.)),
        eval(&root(&f, "x", Some((&real(0.), &real(2.))))),
        eval(&id("x")),
    ]);
    near(scalar(&r[0]).value.re, 2_f64.sqrt(), 1e-5);
    near(scalar(&r[2]).value.re, 2_f64.sqrt(), 1e-5);
    assert_eq!(scalar(&r[3]).value.re, 42.);
}

#[test]
fn root_real_and_complex_guesses_and_dimensioned_brackets_work() {
    let f = binary("minus", &binary("pow", &id("x"), &real(2.)), &real(2.));
    let complex_f = binary("plus", &binary("pow", &id("z"), &real(2.)), &real(1.));
    let linear = binary("minus", &id("distance"), &metre(3.));
    let r = run(&[
        define("x", &real(1.)),
        eval(&root(&f, "x", None)),
        define("z", "<ml:imag>1.5</ml:imag>"),
        eval(&root(&complex_f, "z", None)),
        eval(&root(&linear, "distance", Some((&metre(0.), &metre(5.))))),
    ]);
    near(scalar(&r[1]).value.re, 2_f64.sqrt(), 1e-5);
    let z = scalar(&r[3]).value;
    near(z.re, 0., 1e-5);
    near(z.im, 1., 1e-5);
    let q = scalar(&r[4]);
    near(q.value.re, 3., 1e-5);
    assert_eq!(q.dimension[0], 1.);
}

#[test]
fn root_rejects_missing_guess_bad_brackets_and_poles_instead_of_false_roots() {
    let no_root = binary("plus", &binary("pow", &id("x"), &real(2.)), &real(1.));
    let pole = binary(
        "div",
        &real(1.),
        &binary("minus", &id("x"), &real(0.123456789)),
    );
    let rows = run(&[
        eval(&root(&id("x"), "x", None)),
        eval(&root(&no_root, "x", Some((&real(-1.), &real(1.))))),
        eval(&root(&id("x"), "x", Some((&real(2.), &real(1.))))),
        eval(&root(&pole, "x", Some((&real(0.), &real(1.))))),
    ]);
    for row in rows {
        assert!(row.error.is_some(), "{row:?}");
        assert!(row.value.is_none());
    }
}

#[test]
fn derivative_orders_zero_through_five_and_blank_degree_use_bound_point() {
    let polynomial = binary("pow", &id("x"), &real(5.));
    let mut fragments = vec![define("x", &real(2.))];
    for n in 0..=5 {
        fragments.push(eval(&derivative(&polynomial, "x", &real(n as f64))));
    }
    fragments.push(eval(&derivative(&polynomial, "x", "<ml:placeholder/>")));
    fragments.push(eval(&id("x")));
    let rows = run(&fragments);
    for (row, expected) in rows[1..7].iter().zip([32., 80., 160., 240., 240., 120.]) {
        near(scalar(row).value.re, expected, 2e-5);
    }
    near(scalar(&rows[7]).value.re, 80., 1e-6);
    assert_eq!(scalar(&rows[8]).value.re, 2.);
}

#[test]
fn derivative_units_complex_values_and_partial_operator_are_preserved() {
    let square = binary("pow", &id("x"), &real(2.));
    let complex = binary("mult", "<ml:imag>1</ml:imag>", &square);
    let rows = run(&[
        define("x", &metre(3.)),
        eval(&derivative(&complex, "x", &real(1.))),
        eval(&calculus("partDerivative", &square, "x", "")),
        eval(&derivative(&square, "x", &real(6.))),
    ]);
    let q = scalar(&rows[1]);
    near(q.value.re, 0., 1e-8);
    near(q.value.im, 6., 1e-6);
    assert_eq!(q.dimension[0], 1.);
    near(scalar(&rows[2]).value.re, 6., 1e-6);
    assert_eq!(rows[3].error.as_ref().unwrap().code, "numeric_domain");
}

#[test]
fn finite_integrals_support_reversed_bounds_complex_values_units_and_nested_binders() {
    let cube = binary("pow", &id("x"), &real(3.));
    let imaginary = binary("mult", "<ml:imag>2</ml:imag>", &id("x"));
    let nested = integral(
        &integral(
            &binary("plus", &id("x"), &id("y")),
            "y",
            &real(0.),
            &id("x"),
        ),
        "x",
        &real(0.),
        &real(1.),
    );
    let rows = run(&[
        eval(&integral(&cube, "x", &real(0.), &real(2.))),
        eval(&integral(&cube, "x", &real(2.), &real(0.))),
        eval(&integral(&imaginary, "x", &metre(0.), &metre(2.))),
        eval(&nested),
    ]);
    near(scalar(&rows[0]).value.re, 4., 1e-10);
    near(scalar(&rows[1]).value.re, -4., 1e-10);
    let q = scalar(&rows[2]);
    near(q.value.im, 4., 1e-8);
    assert_eq!(q.dimension[0], 2.);
    near(scalar(&rows[3]).value.re, 0.5, 1e-8);
}

#[test]
fn integral_tolerance_and_unimplemented_algorithms_fail_explicitly() {
    let tol = "<ml:define><ml:id labels=\"SYSTEM\">TOL</ml:id><ml:real>0</ml:real></ml:define>"
        .to_string();
    let normal = integral(&id("x"), "x", &real(0.), &real(1.));
    let romberg = normal.replace("<ml:integral/>", "<ml:integral algorithm=\"romberg\"/>");
    let rows = run(&[eval(&romberg), tol, eval(&normal)]);
    assert!(rows[0].error.as_ref().unwrap().unsupported);
    assert_eq!(rows[2].error.as_ref().unwrap().code, "numeric_domain");
}

#[test]
fn nested_numerical_work_is_bounded_and_failure_does_not_poison_the_next_region() {
    let mut body = real(1.);
    for i in 0..6 {
        body = integral(&body, &format!("x{i}"), &real(0.), &real(1.));
    }
    let rows = run(&[
        eval(&body),
        eval(&integral(&id("x"), "x", &real(0.), &real(1.))),
    ]);
    assert_eq!(
        rows[0].error.as_ref().unwrap().code,
        "numeric_nonconvergence",
        "{:?}",
        rows[0].error
    );
    near(scalar(&rows[1]).value.re, 0.5, 1e-10);
}

#[test]
fn excessive_operator_nesting_is_explicitly_unsupported() {
    let mut body = real(1.);
    for i in 0..10 {
        body = integral(&body, &format!("x{i}"), &real(0.), &real(1.));
    }
    let rows = run(&[eval(&body)]);
    assert!(rows[0].error.as_ref().unwrap().unsupported);
}

#[test]
fn adaptive_subdivision_and_varying_steps_handle_nonpolynomial_functions() {
    let exp = format!(
        "<ml:apply><ml:id labels=\"FUNCTION\">exp</ml:id>{}</ml:apply>",
        id("x")
    );
    let sin = format!(
        "<ml:apply><ml:id labels=\"FUNCTION\">sin</ml:id>{}</ml:apply>",
        id("x")
    );
    let rows=run(&[
        "<ml:define><ml:id labels=\"SYSTEM\">TOL</ml:id><ml:real>0.0000000001</ml:real></ml:define>".into(),
        eval(&integral(&exp,"x",&real(0.),&real(1.))),
        define("x",&real(0.3)),eval(&derivative(&sin,"x",&real(1.)))
    ]);
    near(scalar(&rows[1]).value.re, std::f64::consts::E - 1., 1e-9);
    near(scalar(&rows[3]).value.re, 0.3_f64.cos(), 1e-8);
}

#[test]
fn calculus_does_not_discard_unknown_semantic_attributes_or_incompatible_units() {
    let body = integral(&id("x"), "x", &real(0.), &metre(1.));
    let unknown = body.replace("<ml:integral/>", "<ml:integral custom-mode=\"different\"/>");
    let derivative_unknown = derivative(&id("x"), "x", &real(1.)).replace(
        "<ml:derivative/>",
        "<ml:derivative custom-mode=\"different\"/>",
    );
    let rows = run(&[
        eval(&body),
        eval(&unknown),
        define("x", &real(1.)),
        eval(&derivative_unknown),
    ]);
    assert_eq!(rows[0].error.as_ref().unwrap().code, "bad%_dimensions");
    assert!(rows[1].error.as_ref().unwrap().unsupported);
    assert!(rows[3].error.as_ref().unwrap().unsupported);
}

fn fold(op: &str, body: &str, variable: &str, a: &str, b: &str) -> String {
    calculus(
        op,
        body,
        variable,
        &format!("<ml:lowerBound>{a}</ml:lowerBound><ml:upperBound>{b}</ml:upperBound>"),
    )
}
fn polynomial(coefficients: &[&str]) -> String {
    format!(
        "<ml:apply><ml:id labels=\"FUNCTION\">polyroots</ml:id><ml:matrix rows=\"{}\" cols=\"1\">{}</ml:matrix></ml:apply>",
        coefficients.len(),
        coefficients.join("")
    )
}
fn root_values(row: &RegionEvaluation) -> Vec<num_complex::Complex64> {
    assert!(row.error.is_none(), "{:?}", row.error);
    let Some(Value::Matrix {
        rows,
        cols,
        elements,
    }) = &row.value
    else {
        panic!("{row:?}")
    };
    assert_eq!(*cols, 1);
    assert_eq!(*rows, elements.len());
    elements
        .iter()
        .map(|q| {
            assert!(q.is_scalar());
            q.value
        })
        .collect()
}

#[test]
fn bounded_sums_products_and_nested_binders_are_local_and_preserve_dimensions() {
    let square = binary("pow", &id("j"), &real(2.));
    let nested = fold(
        "summation",
        &fold("product", &id("j"), "j", &real(1.), &real(3.)),
        "j",
        &real(1.),
        &real(2.),
    );
    let rows = run(&[
        define("j", &metre(3.)),
        eval(&fold("summation", &square, "j", &real(1.), &real(4.))),
        eval(&fold("product", &id("j"), "j", &real(1.), &real(5.))),
        eval(&fold(
            "summation",
            &binary("mult", &id("j"), &metre(2.)),
            "j",
            &real(1.),
            &real(3.),
        )),
        eval(&fold("product", &metre(2.), "j", &real(1.), &real(3.))),
        eval(&nested),
        eval(&id("j")),
    ]);
    assert_eq!(scalar(&rows[1]).value.re, 30.);
    assert_eq!(scalar(&rows[2]).value.re, 120.);
    let sum = scalar(&rows[3]);
    assert_eq!(sum.value.re, 12.);
    assert_eq!(sum.dimension[0], 1.);
    let product = scalar(&rows[4]);
    assert_eq!(product.value.re, 8.);
    assert_eq!(product.dimension[0], 3.);
    assert_eq!(scalar(&rows[5]).value.re, 12.);
    let outer = scalar(&rows[6]);
    assert_eq!(outer.value.re, 3.);
    assert_eq!(outer.dimension[0], 1.);
}

#[test]
fn folds_reject_unreconstructed_noninteger_limits_invalid_units_and_excessive_work() {
    let rows = run(&[
        eval(&fold("summation", &id("j"), "j", &real(0.4), &real(3.7))),
        eval(&fold("product", &id("j"), "j", &metre(1.), &metre(3.))),
        eval(&fold("summation", &id("j"), "j", &real(0.), &real(10000.))),
        eval(&fold("product", &id("j"), "j", &real(3.), &real(1.))),
    ]);
    assert!(rows[0].error.as_ref().unwrap().unsupported);
    assert_eq!(rows[1].error.as_ref().unwrap().code, "numeric_domain");
    assert_eq!(
        rows[2].error.as_ref().unwrap().code,
        "numeric_nonconvergence"
    );
    assert!(rows[3].error.is_some());
}

#[test]
fn polyroots_constant_first_order_and_origin_do_not_change_polynomial_coefficients() {
    let p = polynomial(&[&real(-6.), &real(11.), &real(-6.), &real(1.)]);
    let rows = run(&[
        eval(&p),
        "<ml:define><ml:id labels=\"SYSTEM\">ORIGIN</ml:id><ml:real>1</ml:real></ml:define>".into(),
        eval(&p),
    ]);
    for row in [&rows[0], &rows[2]] {
        let mut roots = root_values(row);
        roots.sort_by(|a, b| a.re.total_cmp(&b.re));
        assert_eq!(roots.len(), 3);
        for (r, expected) in roots.iter().zip([1., 2., 3.]) {
            near(r.re, expected, 1e-8);
            near(r.im, 0., 1e-8);
        }
    }
}

#[test]
fn polyroots_complex_repeated_and_zero_roots_are_computed_and_verified() {
    for coefficients in [[1., 0., 1.], [1., -2., 1.], [0., 0., 1.]] {
        let strings: Vec<_> = coefficients.into_iter().map(real).collect();
        let refs: Vec<_> = strings.iter().map(String::as_str).collect();
        let rows = run(&[eval(&polynomial(&refs))]);
        for r in root_values(&rows[0]) {
            let residual = coefficients[0] + r * (coefficients[1] + r * coefficients[2]);
            assert!(residual.norm() < 1e-9, "{r:?}: {residual:?}");
        }
    }
    let p = polynomial(&["<ml:imag>-1</ml:imag>", &real(0.), &real(1.)]);
    let rows = run(&[eval(&p)]);
    for r in root_values(&rows[0]) {
        assert!((r * r - num_complex::Complex64::new(0., 1.)).norm() < 1e-9);
    }
}

#[test]
fn polyroots_common_units_cancel_but_invalid_coefficients_and_shapes_fail() {
    let valid = polynomial(&[&metre(-1.), &metre(0.), &metre(1.)]);
    let bad_units = polynomial(&[&real(-1.), &metre(1.)]);
    let leading_zero = polynomial(&[&real(1.), &real(0.)]);
    let row_vector = polynomial(&[&real(1.), &real(1.)])
        .replace("rows=\"2\" cols=\"1\"", "rows=\"1\" cols=\"2\"");
    let rows = run(&[
        eval(&valid),
        eval(&bad_units),
        eval(&leading_zero),
        eval(&row_vector),
        eval(&polynomial(&[&real(1.)])),
    ]);
    assert_eq!(root_values(&rows[0]).len(), 2);
    for row in &rows[1..] {
        assert!(row.error.is_some(), "{row:?}");
    }
}
