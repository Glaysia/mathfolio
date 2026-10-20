//! Native calculation opcodes are distinct from FUNCTION-labelled calls.
use mathcraft_compat::{CalculationControl, RegionEvaluation, Value, arrays, evaluate_worksheet};
use mathcraft_core::{Quantity, unit};
use num_complex::Complex64;

fn real(n: f64) -> String {
    format!("<ml:real>{n}</ml:real>")
}
fn id(n: &str) -> String {
    format!("<ml:id labels=\"VARIABLE\">{n}</ml:id>")
}
fn apply(op: &str, args: &[String]) -> String {
    format!("<ml:apply><ml:{op}/>{}</ml:apply>", args.join(""))
}
fn unary(op: &str, arg: &str) -> String {
    apply(op, &[arg.to_string()])
}
fn named(n: &str, a: &str) -> String {
    format!("<ml:apply><ml:id labels=\"FUNCTION\">{n}</ml:id>{a}</ml:apply>")
}
fn quantity(n: f64, u: &str) -> String {
    apply(
        "scale",
        &[real(n), format!("<ml:id labels=\"UNIT\">{u}</ml:id>")],
    )
}
fn matrix(r: usize, c: usize, values: &[String]) -> String {
    let mut cells = String::new();
    for col in 0..c {
        for row in 0..r {
            cells.push_str(&values[row * c + col]);
        }
    }
    format!("<ml:matrix rows=\"{r}\" cols=\"{c}\">{cells}</ml:matrix>")
}
fn define(n: &str, body: &str) -> String {
    format!("<ml:define>{}{body}</ml:define>", id(n))
}
fn origin(n: f64) -> String {
    format!(
        "<ml:define><ml:id labels=\"SYSTEM\">ORIGIN</ml:id>{}</ml:define>",
        real(n)
    )
}
fn query(body: &str) -> String {
    format!("<ml:eval>{body}<ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>")
}
fn run(parts: &[String]) -> Vec<RegionEvaluation> {
    let regions = parts
        .iter()
        .enumerate()
        .map(|(i, p)| {
            format!(
                "<region region-id=\"{i}\" left=\"20\" top=\"{}\"><math>{p}</math></region>",
                i * 65
            )
        })
        .collect::<String>();
    evaluate_worksheet(&format!("<worksheet xmlns=\"http://schemas.mathsoft.com/worksheet50\" xmlns:ml=\"http://schemas.mathsoft.com/math50\"><regions>{regions}</regions></worksheet>")).unwrap()
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
fn numbers(row: &RegionEvaluation, shape: (usize, usize)) -> Vec<f64> {
    cells(row, shape).iter().map(|q| q.value.re).collect()
}

#[test]
fn native_rows_and_columns_preserve_orientation_complex_values_and_each_cells_units() {
    let input = matrix(
        2,
        3,
        &[
            quantity(1., "m"),
            quantity(2., "s"),
            "<ml:imag>3</ml:imag>".into(),
            quantity(4., "kg"),
            quantity(5., "A"),
            real(6.),
        ],
    );
    let rows = run(&[
        define("M", &input),
        query(&apply("matrow", &[id("M"), real(0.)])),
        query(&apply("matcol", &[id("M"), real(1.)])),
        query(&apply("matcol", &[unary("transpose", &id("M")), real(0.)])),
    ]);
    let row = cells(&rows[1], (1, 3));
    assert_eq!(row[0].dimension, unit("m").unwrap().dimension);
    assert_eq!(row[1].dimension, unit("s").unwrap().dimension);
    assert_eq!(row[2].value, Complex64::new(0., 3.));
    let col = cells(&rows[2], (2, 1));
    assert_eq!(col[0].dimension, unit("s").unwrap().dimension);
    assert_eq!(col[1].dimension, unit("A").unwrap().dimension);
    assert_eq!(cells(&rows[3], (3, 1)), row);
}

#[test]
fn origin_controls_native_selection_including_negative_origin_and_function_capture() {
    let m = matrix(2, 2, &[real(1.), real(2.), real(3.), real(4.)]);
    let function = format!(
        "<ml:define><ml:function><ml:id labels=\"FUNCTION\">f</ml:id><ml:boundVars>{}</ml:boundVars></ml:function>{}</ml:define>",
        id("k"),
        apply("matcol", &[id("M"), id("k")])
    );
    let rows = run(&[
        define("M", &m),
        origin(-2.),
        query(&apply("matrow", &[id("M"), real(-1.)])),
        function,
        origin(1.),
        query(&apply("matcol", &[id("M"), real(1.)])),
        query(&named("f", &real(-1.))),
    ]);
    assert_eq!(numbers(&rows[2], (1, 2)), [3., 4.]);
    assert_eq!(numbers(&rows[5], (2, 1)), [1., 3.]);
    assert_eq!(numbers(&rows[6], (2, 1)), [2., 4.]);
}

#[test]
fn native_selection_rejects_bad_shapes_indices_and_origin_without_creating_named_aliases() {
    let m = matrix(2, 2, &[real(1.), real(2.), real(3.), real(4.)]);
    for index in [
        real(-1.),
        real(2.),
        real(0.5),
        quantity(1., "m"),
        "<ml:imag>1</ml:imag>".into(),
    ] {
        for op in ["matrow", "matcol"] {
            assert!(
                run(&[query(&apply(op, &[m.clone(), index.clone()]))])[0]
                    .error
                    .is_some(),
                "{op}/{index}"
            );
        }
    }
    for body in [
        apply("matcol", &[real(3.), real(0.)]),
        unary("matrow", &m),
        apply("matcol", &[m.clone(), real(0.), real(1.)]),
        named("matcol", &m),
        unary("norm", &m),
        apply("column", &[m.clone(), real(0.)]),
    ] {
        assert!(run(&[query(&body)])[0].error.is_some(), "{body}");
    }
    let rows = run(&[
        origin(0.5),
        query(&apply("matcol", &[m.clone(), real(1.)])),
        query(&unary("transpose", &m)),
    ]);
    assert!(rows[1].error.is_some());
    assert!(
        rows[2].error.is_none(),
        "Transpose must not depend on ORIGIN"
    );
}

#[test]
fn absolute_and_determinant_operators_share_shape_dispatch_but_named_functions_do_not() {
    let square = matrix(
        2,
        2,
        &[
            quantity(1., "m"),
            quantity(2., "m"),
            quantity(3., "m"),
            quantity(4., "m"),
        ],
    );
    let vector = matrix(2, 1, &[quantity(3., "m"), quantity(4., "m")]);
    for op in ["absval", "determinant"] {
        let rows = run(&[
            query(&unary(op, &square)),
            query(&unary(op, &vector)),
            query(&unary(op, "<ml:imag>3</ml:imag>")),
        ]);
        let det = scalar(&rows[0]);
        assert!((det.value.re + 2.).abs() < 1e-12);
        assert_eq!(det.dimension, unit("m").unwrap().dimension.map(|d| d * 2.));
        assert_eq!(scalar(&rows[1]).value.re, 5.);
        assert_eq!(scalar(&rows[1]).dimension, unit("m").unwrap().dimension);
        assert_eq!(scalar(&rows[2]).value.re, 3.);
    }
    for body in [
        named("det", &square),
        named("norm", &square),
        named("abs", &vector),
        unary("absval", &matrix(1, 2, &[real(3.), real(4.)])),
    ] {
        assert!(run(&[query(&body)])[0].error.is_some(), "{body}");
    }
    assert_eq!(
        scalar(&run(&[query(&named("norm", &vector))])[0]).value.re,
        5.
    );
}

#[test]
fn transpose_does_not_conjugate_and_factorial_retains_integer_domain_and_column_mapping() {
    let z = apply("plus", &[real(2.), "<ml:imag>3</ml:imag>".into()]);
    let row = matrix(1, 2, &[z.clone(), real(4.)]);
    let rows = run(&[
        query(&unary("conjugate", &quantity(2., "m"))),
        query(&unary("conjugate", &z)),
        query(&unary("transpose", &row)),
        query(&unary("factorial", &real(0.))),
        query(&unary("factorial", &real(170.))),
    ]);
    assert_eq!(scalar(&rows[0]).dimension, unit("m").unwrap().dimension);
    assert_eq!(scalar(&rows[1]).value, Complex64::new(2., -3.));
    assert_eq!(cells(&rows[2], (2, 1))[0].value, Complex64::new(2., 3.));
    assert_eq!(scalar(&rows[3]).value.re, 1.);
    assert!(scalar(&rows[4]).value.re.is_finite());
    assert_eq!(
        numbers(
            &run(&[query(&unary(
                "factorial",
                &matrix(2, 1, &[real(3.), real(4.)])
            ))])[0],
            (2, 1)
        ),
        [6., 24.]
    );
    for value in [
        real(-1.),
        real(0.5),
        real(171.),
        quantity(2., "m"),
        "<ml:imag>2</ml:imag>".into(),
        matrix(1, 2, &[real(3.), real(4.)]),
    ] {
        assert!(
            run(&[query(&unary("factorial", &value))])[0]
                .error
                .is_some(),
            "{value}"
        );
    }
}

#[test]
fn selected_arrays_remain_owned_and_failed_dependencies_and_cancellation_are_retained() {
    let input = matrix(2, 1, &[real(3.), real(4.)]);
    let rows = run(&[
        define("M", &input),
        define("M", "<ml:unknown/>"),
        query(&apply("matcol", &[id("M"), real(0.)])),
    ]);
    assert_eq!(
        rows[2].error.as_ref().unwrap().previous_region.as_deref(),
        Some("1")
    );
    let original = Value::Matrix {
        rows: 2,
        cols: 1,
        elements: vec![Quantity::scalar(3.), Quantity::scalar(4.)],
    };
    let mut selected = arrays::row_or_column(
        "matcol",
        &original,
        Quantity::scalar(0.),
        Quantity::scalar(0.),
    )
    .unwrap();
    if let Value::Matrix { elements, .. } = &mut selected {
        elements[0] = Quantity::scalar(99.);
    }
    assert_eq!(
        arrays::row_or_column(
            "matrow",
            &original,
            Quantity::scalar(0.),
            Quantity::scalar(0.)
        )
        .unwrap()
        .numeric_json()["elements"][0]["real"],
        3.
    );
    let control = CalculationControl::default();
    control.cancel();
    assert!(
        control
            .run(|| arrays::row_or_column(
                "matcol",
                &original,
                Quantity::scalar(0.),
                Quantity::scalar(0.)
            ))
            .unwrap_err()
            .is_cancelled()
    );
}
