use mathcraft_compat::{
    CalculationRequest, RegionEvaluation, Value, WorksheetCalculator,
    editor_bridge::{tree_from_mathml, tree_to_mathml, worksheet_xml},
};
use mathcraft_core::Quantity;

fn real(value: f64) -> String {
    format!("<ml:real>{value}</ml:real>")
}
fn id(name: &str) -> String {
    format!("<ml:id>{name}</ml:id>")
}
fn apply(operator: &str, args: &[String]) -> String {
    format!("<ml:apply><ml:{operator}/>{}</ml:apply>", args.join(""))
}
fn index(name: &str, indices: &[String]) -> String {
    let mut args = vec![id(name)];
    if indices.len() == 1 {
        args.extend_from_slice(indices);
    } else {
        args.push(format!("<ml:sequence>{}</ml:sequence>", indices.join("")));
    }
    apply("indexer", &args)
}
fn define(target: &str, body: &str) -> String {
    format!("<ml:define>{target}{body}</ml:define>")
}
fn unit(value: f64, name: &str) -> String {
    apply(
        "scale",
        &[
            real(value),
            format!("<ml:id labels=\"UNIT\">{name}</ml:id>"),
        ],
    )
}
fn query(body: &str) -> String {
    format!("<ml:eval>{body}<ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>")
}
fn sheet(fragments: &[String]) -> String {
    worksheet_xml(
        &fragments
            .iter()
            .enumerate()
            .map(|(i, source)| (i as u64 + 1, 0., i as f64 * 40., source.clone()))
            .collect::<Vec<_>>(),
    )
    .unwrap()
}
fn run(fragments: &[String]) -> Vec<RegionEvaluation> {
    mathcraft_compat::evaluate_worksheet(&sheet(fragments)).unwrap()
}
fn matrix(row: &RegionEvaluation) -> (usize, usize, &[Quantity]) {
    assert!(row.error.is_none(), "{row:?}");
    let Some(Value::Matrix {
        rows,
        cols,
        elements,
    }) = &row.value
    else {
        panic!("{row:?}")
    };
    (*rows, *cols, elements)
}
fn scalar(row: &RegionEvaluation) -> Quantity {
    assert!(row.error.is_none(), "{row:?}");
    let Some(Value::Scalar { quantity }) = &row.value else {
        panic!("{row:?}")
    };
    *quantity
}

#[test]
fn missing_arrays_grow_to_the_address_and_zero_fill_with_the_first_value_dimension() {
    let rows = run(&[
        define(&index("v", &[real(2.)]), &unit(5., "m")),
        query(&id("v")),
        define(&index("M", &[real(2.), real(5.)]), &real(13.)),
        query(&id("M")),
    ]);
    assert!(rows[0].error.is_none(), "{rows:?}");
    let (r, c, v) = matrix(&rows[1]);
    assert_eq!((r, c), (3, 1));
    assert_eq!(
        v.iter().map(|q| q.value.re).collect::<Vec<_>>(),
        [0., 0., 5.]
    );
    assert!(
        v.iter()
            .all(|q| q.dimension == mathcraft_core::unit("m").unwrap().dimension)
    );
    let (r, c, m) = matrix(&rows[3]);
    assert_eq!((r, c), (3, 6));
    assert_eq!(m[17].value.re, 13.);
    assert!(m[..17].iter().all(|q| q.value.re == 0.));
}

#[test]
fn heterogeneous_updates_keep_fill_basis_and_old_aliases_through_checkpoint_replay() {
    let xml = sheet(&[
        define(&index("v", &[real(2.)]), &unit(5., "m")),
        define(&id("saved"), &id("v")),
        define(&index("v", &[real(0.)]), &unit(7., "s")),
        define(&index("v", &[real(4.)]), &unit(9., "m")),
        query(&id("v")),
        query(&id("saved")),
    ]);
    let mut calculator = WorksheetCalculator::default();
    calculator.update(&xml, None).unwrap();
    calculator
        .calculate(CalculationRequest::ThroughRegion("3".into()))
        .unwrap();
    let result = calculator
        .calculate(CalculationRequest::ThroughRegion("6".into()))
        .unwrap();
    assert_eq!(result.executed_region_ids, ["4", "5", "6"]);
    let (r, _, v) = matrix(&result.rows[4]);
    assert_eq!(r, 5);
    assert_eq!(v[0].dimension, mathcraft_core::unit("s").unwrap().dimension);
    assert_eq!(v[3].value.re, 0.);
    assert_eq!(v[3].dimension, mathcraft_core::unit("m").unwrap().dimension);
    let (r, _, saved) = matrix(&result.rows[5]);
    assert_eq!(r, 3);
    assert_eq!(saved[0].value.re, 0.);
    assert_eq!(
        saved[0].dimension,
        mathcraft_core::unit("m").unwrap().dimension
    );
    let rows = run(&[
        define(&index("v", &[real(2.)]), &unit(5., "m")),
        define(&index("v", &[real(0.)]), &unit(7., "s")),
        define(&id("alias"), &id("v")),
        define(&index("alias", &[real(4.)]), &unit(9., "m")),
        query(&id("alias")),
        query(&id("v")),
    ]);
    let (r, _, alias) = matrix(&rows[4]);
    assert_eq!(r, 5);
    assert_eq!(
        alias[3].dimension,
        mathcraft_core::unit("m").unwrap().dimension
    );
    assert_eq!(matrix(&rows[5]).0, 3);
}

#[test]
fn scalar_self_update_and_captured_function_use_their_definition_scope_without_alias_mutation() {
    let function = format!(
        "<ml:function>{}<ml:boundVars>{}</ml:boundVars></ml:function>",
        id("f"),
        id("x")
    );
    let rows = run(&[
        define(&index("v", &[real(0.)]), &unit(2., "m")),
        define(
            &function,
            &apply("mult", &[index("v", &[real(0.)]), id("x")]),
        ),
        define(
            &index("v", &[real(0.)]),
            &apply("plus", &[index("v", &[real(0.)]), unit(3., "m")]),
        ),
        query(&index("v", &[real(0.)])),
        query(&format!("<ml:apply>{}{}</ml:apply>", id("f"), real(2.))),
    ]);
    assert_eq!(scalar(&rows[3]).value.re, 5.);
    assert_eq!(scalar(&rows[4]).value.re, 4.);
    let rows = run(&[
        define(&id("a"), &real(42.)),
        define(&index("a", &[real(2.)]), &id("a")),
        query(&id("a")),
    ]);
    assert_eq!(
        matrix(&rows[2])
            .2
            .iter()
            .map(|q| q.value.re)
            .collect::<Vec<_>>(),
        [0., 0., 42.]
    );
}

#[test]
fn origin_changes_index_mapping_without_relocating_existing_values() {
    let rows = run(&[
        define(&id("ORIGIN"), &real(-2.)),
        define(&index("v", &[real(0.)]), &real(7.)),
        query(&index("v", &[real(-2.)])),
        query(&index("v", &[real(0.)])),
        define(&id("ORIGIN"), &real(1.)),
        query(&index("v", &[real(3.)])),
    ]);
    assert_eq!(scalar(&rows[2]).value.re, 0.);
    assert_eq!(scalar(&rows[3]).value.re, 7.);
    assert_eq!(scalar(&rows[5]).value.re, 7.);
    for invalid in [real(-1.), real(0.5), unit(1., "m"), real(16384.)] {
        let rows = run(&[
            define(&index("a", &[real(0.)]), &real(99.)),
            define(&index("a", &[invalid]), &real(1.)),
            query(&id("a")),
        ]);
        assert!(rows[1].error.is_some());
        assert!(rows[2].error.is_some());
        assert!(rows[2].value.is_none());
        assert_eq!(
            rows[2].error.as_ref().unwrap().previous_region.as_deref(),
            Some("2")
        );
    }
}

#[test]
fn named_ranges_define_rectangles_and_repeated_binder_defines_diagonal_without_leaking_points() {
    let range = |name: &str, end| {
        define(
            &id(name),
            &format!("<ml:range>{}{}</ml:range>", real(0.), real(end)),
        )
    };
    let rows = run(&[
        range("i", 2.),
        range("j", 3.),
        define(
            &index("X", &[id("i"), id("j")]),
            &apply("plus", &[apply("mult", &[id("i"), real(10.)]), id("j")]),
        ),
        query(&id("X")),
        define(&index("D", &[id("i"), id("i")]), &real(1.)),
        query(&id("D")),
        query(&id("i")),
    ]);
    let (r, c, x) = matrix(&rows[3]);
    assert_eq!((r, c), (3, 4));
    assert_eq!(
        x.iter().map(|q| q.value.re).collect::<Vec<_>>(),
        [0., 1., 2., 3., 10., 11., 12., 13., 20., 21., 22., 23.]
    );
    let (r, c, d) = matrix(&rows[5]);
    assert_eq!((r, c), (3, 3));
    assert_eq!(
        d.iter().map(|q| q.value.re).collect::<Vec<_>>(),
        [1., 0., 0., 0., 1., 0., 0., 0., 1.]
    );
    assert!(matches!(rows[6].value, Some(Value::Range { count: 3, .. })));
    let rows = run(&[
        range("i", 2.),
        define(&index("a", &[real(0.)]), &real(9.)),
        define(&index("a", &[id("i")]), &index("a", &[id("i")])),
        query(&id("a")),
    ]);
    assert!(
        rows[2]
            .error
            .as_ref()
            .unwrap()
            .detail
            .contains("execution-order")
    );
    assert!(rows[3].error.is_some());
}

#[test]
fn indexed_definition_preserves_native_shape_labels_and_scalar_define_evaluation_units() {
    let source = define(
        &index("m", &[real(2.)]),
        &format!(
            "<ml:eval>{}<ml:unitOverride>{}</ml:unitOverride></ml:eval>",
            unit(2., "m"),
            id("cm")
        ),
    );
    let tree = tree_from_mathml(&source).unwrap();
    let serialized = tree_to_mathml(&tree).unwrap();
    assert_eq!(tree_from_mathml(&serialized).unwrap(), tree);
    let rows = run(&[source, query(&id("m"))]);
    assert_eq!(scalar(&rows[0]).value.re, 2.);
    let Some(Value::Scalar { quantity }) = rows[0].display_value else {
        panic!("{rows:?}")
    };
    assert_eq!(quantity.value.re, 200.);
    assert!(quantity.is_scalar());
    assert_eq!(matrix(&rows[1]).0, 3);
}

#[test]
fn one_global_indexed_definition_is_a_prerequisite_and_conflicts_are_not_silent_updates() {
    let global =
        define(&index("g", &[real(1.)]), &real(7.)).replace("ml:define", "ml:globalDefine");
    let rows = run(&[query(&index("g", &[real(1.)])), global.clone()]);
    assert_eq!(scalar(&rows[0]).value.re, 7.);
    let rows = run(&[
        global.clone(),
        define(&index("g", &[real(0.)]), &real(1.)),
        query(&id("g")),
    ]);
    assert_eq!(
        rows[1].error.as_ref().unwrap().code,
        "normal_and_global_definitions"
    );
    assert_eq!(matrix(&rows[2]).2[1].value.re, 7.);
    let rows = run(&[global.clone(), global, query(&id("g"))]);
    assert!(rows.iter().all(|row| row.error.is_some()));
}

#[test]
fn row_vector_assignments_follow_the_documented_authoring_example_and_keep_orientation() {
    let rows = run(&[
        define(
            &id("v"),
            "<ml:matrix rows=\"1\" cols=\"3\"><ml:real>0</ml:real><ml:real>1</ml:real><ml:real>2</ml:real></ml:matrix>",
        ),
        define(
            &index("v", &[real(1.)]),
            &apply("plus", &[real(1.), "<ml:imag>3</ml:imag>".into()]),
        ),
        define(&index("v", &[real(4.)]), &real(9.)),
        query(&id("v")),
    ]);
    assert!(rows.iter().all(|r| r.error.is_none()), "{rows:?}");
    let (r, c, v) = matrix(&rows[3]);
    assert_eq!((r, c), (1, 5));
    assert_eq!(
        v.iter().map(|q| q.value.re).collect::<Vec<_>>(),
        [0., 1., 2., 0., 9.]
    );
    assert_eq!(v[1].value.im, 3.);

    let rows = run(&[
        define(&id("ORIGIN"), &real(-2.)),
        define(
            &id("v"),
            &format!(
                "<ml:matrix rows=\"1\" cols=\"2\">{}{}</ml:matrix>",
                unit(1., "m"),
                unit(2., "m")
            ),
        ),
        define(&index("v", &[real(-1.)]), &unit(3., "s")),
        define(
            &index("v", &[real(2.)]),
            &format!(
                "<ml:eval>{}<ml:unitOverride>{}</ml:unitOverride></ml:eval>",
                unit(4., "s"),
                id("ms")
            ),
        ),
        query(&id("v")),
    ]);
    assert!(rows.iter().all(|r| r.error.is_none()), "{rows:?}");
    assert_eq!(scalar(&rows[3]).value.re, 4.);
    let Some(Value::Scalar { quantity }) = rows[3].display_value else {
        panic!("{rows:?}")
    };
    assert!((quantity.value.re - 4000.).abs() < 1e-9);
    let (r, c, v) = matrix(&rows[4]);
    assert_eq!((r, c), (1, 5));
    assert_eq!(v[1].dimension, mathcraft_core::unit("s").unwrap().dimension);
    assert_eq!(v[2].value.re, 0.);
    assert_eq!(v[2].dimension, mathcraft_core::unit("m").unwrap().dimension);
}

#[test]
fn explicit_limits_do_not_guess_general_matrix_single_indices_or_mixed_literal_fill_units() {
    let literal = |values: &[String], rows, cols| {
        format!(
            "<ml:matrix rows=\"{rows}\" cols=\"{cols}\">{}</ml:matrix>",
            values.join("")
        )
    };
    let rows = run(&[
        define(
            &id("v"),
            &literal(&[real(1.), real(2.), real(3.), real(4.)], 2, 2),
        ),
        define(&index("v", &[real(1.)]), &real(3.)),
        query(&id("v")),
    ]);
    assert!(rows[1].error.is_some());
    assert!(rows[2].error.is_some());
    let rows = run(&[
        define(&id("v"), &literal(&[unit(1., "m"), unit(2., "s")], 2, 1)),
        define(&index("v", &[real(0.)]), &unit(3., "m")),
        query(&id("v")),
        define(&index("v", &[real(2.)]), &unit(4., "m")),
        query(&id("v")),
    ]);
    assert_eq!(matrix(&rows[2]).2[0].value.re, 3.);
    assert!(rows[3].error.is_some());
    assert!(rows[4].error.is_some());
}

#[test]
fn indexed_lhs_names_that_match_si_units_bind_variables_and_preserve_explicit_rhs_units() {
    for name in ["A", "m", "V"] {
        let source = define(&index(name, &[real(2.)]), &unit(7., name));
        let xml = sheet(&[source, query(&index(name, &[real(2.)])), query(&id(name))]);
        let rows = mathcraft_compat::evaluate_worksheet(&xml).unwrap();
        assert!(
            rows.iter().all(|r| r.error.is_none()),
            "{name}: {rows:?}\n{xml}"
        );
        assert_eq!(scalar(&rows[1]).value.re, 7.);
        assert_eq!(
            scalar(&rows[1]).dimension,
            mathcraft_core::unit(name).unwrap().dimension
        );
        assert_eq!(matrix(&rows[2]).0, 3);
    }
}

#[test]
fn a_late_range_cell_error_poisons_the_new_binding_without_modifying_earlier_aliases() {
    let rows = run(&[
        define(&index("a", &[real(0.)]), &real(99.)),
        define(&id("old"), &id("a")),
        define(
            &id("i"),
            &format!("<ml:range>{}{}</ml:range>", real(0.), real(2.)),
        ),
        define(
            &index("a", &[id("i")]),
            &apply("div", &[real(1.), apply("minus", &[real(1.), id("i")])]),
        ),
        query(&id("a")),
        query(&id("old")),
    ]);
    assert!(rows[3].error.is_some());
    assert!(rows[4].error.is_some());
    assert!(rows[4].value.is_none());
    assert_eq!(matrix(&rows[5]).2[0].value.re, 99.);
}
