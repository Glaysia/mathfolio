//! Independent fixtures, not original program outputs.
use mathcraft_compat::{Value, statistics};
use mathcraft_core::Quantity;
use num_complex::Complex64;

fn scalar(x: f64) -> Value {
    Value::Scalar {
        quantity: Quantity::scalar(x),
    }
}
fn matrix(rows: usize, cols: usize, x: &[f64]) -> Value {
    Value::Matrix {
        rows,
        cols,
        elements: x.iter().map(|&x| Quantity::scalar(x)).collect(),
    }
}
fn q(v: Value) -> Quantity {
    match v {
        Value::Scalar { quantity } => quantity,
        _ => panic!("Expected scalar"),
    }
}
fn elements(v: Value) -> Vec<Quantity> {
    match v {
        Value::Matrix { elements, .. } => elements,
        _ => panic!("Expected array"),
    }
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-12, "{a} != {b}");
}

#[test]
fn variadic_means_count_elements_and_keep_units() {
    let a = matrix(2, 2, &[1., 2., 3., 4.]);
    close(
        q(statistics::call("mean", &[a, scalar(10.)]).unwrap())
            .value
            .re,
        4.,
    );
    close(
        q(statistics::call("gmean", &[scalar(2.), scalar(8.)]).unwrap())
            .value
            .re,
        4.,
    );
    close(
        q(statistics::call("hmean", &[scalar(2.), scalar(6.)]).unwrap())
            .value
            .re,
        3.,
    );
    let mut x = Quantity::scalar(3.);
    x.dimension[0] = 1.;
    let mut y = x;
    y.value.re = 5.;
    let result = q(statistics::call(
        "mean",
        &[Value::Scalar { quantity: x }, Value::Scalar { quantity: y }],
    )
    .unwrap());
    assert_eq!(result.dimension, x.dimension);
    close(result.value.re, 4.);
}

#[test]
fn sample_population_capitalization_and_complex_modulus_squared() {
    let data = matrix(3, 1, &[2., 4., 6.]);
    close(
        q(statistics::call("var", std::slice::from_ref(&data)).unwrap())
            .value
            .re,
        8. / 3.,
    );
    close(
        q(statistics::call("Var", std::slice::from_ref(&data)).unwrap())
            .value
            .re,
        4.,
    );
    close(
        q(statistics::call("stdev", std::slice::from_ref(&data)).unwrap())
            .value
            .re,
        (8f64 / 3.).sqrt(),
    );
    close(q(statistics::call("Stdev", &[data]).unwrap()).value.re, 2.);
    let mut x = Quantity::scalar(1.);
    x.value.im = 2.;
    x.dimension[0] = 1.;
    let mut y = x;
    y.value = Complex64::new(3., 4.);
    let values = [Value::Scalar { quantity: x }, Value::Scalar { quantity: y }];
    let result = q(statistics::call("var", &values).unwrap());
    close(result.value.re, 2.);
    assert_eq!(result.value.im, 0.);
    assert_eq!(result.dimension[0], 2.);
    let result = q(statistics::call("stdev", &values).unwrap());
    assert_eq!(result.dimension[0], 1.);
    assert_eq!(
        q(statistics::call("Var", &[scalar(7.)]).unwrap()).value.re,
        0.
    );
    assert_eq!(
        q(statistics::call("var", &[scalar(7.)]).unwrap()).value.re,
        0.
    );
}

#[test]
fn median_and_componentwise_complex_extrema() {
    close(
        q(statistics::call("median", &[matrix(2, 2, &[8., 1., 4., 2.])]).unwrap())
            .value
            .re,
        3.,
    );
    let mut a = Quantity::scalar(1.);
    a.value.im = 9.;
    let mut b = Quantity::scalar(5.);
    b.value.im = -2.;
    let values = [Value::Scalar { quantity: a }, Value::Scalar { quantity: b }];
    assert_eq!(
        q(statistics::call("max", &values).unwrap()).value,
        Complex64::new(5., 9.)
    );
    assert_eq!(
        q(statistics::call("min", &values).unwrap()).value,
        Complex64::new(1., -2.)
    );
    assert!(statistics::call("median", &values).is_err());
}

#[test]
fn sort_ignores_imaginary_keys_rank_averages_ties_reverse_reverses_rows() {
    let mut e = vec![
        Quantity::scalar(3.),
        Quantity::scalar(1.),
        Quantity::scalar(1.),
    ];
    e[0].value.im = -8.;
    e[1].value.im = 7.;
    e[2].value.im = 9.;
    let v = Value::Matrix {
        rows: 3,
        cols: 1,
        elements: e,
    };
    let sorted = elements(statistics::call("sort", std::slice::from_ref(&v)).unwrap());
    assert_eq!(
        sorted.iter().map(|q| q.value.re).collect::<Vec<_>>(),
        [1., 1., 3.]
    );
    assert_eq!(sorted.last().unwrap().value.im, -8.);
    let rank = elements(statistics::call("Rank", &[v]).unwrap());
    assert_eq!(
        rank.iter().map(|q| q.value.re).collect::<Vec<_>>(),
        [3., 1.5, 1.5]
    );
    let reverse =
        elements(statistics::call("reverse", &[matrix(2, 3, &[1., 2., 3., 4., 5., 6.])]).unwrap());
    assert_eq!(
        reverse.iter().map(|q| q.value.re).collect::<Vec<_>>(),
        [4., 5., 6., 1., 2., 3.]
    );
    let row = elements(statistics::call("reverse", &[matrix(1, 3, &[1., 2., 3.])]).unwrap());
    assert_eq!(
        row.iter().map(|q| q.value.re).collect::<Vec<_>>(),
        [1., 2., 3.]
    );
}

#[test]
fn covariance_population_denominator_shapes_and_units() {
    let a = matrix(3, 1, &[1., 2., 3.]);
    let b = matrix(3, 1, &[5., 7., 9.]);
    close(
        q(statistics::call("cvar", &[a.clone(), b.clone()]).unwrap())
            .value
            .re,
        4. / 3.,
    );
    close(q(statistics::call("corr", &[a, b]).unwrap()).value.re, 1.);
    assert!(statistics::call("corr", &[matrix(2, 1, &[1., 1.]), matrix(2, 1, &[1., 2.])]).is_err());
    assert!(statistics::call("cvar", &[matrix(1, 2, &[1., 2.]), matrix(2, 1, &[1., 2.])]).is_err());
    assert!(!statistics::is_builtin("covar"));
    let dimensional = |dimension: usize, numbers: &[f64]| Value::Matrix {
        rows: numbers.len(),
        cols: 1,
        elements: numbers
            .iter()
            .map(|&x| {
                let mut q = Quantity::scalar(x);
                q.dimension[dimension] = 1.;
                q
            })
            .collect(),
    };
    let args = [dimensional(0, &[1., 2., 3.]), dimensional(1, &[5., 7., 9.])];
    let covariance = q(statistics::call("cvar", &args).unwrap());
    assert_eq!(covariance.dimension, [1., 1., 0., 0., 0., 0., 0.]);
    assert_eq!(
        q(statistics::call("corr", &args).unwrap()).dimension,
        [0.; 7]
    );
}

#[test]
fn native_worksheet_function_dispatch_reaches_statistics() {
    let xml = r#"<worksheet xmlns="http://schemas.mathsoft.com/worksheet50"><regions><region region-id="1" left="0" top="0"><math><eval xmlns="http://schemas.mathsoft.com/math50"><apply><id labels="FUNCTION">Stdev</id><sequence><real>2</real><real>4</real><real>6</real></sequence></apply><unitOverride><placeholder/></unitOverride></eval></math></region></regions></worksheet>"#;
    let rows = mathcraft_compat::evaluate_worksheet(xml).unwrap();
    assert!(rows[0].error.is_none(), "{:?}", rows[0].error);
    close(q(rows[0].value.clone().unwrap()).value.re, 2.);
}

#[test]
fn malformed_shapes_ranges_nonfinite_and_mixed_dimensions_are_rejected() {
    assert!(statistics::call("mean", &[]).is_err());
    assert!(statistics::call("mean", &[matrix(2, 2, &[1.])]).is_err());
    assert!(statistics::call("mean", &[scalar(f64::NAN)]).is_err());
    assert!(statistics::call("hmean", &[scalar(0.)]).is_err());
    assert!(statistics::call("gmean", &[scalar(-1.)]).is_err());
    let mut unit = Quantity::scalar(1.);
    unit.dimension[0] = 1.;
    assert_eq!(
        statistics::call("mean", &[scalar(1.), Value::Scalar { quantity: unit }])
            .unwrap_err()
            .code,
        "bad%_dimensions"
    );
    let range = Value::Range {
        start: Quantity::scalar(1.),
        step: Quantity::scalar(1.),
        end: Quantity::scalar(3.),
        count: 3,
    };
    assert!(statistics::call("mean", &[range]).is_err());
    assert!(statistics::call("sort", &[matrix(1, 2, &[2., 1.])]).is_err());
}
