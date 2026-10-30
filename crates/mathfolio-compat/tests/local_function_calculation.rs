use mathfolio_compat::{RegionEvaluation, Value, editor_bridge::worksheet_xml};

fn id(name: &str) -> String {
    format!("<ml:id>{name}</ml:id>")
}
fn real(n: f64) -> String {
    format!("<ml:real>{n}</ml:real>")
}
fn apply(op: &str, args: &[String]) -> String {
    format!("<ml:apply><ml:{op}/>{}</ml:apply>", args.join(""))
}
fn call(name: &str, args: &[String]) -> String {
    format!(
        "<ml:apply>{}<ml:sequence>{}</ml:sequence></ml:apply>",
        id(name),
        args.join("")
    )
}
fn declaration(name: &str, args: &[&str]) -> String {
    format!(
        "<ml:function>{}<ml:boundVars>{}</ml:boundVars></ml:function>",
        id(name),
        args.iter().map(|s| id(s)).collect::<String>()
    )
}
fn define(lhs: &str, rhs: &str) -> String {
    format!("<ml:define>{lhs}{rhs}</ml:define>")
}
fn local(lhs: &str, rhs: &str) -> String {
    format!("<ml:localDefine>{lhs}{rhs}</ml:localDefine>")
}
fn function(name: &str, args: &[&str], rhs: &str) -> String {
    local(&declaration(name, args), rhs)
}
fn program(lines: &[String]) -> String {
    format!("<ml:program>{}</ml:program>", lines.join(""))
}
fn query(rhs: &str) -> String {
    format!("<ml:eval>{rhs}<ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>")
}
fn run(lines: &[String]) -> Vec<RegionEvaluation> {
    let xml = worksheet_xml(
        &lines
            .iter()
            .enumerate()
            .map(|(i, s)| (i as u64 + 1, 0., i as f64 * 40., s.clone()))
            .collect::<Vec<_>>(),
    )
    .unwrap();
    mathfolio_compat::evaluate_worksheet(&xml).unwrap()
}
fn scalar(row: &RegionEvaluation) -> f64 {
    assert!(row.error.is_none(), "{row:?}");
    let Some(Value::Scalar { quantity }) = &row.value else {
        panic!("{row:?}")
    };
    quantity.value.re
}
fn output(body: &str) -> RegionEvaluation {
    run(&[define(&id("output"), body), query(&id("output"))])
        .pop()
        .unwrap()
}

#[test]
fn local_function_captures_outer_arguments_and_preceding_locals_without_leaking() {
    let body = program(&[
        local(&id("offset"), &apply("plus", &[id("a"), real(1.)])),
        function(
            "f",
            &["x"],
            &apply("plus", &[apply("pow", &[id("x"), real(2.)]), id("offset")]),
        ),
        call("f", &[real(2.)]),
    ]);
    let rows = run(&[
        define(&id("x"), &real(90.)),
        define(&declaration("outer", &["a"]), &body),
        query(&call("outer", &[real(2.)])),
        query(&id("x")),
        query(&id("f")),
    ]);
    assert_eq!(scalar(&rows[2]), 7.);
    assert_eq!(scalar(&rows[3]), 90.);
    assert!(rows[4].error.is_some());
}

#[test]
fn parameter_shadowing_and_calls_through_another_local_function_do_not_change_capture_ledger() {
    let body = program(&[
        local(&id("a"), &real(1.)),
        function("f", &["x"], &apply("plus", &[id("a"), id("x")])),
        function("h", &["a"], &call("f", &[id("a")])),
        call("h", &[real(9.)]),
    ]);
    assert_eq!(scalar(&output(&body)), 10.);
}

#[test]
fn changing_a_free_capture_is_an_explicit_boundary_but_unrelated_or_equal_writes_are_allowed() {
    let make = |assignment: String, parameter: &str, expression: String| {
        program(&[
            local(&id("a"), &real(1.)),
            function("f", &[parameter], &expression),
            assignment,
            call("f", &[real(3.)]),
        ])
    };
    let expression = apply("plus", &[id("a"), id("x")]);
    let rejected = output(&make(local(&id("a"), &real(2.)), "x", expression.clone()));
    assert!(
        rejected
            .error
            .as_ref()
            .unwrap()
            .detail
            .contains("captured bindings")
    );
    assert_eq!(
        scalar(&output(&make(
            local(&id("b"), &real(9.)),
            "x",
            expression.clone()
        ))),
        4.
    );
    assert_eq!(
        scalar(&output(&make(local(&id("a"), &real(1.)), "x", expression))),
        4.
    );
    assert_eq!(
        scalar(&output(&make(
            local(&id("a"), &real(2.)),
            "a",
            apply("plus", &[id("a"), real(1.)])
        ))),
        4.
    );
    for negative_zero in [real(-0.), "<ml:imag>-0</ml:imag>".into()] {
        let signed = program(&[
            local(&id("a"), &real(0.)),
            function("f", &["x"], &id("a")),
            local(&id("a"), &negative_zero),
            call("f", &[real(0.)]),
        ]);
        assert!(
            output(&signed)
                .error
                .as_ref()
                .unwrap()
                .detail
                .contains("captured bindings")
        );
    }
    // The capture ledger's conservative identity check is not numeric equality.
    let rows = run(&[query(&apply("equal", &[real(0.), real(-0.)]))]);
    assert_eq!(scalar(&rows[0]), 1.);
}

#[test]
fn an_unexecuted_write_and_a_nested_function_body_local_do_not_invalidate_the_capture() {
    let conditional = format!(
        "<ml:if><ml:test>{}</ml:test><ml:then>{}</ml:then></ml:if>",
        real(0.),
        program(&[local(&id("a"), &real(2.))])
    );
    let body = program(&[
        local(&id("a"), &real(1.)),
        function("f", &["x"], &apply("plus", &[id("a"), id("x")])),
        conditional,
        call("f", &[real(3.)]),
    ]);
    assert_eq!(scalar(&output(&body)), 4.);
    let inner = program(&[
        local(&id("a"), &id("x")),
        apply("plus", &[id("a"), real(1.)]),
    ]);
    let body = program(&[
        local(&id("a"), &real(1.)),
        function("f", &["x"], &inner),
        local(&id("a"), &real(2.)),
        call("f", &[real(3.)]),
    ]);
    assert_eq!(scalar(&output(&body)), 4.);
}

#[test]
fn a_changed_capture_is_still_detected_when_the_function_is_called_through_a_snapshot() {
    let body = program(&[
        local(&id("a"), &real(1.)),
        function("f", &["x"], &apply("plus", &[id("a"), id("x")])),
        function("h", &["x"], &call("f", &[id("x")])),
        local(&id("a"), &real(2.)),
        call("h", &[real(3.)]),
    ]);
    assert!(
        output(&body)
            .error
            .as_ref()
            .unwrap()
            .detail
            .contains("captured bindings")
    );
}

#[test]
fn local_function_redefinition_can_refer_to_the_previous_function_binding() {
    let body = program(&[
        function(
            "f",
            &["x"],
            &apply("mult", &[call("f", &[id("x")]), real(2.)]),
        ),
        call("f", &[real(3.)]),
    ]);
    let rows = run(&[
        define(
            &declaration("f", &["x"]),
            &apply("plus", &[id("x"), real(1.)]),
        ),
        query(&body),
        query(&call("f", &[real(3.)])),
    ]);
    assert_eq!(scalar(&rows[1]), 8.);
    assert_eq!(scalar(&rows[2]), 4.);
}

#[test]
fn parameters_keep_units_complex_values_and_arrays_through_existing_function_calls() {
    let complex = apply("plus", &[real(2.), "<ml:imag>3</ml:imag>".into()]);
    let body = program(&[
        function("f", &["x", "y"], &apply("mult", &[id("x"), id("y")])),
        call("f", &[complex, real(2.)]),
    ]);
    let row = output(&body);
    let Some(Value::Scalar { quantity }) = row.value else {
        panic!("{row:?}")
    };
    assert_eq!((quantity.value.re, quantity.value.im), (4., 6.));
    let unit = apply(
        "scale",
        &[real(3.), "<ml:id labels=\"UNIT\">m</ml:id>".into()],
    );
    let body = program(&[
        function("f", &["m"], &apply("mult", &[id("m"), real(2.)])),
        call("f", &[unit]),
    ]);
    let row = output(&body);
    let Some(Value::Scalar { quantity }) = row.value else {
        panic!("{row:?}")
    };
    assert_eq!(quantity.value.re, 6.);
    assert_eq!(
        quantity.dimension,
        mathfolio_core::unit("m").unwrap().dimension
    );
}

#[test]
fn invalid_local_function_declarations_calls_and_function_values_fail_explicitly() {
    for body in [
        program(&[function("f", &["x"], &id("x")), call("f", &[])]),
        program(&[function("f", &["x", "x"], &id("x")), real(1.)]),
        program(&[function("f", &["x"], &id("missing")), real(1.)]),
        program(&[function("f", &["x"], &id("x"))]),
        program(&[function("f", &["x"], &id("x")), id("f")]),
    ] {
        assert!(output(&body).error.is_some(), "{body}");
    }
    let rows = run(&[
        define(&declaration("g", &["x"]), &id("x")),
        query(&program(&[
            local(&id("f"), &id("g")),
            call("f", &[real(1.)]),
        ])),
    ]);
    assert!(rows[1].error.is_some());
}

#[test]
fn functions_defined_inside_a_for_loop_capture_each_current_iteration_without_leaking() {
    let loop_body = program(&[
        function("f", &["x"], &apply("plus", &[id("x"), id("i")])),
        local(
            &id("total"),
            &apply("plus", &[id("total"), call("f", &[id("i")])]),
        ),
    ]);
    let loop_xml = format!(
        "<ml:for>{}<ml:range>{}{}</ml:range>{}</ml:for>",
        id("i"),
        real(0.),
        real(2.),
        loop_body
    );
    let body = program(&[local(&id("total"), &real(0.)), loop_xml, id("total")]);
    assert_eq!(scalar(&output(&body)), 6.);
}

#[test]
fn indexed_and_slice_writes_only_invalidate_the_array_that_the_function_captures() {
    let array =
        "<ml:matrix rows=\"2\" cols=\"1\"><ml:real>1</ml:real><ml:real>2</ml:real></ml:matrix>";
    let index = |name: &str, n: f64| apply("indexer", &[id(name), real(n)]);
    let make = |write: String| {
        program(&[
            local(&id("a"), array),
            local(&id("b"), array),
            function("f", &["i"], &apply("indexer", &[id("a"), id("i")])),
            write,
            call("f", &[real(0.)]),
        ])
    };
    let bad = output(&make(local(&index("a", 0.), &real(9.))));
    assert!(
        bad.error
            .as_ref()
            .unwrap()
            .detail
            .contains("captured bindings")
    );
    assert_eq!(
        scalar(&output(&make(local(&index("a", 0.), &real(1.))))),
        1.
    );
    assert_eq!(
        scalar(&output(&make(local(&index("b", 0.), &real(9.))))),
        1.
    );
    let bad = output(&make(local(
        &apply("matcol", &[id("a"), real(0.)]),
        &real(9.),
    )));
    assert!(
        bad.error
            .as_ref()
            .unwrap()
            .detail
            .contains("captured bindings")
    );
    let unit = |n: f64, name: &str| {
        apply(
            "scale",
            &[real(n), format!("<ml:id labels=\"UNIT\">{name}</ml:id>")],
        )
    };
    let captured = program(&[
        function("f", &["i"], &apply("indexer", &[id("a"), id("i")])),
        local(&id("a"), &id("b")),
        call("f", &[real(0.)]),
    ]);
    let rows = run(&[
        define(&index("a", 0.), &unit(0., "m")),
        define(&index("a", 0.), &unit(1., "s")),
        define(&index("a", 1.), &unit(2., "s")),
        define(&index("b", 0.), &unit(1., "s")),
        define(&index("b", 1.), &unit(2., "s")),
        query(&captured),
    ]);
    assert!(
        rows[5]
            .error
            .as_ref()
            .unwrap()
            .detail
            .contains("captured bindings")
    );
}

#[test]
fn nested_local_function_calls_keep_the_existing_depth_limit() {
    let mut lines = vec![function("f0", &["x"], &id("x"))];
    for i in 1..160 {
        lines.push(function(
            &format!("f{i}"),
            &["x"],
            &call(&format!("f{}", i - 1), &[id("x")]),
        ));
    }
    lines.push(call("f159", &[real(1.)]));
    assert!(
        output(&program(&lines))
            .error
            .as_ref()
            .unwrap()
            .detail
            .contains("depth")
    );
}
