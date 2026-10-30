use mathfolio_compat::{Value, editor_bridge::worksheet_xml, evaluate_worksheet};
use mathfolio_core::Quantity;

fn evaluate(body: &str) -> mathfolio_compat::RegionEvaluation {
    let fragment =
        format!("<ml:eval>{body}<ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>");
    evaluate_worksheet(&worksheet_xml(&[(0, 0., 0., fragment)]).unwrap())
        .unwrap()
        .remove(0)
}
fn call(name: &str, args: &str) -> mathfolio_compat::RegionEvaluation {
    evaluate(&format!(
        "<ml:apply><ml:id labels=\"FUNCTION\">{name}</ml:id><ml:sequence>{args}</ml:sequence></ml:apply>"
    ))
}
fn scalar(row: mathfolio_compat::RegionEvaluation) -> Quantity {
    assert!(row.error.is_none(), "{:?}", row.error);
    match row.value.unwrap() {
        Value::Scalar { quantity } => quantity,
        other => panic!("{other:?}"),
    }
}
fn real(value: f64) -> String {
    format!("<ml:real>{value}</ml:real>")
}

#[test]
fn inverse_reciprocal_and_hyperbolic_families_compute_actual_values() {
    for (name, argument, answer) in [
        ("asin", 0.5, std::f64::consts::PI / 6.),
        ("acos", 0.5, std::f64::consts::PI / 3.),
        ("atan", 1., std::f64::consts::FRAC_PI_4),
        ("acot", -1., 3. * std::f64::consts::FRAC_PI_4),
        ("acot", 0., std::f64::consts::FRAC_PI_2),
        ("asec", 2., std::f64::consts::PI / 3.),
        ("acsc", 2., std::f64::consts::PI / 6.),
        ("cosh", 0., 1.),
        ("sinh", 0., 0.),
        ("tanh", 0., 0.),
        ("sech", 0., 1.),
        ("asinh", 0., 0.),
        ("acosh", 1., 0.),
        ("atanh", 0., 0.),
        ("sinc", 0., 1.),
    ] {
        assert!(
            (scalar(call(name, &real(argument))).value.re - answer).abs() < 1e-12,
            "{name}"
        );
    }
}

#[test]
fn logarithm_optional_base_and_complex_continuation_are_preserved() {
    assert!((scalar(call("log", &(real(8.) + &real(2.)))).value.re - 3.).abs() < 1e-12);
    let q = scalar(call("ln", &real(-1.)));
    assert!(q.value.re.abs() < 1e-12 && (q.value.im.abs() - std::f64::consts::PI).abs() < 1e-12);
    assert!(call("log", &(real(8.) + &real(1.))).error.is_some());
}

#[test]
fn polar_coordinate_order_and_branch_are_mathcad_order() {
    let cases = [
        (1., 0., 0.),
        (0., 1., std::f64::consts::FRAC_PI_2),
        (-1., 0., std::f64::consts::PI),
        (0., -1., -std::f64::consts::FRAC_PI_2),
    ];
    for (x, y, expected) in cases {
        let args = real(x) + &real(y);
        assert!((scalar(call("atan2", &args)).value.re - expected).abs() < 1e-12);
        let angle = scalar(call("angle", &args)).value.re;
        assert!((0. ..std::f64::consts::TAU).contains(&angle));
        assert!((angle - expected.rem_euclid(std::f64::consts::TAU)).abs() < 1e-12);
    }
    assert!(call("atan2", &(real(0.) + &real(0.))).error.is_some());
}

#[test]
fn uppercase_rounding_accepts_compatible_units_lowercase_rejects_them() {
    let metres =
        "<ml:apply><ml:scale/><ml:real>2.6</ml:real><ml:id labels=\"UNIT\">m</ml:id></ml:apply>";
    let centimetres =
        "<ml:apply><ml:scale/><ml:real>50</ml:real><ml:id labels=\"UNIT\">cm</ml:id></ml:apply>";
    for (name, expected) in [("Floor", 2.5), ("Ceil", 3.), ("Round", 2.5), ("Trunc", 2.5)] {
        let q = scalar(call(name, &(metres.to_string() + centimetres)));
        assert!((q.value.re - expected).abs() < 1e-12);
        assert_eq!(q.dimension, [1., 0., 0., 0., 0., 0., 0.]);
    }
    assert_eq!(
        call("floor", metres).error.unwrap().code,
        "bad%_dimensions%_unitless"
    );
    assert_eq!(
        scalar(call("round", &(real(1250.) + &real(-2.)))).value.re,
        1300.
    );
    assert_eq!(scalar(call("floor", &real(-2.1))).value.re, -3.);
    assert_eq!(scalar(call("trunc", &real(-2.1))).value.re, -2.);
    let q = scalar(call(
        "round",
        "<ml:apply><ml:plus/><ml:real>2.6</ml:real><ml:imag>-3.6</ml:imag></ml:apply>",
    ));
    assert_eq!((q.value.re, q.value.im), (3., -4.));
}

#[test]
fn remainder_uses_dividend_sign_and_signum_has_distinct_zero_default() {
    assert_eq!(scalar(call("mod", &(real(-7.) + &real(3.)))).value.re, -1.);
    assert_eq!(scalar(call("mod", &(real(7.) + &real(-3.)))).value.re, 1.);
    assert_eq!(scalar(call("sign", &real(0.))).value.re, 0.);
    assert_eq!(scalar(call("signum", &real(0.))).value.re, 1.);
    assert_eq!(
        scalar(call("signum", &(real(0.) + &real(42.)))).value.re,
        42.
    );
    assert_eq!(scalar(call("csgn", "<ml:imag>-2</ml:imag>")).value.re, -1.);
    let q = scalar(call(
        "signum",
        "<ml:apply><ml:plus/><ml:real>3</ml:real><ml:imag>4</ml:imag></ml:apply>",
    ));
    assert!((q.value.re - 0.6).abs() < 1e-12 && (q.value.im - 0.8).abs() < 1e-12);
}

#[test]
fn functions_map_column_vectors_and_complex_parts_map_general_matrices() {
    let column =
        "<ml:matrix rows=\"2\" cols=\"1\"><ml:real>0</ml:real><ml:real>1</ml:real></ml:matrix>";
    let row = call("cos", column);
    assert!(row.error.is_none(), "{:?}", row.error);
    let Value::Matrix {
        rows,
        cols,
        elements,
    } = row.value.unwrap()
    else {
        panic!()
    };
    assert_eq!((rows, cols), (2, 1));
    assert_eq!(elements[0].value.re, 1.);
    assert!((elements[1].value.re - 1_f64.cos()).abs() < 1e-12);
    let matrix =
        "<ml:matrix rows=\"1\" cols=\"2\"><ml:imag>1</ml:imag><ml:imag>2</ml:imag></ml:matrix>";
    assert!(call("sin", matrix).error.is_some());
    assert!(call("Im", matrix).error.is_none());
    assert_eq!(scalar(call("IsArray", column)).value.re, 1.);
    assert_eq!(scalar(call("IsScalar", column)).value.re, 0.);
}

#[test]
fn factorial_operator_and_dimensioned_complex_parts_reach_engine() {
    assert_eq!(
        scalar(evaluate(
            "<ml:apply><ml:factorial/><ml:real>10</ml:real></ml:apply>"
        ))
        .value
        .re,
        3628800.
    );
    assert!(
        evaluate("<ml:apply><ml:factorial/><ml:real>-1</ml:real></ml:apply>")
            .error
            .is_some()
    );
    let q = scalar(call(
        "Im",
        "<ml:apply><ml:scale/><ml:imag>3</ml:imag><ml:id labels=\"UNIT\">m</ml:id></ml:apply>",
    ));
    assert_eq!(q.value.re, 3.);
    assert_eq!(q.dimension[0], 1.);
}

#[test]
fn new_formula_labeling_distinguishes_min_function_from_minute_unit() {
    let fragments = [
        "<ml:eval><ml:apply><ml:id>min</ml:id><ml:sequence><ml:real>3</ml:real><ml:real>2</ml:real></ml:sequence></ml:apply><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
        "<ml:eval><ml:id>min</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
    ];
    let inputs: Vec<_> = fragments
        .iter()
        .enumerate()
        .map(|(i, x)| (i as u64, 0., i as f64 * 80., x.to_string()))
        .collect();
    let xml = worksheet_xml(&inputs).unwrap();
    let rows = evaluate_worksheet(&xml).unwrap();
    assert_eq!(scalar(rows[0].clone()).value.re, 2.);
    let minute = scalar(rows[1].clone());
    assert_eq!(minute.value.re, 60.);
    assert_eq!(minute.dimension[2], 1.);
}

#[test]
fn installed_unit_facts_override_ambiguous_legacy_spelling() {
    let g = scalar(evaluate("<ml:id labels=\"UNIT\">g</ml:id>"));
    assert_eq!(g.dimension, [1., 0., -2., 0., 0., 0., 0.]);
    assert_eq!(g.value.re, 9.80665);
    let gram = scalar(evaluate("<ml:id labels=\"UNIT\">gm</ml:id>"));
    assert_eq!(gram.dimension[1], 1.);
    assert_eq!(gram.value.re, 0.001);
    let planck = scalar(evaluate("<ml:id labels=\"CONSTANT\">h</ml:id>"));
    assert_eq!(planck.dimension, [2., 1., -1., 0., 0., 0., 0.]);
    assert!(planck.value.re > 0. && planck.value.re < 1e-33);
}

#[test]
fn failed_origin_does_not_poison_array_functions_without_origin_argument() {
    let fragments = [
        "<ml:define><ml:id labels=\"SYSTEM\">ORIGIN</ml:id><ml:id labels=\"VARIABLE\">missing</ml:id></ml:define>",
        "<ml:eval><ml:apply><ml:id labels=\"FUNCTION\">identity</ml:id><ml:real>2</ml:real></ml:apply><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
    ];
    let inputs: Vec<_> = fragments
        .iter()
        .enumerate()
        .map(|(i, x)| (i as u64, 0., i as f64 * 80., x.to_string()))
        .collect();
    let rows = evaluate_worksheet(&worksheet_xml(&inputs).unwrap()).unwrap();
    assert!(rows[0].error.is_some());
    assert!(rows[1].error.is_none(), "{:?}", rows[1].error);
    assert!(matches!(
        rows[1].value,
        Some(Value::Matrix {
            rows: 2,
            cols: 2,
            ..
        })
    ));
}
