use mathcraft_compat::{Value, evaluate_worksheet};

#[test]
fn engineering_document_composes_arrays_programs_calculus_and_statistics() {
    let rows = evaluate_worksheet(include_str!("../../../examples/engineering.xml")).unwrap();
    assert_eq!(rows.len(), 20);
    assert!(rows.iter().all(|row| row.error.is_none()), "{rows:?}");
    for (index, expected) in [
        (1, -2.),
        (4, 5.),
        (6, 55.),
        (7, 2_f64.sqrt()),
        (8, 9.),
        (10, 12.),
        (11, 3.),
        (12, std::f64::consts::FRAC_PI_2),
        (13, 3.5),
        (14, 0.5),
        (15, 0.25),
    ] {
        let Value::Scalar { quantity } = rows[index].value.as_ref().unwrap() else {
            panic!()
        };
        assert!(
            (quantity.value.re - expected).abs() < 1e-5,
            "Row {index}: {quantity:?}"
        );
    }
    let Value::Matrix { elements, .. } = rows[2].value.as_ref().unwrap() else {
        panic!()
    };
    for (actual, expected) in elements.iter().zip([1., 0., 0., 1.]) {
        assert!((actual.value.re - expected).abs() < 1e-12);
    }
    let Value::Matrix { elements, .. } = rows[16].value.as_ref().unwrap() else {
        panic!()
    };
    let mut roots: Vec<_> = elements
        .iter()
        .map(|q| {
            assert!(q.value.im.abs() < 1e-6);
            q.value.re
        })
        .collect();
    roots.sort_by(f64::total_cmp);
    for (actual, expected) in roots.iter().zip([1., 2., 3.]) {
        assert!((actual - expected).abs() < 1e-5);
    }
    let Value::Symbolic { mathml } = rows[17].value.as_ref().unwrap() else {
        panic!()
    };
    assert!(mathml.contains("<div/><real>1</real><real>2</real>"));
    for row in &rows[18..20] {
        let Value::Matrix { elements, .. } = row.value.as_ref().unwrap() else {
            panic!()
        };
        assert!((elements[0].value.re - 3.).abs() < 1e-3);
        assert!((elements[1].value.re - 2.).abs() < 1e-3);
    }
}
