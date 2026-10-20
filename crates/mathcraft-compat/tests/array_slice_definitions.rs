use mathcraft_compat::{
    CalculationRequest, RegionEvaluation, Value, WorksheetCalculator, editor_bridge::worksheet_xml,
};
use mathcraft_core::Quantity;

fn real(value: f64) -> String {
    format!("<ml:real>{value}</ml:real>")
}
fn id(name: &str) -> String {
    format!("<ml:id>{name}</ml:id>")
}
fn apply(op: &str, args: &[String]) -> String {
    format!("<ml:apply><ml:{op}/>{}</ml:apply>", args.join(""))
}
fn slice(op: &str, name: &str, offset: f64) -> String {
    apply(op, &[id(name), real(offset)])
}
fn matrix(rows: usize, cols: usize, values: &[String]) -> String {
    let native = (0..cols)
        .flat_map(|col| (0..rows).map(move |row| values[row * cols + col].as_str()))
        .collect::<String>();
    format!("<ml:matrix rows=\"{rows}\" cols=\"{cols}\">{native}</ml:matrix>")
}
fn numbers(rows: usize, cols: usize, values: &[f64]) -> String {
    matrix(
        rows,
        cols,
        &values.iter().map(|n| real(*n)).collect::<Vec<_>>(),
    )
}
fn define(target: &str, rhs: &str) -> String {
    format!("<ml:define>{target}{rhs}</ml:define>")
}
fn query(rhs: &str) -> String {
    format!("<ml:eval>{rhs}<ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>")
}
fn unit(n: f64, name: &str) -> String {
    apply(
        "scale",
        &[real(n), format!("<ml:id labels=\"UNIT\">{name}</ml:id>")],
    )
}
fn sheet(parts: &[String]) -> String {
    worksheet_xml(
        &parts
            .iter()
            .enumerate()
            .map(|(i, text)| (i as u64 + 1, 0., i as f64 * 40., text.clone()))
            .collect::<Vec<_>>(),
    )
    .unwrap()
}
fn run(parts: &[String]) -> Vec<RegionEvaluation> {
    mathcraft_compat::evaluate_worksheet(&sheet(parts)).unwrap()
}
fn values(row: &RegionEvaluation, expected_rows: usize, expected_cols: usize) -> &[Quantity] {
    assert!(row.error.is_none(), "{row:?}");
    let Some(Value::Matrix {
        rows,
        cols,
        elements,
    }) = &row.value
    else {
        panic!("{row:?}")
    };
    assert_eq!((*rows, *cols), (expected_rows, expected_cols));
    elements
}
fn assert_values(row: &RegionEvaluation, rows: usize, cols: usize, expected: &[f64]) {
    assert_eq!(
        values(row, rows, cols)
            .iter()
            .map(|q| q.value.re)
            .collect::<Vec<_>>(),
        expected
    );
}
fn function(name: &str, parameter: &str, body: &str) -> String {
    define(
        &format!(
            "<ml:function>{}<ml:boundVars>{}</ml:boundVars></ml:function>",
            id(name),
            id(parameter)
        ),
        body,
    )
}
fn call(name: &str, arg: f64) -> String {
    format!(
        "<ml:apply>{}<ml:sequence>{}</ml:sequence></ml:apply>",
        id(name),
        real(arg)
    )
}

#[test]
fn undefined_and_scalar_targets_grow_without_preserving_an_old_scalar() {
    let rows = run(&[
        define(&slice("matcol", "a", 2.), &numbers(2, 1, &[4., 5.])),
        query(&id("a")),
        define(&slice("matrow", "b", 1.), &numbers(1, 3, &[6., 7., 8.])),
        query(&id("b")),
        define(&id("c"), &real(99.)),
        define(&slice("matcol", "c", 1.), &real(3.)),
        query(&id("c")),
        define(&slice("matrow", "d", 2.), &real(9.)),
        query(&id("d")),
    ]);
    assert_values(&rows[1], 2, 3, &[0., 0., 4., 0., 0., 5.]);
    assert_values(&rows[3], 2, 3, &[0., 0., 0., 6., 7., 8.]);
    assert_values(&rows[6], 1, 2, &[0., 3.]);
    assert_values(&rows[8], 3, 1, &[0., 0., 9.]);
}

#[test]
fn short_prefix_updates_keep_the_remainder_and_longer_inputs_extend() {
    let rows = run(&[
        define(&id("a"), &numbers(3, 2, &[1., 2., 3., 4., 5., 6.])),
        define(&slice("matcol", "a", 0.), &numbers(2, 1, &[7., 8.])),
        query(&id("a")),
        define(&slice("matrow", "a", 1.), &real(9.)),
        query(&id("a")),
        define(&slice("matrow", "a", 4.), &numbers(1, 3, &[10., 11., 12.])),
        query(&id("a")),
        define(
            &slice("matcol", "a", 3.),
            &numbers(6, 1, &[13., 14., 15., 16., 17., 18.]),
        ),
        query(&id("a")),
    ]);
    assert_values(&rows[2], 3, 2, &[7., 2., 8., 4., 5., 6.]);
    assert_values(&rows[4], 3, 2, &[7., 2., 9., 4., 5., 6.]);
    assert_values(
        &rows[6],
        5,
        3,
        &[
            7., 2., 0., 9., 4., 0., 5., 6., 0., 0., 0., 0., 10., 11., 12.,
        ],
    );
    assert_values(
        &rows[8],
        6,
        4,
        &[
            7., 2., 0., 13., 9., 4., 0., 14., 5., 6., 0., 15., 0., 0., 0., 16., 10., 11., 12., 17.,
            0., 0., 0., 18.,
        ],
    );
}

#[test]
fn row_setter_copies_a_matrix_block_and_column_rejects_wrong_orientation() {
    let rows = run(&[
        define(&slice("matrow", "a", 1.), &numbers(2, 2, &[1., 2., 3., 4.])),
        query(&id("a")),
        define(&id("saved"), &id("a")),
        define(&slice("matcol", "a", 0.), &numbers(1, 2, &[9., 8.])),
        query(&id("a")),
        query(&id("saved")),
    ]);
    assert_values(&rows[1], 3, 2, &[0., 0., 1., 2., 3., 4.]);
    assert!(rows[3].error.is_some());
    assert_eq!(
        rows[4].error.as_ref().unwrap().previous_region.as_deref(),
        Some("4")
    );
    assert_values(&rows[5], 3, 2, &[0., 0., 1., 2., 3., 4.]);
}

#[test]
fn negative_origin_and_unit_complex_payloads_keep_fill_basis_aliases_and_capture() {
    let origin = "<ml:id labels=\"SYSTEM\">ORIGIN</ml:id>";
    let complex = apply("plus", &[real(2.), "<ml:imag>-3</ml:imag>".into()]);
    let rows = run(&[
        define(origin, &real(-2.)),
        define(
            &slice("matcol", "a", -1.),
            &matrix(2, 1, &[unit(5., "m"), unit(6., "m")]),
        ),
        define(&id("saved"), &id("a")),
        function("f", "x", &apply("matcol", &[id("a"), id("x")])),
        define(&slice("matrow", "a", -2.), &unit(7., "s")),
        define(&slice("matcol", "a", 1.), &matrix(1, 1, &[complex])),
        query(&id("a")),
        define(origin, &real(0.)),
        query(&call("f", -1.)),
        query(&id("saved")),
    ]);
    let a = values(&rows[6], 2, 4);
    assert_eq!(a[0].dimension, mathcraft_core::unit("s").unwrap().dimension);
    assert_eq!(a[3].value.im, -3.);
    assert_eq!(a[7].value.re, 0.);
    assert_eq!(a[7].dimension, mathcraft_core::unit("m").unwrap().dimension);
    assert_values(&rows[8], 2, 1, &[5., 6.]);
    assert_values(&rows[9], 2, 2, &[0., 5., 0., 6.]);
}

#[test]
fn global_slice_prepass_conflicts_and_failed_global_identity_are_preserved() {
    let global =
        |target: String, body: String| format!("<ml:globalDefine>{target}{body}</ml:globalDefine>");
    let rows = run(&[
        query(&id("a")),
        global(slice("matcol", "a", 1.), numbers(2, 1, &[2., 3.])),
        define(&slice("matrow", "a", 0.), &real(8.)),
        query(&id("a")),
    ]);
    assert_values(&rows[0], 2, 2, &[0., 2., 0., 3.]);
    assert_eq!(
        rows[2].error.as_ref().unwrap().code,
        "normal_and_global_definitions"
    );
    assert_values(&rows[3], 2, 2, &[0., 2., 0., 3.]);
    let rows = run(&[
        query(&id("a")),
        global(slice("matrow", "a", 0.), real(1.)),
        global(slice("matcol", "a", 1.), real(2.)),
    ]);
    assert!(rows.iter().all(|row| row.error.is_some()));
    let rows = run(&[query(&id("a")), global(slice("matcol", "a", 0.5), real(1.))]);
    assert_eq!(
        rows[0].error.as_ref().unwrap().previous_region.as_deref(),
        Some("2")
    );
}

#[test]
fn ranged_slices_reject_unestablished_lowering_without_breaking_indexed_ranges() {
    let range = "<ml:range><ml:real>0</ml:real><ml:real>2</ml:real></ml:range>";
    let rows = run(&[
        define(&id("i"), range),
        define(
            &apply("matcol", &[id("a"), id("i")]),
            &matrix(2, 1, &[id("i"), apply("plus", &[id("i"), real(10.)])]),
        ),
        query(&id("a")),
        define(&apply("indexer", &[id("b"), id("i")]), &id("i")),
        query(&id("b")),
    ]);
    assert!(
        rows[1]
            .error
            .as_ref()
            .unwrap()
            .detail
            .contains("original lowering contract")
    );
    assert_eq!(
        rows[2].error.as_ref().unwrap().previous_region.as_deref(),
        Some("2")
    );
    assert_values(&rows[4], 3, 1, &[0., 1., 2.]);
}

#[test]
fn checkpoint_replay_and_inline_result_use_the_updated_full_slice() {
    let parts = [
        define(&id("a"), &numbers(3, 1, &[1., 2., 3.])),
        define(&slice("matcol", "a", 0.), &query(&unit(4., "m"))),
        query(&id("a")),
        define(&slice("matcol", "a", 2.), &real(5.)),
        query(&id("a")),
    ];
    let mut calculator = WorksheetCalculator::default();
    calculator.update(&sheet(&parts), None).unwrap();
    let first = calculator
        .calculate(CalculationRequest::ThroughRegion("2".into()))
        .unwrap();
    assert_values(&first.rows[1], 3, 1, &[4., 2., 3.]);
    let last = calculator.calculate(CalculationRequest::Worksheet).unwrap();
    assert_values(&last.rows[4], 3, 3, &[4., 0., 5., 2., 0., 0., 3., 0., 0.]);
    assert_eq!(values(&last.rows[4], 3, 3)[4].dimension, [0.; 7]);
}

#[test]
fn malformed_indices_and_oversize_slices_poison_only_the_defined_base() {
    for index in [
        real(-1.),
        real(0.25),
        unit(1., "m"),
        "<ml:complex><ml:real>0</ml:real><ml:real>1</ml:real></ml:complex>".into(),
        real(16384.),
    ] {
        let rows = run(&[
            define(&id("a"), &numbers(1, 1, &[9.])),
            define(&apply("matcol", &[id("a"), index]), &real(2.)),
            query(&id("a")),
            query(&real(7.)),
        ]);
        assert!(rows[1].error.is_some(), "{rows:?}");
        assert_eq!(
            rows[2].error.as_ref().unwrap().previous_region.as_deref(),
            Some("2")
        );
        assert!(rows[3].error.is_none());
    }
    let rows = run(&[
        define(
            &slice("matcol", "a", 128.),
            &numbers(128, 1, &vec![1.; 128]),
        ),
        query(&id("a")),
    ]);
    assert!(rows.iter().all(|row| row.error.is_some()));
}

#[test]
fn program_slice_and_index_assignments_preserve_caller_arrays_and_origin_capture() {
    let local =
        |target: String, body: String| format!("<ml:localDefine>{target}{body}</ml:localDefine>");
    let target = apply(
        "indexer",
        &[
            id("a"),
            "<ml:sequence><ml:real>1</ml:real><ml:real>1</ml:real></ml:sequence>".into(),
        ],
    );
    let program = format!(
        "<ml:program>{}{}{}{}</ml:program>",
        local(slice("matcol", "a", 1.), numbers(2, 1, &[7., 8.])),
        local(slice("matrow", "a", 2.), numbers(1, 2, &[9., 10.])),
        local(target, real(11.)),
        id("a")
    );
    let rows = run(&[
        define("<ml:id labels=\"SYSTEM\">ORIGIN</ml:id>", &real(1.)),
        define(&id("a"), &numbers(2, 2, &[1., 2., 3., 4.])),
        function("f", "x", &program),
        define("<ml:id labels=\"SYSTEM\">ORIGIN</ml:id>", &real(0.)),
        query(&call("f", 0.)),
        query(&id("a")),
    ]);
    assert_values(&rows[4], 2, 2, &[11., 2., 9., 10.]);
    assert_values(&rows[5], 2, 2, &[1., 2., 3., 4.]);
}

#[test]
fn mixed_source_fill_metadata_survives_slice_copy_and_program_aliasing() {
    let index = |name: &str, offset: f64| apply("indexer", &[id(name), real(offset)]);
    let program = format!(
        "<ml:program><ml:localDefine>{}{}</ml:localDefine><ml:localDefine>{}{}</ml:localDefine>{}</ml:program>",
        id("b"),
        id("a"),
        slice("matcol", "b", 2.),
        unit(9., "m"),
        id("b")
    );
    let rows = run(&[
        define(&index("a", 1.), &unit(2., "m")),
        define(&index("a", 0.), &unit(3., "s")),
        define(&slice("matcol", "copied", 1.), &id("a")),
        query(&id("copied")),
        function("f", "x", &program),
        query(&call("f", 0.)),
        query(&id("a")),
    ]);
    let copied = values(&rows[3], 2, 2);
    assert_eq!(
        copied[0].dimension,
        mathcraft_core::unit("m").unwrap().dimension
    );
    assert_eq!(
        copied[1].dimension,
        mathcraft_core::unit("s").unwrap().dimension
    );
    let b = values(&rows[5], 2, 3);
    assert_eq!(b[4].dimension, mathcraft_core::unit("m").unwrap().dimension);
    assert_values(&rows[6], 2, 1, &[3., 2.]);
}

#[test]
fn nested_program_array_assignment_keeps_the_call_depth_budget() {
    let mut parts = vec![function("f0", "x", &real(1.))];
    for level in 1..50 {
        let body = format!(
            "<ml:program><ml:localDefine>{}{}</ml:localDefine>{}</ml:program>",
            slice("matcol", "a", 0.),
            call(&format!("f{}", level - 1), 0.),
            id("a")
        );
        parts.push(function(&format!("f{level}"), "x", &body));
    }
    parts.push(query(&call("f49", 0.)));
    let rows = run(&parts);
    let fault = rows
        .last()
        .unwrap()
        .error
        .as_ref()
        .expect("Nested evaluation must respect its depth budget");
    assert!(fault.detail.contains("depth"), "{fault:?}");
}
