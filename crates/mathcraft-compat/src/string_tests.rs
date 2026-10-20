use super::*;
fn literal(value: &str) -> String {
    format!(
        "<ml:str xml:space=\"preserve\">{}</ml:str>",
        value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    )
}
fn id(name: &str) -> String {
    format!("<ml:id labels=\"VARIABLE\">{name}</ml:id>")
}
fn call(name: &str, args: &[String]) -> String {
    format!(
        "<ml:apply><ml:id labels=\"FUNCTION\">{name}</ml:id>{}</ml:apply>",
        args.concat()
    )
}
fn number(n: i32) -> String {
    format!("<ml:real>{n}</ml:real>")
}
fn eval(value: &str) -> String {
    format!("<ml:eval>{value}<ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>")
}
fn worksheet(parts: &[String]) -> String {
    editor_bridge::worksheet_xml(
        &parts
            .iter()
            .enumerate()
            .map(|(i, p)| ((i + 1) as u64, 0., i as f64 * 30., p.clone()))
            .collect::<Vec<_>>(),
    )
    .unwrap()
}
fn context(parts: &[String]) -> CalculationContext {
    let mut calculator = WorksheetCalculator::default();
    calculator.update(&worksheet(parts), None).unwrap();
    let result = calculator.calculate(CalculationRequest::Worksheet).unwrap();
    assert!(
        result.rows.iter().all(|r| r.error.is_none()),
        "{:?}",
        result.rows
    );
    calculator.context_before(0., 10000.).unwrap()
}
fn query(context: &CalculationContext, expression: &str) -> Value {
    context.evaluate_mathml(&eval(expression)).unwrap()
}
fn text(value: Value) -> String {
    match value {
        Value::String { value } => value.to_string(),
        _ => panic!("Expected string"),
    }
}
fn scalar(value: Value) -> f64 {
    value.as_quantity().unwrap().value.re
}
#[test]
fn literal_definition_argument_return_and_stable_json_keep_inert_text() {
    let value = "  <&> \" quote 한글  ";
    let def = format!("<ml:define>{}{}</ml:define>", id("a"), literal(value));
    let function = format!(
        "<ml:define><ml:function><ml:id labels=\"FUNCTION\">echo</ml:id><ml:boundVars>{}</ml:boundVars></ml:function>{}</ml:define>",
        id("s"),
        call("concat", &[id("s"), literal("!")])
    );
    let context = context(&[def, function]);
    assert_eq!(text(query(&context, &id("a"))), value);
    let result = query(&context, &call("echo", &[id("a")]));
    assert_eq!(
        result.numeric_json(),
        serde_json::json!({"kind":"string","value":format!("{value}!")})
    );
    assert_eq!(
        text(serde_json::from_str::<Value>(&serde_json::to_string(&result).unwrap()).unwrap()),
        format!("{value}!")
    );
    assert_eq!(text(query(&context, &literal(""))), "");
}
#[test]
fn program_local_assignment_and_return_transport_strings_without_quantities() {
    let body = format!(
        "<ml:program><ml:localDefine>{}{}</ml:localDefine><ml:return>{}</ml:return><ml:real>999</ml:real></ml:program>",
        id("local"),
        literal("local text"),
        id("local")
    );
    let context = context(&[]);
    assert_eq!(text(query(&context, &body)), "local text");
}
#[test]
fn concat_and_predicates_do_not_coerce_strings_into_numbers() {
    let context = context(&[]);
    assert_eq!(
        text(query(
            &context,
            &call("concat", &[literal("a"), literal(""), literal(" b")])
        )),
        "a b"
    );
    for (name, expected) in [
        ("IsString", 1.),
        ("IsScalar", 0.),
        ("IsArray", 0.),
        ("IsRange", 0.),
    ] {
        assert_eq!(
            scalar(query(&context, &call(name, &[literal("2")]))),
            expected
        );
    }
    assert!(
        context
            .evaluate_mathml(&eval(&call("concat", &[literal("x"), number(2)])))
            .is_err()
    );
    assert!(
        context
            .evaluate_mathml(&eval(&format!(
                "<ml:apply><ml:plus/>{}{}</ml:apply>",
                literal("a"),
                literal("b")
            )))
            .is_err()
    );
}
#[test]
fn utf16_length_and_substring_keep_scalar_boundaries_and_truncate() {
    let context = context(&[]);
    assert_eq!(
        scalar(query(&context, &call("strlen", &[literal("A𝛼한")]))),
        4.
    );
    let sub = |s: &str, start, count| call("substr", &[literal(s), number(start), number(count)]);
    assert_eq!(text(query(&context, &sub("abcd", 1, 99))), "bcd");
    assert_eq!(text(query(&context, &sub("𝛼x", 0, 2))), "𝛼");
    assert_eq!(text(query(&context, &sub("𝛼x", 2, 1))), "x");
    assert_eq!(text(query(&context, &sub("𝛼x", 1, 0))), "");
    assert_eq!(text(query(&context, &sub("abc", 3, 100))), "");
    let fault = context
        .evaluate_mathml(&eval(&sub("𝛼x", 0, 1)))
        .unwrap_err();
    assert!(fault.unsupported && fault.detail.contains("surrogate"));
    for (start, count, arg) in [
        (4, 1, "argument 2"),
        (-1, 1, "argument 2"),
        (0, -1, "argument 3"),
    ] {
        let e = context
            .evaluate_mathml(&eval(&sub("abc", start, count)))
            .unwrap_err();
        assert!(e.detail.contains(arg), "{:?}", e);
    }
}
#[test]
fn search_start_and_missing_positions_follow_the_explicit_origin_setting() {
    let context = context(&[format!(
        "<ml:define><ml:id labels=\"SYSTEM\">ORIGIN</ml:id>{}</ml:define>",
        number(1)
    )]);
    let search = |needle: &str, start| {
        call(
            "search",
            &[literal("ababa"), literal(needle), number(start)],
        )
    };
    assert_eq!(scalar(query(&context, &search("ba", 0))), 1.);
    assert_eq!(scalar(query(&context, &search("z", 0))), -1.);
    let enabled = context.with_strings_use_origin(true);
    assert_eq!(scalar(query(&enabled, &search("ba", 1))), 2.);
    assert_eq!(scalar(query(&enabled, &search("z", 1))), 0.);
    assert_eq!(
        text(query(
            &enabled,
            &call("substr", &[literal("abc"), number(1), number(1)])
        )),
        "a"
    );
    assert!(enabled.evaluate_mathml(&eval(&search("ba", 0))).is_err());
    let unicode = call("search", &[literal("한글"), literal("글"), number(1)]);
    assert!(
        enabled
            .evaluate_mathml(&eval(&unicode))
            .unwrap_err()
            .unsupported
    );
}
#[test]
fn changing_string_origin_invalidates_global_and_captured_bindings() {
    let origin = format!(
        "<ml:globalDefine><ml:id labels=\"SYSTEM\">ORIGIN</ml:id>{}</ml:globalDefine>",
        number(1)
    );
    let expr = call("substr", &[literal("abc"), number(1), number(1)]);
    let function = format!(
        "<ml:define><ml:function><ml:id labels=\"FUNCTION\">pick</ml:id><ml:boundVars>{}</ml:boundVars></ml:function>{}</ml:define>",
        id("s"),
        call("substr", &[id("s"), number(1), number(1)])
    );
    let source = worksheet(&[
        origin,
        function,
        eval(&expr),
        eval(&call("pick", &[literal("abc")])),
    ]);
    let mut calculator = WorksheetCalculator::default();
    calculator.update(&source, None).unwrap();
    assert_eq!(
        text(
            calculator
                .calculate(CalculationRequest::Worksheet)
                .unwrap()
                .rows[2]
                .value
                .clone()
                .unwrap()
        ),
        "b"
    );
    assert!(calculator.set_strings_use_origin(true));
    assert!(!calculator.set_strings_use_origin(true));
    assert_eq!(
        text(
            calculator
                .calculate(CalculationRequest::ThroughRegion("4".into()))
                .unwrap()
                .rows[2]
                .value
                .clone()
                .unwrap()
        ),
        "a"
    );
    assert_eq!(
        text(
            calculator
                .calculate(CalculationRequest::Worksheet)
                .unwrap()
                .rows[3]
                .value
                .clone()
                .unwrap()
        ),
        "a"
    );
}
#[test]
fn conversion_subset_is_inert_finite_dimensionless_and_num2str_is_not_fabricated() {
    let context = context(&[]);
    for (source, expected) in [(" 1.25 ", 1.25), ("-2e3", -2000.), ("1 2", 12.)] {
        let v = query(&context, &call("str2num", &[literal(source)]));
        let q = v.as_quantity().unwrap();
        assert_eq!(q.value.re, expected);
        assert!(q.is_scalar());
    }
    for source in ["1+2", "1 m", "NaN", "inf", "1i", "0x10"] {
        assert!(
            context
                .evaluate_mathml(&eval(&call("str2num", &[literal(source)])))
                .is_err()
        );
    }
    assert!(
        context
            .evaluate_mathml(&eval(&call("num2str", &[number(2)])))
            .unwrap_err()
            .unsupported
    );
}
#[test]
fn malformed_literal_and_mixed_matrix_have_no_fake_numeric_value() {
    let context = context(&[]);
    for value in [
        "<ml:str future='x'>a</ml:str>",
        "<ml:str xml:space='default'>a</ml:str>",
        "<ml:str>a<ml:real>2</ml:real></ml:str>",
        "<ml:str>a<!--split-->b</ml:str>",
    ] {
        assert!(context.evaluate_mathml(&eval(value)).is_err(), "{value}");
    }
    let matrix = format!(
        "<ml:matrix rows='1' cols='2'>{}{}</ml:matrix>",
        literal("a"),
        number(1)
    );
    assert!(context.evaluate_mathml(&eval(&matrix)).is_err());
    let a = Value::String {
        value: "a".repeat(strings::MAX_STRING_BYTES).into(),
    };
    let b = Value::String { value: "b".into() };
    assert!(strings::call("concat", &[a, b], 0).unwrap_err().unsupported);
}

#[test]
fn unavailable_origin_blocks_only_index_sensitive_calls_and_invalidates_captures() {
    let context = context(&[]).with_strings_use_origin_unavailable();
    assert_eq!(scalar(query(&context, &number(9))), 9.);
    assert_eq!(
        scalar(query(&context, &call("strlen", &[literal("abc")]))),
        3.
    );
    assert_eq!(
        text(query(
            &context,
            &call("concat", &[literal("a"), literal("b")])
        )),
        "ab"
    );
    assert_eq!(
        scalar(query(&context, &call("str2num", &[literal("2.5")]))),
        2.5
    );
    for expr in [
        call("substr", &[literal("abc"), number(1), number(1)]),
        call("search", &[literal("abc"), literal("b"), number(0)]),
    ] {
        let fault = context.evaluate_mathml(&eval(&expr)).unwrap_err();
        assert!(fault.unsupported);
        assert!(fault.detail.contains("String origin setting unavailable"));
    }
    let function = format!(
        "<ml:define><ml:function><ml:id labels=\"FUNCTION\">pick</ml:id><ml:boundVars>{}</ml:boundVars></ml:function>{}</ml:define>",
        id("s"),
        call("substr", &[id("s"), number(1), number(1)])
    );
    let source = worksheet(&[function, eval(&call("pick", &[literal("abc")]))]);
    let mut calculator = WorksheetCalculator::default();
    calculator.update(&source, None).unwrap();
    assert!(calculator.set_strings_use_origin_unavailable());
    assert!(!calculator.set_strings_use_origin_unavailable());
    assert!(
        calculator
            .calculate(CalculationRequest::Worksheet)
            .unwrap()
            .rows[1]
            .error
            .as_ref()
            .unwrap()
            .unsupported
    );
    assert!(calculator.set_strings_use_origin(false));
    assert_eq!(
        text(
            calculator
                .calculate(CalculationRequest::Worksheet)
                .unwrap()
                .rows[1]
                .value
                .clone()
                .unwrap()
        ),
        "b"
    );
}
