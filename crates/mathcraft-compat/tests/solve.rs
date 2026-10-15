//! Native nested solve-block tests, all self-authored rather than original caches.
use mathcraft_compat::{RegionEvaluation, Value, evaluate_worksheet};
use mathcraft_core::Quantity;

fn id(x: &str) -> String {
    format!("<ml:id labels=\"VARIABLE\">{x}</ml:id>")
}
fn real(x: f64) -> String {
    format!("<ml:real>{x}</ml:real>")
}
fn binary(op: &str, a: &str, b: &str) -> String {
    format!("<ml:apply><ml:{op}/>{a}{b}</ml:apply>")
}
fn define(name: &str, body: &str) -> String {
    format!("<ml:define>{}{body}</ml:define>", id(name))
}
fn eval(body: &str) -> String {
    format!("<ml:eval>{body}<ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>")
}
fn region(name: &str, y: f64, category: Option<&str>, math: &str) -> String {
    let c = category
        .map(|x| format!(" solve-block-category=\"{x}\""))
        .unwrap_or_default();
    format!("<region region-id=\"{name}\" left=\"0\" top=\"{y}\"{c}><math>{math}</math></region>")
}
fn initial(name: &str, value: &str, y: f64) -> String {
    region(name, y, Some("guess-value"), &define(name, value))
}
fn equation(a: &str, b: &str, y: f64) -> String {
    region(
        &format!("constraint{y}"),
        y,
        Some("constraint"),
        &binary("equal", a, b),
    )
}
fn solver(name: &str, variables: &[&str], output: Option<&str>, y: f64) -> String {
    let args = variables.iter().map(|x| id(x)).collect::<String>();
    let expression = format!(
        "<ml:apply><ml:id labels=\"KEYWORD\">{name}</ml:id><ml:sequence>{args}</ml:sequence></ml:apply>"
    );
    let statement = output
        .map(|x| define(x, &eval(&expression)))
        .unwrap_or_else(|| eval(&expression));
    region("solver", y, Some("solver"), &statement)
        .replace("<math>", "<math resultRef=\"solver-result\">")
}
fn block(children: &[String], y: f64) -> String {
    format!(
        "<region region-id=\"block\" left=\"0\" top=\"{y}\"><solveblock><regions>{}</regions></solveblock></region>",
        children.join("")
    )
}
fn run(regions: &[String]) -> Vec<RegionEvaluation> {
    evaluate_worksheet(&format!("<worksheet xmlns=\"http://schemas.mathsoft.com/worksheet50\" xmlns:ml=\"http://schemas.mathsoft.com/math50\"><regions>{}</regions></worksheet>",regions.join(""))).unwrap()
}
fn scalar(row: &RegionEvaluation) -> Quantity {
    assert!(row.error.is_none(), "{:?}", row.error);
    let Some(Value::Scalar { quantity }) = &row.value else {
        panic!("{row:?}")
    };
    *quantity
}
fn column(row: &RegionEvaluation) -> Vec<Quantity> {
    assert!(row.error.is_none(), "{:?}", row.error);
    let Some(Value::Matrix {
        rows,
        cols,
        elements,
    }) = &row.value
    else {
        panic!("{row:?}")
    };
    assert_eq!(*cols, 1);
    assert_eq!(*rows, elements.len());
    elements.clone()
}
fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 0.001, "{a} != {b}")
}
fn metre(x: f64) -> String {
    binary("scale", &real(x), "<ml:id labels=\"UNIT\">m</ml:id>")
}

#[test]
fn nested_native_find_uses_geometry_and_exports_only_the_solver_result() {
    // XML is intentionally scrambled: local document geometry controls order.
    let solve = block(
        &[
            solver("Find", &["x", "y"], Some("answer"), 400.),
            equation(&binary("minus", &id("x"), &id("y")), &real(1.), 300.),
            initial("y", &binary("plus", &id("x"), &real(1.)), 100.),
            equation(&binary("plus", &id("x"), &id("y")), &real(3.), 200.),
            initial("x", &real(1.), 0.),
        ],
        100.,
    );
    let rows = run(&[
        region("outer-x", 0., None, &define("x", &real(99.))),
        solve,
        region("export", 200., None, &eval(&id("answer"))),
        region("old-x", 300., None, &eval(&id("x"))),
        region("local-y", 400., None, &eval(&id("y"))),
    ]);
    let solution = column(&rows[1]);
    near(solution[0].value.re, 2.);
    near(solution[1].value.re, 1.);
    assert_eq!(rows[1].result_ref.as_deref(), Some("solver-result"));
    assert_eq!(column(&rows[2]), solution);
    assert_eq!(scalar(&rows[3]).value.re, 99.);
    assert!(rows[4].error.is_some());
}

#[test]
fn find_solves_nonlinear_equalities_from_local_guesses() {
    let sum = binary(
        "plus",
        &binary("pow", &id("x"), &real(2.)),
        &binary("pow", &id("y"), &real(2.)),
    );
    let rows = run(&[block(
        &[
            initial("x", &real(1.5), 0.),
            initial("y", &real(0.5), 100.),
            equation(&sum, &real(5.), 200.),
            equation(&binary("minus", &id("x"), &id("y")), &real(1.), 300.),
            solver("find", &["x", "y"], None, 400.),
        ],
        0.,
    )]);
    let solution = column(&rows[0]);
    near(solution[0].value.re, 2.);
    near(solution[1].value.re, 1.);
}

#[test]
fn solver_and_constraints_use_only_children_above_the_selected_solver() {
    let rows = run(&[block(
        &[
            initial("x", &real(1.), 0.),
            equation(&id("x"), &real(2.), 100.),
            solver("find", &["x"], None, 200.),
            equation(&id("x"), &real(99.), 300.),
            initial("x", &real(99.), 400.),
        ],
        0.,
    )]);
    near(scalar(&rows[0]).value.re, 2.);
    let missing = run(&[block(
        &[
            equation(&id("x"), &real(2.), 100.),
            solver("find", &["x"], None, 200.),
            initial("x", &real(1.), 300.),
        ],
        0.,
    )]);
    assert!(missing[0].error.is_some());
}

#[test]
fn external_guess_is_used_without_mutating_it_and_defined_output_need_not_be_shown() {
    let solver = solver("find", &["x"], Some("answer"), 200.)
        .replace("<ml:eval>", "")
        .replace(
            "<ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
            "",
        );
    let rows = run(&[
        region("initial", 0., None, &define("x", &real(1.))),
        block(&[equation(&id("x"), &real(3.), 100.), solver], 100.),
        region("answer", 200., None, &eval(&id("answer"))),
        region("original", 300., None, &eval(&id("x"))),
    ]);
    assert!(rows[1].error.is_none(), "{:?}", rows[1]);
    assert!(rows[1].value.is_none());
    near(scalar(&rows[2]).value.re, 3.);
    assert_eq!(scalar(&rows[3]).value.re, 1.);
}

#[test]
fn mixed_unit_unknowns_preserve_their_dimensions_and_mismatch_fails() {
    let seconds = |x| binary("scale", &real(x), "<ml:id labels=\"UNIT\">s</ml:id>");
    let rows = run(&[block(
        &[
            initial("x", &metre(1.), 0.),
            initial("y", &seconds(1.), 100.),
            equation(&id("x"), &metre(3.), 200.),
            equation(&id("y"), &seconds(4.), 300.),
            solver("find", &["x", "y"], None, 400.),
        ],
        0.,
    )]);
    let solution = column(&rows[0]);
    near(solution[0].value.re, 3.);
    near(solution[1].value.re, 4.);
    assert_eq!(solution[0].dimension[0], 1.);
    assert_eq!(solution[1].dimension[2], 1.);
    let wrong = run(&[block(
        &[
            initial("x", &metre(1.), 0.),
            equation(&id("x"), &seconds(3.), 100.),
            solver("find", &["x"], None, 200.),
        ],
        0.,
    )]);
    assert_eq!(wrong[0].error.as_ref().unwrap().code, "bad%_dimensions");
}

#[test]
fn minerr_returns_an_approximate_least_squares_solution_for_inconsistent_equations() {
    let children = vec![
        initial("x", &real(0.), 0.),
        equation(&id("x"), &real(1.), 100.),
        equation(&id("x"), &real(3.), 200.),
        solver("Minerr", &["x"], None, 300.),
    ];
    let rows = run(&[block(&children, 0.)]);
    near(scalar(&rows[0]).value.re, 2.);
    let find = run(&[block(
        &children
            .into_iter()
            .map(|s| s.replace(">Minerr<", ">Find<"))
            .collect::<Vec<_>>(),
        0.,
    )]);
    assert!(find[0].error.is_some());
}

#[test]
fn failed_or_unsupported_block_poisons_previous_export_and_keeps_guesses_local() {
    let solve = block(
        &[
            initial("x", &real(0.), 0.),
            equation(
                &binary("plus", &binary("pow", &id("x"), &real(2.)), &real(1.)),
                &real(0.),
                100.,
            ),
            solver("Find", &["x"], Some("answer"), 200.),
        ],
        100.,
    );
    let rows = run(&[
        region("prior", 0., None, &define("answer", &real(42.))),
        solve,
        region("read", 200., None, &eval(&id("answer"))),
        region("local", 300., None, &eval(&id("x"))),
    ]);
    assert_eq!(rows[1].error.as_ref().unwrap().code, "solve_nonconvergence");
    assert!(rows[2].error.is_some());
    assert!(rows[2].value.is_none());
    assert_eq!(
        rows[2].error.as_ref().unwrap().previous_region.as_deref(),
        Some("block")
    );
    assert!(rows[3].error.is_some());
}

#[test]
fn constraints_do_not_lose_inequalities_options_or_complex_domain_information() {
    let basic = [
        initial("x", &real(1.), 0.),
        equation(&id("x"), &real(2.), 100.),
        solver("find", &["x"], Some("answer"), 200.),
    ];
    for children in [
        basic
            .iter()
            .map(|s| s.replace("<ml:equal/>", "<ml:lessThan/>"))
            .collect::<Vec<_>>(),
        basic
            .iter()
            .map(|s| s.replace("<ml:real>1</ml:real>", "<ml:imag>1</ml:imag>"))
            .collect(),
        basic
            .iter()
            .map(|s| s.replace(">find<", ">Minimize<"))
            .collect(),
        basic
            .iter()
            .map(|s| s.replace("<ml:equal/>", "<ml:equal custom-mode=\"different\"/>"))
            .collect(),
    ] {
        let rows = run(&[block(&children, 0.)]);
        assert!(rows[0].error.as_ref().unwrap().unsupported, "{:?}", rows[0]);
    }
}

#[test]
fn missing_guess_global_definition_inside_block_and_invalid_tolerance_fail() {
    let missing = block(
        &[
            equation(&id("x"), &real(2.), 100.),
            solver("find", &["x"], None, 200.),
        ],
        0.,
    );
    assert!(run(&[missing])[0].error.is_some());
    let global = block(
        &[
            initial("x", &real(1.), 0.).replace("ml:define", "ml:globalDefine"),
            equation(&id("x"), &real(2.), 100.),
            solver("find", &["x"], None, 200.),
        ],
        0.,
    );
    assert!(run(&[global])[0].error.as_ref().unwrap().unsupported);
    let tolerance = region(
        "tol",
        0.,
        None,
        "<ml:define><ml:id labels=\"SYSTEM\">CTOL</ml:id><ml:real>0</ml:real></ml:define>",
    );
    let rows = run(&[
        tolerance,
        block(
            &[
                initial("x", &real(1.), 0.),
                equation(&id("x"), &real(2.), 100.),
                solver("find", &["x"], None, 200.),
            ],
            100.,
        ),
    ]);
    assert_eq!(rows[1].error.as_ref().unwrap().code, "solve_domain");
}

#[test]
fn unsupported_solver_metadata_and_disabled_blocks_cannot_reuse_old_output() {
    let basic = block(
        &[
            initial("x", &real(1.), 0.),
            equation(&id("x"), &real(2.), 100.),
            solver("find", &["x"], Some("answer"), 200.),
        ],
        100.,
    );
    for invalid in [
        basic.replace("<solveblock>", "<solveblock disable-calc=\"true\">"),
        basic.replace(
            "<ml:apply><ml:id labels=\"KEYWORD\">",
            "<ml:apply algorithm=\"unknown\"><ml:id labels=\"KEYWORD\">",
        ),
    ] {
        let rows = run(&[
            region("old", 0., None, &define("answer", &real(99.))),
            invalid,
            region("read", 200., None, &eval(&id("answer"))),
        ]);
        assert!(rows[1].error.as_ref().unwrap().unsupported);
        assert!(rows[2].error.is_some());
        assert!(rows[2].value.is_none());
    }
}

#[test]
fn a_solver_export_cannot_replace_a_worksheet_global() {
    let global = region(
        "global",
        500.,
        None,
        &define("answer", &real(99.)).replace("ml:define", "ml:globalDefine"),
    );
    let solve = block(
        &[
            initial("x", &real(1.), 0.),
            equation(&id("x"), &real(2.), 100.),
            solver("find", &["x"], Some("answer"), 200.),
        ],
        0.,
    );
    let rows = run(&[
        solve,
        region("read", 100., None, &eval(&id("answer"))),
        global,
    ]);
    assert!(rows[0].error.is_some());
    assert_eq!(scalar(&rows[1]).value.re, 99.);
}
