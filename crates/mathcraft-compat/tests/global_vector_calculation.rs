use mathcraft_compat::{
    CalculationControl, CalculationRequest, RegionEvaluation, Value, WorksheetCalculator,
    evaluate_worksheet,
};
use mathcraft_core::{Quantity, unit};
use num_complex::Complex64;

fn n(x: f64) -> String {
    format!("<ml:real>{x}</ml:real>")
}
fn id(x: &str) -> String {
    format!("<ml:id labels=\"VARIABLE\">{x}</ml:id>")
}
fn app(op: &str, args: &[String]) -> String {
    format!("<ml:apply><ml:{op}/>{}</ml:apply>", args.join(""))
}
fn vec(body: &str) -> String {
    app("vectorize", &[body.into()])
}
fn call(name: &str, args: &[String]) -> String {
    format!(
        "<ml:apply><ml:id labels=\"FUNCTION\">{name}</ml:id><ml:sequence>{}</ml:sequence></ml:apply>",
        args.join("")
    )
}
fn matrix(r: usize, c: usize, v: &[String]) -> String {
    let mut text = String::new();
    for col in 0..c {
        for row in 0..r {
            text.push_str(&v[row * c + col]);
        }
    }
    format!("<ml:matrix rows=\"{r}\" cols=\"{c}\">{text}</ml:matrix>")
}
fn m(r: usize, c: usize, v: &[f64]) -> String {
    matrix(r, c, &v.iter().map(|x| n(*x)).collect::<Vec<_>>())
}
fn q(x: f64, u: &str) -> String {
    app(
        "scale",
        &[n(x), format!("<ml:id labels=\"UNIT\">{u}</ml:id>")],
    )
}
fn definition(global: bool, lhs: &str, rhs: &str) -> String {
    let op = if global { "globalDefine" } else { "define" };
    format!("<ml:{op}>{lhs}{rhs}</ml:{op}>")
}
fn def(name: &str, rhs: &str) -> String {
    definition(false, &id(name), rhs)
}
fn global(name: &str, rhs: &str) -> String {
    definition(true, &id(name), rhs)
}
fn function(global: bool, name: &str, body: &str) -> String {
    definition(
        global,
        &format!(
            "<ml:function><ml:id labels=\"FUNCTION\">{name}</ml:id><ml:boundVars>{}</ml:boundVars></ml:function>",
            id("x")
        ),
        body,
    )
}
fn query(e: &str) -> String {
    format!("<ml:eval>{e}<ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>")
}
fn sheet(parts: &[String]) -> String {
    let rs = parts
        .iter()
        .enumerate()
        .map(|(i, e)| {
            format!(
                "<region region-id=\"{i}\" top=\"{}\" left=\"20\"><math>{e}</math></region>",
                i * 65
            )
        })
        .collect::<String>();
    format!(
        "<worksheet xmlns=\"http://schemas.mathsoft.com/worksheet50\" xmlns:ml=\"http://schemas.mathsoft.com/math50\"><regions>{rs}</regions></worksheet>"
    )
}
fn run(parts: &[String]) -> Vec<RegionEvaluation> {
    evaluate_worksheet(&sheet(parts)).unwrap()
}
fn scalar(row: &RegionEvaluation) -> Quantity {
    assert!(row.error.is_none(), "{row:?}");
    let Some(Value::Scalar { quantity }) = row.value else {
        panic!("{row:?}")
    };
    quantity
}
fn cells(row: &RegionEvaluation, shape: (usize, usize)) -> &[Quantity] {
    assert!(row.error.is_none(), "{row:?}");
    let Some(Value::Matrix {
        rows,
        cols,
        elements,
    }) = &row.value
    else {
        panic!("{row:?}")
    };
    assert_eq!((*rows, *cols), shape);
    elements
}
fn nums(row: &RegionEvaluation, shape: (usize, usize)) -> Vec<f64> {
    cells(row, shape).iter().map(|q| q.value.re).collect()
}

#[test]
fn vectorize_changes_every_operator_but_does_not_recalculate_a_stored_intermediate() {
    let a = m(2, 2, &[1., 2., 3., 4.]);
    let b = m(2, 2, &[5., 6., 7., 8.]);
    let product = app("mult", &[id("A"), id("B")]);
    let nested = app("mult", &[app("pow", &[id("A"), n(2.)]), id("A")]);
    let rows = run(&[
        def("A", &a),
        def("B", &b),
        def("P", &product),
        query(&product),
        query(&vec(&product)),
        query(&vec(&nested)),
        query(&vec(&app("mult", &[id("P"), id("A")]))),
    ]);
    assert_eq!(nums(&rows[3], (2, 2)), [19., 22., 43., 50.]);
    assert_eq!(nums(&rows[4], (2, 2)), [5., 12., 21., 32.]);
    assert_eq!(nums(&rows[5], (2, 2)), [1., 8., 27., 64.]);
    assert_eq!(nums(&rows[6], (2, 2)), [19., 44., 129., 200.]);
}

#[test]
fn vectorized_scalar_broadcast_complex_units_and_exact_shape_are_preserved() {
    let a = m(2, 2, &[1., 4., 9., 16.]);
    let roots = app("pow", &[a.clone(), n(0.5)]);
    let physical = matrix(2, 1, &[q(2., "m"), q(4., "m")]);
    let complex = matrix(
        1,
        2,
        &[
            "<ml:imag>3</ml:imag>".into(),
            app("plus", &[n(3.), "<ml:imag>4</ml:imag>".into()]),
        ],
    );
    let rows = run(&[
        query(&vec(&roots)),
        query(&vec(&app("div", &[n(16.), a.clone()]))),
        query(&vec(&app("div", &[physical, q(2., "s")]))),
        query(&vec(&app("absval", &[complex]))),
        query(&vec(&app("plus", &[n(2.), n(3.)]))),
    ]);
    assert_eq!(nums(&rows[0], (2, 2)), [1., 2., 3., 4.]);
    let quot = nums(&rows[1], (2, 2));
    assert_eq!(quot, [16., 4., 16. / 9., 1.]);
    let dim = unit("m")
        .unwrap()
        .multiply(unit("s").unwrap(), true)
        .unwrap()
        .dimension;
    assert!(cells(&rows[2], (2, 1)).iter().all(|q| q.dimension == dim));
    assert_eq!(nums(&rows[3], (1, 2)), [3., 5.]);
    assert_eq!(scalar(&rows[4]).value.re, 5.);
    for other in [
        m(4, 1, &[1., 2., 3., 4.]),
        m(1, 4, &[1., 2., 3., 4.]),
        m(1, 1, &[2.]),
    ] {
        let rows = run(&[query(&vec(&app("plus", &[a.clone(), other])))]);
        assert!(
            rows[0].error.is_some(),
            "Same element count or 1x1 is not scalar broadcast"
        );
    }
}

#[test]
fn vectorized_functions_receive_scalar_arguments_without_rewriting_function_bodies() {
    let a = m(2, 2, &[1., 2., 3., 4.]);
    let rows = run(&[
        def("A", &a),
        function(false, "f", &app("mult", &[id("x"), id("x")])),
        query(&call("f", &[id("A")])),
        query(&vec(&call("f", &[id("A")]))),
        function(false, "g", &app("mult", &[id("A"), id("x")])),
        query(&vec(&call("g", &[id("A")]))),
        query(&vec(&call("rows", &[id("A")]))),
        query(&vec(&call("sin", &[m(2, 2, &[0., 0., 0., 0.])]))),
    ]);
    assert_eq!(nums(&rows[2], (2, 2)), [7., 10., 15., 22.]);
    assert_eq!(nums(&rows[3], (2, 2)), [1., 4., 9., 16.]);
    assert!(rows[5].error.as_ref().unwrap().detail.contains("nested"));
    assert!(
        rows[6].error.is_some(),
        "An array-only function must not receive its original matrix"
    );
    assert_eq!(nums(&rows[7], (2, 2)), [0.; 4]);
}

#[test]
fn vectorized_failure_poison_is_atomic_and_aliases_are_not_modified() {
    let a = m(2, 1, &[2., 0.]);
    let rows = run(&[
        def("A", &a),
        def("alias", &id("A")),
        def("x", &n(77.)),
        def("x", &vec(&app("div", &[n(1.), id("A")]))),
        query(&id("x")),
        query(&id("alias")),
        query(&id("A")),
    ]);
    assert!(rows[3].error.is_some());
    assert!(rows[4].value.is_none());
    assert_eq!(
        rows[4].error.as_ref().unwrap().previous_region.as_deref(),
        Some("3")
    );
    assert_eq!(nums(&rows[5], (2, 1)), [2., 0.]);
    assert_eq!(nums(&rows[6], (2, 1)), [2., 0.]);
    for e in [
        "<ml:apply><ml:vectorize changed=\"true\"/><ml:real>2</ml:real></ml:apply>".into(),
        app("vectorize", &[n(1.), n(2.)]),
        app("crossProduct", &[n(1.)]),
        call("vectorize", &[n(2.)]),
        call("crossProduct", &[a.clone(), a]),
    ] {
        assert!(run(&[query(&e)])[0].error.is_some(), "{e}");
    }
}

#[test]
fn cross_product_is_three_column_bilinear_right_handed_and_multiplies_units() {
    let i = m(3, 1, &[1., 0., 0.]);
    let j = m(3, 1, &[0., 1., 0.]);
    let a = matrix(
        3,
        1,
        &[
            app("plus", &[n(1.), "<ml:imag>1</ml:imag>".into()]),
            n(0.),
            n(0.),
        ],
    );
    let b = matrix(
        3,
        1,
        &[
            n(0.),
            app("plus", &[n(1.), "<ml:imag>2</ml:imag>".into()]),
            n(0.),
        ],
    );
    let meters = matrix(3, 1, &[q(1., "m"), q(0., "m"), q(0., "m")]);
    let seconds = matrix(3, 1, &[q(0., "s"), q(2., "s"), q(0., "s")]);
    let rows = run(&[
        query(&app("crossProduct", &[i.clone(), j.clone()])),
        query(&app("crossProduct", &[j, i])),
        query(&app("crossProduct", &[a, b])),
        query(&app("crossProduct", &[meters, seconds])),
    ]);
    assert_eq!(nums(&rows[0], (3, 1)), [0., 0., 1.]);
    assert_eq!(nums(&rows[1], (3, 1)), [0., 0., -1.]);
    assert_eq!(cells(&rows[2], (3, 1))[2].value, Complex64::new(-1., 3.));
    let dim = unit("m")
        .unwrap()
        .multiply(unit("s").unwrap(), false)
        .unwrap()
        .dimension;
    assert!(cells(&rows[3], (3, 1)).iter().all(|q| q.dimension == dim));
    assert_eq!(nums(&rows[3], (3, 1)), [0., 0., 2.]);
}

#[test]
fn cross_rejects_wrong_shape_and_nonuniform_units_and_never_reads_origin() {
    let valid = m(3, 1, &[1., 2., 3.]);
    for bad in [
        n(1.),
        m(1, 3, &[1., 2., 3.]),
        m(2, 1, &[1., 2.]),
        m(3, 2, &[1.; 6]),
        matrix(3, 1, &[q(1., "m"), q(2., "s"), q(3., "m")]),
    ] {
        for args in [[valid.clone(), bad.clone()], [bad, valid.clone()]] {
            assert!(
                run(&[query(&app("crossProduct", &args))])[0]
                    .error
                    .is_some()
            );
        }
    }
    let bad_origin = definition(false, "<ml:id labels=\"SYSTEM\">ORIGIN</ml:id>", &n(0.5));
    let rows = run(&[
        bad_origin,
        query(&app("crossProduct", &[valid.clone(), valid])),
    ]);
    assert_eq!(nums(&rows[1], (3, 1)), [0.; 3]);
}

#[test]
fn globals_are_a_geometric_first_pass_and_cannot_depend_on_local_or_later_globals() {
    let rows = run(&[
        query(&id("b")),
        global("a", &n(3.)),
        global("b", &app("plus", &[id("a"), n(2.)])),
        def("a", &n(9.)),
        query(&id("a")),
        def("local", &n(4.)),
        global("bad", &id("local")),
        query(&id("bad")),
        global("forward", &id("later")),
        global("later", &n(8.)),
        query(&id("forward")),
    ]);
    assert_eq!(scalar(&rows[0]).value.re, 5.);
    assert_eq!(scalar(&rows[4]).value.re, 3.);
    assert_eq!(
        rows[3].error.as_ref().unwrap().code,
        "normal_and_global_definitions"
    );
    assert!(rows[6].error.is_some());
    assert!(rows[8].error.is_some());
    assert_eq!(
        rows[7].error.as_ref().unwrap().previous_region.as_deref(),
        Some("6")
    );
    assert_eq!(
        rows[10].error.as_ref().unwrap().previous_region.as_deref(),
        Some("8")
    );
}

#[test]
fn editing_global_order_invalidates_earlier_results_and_manual_prepare_retains_old_payload() {
    let before = sheet(&[
        query(&id("b")),
        global("a", &n(3.)),
        global("b", &app("plus", &[id("a"), n(2.)])),
    ]);
    let after = before.replace("region-id=\"1\" top=\"65\"", "region-id=\"1\" top=\"195\"");
    let mut calculator = WorksheetCalculator::default();
    calculator.update(&before, None).unwrap();
    let initial = calculator.calculate(CalculationRequest::Worksheet).unwrap();
    assert_eq!(scalar(&initial.rows[0]).value.re, 5.);
    let pending = calculator.update(&after, None).unwrap();
    assert_eq!(pending.pending_region_ids.len(), 3);
    assert_eq!(scalar(&pending.rows[0]).value.re, 5.);
    assert!(pending.executed_region_ids.is_empty());
    let updated = calculator
        .calculate(CalculationRequest::ThroughRegion("0".into()))
        .unwrap();
    assert!(updated.rows[0].value.is_none());
    assert!(updated.rows[0].error.is_some());
    assert!(
        updated.executed_region_ids.contains(&"1".into())
            && updated.executed_region_ids.contains(&"2".into())
    );
}

#[test]
fn global_function_capture_duplicates_and_global_index_definitions_keep_their_existing_contract() {
    let index = app("indexer", &[id("v"), n(2.)]);
    let rows = run(&[
        query(&call("f", &[n(2.)])),
        global("a", &n(4.)),
        function(true, "f", &vec(&app("mult", &[id("a"), id("x")]))),
        definition(true, &index, &q(3., "m")),
        query(&id("v")),
        definition(false, &index, &q(7., "m")),
        query(&id("v")),
    ]);
    assert_eq!(scalar(&rows[0]).value.re, 8.);
    assert_eq!(nums(&rows[4], (3, 1)), [0., 0., 3.]);
    assert!(
        cells(&rows[4], (3, 1))
            .iter()
            .all(|q| q.dimension == unit("m").unwrap().dimension)
    );
    assert_eq!(
        rows[5].error.as_ref().unwrap().code,
        "normal_and_global_definitions"
    );
    assert_eq!(nums(&rows[6], (3, 1)), [0., 0., 3.]);
    for defs in [
        vec![global("a", &n(1.)), global("a", &n(2.)), query(&id("a"))],
        vec![
            definition(true, &index, &n(1.)),
            definition(true, &index, &n(2.)),
            query(&id("v")),
        ],
    ] {
        let rows = run(&defs);
        assert!(rows.iter().all(|r| r.error.is_some() && r.value.is_none()));
    }
}

#[test]
fn stopped_vectorized_recalculation_retains_published_payload_then_resumes() {
    let make = |last| {
        sheet(&[
            def("A", &m(2, 2, &[1., 2., 3., last])),
            query(&vec(&app("pow", &[id("A"), n(2.)]))),
        ])
    };
    let mut calculator = WorksheetCalculator::default();
    calculator.update(&make(4.), None).unwrap();
    let initial = calculator.calculate(CalculationRequest::Worksheet).unwrap();
    assert_eq!(nums(&initial.rows[1], (2, 2)), [1., 4., 9., 16.]);
    calculator.update(&make(5.), None).unwrap();
    let control = CalculationControl::default();
    control.cancel();
    let stopped = calculator
        .calculate_with_control(CalculationRequest::Worksheet, &control)
        .unwrap();
    assert!(stopped.cancelled);
    assert_eq!(nums(&stopped.rows[1], (2, 2)), [1., 4., 9., 16.]);
    assert!(!stopped.pending_region_ids.is_empty());
    let resumed = calculator
        .calculate(CalculationRequest::ThroughRegion("1".into()))
        .unwrap();
    assert_eq!(nums(&resumed.rows[1], (2, 2)), [1., 4., 9., 25.]);
}
