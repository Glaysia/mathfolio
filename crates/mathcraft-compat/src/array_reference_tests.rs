use super::*;
use std::time::Instant;

fn name(text: &str) -> Symbol {
    Symbol {
        name: text.into(),
        label: "VARIABLE".into(),
    }
}
fn number(x: f64) -> Expression {
    Expression::Scalar(Quantity::scalar(x))
}
fn indexed_scope(count: usize, indexed: bool) -> Scope {
    let value = Value::Matrix {
        rows: count,
        cols: 1,
        elements: (0..count).map(|n| Quantity::scalar(n as f64)).collect(),
    };
    let mut scope = Scope::default();
    scope.bindings.insert(
        name("A"),
        if indexed {
            Binding::IndexedArray {
                value,
                zero_dimension: Some([0.; 7]),
            }
        } else {
            Binding::Value(value)
        },
    );
    scope
}
fn index(base: &str, coordinate: Expression) -> Expression {
    Expression::Index(Box::new(Expression::Name(name(base))), vec![coordinate])
}

fn read(scope: &Scope, row: f64) -> Quantity {
    scope
        .evaluate(&index("A", number(row)), 0)
        .unwrap()
        .as_quantity()
        .unwrap()
}

#[test]
fn immutable_scope_and_aliases_share_a_version_but_later_writes_are_isolated() {
    let mut scope = indexed_scope(10000, true);
    scope
        .define(&name("alias"), &None, &Expression::Name(name("A")), "alias")
        .unwrap();
    let before = scope.bindings.shared(&name("A")).unwrap();
    assert!(Arc::ptr_eq(
        &before,
        &scope.bindings.shared(&name("alias")).unwrap()
    ));
    let snapshot = scope.clone();
    assert!(Arc::ptr_eq(
        &before,
        &snapshot.bindings.shared(&name("A")).unwrap()
    ));
    scope
        .define_indexed(&name("A"), &[number(9999.)], &number(42.), "write")
        .unwrap();
    assert_eq!(read(&scope, 9999.).value.re, 42.);
    assert_eq!(read(&snapshot, 9999.).value.re, 9999.);
    assert_eq!(
        scope
            .evaluate(&index("alias", number(9999.)), 0)
            .unwrap()
            .as_quantity()
            .unwrap()
            .value
            .re,
        9999.
    );
    let mut returned = snapshot.evaluate(&Expression::Name(name("A")), 0).unwrap();
    let Value::Matrix { elements, .. } = &mut returned else {
        panic!()
    };
    elements[0] = Quantity::scalar(99.);
    assert_eq!(
        read(&snapshot, 0.).value.re,
        0.,
        "public owned results cannot mutate a shared scope"
    );
    let mut alias_scope = snapshot;
    alias_scope
        .define_indexed(&name("alias"), &[number(10001.)], &number(7.), "grow-alias")
        .unwrap();
    assert_eq!(read(&alias_scope, 9999.).value.re, 9999.);
    let Value::Matrix { rows, .. } = alias_scope
        .evaluate(&Expression::Name(name("alias")), 0)
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(rows, 10002);
}

#[test]
fn borrowed_reads_keep_mixed_units_zero_fill_and_captured_definition_scope() {
    let mut scope = Scope::default();
    scope
        .define_indexed(
            &name("A"),
            &[number(2.)],
            &Expression::Scalar(unit("m").unwrap()),
            "m",
        )
        .unwrap();
    scope
        .define_indexed(
            &name("A"),
            &[number(0.)],
            &Expression::Scalar(unit("s").unwrap()),
            "s",
        )
        .unwrap();
    scope
        .define(&name("alias"), &None, &Expression::Name(name("A")), "alias")
        .unwrap();
    scope
        .define(
            &name("f"),
            &Some(vec![name("x")]),
            &index("A", Expression::Name(name("x"))),
            "function",
        )
        .unwrap();
    scope
        .define_indexed(&name("A"), &[number(4.)], &number(8.), "new")
        .unwrap();
    assert_eq!(read(&scope, 3.).dimension, unit("m").unwrap().dimension);
    let old = scope
        .evaluate(&Expression::Call(name("f"), vec![number(0.)]), 0)
        .unwrap()
        .as_quantity()
        .unwrap();
    assert_eq!(old.dimension, unit("s").unwrap().dimension);
    let old = scope
        .evaluate(&Expression::Call(name("f"), vec![number(2.)]), 0)
        .unwrap()
        .as_quantity()
        .unwrap();
    assert_eq!(old.dimension, unit("m").unwrap().dimension);
    let json = scope
        .evaluate(&Expression::Name(name("alias")), 0)
        .unwrap()
        .numeric_json();
    assert_eq!(json["rows"], 3);
    assert_eq!(json["cols"], 1);
    assert_eq!(json["elements"][2]["real"], 1.);
}

#[test]
fn borrowed_index_reads_keep_origin_error_order_and_failed_dependency_identity() {
    let mut scope = indexed_scope(3, true);
    scope.bindings.insert(
        Symbol {
            name: "ORIGIN".into(),
            label: "SYSTEM".into(),
        },
        Binding::Value(Value::scalar(Quantity::scalar(-2.))),
    );
    assert_eq!(read(&scope, 0.).value.re, 2.);
    assert!(scope.evaluate(&index("A", number(-3.)), 0).is_err());
    scope.bindings.insert(
        name("A"),
        Binding::Failed {
            fault: Fault::new("custom_failure", "upstream array failed"),
            region: "upstream".into(),
        },
    );
    let failure = scope.evaluate(&index("A", number(-3.)), 0).unwrap_err();
    assert_eq!(failure.code, "custom_failure");
    assert_eq!(failure.previous_region.as_deref(), Some("upstream"));
    assert!(
        scope
            .validate_names(&index("A", number(0.)), &[], 0)
            .unwrap_err()
            .previous_region
            .is_some()
    );
    let undefined = scope
        .evaluate(&index("missing", number(-3.)), 0)
        .unwrap_err();
    assert_eq!(undefined.code, "bad%_variable");
}

#[test]
fn computed_index_base_and_row_vector_restrictions_are_unchanged() {
    let scope = indexed_scope(3, false);
    let computed = Expression::Index(
        Box::new(Expression::Binary(
            "plus".into(),
            Box::new(Expression::Name(name("A"))),
            Box::new(number(5.)),
        )),
        vec![number(2.)],
    );
    assert_eq!(
        scope
            .evaluate(&computed, 0)
            .unwrap()
            .as_quantity()
            .unwrap()
            .value
            .re,
        7.
    );
    assert_eq!(read(&scope, 2.).value.re, 2.);
    let mut row = Scope::default();
    row.bindings.insert(
        name("A"),
        Binding::Value(Value::Matrix {
            rows: 1,
            cols: 3,
            elements: vec![Quantity::scalar(1.); 3],
        }),
    );
    assert!(row.evaluate(&index("A", number(1.)), 0).is_err());
    assert_eq!(
        row.evaluate(
            &Expression::Index(
                Box::new(Expression::Name(name("A"))),
                vec![number(0.), number(1.)]
            ),
            0
        )
        .unwrap()
        .as_quantity()
        .unwrap()
        .value
        .re,
        1.
    );
}

#[test]
fn cancellation_keeps_alias_and_shared_checkpoint_binding_versions() {
    let mut scope = indexed_scope(10000, true);
    let before = scope.bindings.shared(&name("A")).unwrap();
    let saved = save_bindings(&scope, &[name("A")]);
    let stop = CalculationControl::default();
    stop.cancel();
    assert!(
        stop.run(|| scope.evaluate(&index("A", number(5.)), 0))
            .unwrap_err()
            .is_cancelled()
    );
    assert!(
        stop.run(|| scope.define(
            &name("alias"),
            &None,
            &Expression::Name(name("A")),
            "cancelled"
        ))
        .unwrap_err()
        .is_cancelled()
    );
    assert!(!scope.bindings.contains_key(&name("alias")));
    assert!(Arc::ptr_eq(
        &before,
        &scope.bindings.shared(&name("A")).unwrap()
    ));
    scope
        .define_indexed(&name("A"), &[number(9999.)], &number(42.), "new")
        .unwrap();
    restore_bindings(&mut scope, saved);
    assert!(Arc::ptr_eq(
        &before,
        &scope.bindings.shared(&name("A")).unwrap()
    ));
    assert_eq!(read(&scope, 9999.).value.re, 9999.);
    scope
        .define(
            &name("alias"),
            &None,
            &Expression::Name(name("A")),
            "resume",
        )
        .unwrap();
    assert!(Arc::ptr_eq(
        &before,
        &scope.bindings.shared(&name("alias")).unwrap()
    ));
}

#[test]
#[ignore = "CPU-only scaling profile for repeated array references"]
fn benchmark_large_array_reference_scaling() {
    for count in [1000, 8000, 16384] {
        for indexed in [false, true] {
            let scope = indexed_scope(count, indexed);
            let expression = index("A", number((count - 1) as f64));
            let started = Instant::now();
            let mut sum = 0.;
            for _ in 0..count {
                sum += scope
                    .evaluate(&expression, 0)
                    .unwrap()
                    .as_quantity()
                    .unwrap()
                    .value
                    .re;
            }
            let elapsed = started.elapsed().as_micros();
            assert_eq!(sum, ((count - 1) * count) as f64);
            println!(
                "ARRAY_READ elements={count} indexed={indexed} reads={count} elapsed_us={elapsed}"
            );
        }
        let mut scope = indexed_scope(count, true);
        scope
            .define(
                &name("f"),
                &Some(vec![name("x")]),
                &index("A", Expression::Name(name("x"))),
                "f",
            )
            .unwrap();
        let call = Expression::Call(name("f"), vec![number((count - 1) as f64)]);
        let started = Instant::now();
        let mut sum = 0.;
        for _ in 0..count {
            sum += scope
                .evaluate(&call, 0)
                .unwrap()
                .as_quantity()
                .unwrap()
                .value
                .re;
        }
        assert_eq!(sum, ((count - 1) * count) as f64);
        println!(
            "ARRAY_FUNCTION elements={count} calls={count} elapsed_us={}",
            started.elapsed().as_micros()
        );
        scope.bindings.insert(
            name("i"),
            Binding::Value(Value::Range {
                start: Quantity::scalar(0.),
                step: Quantity::scalar(1.),
                end: Quantity::scalar((count - 1) as f64),
                count,
            }),
        );
        let started = Instant::now();
        let result = scope
            .evaluate_output(&index("A", Expression::Name(name("i"))))
            .unwrap();
        let Value::RangeValues { elements } = result else {
            panic!()
        };
        assert_eq!(elements.len(), count);
        assert_eq!(elements[count - 1].value.re, (count - 1) as f64);
        println!(
            "ARRAY_RANGE elements={count} samples={count} elapsed_us={}",
            started.elapsed().as_micros()
        );
        let context = CalculationContext {
            scope,
            labels: HashMap::from([
                ("A".into(), "VARIABLE".into()),
                ("f".into(), "VARIABLE".into()),
            ]),
        };
        let query = format!(
            "<ml:eval><ml:apply><ml:id>f</ml:id><ml:real>{}</ml:real></ml:apply><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
            count - 1
        );
        let started = Instant::now();
        for _ in 0..50 {
            assert_eq!(
                context
                    .evaluate_mathml(&query)
                    .unwrap()
                    .as_quantity()
                    .unwrap()
                    .value
                    .re,
                (count - 1) as f64
            );
        }
        println!(
            "ARRAY_CONTEXT elements={count} samples=50 elapsed_us={}",
            started.elapsed().as_micros()
        );
    }
}
