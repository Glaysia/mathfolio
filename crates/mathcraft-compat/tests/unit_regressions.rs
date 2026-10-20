//! Physical bindings and editable display-unit slots must remain independent.
use mathcraft_compat::{
    CalculationRequest, Value, WorksheetCalculator, editor_bridge::worksheet_xml,
};
use mathcraft_core::Quantity;

fn quantity(value: &Option<Value>) -> Quantity {
    match value.as_ref().expect("numeric result") {
        Value::Scalar { quantity } => *quantity,
        other => panic!("scalar expected: {other:?}"),
    }
}
fn sheet(rows: &[&str]) -> String {
    worksheet_xml(
        &rows
            .iter()
            .enumerate()
            .map(|(index, source)| (index as u64 + 1, 0., index as f64 * 40., source.to_string()))
            .collect::<Vec<_>>(),
    )
    .unwrap()
}
fn definition(meters: f64) -> String {
    format!(
        "<ml:define><ml:id>d</ml:id><ml:apply><ml:scale/><ml:real>{meters}</ml:real><ml:id labels=\"UNIT\">m</ml:id></ml:apply></ml:define>"
    )
}
fn evaluate(name: &str, display_unit: &str) -> String {
    format!(
        "<ml:eval><ml:id>{name}</ml:id><ml:unitOverride>{display_unit}</ml:unitOverride></ml:eval>"
    )
}

#[test]
fn display_unit_edit_changes_scaled_answer_without_changing_physical_bindings() {
    let mut calculator = WorksheetCalculator::default();
    let source = |meters, unit: &str| {
        sheet(&[
            &definition(meters),
            &evaluate("d", &format!("<ml:id>{unit}</ml:id>")),
            &evaluate("d", "<ml:placeholder/>"),
        ])
    };
    calculator.update(&source(2., "cm"), None).unwrap();
    let first = calculator.calculate(CalculationRequest::Worksheet).unwrap();
    assert_eq!(quantity(&first.rows[1].value).value.re, 2.);
    assert_eq!(quantity(&first.rows[1].display_value).value.re, 200.);
    assert!(quantity(&first.rows[1].display_value).is_scalar());
    assert_eq!(
        quantity(&first.rows[2].value).dimension,
        mathcraft_core::unit("m").unwrap().dimension
    );

    let pending = calculator.update(&source(2., "mm"), None).unwrap();
    assert_eq!(quantity(&pending.rows[1].display_value).value.re, 200.);
    let changed = calculator.calculate(CalculationRequest::Worksheet).unwrap();
    assert!((quantity(&changed.rows[1].display_value).value.re - 2000.).abs() < 1e-9);
    assert_eq!(quantity(&changed.rows[2].value).value.re, 2.);

    calculator.update(&source(3., "cm"), None).unwrap();
    let updated = calculator.calculate(CalculationRequest::Worksheet).unwrap();
    assert_eq!(quantity(&updated.rows[1].display_value).value.re, 300.);
    let mut fresh = WorksheetCalculator::default();
    fresh.update(&source(2., "cm"), None).unwrap();
    let reopened = fresh.calculate(CalculationRequest::Worksheet).unwrap();
    assert_eq!(quantity(&reopened.rows[1].display_value).value.re, 200.);
}

#[test]
fn explicit_unit_slots_survive_variable_shadowing_and_nested_function_calls() {
    let xml = sheet(&[
        "<ml:define><ml:id>m</ml:id><ml:real>7</ml:real></ml:define>",
        &definition(2.),
        "<ml:define><ml:function><ml:id>f</ml:id><ml:boundVars><ml:id>m</ml:id></ml:boundVars></ml:function><ml:apply><ml:scale/><ml:real>2</ml:real><ml:id>m</ml:id></ml:apply></ml:define>",
        "<ml:define><ml:function><ml:id>g</ml:id><ml:boundVars><ml:id>x</ml:id></ml:boundVars></ml:function><ml:apply><ml:id>f</ml:id><ml:id>x</ml:id></ml:apply></ml:define>",
        "<ml:eval><ml:apply><ml:id>g</ml:id><ml:id>d</ml:id></ml:apply><ml:unitOverride><ml:id>cm</ml:id></ml:unitOverride></ml:eval>",
        &evaluate("m", "<ml:placeholder/>"),
    ]);
    let rows = mathcraft_compat::evaluate_worksheet(&xml).unwrap();
    assert!(rows.iter().all(|row| row.error.is_none()), "{rows:?}");
    assert_eq!(quantity(&rows[4].value).value.re, 4.);
    assert_eq!(quantity(&rows[4].display_value).value.re, 400.);
    assert_eq!(quantity(&rows[5].value).value.re, 7.);
    assert!(quantity(&rows[5].value).is_scalar());
}

#[test]
fn prefixed_compound_and_angle_display_units_use_canonical_values_once() {
    for (source_unit, display_unit, expected) in [
        ("kHz", "Hz", 2000.),
        ("ns", "s", 2e-9),
        ("deg", "rad", std::f64::consts::PI / 90.),
    ] {
        let xml = sheet(&[&format!(
            "<ml:eval><ml:apply><ml:scale/><ml:real>2</ml:real><ml:id labels=\"UNIT\">{source_unit}</ml:id></ml:apply><ml:unitOverride><ml:id>{display_unit}</ml:id></ml:unitOverride></ml:eval>"
        )]);
        let rows = mathcraft_compat::evaluate_worksheet(&xml).unwrap();
        assert!(rows[0].error.is_none(), "{rows:?}");
        assert!(
            (quantity(&rows[0].display_value).value.re - expected).abs() <= expected.abs() * 1e-12
        );
    }
    let xml = sheet(&[
        "<ml:eval><ml:apply><ml:scale/><ml:real>2</ml:real><ml:id labels=\"UNIT\">kHz</ml:id></ml:apply><ml:unitOverride><ml:apply><ml:div/><ml:id>rad</ml:id><ml:id>s</ml:id></ml:apply></ml:unitOverride></ml:eval>",
    ]);
    let rows = mathcraft_compat::evaluate_worksheet(&xml).unwrap();
    assert!(rows[0].error.is_none(), "{rows:?}");
    assert_eq!(quantity(&rows[0].display_value).value.re, 2000.);
    // The percent constant is an input scale, not a physical unit or a result
    // format. Formatting controls must not change this binding's meaning.
    let xml = sheet(&[
        "<ml:eval><ml:apply><ml:scale/><ml:real>2</ml:real><ml:id>%</ml:id></ml:apply><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>",
    ]);
    let rows = mathcraft_compat::evaluate_worksheet(&xml).unwrap();
    assert!(rows[0].error.is_none(), "{rows:?}");
    assert_eq!(quantity(&rows[0].value).value.re, 0.02);
    assert!(quantity(&rows[0].value).is_scalar());
    assert!(rows[0].display_value.is_none());
}
