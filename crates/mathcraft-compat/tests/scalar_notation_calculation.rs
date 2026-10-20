use mathcraft_compat::{
    CalculationControl, RegionEvaluation, Value, arrays, editor_bridge::worksheet_xml,
    evaluate_worksheet,
};
use mathcraft_core::{Quantity, unit};
use num_complex::Complex64;
use std::f64::consts::{FRAC_PI_2, PI};

fn real(x: f64) -> String {
    format!("<ml:real>{x}</ml:real>")
}
fn apply(op: &str, arguments: &[String]) -> String {
    format!("<ml:apply><ml:{op}/>{}</ml:apply>", arguments.join(""))
}
fn unary(op: &str, argument: &str) -> String {
    apply(op, &[argument.into()])
}
fn polar(magnitude: &str, angle: &str) -> String {
    apply("polar", &[magnitude.into(), angle.into()])
}
fn complex(re: f64, im: f64) -> String {
    apply("plus", &[real(re), format!("<ml:imag>{im}</ml:imag>")])
}
fn physical(value: f64, name: &str) -> String {
    apply(
        "scale",
        &[
            real(value),
            format!("<ml:id labels=\"UNIT\">{name}</ml:id>"),
        ],
    )
}
fn matrix(rows: usize, cols: usize, elements: &[String]) -> String {
    assert_eq!(rows * cols, elements.len());
    let body = (0..cols)
        .flat_map(|col| (0..rows).map(move |row| elements[row * cols + col].as_str()))
        .collect::<String>();
    format!("<ml:matrix rows=\"{rows}\" cols=\"{cols}\">{body}</ml:matrix>")
}
fn m(rows: usize, cols: usize, elements: &[f64]) -> String {
    matrix(
        rows,
        cols,
        &elements.iter().copied().map(real).collect::<Vec<_>>(),
    )
}
fn query(expression: &str) -> String {
    format!("<ml:eval>{expression}<ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>")
}
fn run(parts: &[String]) -> Vec<RegionEvaluation> {
    let regions = parts
        .iter()
        .enumerate()
        .map(|(i, part)| (i as u64, 0., i as f64 * 50., part.clone()))
        .collect::<Vec<_>>();
    evaluate_worksheet(&worksheet_xml(&regions).unwrap()).unwrap()
}
fn evaluate(expression: &str) -> RegionEvaluation {
    run(&[query(expression)]).remove(0)
}
fn scalar(row: &RegionEvaluation) -> Quantity {
    assert!(row.error.is_none(), "{row:?}");
    let Some(Value::Scalar { quantity }) = &row.value else {
        panic!("Expected scalar: {row:?}");
    };
    *quantity
}
fn cells(row: &RegionEvaluation, shape: (usize, usize)) -> &[Quantity] {
    assert!(row.error.is_none(), "{row:?}");
    let Some(Value::Matrix {
        rows,
        cols,
        elements,
    }) = &row.value
    else {
        panic!("Expected matrix: {row:?}");
    };
    assert_eq!((*rows, *cols), shape);
    elements
}
fn near(actual: Complex64, expected: Complex64) {
    assert!((actual - expected).norm() < 1e-12, "{actual} != {expected}");
}

#[test]
fn percent_is_a_numeric_operator_on_complex_values_not_result_formatting() {
    assert_eq!(
        scalar(&evaluate(&unary("percent", &real(25.)))).value.re,
        0.25
    );
    near(
        scalar(&evaluate(&unary("percent", &complex(200., -300.)))).value,
        Complex64::new(2., -3.),
    );
    assert_eq!(
        scalar(&evaluate(&apply(
            "plus",
            &[real(1.), unary("percent", &real(25.))]
        )))
        .value
        .re,
        1.25
    );
    assert_eq!(
        scalar(&evaluate(&unary("percent", &unary("percent", &real(100.)))))
            .value
            .re,
        0.01
    );
}

#[test]
fn percent_and_degrees_keep_rectangular_shape_cell_units_and_complex_components() {
    let source = matrix(
        2,
        3,
        &[
            physical(100., "m"),
            complex(200., 300.),
            real(-100.),
            physical(400., "s"),
            real(0.),
            physical(600., "A"),
        ],
    );
    for (op, factor) in [("percent", 0.01), ("degree_op", PI / 180.)] {
        let result = evaluate(&unary(op, &source));
        let values = cells(&result, (2, 3));
        for (q, expected) in values.iter().zip([
            Complex64::new(100., 0.),
            Complex64::new(200., 300.),
            Complex64::new(-100., 0.),
            Complex64::new(400., 0.),
            Complex64::new(0., 0.),
            Complex64::new(600., 0.),
        ]) {
            near(q.value, expected * factor);
        }
        assert_eq!(values[0].dimension, unit("m").unwrap().dimension);
        assert_eq!(values[3].dimension, unit("s").unwrap().dimension);
        assert_eq!(values[5].dimension, unit("A").unwrap().dimension);
        assert!(values[1].is_scalar() && values[2].is_scalar() && values[4].is_scalar());
    }
    let one = evaluate(&unary("percent", &m(1, 1, &[100.])));
    assert_eq!(cells(&one, (1, 1))[0].value.re, 1.);
}

#[test]
fn degree_operator_converts_to_radians_without_variable_or_unit_label_aliasing() {
    near(
        scalar(&evaluate(&unary("degree_op", &real(180.)))).value,
        Complex64::new(PI, 0.),
    );
    let rows = run(&[
        "<ml:define><ml:id labels=\"VARIABLE\">deg</ml:id><ml:real>7</ml:real></ml:define>".into(),
        query(&unary("degree_op", &real(90.))),
        query(&physical(90., "deg")),
        query("<ml:id labels=\"VARIABLE\">deg</ml:id>"),
    ]);
    assert_eq!(scalar(&rows[1]).value.re, FRAC_PI_2);
    assert_eq!(scalar(&rows[2]).value.re, FRAC_PI_2);
    assert_eq!(scalar(&rows[3]).value.re, 7.);
    let angle = unary("degree_op", &real(90.));
    let sine = format!("<ml:apply><ml:id labels=\"FUNCTION\">sin</ml:id>{angle}</ml:apply>");
    assert_eq!(scalar(&evaluate(&sine)).value.re, 1.);
}

#[test]
fn polar_uses_radians_or_explicit_degree_operand_and_accepts_zero_magnitude() {
    for angle in [real(FRAC_PI_2), unary("degree_op", &real(90.))] {
        near(
            scalar(&evaluate(&polar(&real(2.), &angle))).value,
            Complex64::new(0., 2.),
        );
    }
    near(
        scalar(&evaluate(&polar(&real(2.), &real(-FRAC_PI_2)))).value,
        Complex64::new(0., -2.),
    );
    assert_eq!(
        scalar(&evaluate(&polar(&real(0.), &real(1.)))).value,
        Complex64::new(0., 0.)
    );
    assert_eq!(scalar(&evaluate(&polar(&real(2.), &real(0.)))).value.re, 2.);
}

#[test]
fn polar_array_path_keeps_ordinary_multiplication_separate_from_vectorized_mapping() {
    let magnitudes = m(2, 1, &[1., 2.]);
    let angles = m(2, 1, &[0., FRAC_PI_2]);
    let normal = polar(&magnitudes, &angles);
    near(scalar(&evaluate(&normal)).value, Complex64::new(1., -2.));
    let mapped = evaluate(&unary("vectorize", &normal));
    let values = cells(&mapped, (2, 1));
    near(values[0].value, Complex64::new(1., 0.));
    near(values[1].value, Complex64::new(0., 2.));

    let scalar_magnitude = evaluate(&polar(&real(2.), &angles));
    let values = cells(&scalar_magnitude, (2, 1));
    near(values[0].value, Complex64::new(2., 0.));
    near(values[1].value, Complex64::new(0., 2.));
    let matrix_magnitude = evaluate(&polar(&m(2, 2, &[1., 2., 3., 4.]), &angles));
    let values = cells(&matrix_magnitude, (2, 1));
    near(values[0].value, Complex64::new(1., 2.));
    near(values[1].value, Complex64::new(3., 4.));
    let scalar_angle = evaluate(&polar(&m(1, 2, &[1., 2.]), &real(FRAC_PI_2)));
    let values = cells(&scalar_angle, (1, 2));
    near(values[0].value, Complex64::new(0., 1.));
    near(values[1].value, Complex64::new(0., 2.));
    let one_angle = evaluate(&polar(&real(2.), &m(1, 1, &[0.])));
    assert_eq!(cells(&one_angle, (1, 1))[0].value.re, 2.);
}

#[test]
fn polar_rejects_negative_complex_dimensional_or_noncolumn_angle_operands() {
    for expression in [
        polar(&real(-1.), &real(0.)),
        polar(&complex(1., 1.), &real(0.)),
        polar(&real(1.), &complex(0., 1.)),
        polar(&real(1.), &m(1, 2, &[0., 1.])),
        polar(&m(2, 1, &[1., -1.]), &real(0.)),
        polar(&real(1.), &matrix(2, 1, &[real(0.), complex(0., 1.)])),
    ] {
        let row = evaluate(&expression);
        assert!(row.value.is_none(), "{expression}: {row:?}");
        assert_eq!(row.error.unwrap().code, "invalid_argument");
    }
    for expression in [
        polar(&physical(2., "V"), &real(0.)),
        polar(&real(2.), &physical(1., "m")),
        polar(
            &m(2, 1, &[1., 2.]),
            &matrix(2, 1, &[real(0.), physical(1., "s")]),
        ),
    ] {
        let row = evaluate(&expression);
        assert!(row.value.is_none());
        assert_eq!(row.error.unwrap().code, "bad%_dimensions%_unitless");
    }
    let wrong_shape = evaluate(&polar(&m(3, 1, &[1., 2., 3.]), &m(2, 1, &[0., 1.])));
    assert!(wrong_shape.error.is_some() && wrong_shape.value.is_none());
}

#[test]
fn strict_native_identity_arity_attributes_and_named_functions_remain_distinct() {
    for expression in [
        "<ml:apply><ml:percent/></ml:apply>",
        "<ml:apply><ml:percent/><ml:real>1</ml:real><ml:real>2</ml:real></ml:apply>",
        "<ml:apply><ml:degree_op invalid=\"yes\"/><ml:real>1</ml:real></ml:apply>",
        "<ml:apply><ml:polar/><ml:real>1</ml:real></ml:apply>",
        "<ml:apply><ml:degree/><ml:real>1</ml:real></ml:apply>",
        "<ml:apply><foreign:percent xmlns:foreign=\"urn:wrong\"/><ml:real>1</ml:real></ml:apply>",
    ] {
        let row = evaluate(expression);
        assert!(row.error.is_some() && row.value.is_none(), "{expression}");
    }
    for name in ["percent", "degree_op", "polar"] {
        let function = format!(
            "<ml:define><ml:function><ml:id labels=\"FUNCTION\">{name}</ml:id><ml:boundVars><ml:id labels=\"VARIABLE\">x</ml:id></ml:boundVars></ml:function><ml:apply><ml:plus/><ml:id labels=\"VARIABLE\">x</ml:id><ml:real>1</ml:real></ml:apply></ml:define>"
        );
        let call = format!(
            "<ml:apply><ml:id labels=\"FUNCTION\">{name}</ml:id><ml:real>7</ml:real></ml:apply>"
        );
        assert!(
            evaluate(&call).error.is_some(),
            "No synthetic builtin {name}"
        );
        let rows = run(&[function, query(&call)]);
        assert_eq!(scalar(&rows[1]).value.re, 8.);
    }
}

#[test]
fn notation_numeric_helpers_reject_ranges_bad_shapes_nonfinite_values_and_cancel() {
    let range = Value::Range {
        start: Quantity::scalar(0.),
        step: Quantity::scalar(1.),
        end: Quantity::scalar(2.),
        count: 3,
    };
    let malformed = Value::Matrix {
        rows: 2,
        cols: 2,
        elements: vec![Quantity::scalar(1.)],
    };
    let nonfinite = Value::Scalar {
        quantity: Quantity::scalar(f64::INFINITY),
    };
    let one = Value::Scalar {
        quantity: Quantity::scalar(1.),
    };
    for bad in [&range, &malformed, &nonfinite] {
        assert!(arrays::unary("percent", bad).is_err());
        assert!(arrays::unary("degree_op", bad).is_err());
        assert!(arrays::polar(bad, &one).is_err());
        assert!(arrays::polar(&one, bad).is_err());
    }
    let control = CalculationControl::default();
    control.cancel();
    for op in ["percent", "degree_op"] {
        let fault = control.run(|| arrays::unary(op, &one)).unwrap_err();
        assert!(fault.is_cancelled());
    }
    assert!(
        control
            .run(|| arrays::polar(&one, &one))
            .unwrap_err()
            .is_cancelled()
    );
}
