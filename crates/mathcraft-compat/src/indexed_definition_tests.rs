use super::*;

#[test]
fn cancellation_during_range_assignment_preserves_previous_array_and_fill_basis() {
    let symbol = Symbol {
        name: "v".into(),
        label: "VARIABLE".into(),
    };
    let variable = Symbol {
        name: "i".into(),
        label: "VARIABLE".into(),
    };
    let quantity = unit("m").unwrap();
    let mut scope = Scope::default();
    scope
        .define_indexed(
            &symbol,
            &[Expression::Scalar(Quantity::scalar(0.))],
            &Expression::Scalar(quantity),
            "old",
        )
        .unwrap();
    scope
        .define(
            &variable,
            &None,
            &Expression::Range {
                start: Box::new(Expression::Scalar(Quantity::scalar(0.))),
                second: None,
                end: Box::new(Expression::Scalar(Quantity::scalar(300.))),
            },
            "range",
        )
        .unwrap();
    let index = Expression::Binary(
        "plus".into(),
        Box::new(Expression::Name(variable.clone())),
        Box::new(Expression::Scalar(Quantity::scalar(1.))),
    );
    let control = CalculationControl::stop_after_checks(80);
    let interrupted = control.run(|| {
        scope.define_indexed(
            &symbol,
            std::slice::from_ref(&index),
            &Expression::Scalar(Quantity {
                value: quantity.value * 2.,
                ..quantity
            }),
            "new",
        )
    });
    assert!(interrupted.unwrap_err().is_cancelled());
    let Binding::Value(Value::Matrix {
        rows,
        cols,
        elements,
    }) = scope.lookup(&symbol).unwrap()
    else {
        panic!("old array missing")
    };
    assert_eq!((rows, cols), (1, 1));
    assert_eq!(elements[0], quantity);
    scope
        .define_indexed(
            &symbol,
            &[index],
            &Expression::Scalar(Quantity {
                value: quantity.value * 2.,
                ..quantity
            }),
            "new",
        )
        .unwrap();
    let Binding::Value(Value::Matrix {
        rows,
        cols,
        elements,
    }) = scope.lookup(&symbol).unwrap()
    else {
        panic!("resumed array missing")
    };
    assert_eq!((rows, cols), (302, 1));
    assert_eq!(elements[0], quantity);
    assert_eq!(elements[301].dimension, quantity.dimension);
    assert_eq!(elements[301].value.re, 2.);
    assert!(matches!(
        scope.lookup(&variable).unwrap(),
        Binding::Value(Value::Range { count: 301, .. })
    ));
}

#[test]
fn cancellation_during_slice_growth_keeps_the_published_array_and_can_resume() {
    let name = Symbol {
        name: "a".into(),
        label: "VARIABLE".into(),
    };
    let mut scope = Scope::default();
    let unit_value = Expression::Scalar(unit("m").unwrap());
    scope
        .define_array(
            &name,
            &[Expression::Scalar(Quantity::scalar(0.))],
            Some("matcol"),
            &unit_value,
            "old",
        )
        .unwrap();
    let old = scope.bindings.shared(&name).unwrap();
    let body = Expression::Matrix {
        rows: 100,
        cols: 1,
        elements: vec![unit_value; 100],
    };
    let index = [Expression::Scalar(Quantity::scalar(80.))];
    let control = CalculationControl::stop_after_checks(60);
    let fault = control
        .run(|| scope.define_array(&name, &index, Some("matcol"), &body, "new"))
        .unwrap_err();
    assert!(fault.is_cancelled());
    assert!(Arc::ptr_eq(&old, &scope.bindings.shared(&name).unwrap()));
    scope
        .define_array(&name, &index, Some("matcol"), &body, "new")
        .unwrap();
    let Some(Binding::IndexedArray {
        value: Value::Matrix {
            rows,
            cols,
            elements,
        },
        zero_dimension,
    }) = scope.bindings.get(&name)
    else {
        panic!("missing array")
    };
    assert_eq!((*rows, *cols), (100, 81));
    assert_eq!(*zero_dimension, Some(unit("m").unwrap().dimension));
    assert_eq!(elements[1].value.re, 0.);
    assert_eq!(elements[1].dimension, unit("m").unwrap().dimension);
    let Binding::IndexedArray {
        value: Value::Matrix { rows, cols, .. },
        ..
    } = old.as_ref()
    else {
        panic!("old binding changed")
    };
    assert_eq!((*rows, *cols), (1, 1));
}
