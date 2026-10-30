use mathfolio_compat::{
    CalculationRequest, CalculationUpdate, Value, WorksheetCalculator, editor_bridge::worksheet_xml,
};

fn define(value: &str) -> String {
    format!("<ml:define><ml:id labels=\"VARIABLE\">x</ml:id>{value}</ml:define>")
}

fn eval(name: &str) -> String {
    format!(
        "<ml:eval><ml:id labels=\"VARIABLE\">{name}</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>"
    )
}

fn worksheet(value: &str) -> String {
    worksheet_xml(&[
        (1, 0., 0., define(value)),
        (2, 0., 30., eval("x")),
        (3, 0., 60., eval("x")),
    ])
    .unwrap()
}

fn number(update: &CalculationUpdate, index: usize) -> f64 {
    let Value::Scalar { quantity } = update.rows[index].value.as_ref().unwrap() else {
        panic!("scalar result expected")
    };
    quantity.value.re
}

#[test]
fn update_does_not_execute_and_dirty_prefix_leaves_tail_pending_with_previous_result() {
    let mut calc = WorksheetCalculator::default();
    let opened = calc
        .update(&worksheet("<ml:real>2</ml:real>"), None)
        .unwrap();
    assert_eq!(opened.pending_region_ids, ["1", "2", "3"]);
    assert!(opened.executed_region_ids.is_empty());
    assert!(opened.rows.iter().all(|row| row.value.is_none()));
    let complete = calc.calculate(CalculationRequest::Worksheet).unwrap();
    assert_eq!(number(&complete, 2), 2.);

    let changed = calc
        .update(&worksheet("<ml:real>7</ml:real>"), None)
        .unwrap();
    assert_eq!(changed.pending_region_ids, ["1", "2", "3"]);
    assert_eq!(number(&changed, 1), 2.);
    let prefix = calc
        .calculate(CalculationRequest::ThroughRegion("2".into()))
        .unwrap();
    assert_eq!(prefix.executed_region_ids, ["1", "2"]);
    assert_eq!(prefix.pending_region_ids, ["3"]);
    assert_eq!(number(&prefix, 1), 7.);
    assert_eq!(number(&prefix, 2), 2.);
    let tail = calc
        .calculate(CalculationRequest::ThroughRegion("3".into()))
        .unwrap();
    assert_eq!(tail.executed_region_ids, ["3"]);
    assert_eq!(number(&tail, 2), 7.);
    assert!(tail.pending_region_ids.is_empty());
}

#[test]
fn clean_prefix_is_not_reexecuted_and_active_force_is_distinct_from_worksheet() {
    let mut calc = WorksheetCalculator::default();
    let xml = worksheet("<ml:real>2</ml:real>");
    calc.update(&xml, None).unwrap();
    calc.calculate(CalculationRequest::Worksheet).unwrap();
    calc.update(&xml, None).unwrap();
    assert!(
        calc.calculate(CalculationRequest::ThroughRegion("3".into()))
            .unwrap()
            .executed_region_ids
            .is_empty()
    );
    let active = calc
        .calculate(CalculationRequest::ActiveRegion("2".into()))
        .unwrap();
    assert_eq!(active.executed_region_ids, ["2"]);
    assert_eq!(active.pending_region_ids, ["3"]);
    let tail = calc
        .calculate(CalculationRequest::ThroughRegion("3".into()))
        .unwrap();
    assert_eq!(tail.executed_region_ids, ["3"]);
    let all = calc.calculate(CalculationRequest::Worksheet).unwrap();
    assert_eq!(all.executed_region_ids, ["1", "2", "3"]);
}

#[test]
fn later_global_is_a_cached_prerequisite_without_publishing_future_rows() {
    let make = |n| {
        worksheet_xml(&[
        (1, 0., 0., eval("g")),
        (2, 0., 30., eval("g")),
        (3, 0., 60., format!("<ml:globalDefine><ml:id labels=\"VARIABLE\">g</ml:id><ml:real>{n}</ml:real></ml:globalDefine>")),
    ]).unwrap()
    };
    let mut calc = WorksheetCalculator::default();
    calc.update(&make(5), None).unwrap();
    let prefix = calc
        .calculate(CalculationRequest::ThroughRegion("1".into()))
        .unwrap();
    assert_eq!(number(&prefix, 0), 5.);
    assert_eq!(prefix.executed_region_ids, ["3", "1"]);
    assert_eq!(prefix.pending_region_ids, ["2", "3"]);
    let tail = calc
        .calculate(CalculationRequest::ThroughRegion("3".into()))
        .unwrap();
    assert_eq!(tail.executed_region_ids, ["2"]);
    assert_eq!(number(&tail, 1), 5.);
    let changed = calc.update(&make(9), None).unwrap();
    assert_eq!(changed.pending_region_ids, ["1", "2", "3"]);
    let prefix = calc
        .calculate(CalculationRequest::ThroughRegion("1".into()))
        .unwrap();
    assert_eq!(number(&prefix, 0), 9.);
    assert_eq!(number(&prefix, 1), 5.);
}

#[test]
fn failed_redefinition_invalidates_dependents_without_reusing_old_scope() {
    let mut calc = WorksheetCalculator::default();
    calc.update(&worksheet("<ml:real>2</ml:real>"), None)
        .unwrap();
    calc.calculate(CalculationRequest::Worksheet).unwrap();
    calc.update(
        &worksheet("<ml:id labels=\"VARIABLE\">missing</ml:id>"),
        None,
    )
    .unwrap();
    let update = calc
        .calculate(CalculationRequest::ThroughRegion("2".into()))
        .unwrap();
    assert!(update.rows[1].value.is_none());
    assert!(
        update.rows[1]
            .error
            .as_ref()
            .unwrap()
            .previous_region
            .is_some()
    );
    assert_eq!(number(&update, 2), 2.);
    assert_eq!(update.pending_region_ids, ["3"]);
}

#[test]
fn invalid_target_and_invalid_xml_do_not_destroy_existing_session() {
    let mut calc = WorksheetCalculator::default();
    calc.update(&worksheet("<ml:real>2</ml:real>"), None)
        .unwrap();
    calc.calculate(CalculationRequest::Worksheet).unwrap();
    assert!(
        calc.calculate(CalculationRequest::ActiveRegion("missing".into()))
            .is_err()
    );
    assert!(calc.update("not xml", None).is_err());
    let result = calc
        .calculate(CalculationRequest::ThroughRegion("3".into()))
        .unwrap();
    assert!(result.executed_region_ids.is_empty());
    assert_eq!(number(&result, 2), 2.);
}

#[test]
fn binding_checkpoints_preserve_redefinitions_and_function_definition_environment() {
    let xml = worksheet_xml(&[
        (1, 0., 0., define("<ml:real>2</ml:real>")),
        (2, 0., 30., "<ml:define><ml:function><ml:id>f</ml:id><ml:boundVars><ml:id>z</ml:id></ml:boundVars></ml:function><ml:apply><ml:plus/><ml:id>x</ml:id><ml:id>z</ml:id></ml:apply></ml:define>".into()),
        (3, 0., 60., define("<ml:real>7</ml:real>")),
        (4, 0., 90., "<ml:eval><ml:apply><ml:id>f</ml:id><ml:real>1</ml:real></ml:apply><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>".into()),
        (5, 0., 120., eval("x")),
    ]).unwrap();
    let mut calc = WorksheetCalculator::default();
    calc.update(&xml, None).unwrap();
    calc.calculate(CalculationRequest::ThroughRegion("3".into()))
        .unwrap();
    let answer = calc
        .calculate(CalculationRequest::ThroughRegion("5".into()))
        .unwrap();
    assert_eq!(answer.executed_region_ids, ["4", "5"]);
    assert_eq!(number(&answer, 3), 3.);
    assert_eq!(number(&answer, 4), 7.);
    let active = calc
        .calculate(CalculationRequest::ActiveRegion("4".into()))
        .unwrap();
    assert_eq!(active.executed_region_ids, ["4"]);
    assert_eq!(number(&active, 3), 3.);
}

#[test]
fn query_context_reuses_computed_bindings_and_later_globals_without_reexecution() {
    let xml = worksheet_xml(&[
        (1, 0., 0., define("<ml:real>2</ml:real>")),
        (2, 0., 60., define("<ml:real>99</ml:real>")),
        (
            3,
            0.,
            90.,
            "<ml:globalDefine><ml:id>g</ml:id><ml:real>5</ml:real></ml:globalDefine>".into(),
        ),
    ])
    .unwrap();
    let mut calc = WorksheetCalculator::default();
    calc.update(&xml, None).unwrap();
    assert!(calc.context_before(0., 30.).is_err());
    let calculated = calc
        .calculate(CalculationRequest::ThroughRegion("1".into()))
        .unwrap();
    assert_eq!(calculated.executed_region_ids, ["3", "1"]);
    let context = calc.context_before(0., 30.).unwrap();
    let query = "<ml:eval><ml:apply><ml:plus/><ml:id>x</ml:id><ml:id>g</ml:id></ml:apply><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>";
    for _ in 0..500 {
        let Value::Scalar { quantity } = context.evaluate_mathml(query).unwrap() else {
            panic!("scalar")
        };
        assert_eq!(quantity.value.re, 7.);
    }
    let unchanged = calc
        .calculate(CalculationRequest::ThroughRegion("1".into()))
        .unwrap();
    assert!(unchanged.executed_region_ids.is_empty());
    assert_eq!(unchanged.pending_region_ids, ["2", "3"]);
    assert!(calc.context_before(0., 70.).is_err());
    assert!(
        context
            .evaluate_mathml(&define("<ml:real>8</ml:real>"))
            .is_err()
    );
    assert!(context.evaluate_mathml("<ml:eval><ml:real>1</ml:real><ml:unitOverride><ml:id labels=\"UNIT\">m</ml:id></ml:unitOverride></ml:eval>").is_err());
    let Value::Scalar { quantity } = context.evaluate_mathml(&eval("x")).unwrap() else {
        panic!("scalar")
    };
    assert_eq!(quantity.value.re, 2.);
    assert!(calc.context_before(f64::NAN, 30.).is_err());
}

#[test]
fn global_only_context_can_be_prepared_before_first_worksheet_region_without_publishing_it() {
    let xml = worksheet_xml(&[(
        1,
        0.,
        90.,
        "<ml:globalDefine><ml:id>g</ml:id><ml:real>5</ml:real></ml:globalDefine>".into(),
    )])
    .unwrap();
    let mut calc = WorksheetCalculator::default();
    calc.update(&xml, None).unwrap();
    assert!(calc.context_before(0., 30.).is_err());
    let update = calc.calculate(CalculationRequest::Prerequisites).unwrap();
    assert_eq!(update.executed_region_ids, ["1"]);
    assert_eq!(update.pending_region_ids, ["1"]);
    assert!(update.rows[0].value.is_none());
    let context = calc.context_before(0., 30.).unwrap();
    let Value::Scalar { quantity } = context.evaluate_mathml(&eval("g")).unwrap() else {
        panic!("scalar")
    };
    assert_eq!(quantity.value.re, 5.);
    assert!(
        calc.calculate(CalculationRequest::Prerequisites)
            .unwrap()
            .executed_region_ids
            .is_empty()
    );
}

#[test]
fn unchanged_source_reuse_still_reparses_changed_placement_and_preserves_state_after_parse_error() {
    let grid = |size| {
        mathfolio_compat::PlacementGrid::from_presentation(&format!(
        "<settings xmlns=\"http://schemas.ptc.com/mathcad/settings/presentation10\"><pageModel paper-code=\"A4\" grid-size=\"{size}\"/></settings>"
    )).unwrap()
    };
    let source = worksheet("<ml:real>3</ml:real>");
    let mut calc = WorksheetCalculator::default();
    calc.update(&source, Some(grid("Standard"))).unwrap();
    let before = calc.calculate(CalculationRequest::Worksheet).unwrap();
    assert!(calc.update("<broken>", None).is_err());
    let recovered = calc.update(&source, Some(grid("Standard"))).unwrap();
    assert!(recovered.pending_region_ids.is_empty());
    assert_eq!(number(&recovered, 2), 3.);
    let moved = calc.update(&source, Some(grid("Fine"))).unwrap();
    assert_ne!(before.rows[1].top, moved.rows[1].top);
    assert!(moved.pending_region_ids.contains(&"3".into()));
    assert_eq!(
        number(&moved, 2),
        3.,
        "Geometry changes retain the last result until requested"
    );
    assert_eq!(
        number(&calc.calculate(CalculationRequest::Worksheet).unwrap(), 2),
        3.
    );
}

#[test]
#[ignore]
fn benchmark_large_matrix_session_update_and_context() {
    for count in [100, 1000] {
        let matrix = format!(
            "<ml:matrix rows=\"8\" cols=\"8\">{}</ml:matrix>",
            "<ml:real>1</ml:real>".repeat(64)
        );
        let mut rows: Vec<_> = (0..count)
            .map(|i| {
                (
                    i as u64 + 1,
                    0.,
                    i as f64 * 30.,
                    format!(
                        "<ml:define><ml:id labels=\"VARIABLE\">A{i}</ml:id>{matrix}</ml:define>"
                    ),
                )
            })
            .collect();
        rows.push((
            count as u64 + 1,
            0.,
            count as f64 * 30.,
            eval(&format!("A{}", count - 1)),
        ));
        let source = worksheet_xml(&rows).unwrap();
        let mut calc = WorksheetCalculator::default();
        let start = std::time::Instant::now();
        calc.update(&source, None).unwrap();
        let parsed = start.elapsed();
        let start = std::time::Instant::now();
        let calculated = calc.calculate(CalculationRequest::Worksheet).unwrap();
        assert!(calculated.rows.iter().all(|row| row.error.is_none()));
        let full = start.elapsed();
        let start = std::time::Instant::now();
        let update = calc.update(&source, None).unwrap();
        assert!(update.pending_region_ids.is_empty());
        let update = start.elapsed();
        let start = std::time::Instant::now();
        for _ in 0..10 {
            let context = calc.context_before(0., (count + 1) as f64 * 30.).unwrap();
            assert!(
                context
                    .evaluate_mathml(&eval(&format!("A{}", count - 1)))
                    .is_ok()
            );
        }
        println!(
            "CALC_PERF matrices={count} cells=64 parse_us={} full_us={} unchanged_update_us={} ten_contexts_us={}",
            parsed.as_micros(),
            full.as_micros(),
            update.as_micros(),
            start.elapsed().as_micros()
        );
    }
}
