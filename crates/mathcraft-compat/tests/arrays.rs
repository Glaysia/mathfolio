use mathcraft_compat::{Value, arrays};
use mathcraft_core::{Quantity, unit};
use num_complex::Complex64;

fn s(n: f64) -> Value {
    Value::Scalar {
        quantity: Quantity::scalar(n),
    }
}
fn m(r: usize, c: usize, x: &[f64]) -> Value {
    Value::Matrix {
        rows: r,
        cols: c,
        elements: x.iter().map(|x| Quantity::scalar(*x)).collect(),
    }
}
fn qm(r: usize, c: usize, x: Vec<Quantity>) -> Value {
    Value::Matrix {
        rows: r,
        cols: c,
        elements: x,
    }
}
fn q(v: &Value) -> Quantity {
    match v {
        Value::Scalar { quantity } => *quantity,
        _ => panic!("Expected scalar, got {v:?}"),
    }
}
fn e(v: &Value) -> &[Quantity] {
    match v {
        Value::Matrix { elements, .. } => elements,
        _ => panic!("Expected array, got {v:?}"),
    }
}
fn nums(v: &Value) -> Vec<f64> {
    e(v).iter().map(|q| q.value.re).collect()
}
fn call(n: &str, a: &[Value]) -> Value {
    arrays::call(n, a, Quantity::scalar(0.)).unwrap()
}
fn close(a: f64, b: f64) {
    assert!(
        (a - b).abs() <= 1e-12_f64.max(a.abs() * 1e-12),
        "{a} != {b}"
    );
}

#[test]
fn products_distinguish_conjugated_vector_dot_matrix_product_and_scalar_mapping() {
    let a = qm(
        2,
        1,
        vec![
            Quantity {
                value: Complex64::new(1., 1.),
                dimension: [0.; 7],
            },
            Quantity::scalar(2.),
        ],
    );
    let b = qm(
        2,
        1,
        vec![
            Quantity {
                value: Complex64::new(0., 1.),
                dimension: [0.; 7],
            },
            Quantity {
                value: Complex64::new(3., -1.),
                dimension: [0.; 7],
            },
        ],
    );
    assert_eq!(
        q(&arrays::binary("mult", &a, &b).unwrap()).value,
        Complex64::new(7., 1.)
    );
    assert_eq!(
        nums(
            &arrays::binary(
                "mult",
                &m(2, 2, &[1., 2., 3., 4.]),
                &m(2, 2, &[5., 6., 7., 8.])
            )
            .unwrap()
        ),
        vec![19., 22., 43., 50.]
    );
    assert_eq!(
        nums(&arrays::binary("mult", &s(2.), &m(2, 2, &[1., 2., 3., 4.])).unwrap()),
        vec![2., 4., 6., 8.]
    );
    assert_eq!(
        q(&arrays::binary("mult", &m(1, 2, &[1., 2.]), &m(2, 1, &[3., 4.])).unwrap())
            .value
            .re,
        11.
    );
}

#[test]
fn sums_broadcast_scalars_and_preserve_units_per_entry() {
    let meter = unit("m").unwrap();
    let centimeter = unit("cm").unwrap();
    let a = qm(
        2,
        1,
        vec![meter, meter.multiply(Quantity::scalar(2.), false).unwrap()],
    );
    let b = Value::Scalar {
        quantity: centimeter.multiply(Quantity::scalar(20.), false).unwrap(),
    };
    let result = arrays::binary("plus", &a, &b).unwrap();
    close(e(&result)[0].value.re, 1.2);
    close(e(&result)[1].value.re, 2.2);
    assert!(e(&result).iter().all(|q| q.same_dimension(&meter)));
    assert!(arrays::binary("plus", &a, &s(1.)).is_err());
    let dot = arrays::binary("mult", &a, &a).unwrap();
    close(q(&dot).value.re, 5.);
    assert_eq!(q(&dot).dimension[0], 2.);
    assert!(arrays::binary("plus", &m(2, 1, &[1., 2.]), &m(1, 2, &[1., 2.])).is_err());
}

#[test]
fn transpose_does_not_conjugate_complex_values() {
    let mut a = m(2, 2, &[1., 2., 3., 4.]);
    if let Value::Matrix { elements, .. } = &mut a {
        elements[1].value.im = 5.;
    }
    let t = arrays::unary("transpose", &a).unwrap();
    assert_eq!(e(&t)[2].value, Complex64::new(2., 5.));
    let restored = arrays::unary("transpose", &t).unwrap();
    assert_eq!(e(&restored), e(&a));
    assert_eq!(e(&arrays::unary("conjugate", &a).unwrap())[1].value.im, -5.);
}

#[test]
fn determinant_and_inverse_handle_pivoting_and_physical_unit_exponents() {
    let a = m(2, 2, &[0., 2., 1., 3.]);
    close(q(&call("det", std::slice::from_ref(&a))).value.re, -2.);
    let inverse = call("inverse", std::slice::from_ref(&a));
    let product = arrays::binary("mult", &a, &inverse).unwrap();
    for (actual, expected) in nums(&product).iter().zip([1., 0., 0., 1.]) {
        close(*actual, expected);
    }
    let meter = unit("m").unwrap();
    let physical = arrays::binary("scale", &a, &Value::Scalar { quantity: meter }).unwrap();
    let determinant = arrays::unary("absval", &physical).unwrap();
    close(q(&determinant).value.re, -2.);
    assert_eq!(q(&determinant).dimension[0], 2.);
    let inverse = call("inverse", &[physical]);
    assert!(e(&inverse).iter().all(|q| q.dimension[0] == -1.));
    assert!(
        arrays::call(
            "det",
            &[arrays::binary("scale", &a, &Value::Scalar { quantity: meter }).unwrap()],
            Quantity::scalar(0.)
        )
        .is_err()
    );
}

#[test]
fn singular_and_mixed_dimensional_factorization_do_not_return_guessed_values() {
    let singular = m(2, 2, &[1., 2., 2., 4.]);
    assert_eq!(
        q(&call("det", std::slice::from_ref(&singular))).value.re,
        0.
    );
    assert!(
        arrays::call("inverse", &[singular], Quantity::scalar(0.))
            .unwrap_err()
            .unsupported
    );
    let a = qm(
        2,
        2,
        vec![
            unit("m").unwrap(),
            unit("s").unwrap(),
            unit("s").unwrap(),
            unit("m").unwrap(),
        ],
    );
    assert!(arrays::unary("absval", &a).unwrap_err().unsupported);
    assert!(arrays::binary("mult", &m(2, 2, &[1., 2., 3., 4.]), &m(3, 1, &[1., 2., 3.])).is_err());
}

#[test]
fn integer_matrix_powers_and_vector_powers_have_distinct_semantics() {
    let a = m(2, 2, &[1., 2., 3., 4.]);
    assert_eq!(
        nums(&arrays::binary("pow", &a, &s(2.)).unwrap()),
        vec![7., 10., 15., 22.]
    );
    assert_eq!(
        nums(&arrays::binary("pow", &a, &s(0.)).unwrap()),
        vec![1., 0., 0., 1.]
    );
    let inverse = arrays::binary("pow", &a, &s(-1.)).unwrap();
    let product = arrays::binary("mult", &a, &inverse).unwrap();
    for (actual, expected) in nums(&product).iter().zip([1., 0., 0., 1.]) {
        close(*actual, expected);
    }
    assert_eq!(
        nums(&arrays::binary("pow", &m(2, 1, &[2., 3.]), &s(2.)).unwrap()),
        vec![4., 9.]
    );
    assert!(arrays::binary("pow", &a, &s(0.5)).is_err());
}

#[test]
fn division_has_elementwise_vector_and_inverse_matrix_paths() {
    assert_eq!(
        nums(&arrays::binary("div", &s(2.), &m(2, 2, &[2., 0., 0., 4.])).unwrap()),
        vec![1., 0., 0., 0.5]
    );
    assert_eq!(
        nums(&arrays::binary("div", &m(2, 1, &[6., 8.]), &m(2, 1, &[2., 4.])).unwrap()),
        vec![3., 2.]
    );
    assert_eq!(
        nums(&arrays::binary("div", &s(12.), &m(2, 1, &[3., 4.])).unwrap()),
        vec![4., 3.]
    );
    let a = m(2, 2, &[1., 2., 3., 4.]);
    let quotient = arrays::binary("div", &a, &a).unwrap();
    for (actual, expected) in nums(&quotient).iter().zip([1., 0., 0., 1.]) {
        close(*actual, expected);
    }
    assert!(arrays::binary("div", &m(2, 1, &[1., 2.]), &s(0.)).is_err());
}

#[test]
fn joining_preserves_layout_and_heterogeneous_physical_quantities() {
    let a = m(2, 1, &[1., 2.]);
    let b = qm(2, 1, vec![unit("m").unwrap(), unit("s").unwrap()]);
    let joined = call("augment", &[a, b]);
    assert_eq!(nums(&joined), vec![1., 1., 2., 1.]);
    assert_eq!(e(&joined)[1].dimension, unit("m").unwrap().dimension);
    assert_eq!(e(&joined)[3].dimension, unit("s").unwrap().dimension);
    assert_eq!(
        nums(&call("stack", &[m(1, 2, &[1., 2.]), m(1, 2, &[3., 4.])])),
        vec![1., 2., 3., 4.]
    );
    assert_eq!(nums(&call("augment", &[s(1.), s(2.)])), vec![1., 2.]);
    assert!(
        arrays::call(
            "augment",
            &[m(2, 1, &[1., 2.]), s(3.)],
            Quantity::scalar(0.)
        )
        .is_err()
    );
}

#[test]
fn submatrix_is_inclusive_supports_reversal_and_uses_system_origin() {
    let a = m(3, 3, &[1., 2., 3., 4., 5., 6., 7., 8., 9.]);
    let args = [a.clone(), s(2.), s(1.), s(2.), s(0.)];
    assert_eq!(
        nums(&arrays::call("submatrix", &args, Quantity::scalar(0.)).unwrap()),
        vec![9., 8., 7., 6., 5., 4.]
    );
    assert_eq!(
        nums(
            &arrays::call(
                "submatrix",
                &[a, s(3.), s(2.), s(3.), s(1.)],
                Quantity::scalar(1.)
            )
            .unwrap()
        ),
        vec![9., 8., 7., 6., 5., 4.]
    );
    assert!(
        arrays::call(
            "submatrix",
            &[m(1, 1, &[1.]), s(0.), s(0.), s(0.), s(0.)],
            Quantity::scalar(0.5)
        )
        .is_err()
    );
}

#[test]
fn size_functions_distinguish_length_from_origin_dependent_last() {
    let a = m(3, 1, &[1., 2., 3.]);
    assert_eq!(q(&call("length", std::slice::from_ref(&a))).value.re, 3.);
    assert_eq!(
        q(&arrays::call("last", std::slice::from_ref(&a), Quantity::scalar(5.)).unwrap())
            .value
            .re,
        7.
    );
    assert_eq!(
        q(&arrays::call("rows", &[m(2, 3, &[1.; 6])], Quantity::scalar(0.5)).unwrap())
            .value
            .re,
        2.
    );
    assert_eq!(q(&call("cols", &[m(2, 3, &[1.; 6])])).value.re, 3.);
    assert!(arrays::call("length", &[m(1, 3, &[1., 2., 3.])], Quantity::scalar(0.)).is_err());
}

#[test]
fn norms_use_magnitudes_and_spectral_norm_handles_complex_hermitian_gram() {
    for scale in [1e-200, 1e200] {
        let result = call("norm2", &[m(2, 2, &[scale, 0., 0., 2. * scale])]);
        close(q(&result).value.re / scale, 2.);
    }
    let a = m(2, 2, &[0., 3., 4., 0.]);
    for (name, expected) in [("norm1", 4.), ("normi", 4.), ("norme", 5.), ("norm2", 4.)] {
        close(q(&call(name, std::slice::from_ref(&a))).value.re, expected);
    }
    close(q(&call("norm", &[m(2, 1, &[3., 4.])])).value.re, 5.);
    let complex = qm(
        2,
        2,
        vec![
            Quantity::scalar(1.),
            Quantity {
                value: Complex64::new(0., 1.),
                dimension: [0.; 7],
            },
            Quantity::scalar(0.),
            Quantity::scalar(1.),
        ],
    );
    close(
        q(&call("norm2", &[complex])).value.re,
        (1. + 5_f64.sqrt()) / 2.,
    );
    assert_eq!(q(&arrays::unary("absval", &a).unwrap()).value.re, -12.);
    assert!(arrays::call("norm", &[a], Quantity::scalar(0.)).is_err());
    assert!(arrays::call("norme", &[m(2, 3, &[1.; 6])], Quantity::scalar(0.)).is_err());
}

#[test]
fn invalid_values_ranges_and_resource_exhaustion_are_explicit_errors() {
    let range = Value::Range {
        start: Quantity::scalar(0.),
        step: Quantity::scalar(1.),
        end: Quantity::scalar(3.),
        count: 4,
    };
    assert!(arrays::call("rows", &[range], Quantity::scalar(0.)).is_err());
    assert!(arrays::call("identity", &[s(129.)], Quantity::scalar(0.)).is_err());
    assert!(arrays::call("identity", &[s(-1.)], Quantity::scalar(0.)).is_err());
    assert!(arrays::unary("transpose", &m(1, 2, &[1.])).is_err());
    assert!(arrays::binary("plus", &m(1, 1, &[f64::INFINITY]), &s(1.)).is_err());
    assert!(!arrays::is_builtin("inverse")); // inverse is an operator helper, not a fabricated builtin.
}
