use super::*;

fn id(name: &str) -> String {
    format!("<ml:id labels=\"VARIABLE\">{name}</ml:id>")
}
fn real(value: f64) -> String {
    format!("<ml:real>{value}</ml:real>")
}
fn evaluation(body: &str) -> String {
    format!("<ml:eval>{body}<ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>")
}
fn define(name: &str, body: &str, global: bool) -> String {
    let tag = if global { "globalDefine" } else { "define" };
    format!("<ml:{tag}>{}{body}</ml:{tag}>", id(name))
}
fn program(value: f64) -> String {
    format!(
        "<ml:program><ml:for>{}<ml:range>{}{}</ml:range><ml:program>{}</ml:program></ml:for>{}</ml:program>",
        id("i"),
        real(1.),
        real(2000.),
        real(1.),
        real(value)
    )
}
fn worksheet(rows: Vec<String>) -> String {
    editor_bridge::worksheet_xml(
        &rows
            .into_iter()
            .enumerate()
            .map(|(i, body)| ((i + 1) as u64, 0., i as f64 * 30., body))
            .collect::<Vec<_>>(),
    )
    .unwrap()
}
fn number(row: &RegionEvaluation) -> f64 {
    let Value::Scalar { quantity } = row.value.as_ref().unwrap() else {
        panic!("scalar expected");
    };
    quantity.value.re
}

#[test]
fn cancellation_control_is_thread_safe_scoped_and_not_a_worksheet_fault() {
    let outer = CalculationControl::default();
    let cancelled = CalculationControl::default();
    let sender = cancelled.clone();
    std::thread::spawn(move || sender.cancel()).join().unwrap();
    outer.run(|| {
        assert!(check_cancelled().is_ok());
        cancelled.run(|| assert!(check_cancelled().unwrap_err().is_cancelled()));
        assert!(check_cancelled().is_ok());
    });
    assert!(check_cancelled().is_ok());
    let mut calc = WorksheetCalculator::default();
    calc.update(&worksheet(vec![evaluation(&real(7.))]), None)
        .unwrap();
    let stopped = calc
        .calculate_with_control(CalculationRequest::Worksheet, &cancelled)
        .unwrap();
    assert!(stopped.cancelled);
    assert_eq!(stopped.pending_region_ids, ["1"]);
    assert!(stopped.executed_region_ids.is_empty());
    assert!(stopped.rows[0].error.is_none());
    let resumed = calc
        .calculate(CalculationRequest::ThroughRegion("1".into()))
        .unwrap();
    assert!(!resumed.cancelled);
    assert_eq!(number(&resumed.rows[0]), 7.);
}

#[test]
fn cancellation_inside_program_preserves_previous_payload_and_resumes_only_pending_regions() {
    let xml = |body: &str| {
        worksheet(vec![
            define("a", &real(2.), false),
            define("x", &evaluation(body), false),
            evaluation(&id("x")),
        ])
    };
    let mut calc = WorksheetCalculator::default();
    calc.update(&xml(&real(3.)), None).unwrap();
    calc.calculate(CalculationRequest::Worksheet).unwrap();
    calc.update(&xml(&program(9.)), None).unwrap();
    let stop = CalculationControl::stop_after_checks(100);
    let stopped = calc
        .calculate_with_control(CalculationRequest::ThroughRegion("3".into()), &stop)
        .unwrap();
    assert!(stopped.cancelled);
    assert_eq!(stopped.pending_region_ids, ["2", "3"]);
    assert!(stopped.executed_region_ids.is_empty());
    assert_eq!(number(&stopped.rows[1]), 3.);
    assert_eq!(number(&stopped.rows[2]), 3.);
    assert!(stopped.rows.iter().all(|row| row.error.is_none()));
    assert!(calc.scopes[1].is_none());
    let resumed = calc
        .calculate_with_control(
            CalculationRequest::ThroughRegion("3".into()),
            &CalculationControl::default(),
        )
        .unwrap();
    assert_eq!(resumed.executed_region_ids, ["2", "3"]);
    assert!(resumed.pending_region_ids.is_empty());
    assert_eq!(number(&resumed.rows[1]), 9.);
    assert_eq!(number(&resumed.rows[2]), 9.);
}

#[test]
fn cancellation_of_global_prepass_keeps_completed_bindings_without_publishing_partial_rows() {
    let source = worksheet(vec![
        evaluation(&id("g")),
        evaluation(&id("h")),
        define("g", &real(3.), true),
        define("h", &program(9.), true),
    ]);
    let mut calc = WorksheetCalculator::default();
    calc.update(&source, None).unwrap();
    let stopped = calc
        .calculate_with_control(
            CalculationRequest::ThroughRegion("2".into()),
            &CalculationControl::stop_after_checks(100),
        )
        .unwrap();
    assert!(stopped.cancelled);
    assert_eq!(stopped.executed_region_ids, ["3"]);
    assert_eq!(stopped.pending_region_ids, ["1", "2", "3", "4"]);
    assert!(
        stopped
            .rows
            .iter()
            .all(|row| row.value.is_none() && row.error.is_none())
    );
    let globals = calc.globals.as_ref().unwrap();
    assert!(globals.bindings.contains_key(&Symbol {
        name: "g".into(),
        label: "VARIABLE".into()
    }));
    assert!(!globals.bindings.contains_key(&Symbol {
        name: "h".into(),
        label: "VARIABLE".into()
    }));
    assert!(calc.context_before(0., 1000.).is_err());
    let resumed = calc
        .calculate_with_control(
            CalculationRequest::ThroughRegion("2".into()),
            &CalculationControl::default(),
        )
        .unwrap();
    assert!(!resumed.cancelled);
    assert_eq!(resumed.executed_region_ids, ["4", "1", "2"]);
    assert_eq!(resumed.pending_region_ids, ["3", "4"]);
    assert_eq!(number(&resumed.rows[0]), 3.);
    assert_eq!(number(&resumed.rows[1]), 9.);
    assert!(resumed.rows[2].value.is_none());
    let published = calc
        .calculate(CalculationRequest::ThroughRegion("4".into()))
        .unwrap();
    assert!(published.executed_region_ids.is_empty());
    assert!(published.pending_region_ids.is_empty());
    let forced = calc.calculate(CalculationRequest::Worksheet).unwrap();
    assert_eq!(forced.executed_region_ids, ["3", "4", "1", "2"]);
}

#[test]
fn cancelling_an_old_job_does_not_cancel_functions_captured_by_a_new_job() {
    let source = worksheet(vec![
        format!(
            "<ml:define><ml:function><ml:id labels=\"FUNCTION\">f</ml:id><ml:boundVars>{}</ml:boundVars></ml:function><ml:apply><ml:plus/>{}{}</ml:apply></ml:define>",
            id("t"),
            id("t"),
            real(1.)
        ),
        evaluation(&format!(
            "<ml:apply><ml:id labels=\"FUNCTION\">f</ml:id>{}</ml:apply>",
            real(2.)
        )),
    ]);
    let mut calc = WorksheetCalculator::default();
    calc.update(&source, None).unwrap();
    let old = CalculationControl::default();
    let defined = calc
        .calculate_with_control(CalculationRequest::ThroughRegion("1".into()), &old)
        .unwrap();
    assert!(defined.rows[0].error.is_none());
    old.cancel();
    let evaluated = calc
        .calculate_with_control(
            CalculationRequest::ThroughRegion("2".into()),
            &CalculationControl::default(),
        )
        .unwrap();
    assert!(!evaluated.cancelled);
    assert!(
        evaluated.rows[1].error.is_none(),
        "{:?}",
        evaluated.rows[1].error
    );
    assert_eq!(number(&evaluated.rows[1]), 3.);
}

#[test]
fn cancelled_definitions_and_solver_outputs_do_not_poison_existing_bindings() {
    let name = Symbol {
        name: "x".into(),
        label: "VARIABLE".into(),
    };
    let mut scope = Scope::default();
    scope.bindings.insert(
        name.clone(),
        Binding::Value(Value::scalar(Quantity::scalar(12.))),
    );
    let cancelled = CalculationControl::default();
    cancelled.cancel();
    let result = cancelled.run(|| {
        scope.define(
            &name,
            &None,
            &Expression::Scalar(Quantity::scalar(99.)),
            "cancelled",
        )
    });
    assert!(result.unwrap_err().is_cancelled());
    let Binding::Value(Value::Scalar { quantity }) = scope.lookup(&name).unwrap() else {
        panic!("previous binding poisoned");
    };
    assert_eq!(quantity.value.re, 12.);
    let source = format!(
        "<solveblock xmlns=\"{WS}\" xmlns:ml=\"{ML}\"><regions><region region-id=\"guess\" left=\"0\" top=\"0\" solve-block-category=\"guess-value\"><math>{}</math></region><region region-id=\"constraint\" left=\"0\" top=\"30\" solve-block-category=\"constraint\"><math><ml:apply><ml:equal/><ml:apply><ml:pow/>{}{}</ml:apply>{}</ml:apply></math></region><region region-id=\"solver\" left=\"0\" top=\"60\" solve-block-category=\"solver\"><math>{}</math></region></regions></solveblock>",
        define("z", &real(1.), false),
        id("z"),
        real(2.),
        real(2.),
        define(
            "x",
            &evaluation(&format!(
                "<ml:apply><ml:id labels=\"KEYWORD\">find</ml:id><ml:sequence>{}</ml:sequence></ml:apply>",
                id("z")
            )),
            false
        )
    );
    let parsed = roxmltree::Document::parse(&source).unwrap();
    let block = solve::parse_block(parsed.root_element(), None).unwrap();
    let result = CalculationControl::stop_after_checks(20)
        .run(|| solve::evaluate(&mut scope, &block, "cancelled-solve"));
    assert!(result.unwrap_err().is_cancelled());
    let Binding::Value(Value::Scalar { quantity }) = scope.lookup(&name).unwrap() else {
        panic!("previous export poisoned");
    };
    assert_eq!(quantity.value.re, 12.);
}

#[test]
fn cancellation_checks_reach_numeric_and_array_work_inside_one_expression() {
    let values = (0..32 * 32)
        .map(|index| Quantity::scalar(if index / 32 == index % 32 { 2. } else { 0.01 }))
        .collect();
    let matrix = Value::Matrix {
        rows: 32,
        cols: 32,
        elements: values,
    };
    let stop = CalculationControl::stop_after_checks(3085);
    let result = stop.run(|| arrays::binary("pow", &matrix, &Value::scalar(Quantity::scalar(-1.))));
    assert!(result.unwrap_err().is_cancelled());
    let stop = CalculationControl::default();
    stop.cancel();
    let coefficients = Value::Matrix {
        rows: 3,
        cols: 1,
        elements: vec![
            Quantity::scalar(-1.),
            Quantity::scalar(0.),
            Quantity::scalar(1.),
        ],
    };
    assert!(
        stop.run(|| numerics::call("polyroots", &[coefficients]))
            .unwrap_err()
            .is_cancelled()
    );
}
