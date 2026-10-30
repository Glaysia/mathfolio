use super::*;
fn eval(body: &str) -> String {
    format!("<ml:eval>{body}<ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>")
}
fn id(name: &str, label: &str) -> String {
    format!("<ml:id labels=\"{label}\">{name}</ml:id>")
}
fn vector() -> &'static str {
    "<ml:matrix rows=\"2\" cols=\"1\"><ml:real>1</ml:real><ml:real>2</ml:real></ml:matrix>"
}
fn define(name: &str, body: &str) -> String {
    format!("<ml:define>{name}{body}</ml:define>")
}
fn context(definitions: &[String]) -> CalculationContext {
    let regions: Vec<_> = definitions
        .iter()
        .enumerate()
        .map(|(i, d)| ((i + 1) as u64, 0., i as f64 * 30., d.clone()))
        .collect();
    let source = editor_bridge::worksheet_xml(&regions).unwrap();
    let mut calculator = WorksheetCalculator::default();
    calculator.update(&source, None).unwrap();
    let result = calculator.calculate(CalculationRequest::Worksheet).unwrap();
    assert!(
        result.rows.iter().all(|r| r.error.is_none()),
        "{:?}",
        result.rows
    );
    calculator.context_before(0., 10000.).unwrap()
}
fn scalar(value: Value) -> Quantity {
    match value {
        Value::Scalar { quantity } => quantity,
        other => panic!("{other:?}"),
    }
}
fn same(a: Result<Value>, b: Result<Value>) {
    match (a, b) {
        (Ok(a), Ok(b)) => assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        ),
        (Err(a), Err(b)) => assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        ),
        (a, b) => panic!("{a:?} != {b:?}"),
    }
}
#[test]
fn prepared_queries_share_old_resolution_and_errors_without_executing_source() {
    let context = context(&[define(&id("x", "VARIABLE"), vector())]);
    for body in [
        id("x", "VARIABLE"),
        "<ml:real>3</ml:real>".into(),
        vector().into(),
        "<ml:range><ml:real>1</ml:real><ml:real>3</ml:real></ml:range>".into(),
        "<ml:apply><ml:sin/><ml:real>1</ml:real></ml:apply>".into(),
        format!(
            "<ml:apply>{}<ml:real>1</ml:real></ml:apply>",
            id("sin", "FUNCTION")
        ),
        id("missing", "VARIABLE"),
    ] {
        let xml = eval(&body);
        match context.prepare_query(&xml) {
            Ok(q) => same(context.evaluate_mathml(&xml), context.evaluate_prepared(&q)),
            Err(fault) => same(context.evaluate_mathml(&xml), Err(fault)),
        }
    }
    for xml in ["<ml:real>1</ml:real>".to_string(),define(&id("new","VARIABLE"),"<ml:real>2</ml:real>"),
        "<ml:eval bad=\"1\"><ml:real>1</ml:real><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>".into(),
        "<foreign xmlns=\"urn:foreign\"/>".into(),"<ml:eval/> <ml:eval/>".into(),"<broken".into()]
    {same(context.evaluate_mathml(&xml),context.prepare_query(&xml).and_then(|q|context.evaluate_prepared(&q)));}
    assert!(
        context
            .evaluate_mathml(&eval(&id("new", "VARIABLE")))
            .is_err()
    );
    assert!(matches!(
        context
            .evaluate_mathml(&eval(&id("x", "VARIABLE")))
            .unwrap(),
        Value::Matrix {
            rows: 2,
            cols: 1,
            ..
        }
    ));
}
#[test]
fn contextual_and_none_identifiers_resolve_before_exact_sample_binding() {
    let context = context(&[define(&id("x", "VARIABLE"), vector())]);
    let root = context.prepare_query(&eval(&id("x", "VARIABLE"))).unwrap();
    let key = root.root_symbol().unwrap();
    assert_eq!(
        key,
        &Symbol {
            name: "x".into(),
            label: "VARIABLE".into()
        }
    );
    for identifier in [
        "<ml:id>x</ml:id>",
        "<ml:id labels=\"UNIT\" label-is-contextual=\"true\">x</ml:id>",
        "<ml:id labels=\"*\">x</ml:id>",
        "<ml:id labels=\"NONE\">x</ml:id>",
    ] {
        let q = context.prepare_query(&eval(identifier)).unwrap();
        assert_eq!(q.root_symbol(), Some(key));
        assert!(q.contains_value_symbol(key).unwrap());
        assert_eq!(
            scalar(
                context
                    .evaluate_prepared_with_sample(&q, key, Quantity::scalar(7.))
                    .unwrap()
            )
            .value
            .re,
            7.
        );
    }
    let unit = context.prepare_query(&eval(&id("x", "UNIT"))).unwrap();
    assert_eq!(unit.root_symbol().unwrap().label, "UNIT");
    assert!(!unit.contains_value_symbol(key).unwrap());
    assert!(
        context
            .evaluate_prepared_with_sample(&unit, key, Quantity::scalar(7.))
            .is_err()
    );
    assert!(
        context
            .evaluate_prepared_with_sample(&root, unit.root_symbol().unwrap(), Quantity::scalar(7.))
            .is_err()
    );
}
#[test]
fn literal_subscript_identity_is_distinct_from_plain_underscore_and_base() {
    let sub = r#"<ml:id labels="VARIABLE"><p:Span xmlns:p="http://schemas.microsoft.com/winfx/2006/xaml/presentation">x<ptc:Subscript xmlns:ptc="clr-namespace:Ptc.Wpf;assembly=Ptc.Core">dc</ptc:Subscript></p:Span></ml:id>"#;
    let context = context(&[
        define(sub, vector()),
        define(&id("x_dc", "VARIABLE"), "<ml:real>1000</ml:real>"),
        define(&id("x", "VARIABLE"), "<ml:real>100</ml:real>"),
    ]);
    let root = context.prepare_query(&eval(sub)).unwrap();
    let key = root.root_symbol().unwrap();
    assert_eq!(key.name, "x\u{1f}dc");
    let y = context
        .prepare_query(&eval(&format!(
            "<ml:apply><ml:plus/>{sub}{}</ml:apply>",
            id("x_dc", "VARIABLE")
        )))
        .unwrap();
    assert_eq!(
        scalar(
            context
                .evaluate_prepared_with_sample(&y, key, Quantity::scalar(9.))
                .unwrap()
        )
        .value
        .re,
        1009.
    );
    for name in ["x", "x_dc"] {
        assert_ne!(
            context
                .prepare_query(&eval(&id(name, "VARIABLE")))
                .unwrap()
                .root_symbol(),
            Some(key)
        );
    }
}
#[test]
fn call_targets_and_captured_function_environment_are_not_sample_bindings() {
    let function = format!(
        "<ml:define><ml:function>{}<ml:boundVars>{}</ml:boundVars></ml:function><ml:apply><ml:plus/>{}<ml:real>1</ml:real></ml:apply></ml:define>",
        id("x", "FUNCTION"),
        id("t", "VARIABLE"),
        id("t", "VARIABLE")
    );
    let captured = format!(
        "<ml:define><ml:function>{}<ml:boundVars>{}</ml:boundVars></ml:function><ml:apply><ml:plus/>{}<ml:apply><ml:indexer/>{}<ml:real>0</ml:real></ml:apply></ml:apply></ml:define>",
        id("f", "FUNCTION"),
        id("t", "VARIABLE"),
        id("t", "VARIABLE"),
        id("x", "VARIABLE")
    );
    let context = context(&[define(&id("x", "VARIABLE"), vector()), function, captured]);
    let root = context.prepare_query(&eval(&id("x", "VARIABLE"))).unwrap();
    let key = root.root_symbol().unwrap();
    for (function, expected) in [("x", 8.), ("f", 8.)] {
        let y = context
            .prepare_query(&eval(&format!(
                "<ml:apply>{}{}</ml:apply>",
                id(function, "FUNCTION"),
                id("x", "VARIABLE")
            )))
            .unwrap();
        assert!(y.contains_value_symbol(key).unwrap());
        assert_eq!(
            scalar(
                context
                    .evaluate_prepared_with_sample(&y, key, Quantity::scalar(7.))
                    .unwrap()
            )
            .value
            .re,
            expected
        );
    }
    let no_value = context
        .prepare_query(&eval(&format!(
            "<ml:apply>{}<ml:real>4</ml:real></ml:apply>",
            id("x", "FUNCTION")
        )))
        .unwrap();
    assert!(!no_value.contains_value_symbol(key).unwrap());
    assert_eq!(
        scalar(
            context
                .evaluate_prepared_with_sample(&no_value, key, Quantity::scalar(7.))
                .unwrap()
        )
        .value
        .re,
        5.
    );
    assert!(matches!(
        context.evaluate_prepared(&root).unwrap(),
        Value::Matrix {
            rows: 2,
            cols: 1,
            ..
        }
    ));
}
#[test]
fn physical_sample_is_a_scalar_ir_value_with_forced_unit_names_untouched() {
    let context = context(&[
        define(&id("x", "VARIABLE"), vector()),
        define(&id("m", "VARIABLE"), "<ml:real>99</ml:real>"),
    ]);
    let root = context.prepare_query(&eval(&id("x", "VARIABLE"))).unwrap();
    let key = root.root_symbol().unwrap();
    let sample = Quantity::scalar(2.)
        .multiply(unit("m").unwrap(), false)
        .unwrap();
    let y=context.prepare_query(&eval(&format!("<ml:apply><ml:plus/>{}<ml:apply><ml:mult/><ml:real>3</ml:real>{}</ml:apply></ml:apply>",id("x","VARIABLE"),id("m","UNIT")))).unwrap();
    let got = scalar(
        context
            .evaluate_prepared_with_sample(&y, key, sample)
            .unwrap(),
    );
    assert_eq!(got.value.re, 5.);
    assert_eq!(got.dimension, sample.dimension);
    for sample in [Quantity::scalar(f64::NAN), Quantity::scalar(f64::INFINITY)] {
        assert!(
            context
                .evaluate_prepared_with_sample(&y, key, sample)
                .is_err()
        );
    }
}
#[test]
fn sample_rejects_binders_logic_and_function_scale_but_direct_query_is_unchanged() {
    let context = context(&[define(&id("x", "VARIABLE"), "<ml:real>2</ml:real>")]);
    let root = context.prepare_query(&eval(&id("x", "VARIABLE"))).unwrap();
    let key = root.root_symbol().unwrap();
    for body in [
        "<ml:program><ml:real>3</ml:real></ml:program>".to_string(),
        format!(
            "<ml:apply><ml:equal/>{}<ml:real>2</ml:real></ml:apply>",
            id("x", "VARIABLE")
        ),
        format!(
            "<ml:apply><ml:scale/>{}{}</ml:apply>",
            id("x", "VARIABLE"),
            id("sin", "FUNCTION")
        ),
    ] {
        let xml = eval(&body);
        let query = context.prepare_query(&xml).unwrap();
        same(
            context.evaluate_mathml(&xml),
            context.evaluate_prepared(&query),
        );
        assert!(query.contains_value_symbol(key).is_err());
        assert!(
            context
                .evaluate_prepared_with_sample(&query, key, Quantity::scalar(4.))
                .is_err()
        );
    }
    let y = context
        .prepare_query(&eval(&format!(
            "<ml:apply><ml:scale/>{}<ml:real>3</ml:real></ml:apply>",
            id("x", "VARIABLE")
        )))
        .unwrap();
    assert_eq!(
        scalar(
            context
                .evaluate_prepared_with_sample(&y, key, Quantity::scalar(4.))
                .unwrap()
        )
        .value
        .re,
        12.
    );
}
#[test]
fn prepared_element_budget_does_not_narrow_legacy_query_budget() {
    let context = context(&[]);
    let matrix = format!(
        "<ml:matrix rows=\"8200\" cols=\"1\">{}</ml:matrix>",
        "<ml:real>1</ml:real>".repeat(8200)
    );
    let xml = eval(&matrix);
    assert!(
        context
            .prepare_query(&xml)
            .unwrap_err()
            .detail
            .contains("8192")
    );
    assert!(matches!(
        context.evaluate_mathml(&xml).unwrap(),
        Value::Matrix {
            rows: 8200,
            cols: 1,
            ..
        }
    ));
}

#[test]
fn sample_rechecks_context_dependent_function_scale_role() {
    let value_context = context(&[
        define(&id("t", "VARIABLE"), "<ml:real>4</ml:real>"),
        define(&id("x", "VARIABLE"), "<ml:real>2</ml:real>"),
    ]);
    let query = value_context
        .prepare_query(&eval(&format!(
            "<ml:apply><ml:scale/>{}{}</ml:apply>",
            id("t", "VARIABLE"),
            id("x", "VARIABLE")
        )))
        .unwrap();
    let key = value_context
        .prepare_query(&eval(&id("t", "VARIABLE")))
        .unwrap()
        .root_symbol()
        .unwrap()
        .clone();
    assert_eq!(
        scalar(
            value_context
                .evaluate_prepared_with_sample(&query, &key, Quantity::scalar(5.))
                .unwrap()
        )
        .value
        .re,
        10.
    );
    let function = format!(
        "<ml:define><ml:function>{}<ml:boundVars>{}</ml:boundVars></ml:function><ml:apply><ml:plus/>{}<ml:real>1</ml:real></ml:apply></ml:define>",
        id("x", "VARIABLE"),
        id("z", "VARIABLE"),
        id("z", "VARIABLE")
    );
    let function_context = context(&[
        define(&id("t", "VARIABLE"), "<ml:real>4</ml:real>"),
        function,
    ]);
    assert_eq!(
        scalar(function_context.evaluate_prepared(&query).unwrap())
            .value
            .re,
        5.
    );
    assert!(
        function_context
            .evaluate_prepared_with_sample(&query, &key, Quantity::scalar(5.))
            .unwrap_err()
            .detail
            .contains("function-role SCALE")
    );
}
