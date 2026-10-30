use mathfolio_compat::{
    CalculationRequest, Value, WorksheetCalculator, editor_bridge::worksheet_xml,
};

const ML: &str = "http://schemas.mathsoft.com/math50";
const WS: &str = "http://schemas.mathsoft.com/worksheet50";

fn column(name: &str, cells: &[Option<f64>], unit: &str) -> String {
    let data = cells
        .iter()
        .map(|cell| {
            cell.map(|value| format!("<ml:real>{value}</ml:real>"))
                .unwrap_or("<ml:placeholder/>".into())
        })
        .collect::<String>();
    format!(
        "<math><ml:define><ml:id>{name}</ml:id><ml:apply><ml:scale/><ml:matrix rows=\"{}\" cols=\"1\">{data}</ml:matrix>{unit}</ml:apply></ml:define></math>",
        cells.len()
    )
}
fn table(columns: &[String], disabled: bool) -> String {
    format!(
        "<spec-table xmlns=\"{WS}\" xmlns:ml=\"{ML}\" disable-calc=\"{disabled}\">{}</spec-table>",
        columns.join("")
    )
}
fn query(name: &str, unit: &str) -> String {
    format!("<ml:eval><ml:id>{name}</ml:id><ml:unitOverride>{unit}</ml:unitOverride></ml:eval>")
}
fn sheet(fragments: &[String]) -> String {
    worksheet_xml(
        &fragments
            .iter()
            .enumerate()
            .map(|(i, xml)| (i as u64 + 1, 0., i as f64 * 40., xml.clone()))
            .collect::<Vec<_>>(),
    )
    .unwrap()
}
fn vector(row: &mathfolio_compat::RegionEvaluation) -> &[mathfolio_core::Quantity] {
    assert!(row.error.is_none(), "{row:?}");
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
    elements
}

#[test]
fn newly_inserted_unnamed_table_column_is_a_local_fault_not_a_worksheet_parse_failure() {
    let unnamed = column("blank", &[None], "<ml:placeholder/>")
        .replace("<ml:id>blank</ml:id>", "<ml:placeholder/>");
    let xml = sheet(&[
        table(&[unnamed, column("valid", &[Some(7.)], "<ml:placeholder/>")], false),
        query("valid", "<ml:placeholder/>"),
        "<ml:eval><ml:apply><ml:plus/><ml:real>1</ml:real><ml:real>2</ml:real></ml:apply><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>".into(),
    ]);
    let rows = mathfolio_compat::evaluate_worksheet(&xml).unwrap();
    assert_eq!(rows.len(), 3);
    assert!(rows[0].error.is_some());
    assert!(rows[1].error.is_none(), "{rows:?}");
    assert!(rows[2].error.is_none(), "{rows:?}");
    let Some(Value::Scalar { quantity, .. }) = &rows[1].value else {
        panic!("{rows:?}");
    };
    assert_eq!(quantity.value.re, 7.);
    let Some(Value::Scalar { quantity, .. }) = &rows[2].value else {
        panic!("{rows:?}");
    };
    assert_eq!(quantity.value.re, 3.);
}

#[test]
fn six_input_columns_bind_55_values_and_do_not_turn_five_padding_cells_into_zero() {
    let names = ["I_dc", "I_rms", "A_mm2", "D_mm", "deltaT", "awg"];
    let counts = [10, 10, 10, 10, 8, 7];
    let units = ["A", "A", "mm", "mm", "K", "A"];
    let columns = names
        .iter()
        .zip(counts)
        .zip(units)
        .map(|((name, count), unit)| {
            let cells = (0..10)
                .map(|i| if i < count { Some(i as f64 + 1.) } else { None })
                .collect::<Vec<_>>();
            column(
                name,
                &cells,
                &format!("<ml:id labels=\"UNIT\">{unit}</ml:id>"),
            )
        })
        .collect::<Vec<_>>();
    let mut fragments = vec![table(&columns, false)];
    fragments.extend(names.iter().map(|name| query(name, "<ml:placeholder/>")));
    let rows = mathfolio_compat::evaluate_worksheet(&sheet(&fragments)).unwrap();
    assert_eq!(rows.len(), 7);
    assert!(rows[0].value.is_none());
    assert!(rows[0].error.is_none(), "{rows:?}");
    let mut total = 0;
    for ((row, count), unit) in rows[1..].iter().zip(counts).zip(units) {
        let values = vector(row);
        assert_eq!(values.len(), count);
        total += count;
        let expected = mathfolio_core::unit(unit).unwrap();
        assert_eq!(values[0].dimension, expected.dimension);
        assert!((values[0].value.re - expected.value.re).abs() < 1e-12);
    }
    assert_eq!(total, 55);
}

#[test]
fn table_scale_is_physical_and_explicit_display_override_scales_once() {
    let xml = sheet(&[
        table(
            &[column("d", &[Some(2.), Some(3.)], "<ml:id>mm</ml:id>")],
            false,
        ),
        query("d", "<ml:id>cm</ml:id>"),
    ]);
    let rows = mathfolio_compat::evaluate_worksheet(&xml).unwrap();
    assert_eq!(vector(&rows[1])[0].value.re, 0.002);
    let Some(Value::Matrix { elements, .. }) = &rows[1].display_value else {
        panic!("{rows:?}")
    };
    assert_eq!(elements[0].value.re, 0.2);
    assert!(elements[0].is_scalar());
}

#[test]
fn one_entered_cell_is_scalar_empty_unit_is_identity_and_interior_blank_is_error() {
    let xml = sheet(&[
        table(
            &[
                column("one", &[Some(42.), None, None], "<ml:placeholder/>"),
                column("bad", &[Some(1.), None, Some(3.)], "<ml:placeholder/>"),
                column("good", &[Some(7.), Some(8.)], "<ml:placeholder/>"),
                column("empty", &[None, None], "<ml:placeholder/>"),
            ],
            false,
        ),
        query("one", "<ml:placeholder/>"),
        query("good", "<ml:placeholder/>"),
        query("bad", "<ml:placeholder/>"),
        query("empty", "<ml:placeholder/>"),
    ]);
    let rows = mathfolio_compat::evaluate_worksheet(&xml).unwrap();
    assert!(
        rows[0]
            .error
            .as_ref()
            .unwrap()
            .detail
            .contains("Table column 2")
    );
    let Some(Value::Scalar { quantity }) = rows[1].value else {
        panic!("{rows:?}")
    };
    assert_eq!(quantity.value.re, 42.);
    assert!(quantity.is_scalar());
    assert_eq!(vector(&rows[2]).len(), 2);
    assert!(rows[3].error.is_some());
    assert!(rows[4].error.is_some());
}

#[test]
fn table_bindings_participate_in_geometric_labels_and_disabled_columns_do_not_leak() {
    let fragment = table(&[column("m", &[Some(9.)], "<ml:placeholder/>")], false);
    let xml = worksheet_xml(&[
        (2, 0., 100., query("m", "<ml:placeholder/>")),
        (1, 0., 0., fragment),
    ])
    .unwrap();
    let rows = mathfolio_compat::evaluate_worksheet(&xml).unwrap();
    let Some(Value::Scalar { quantity }) = rows[1].value else {
        panic!("{rows:?}")
    };
    assert_eq!(quantity.value.re, 9.);
    assert!(quantity.is_scalar());
    let xml = sheet(&[
        table(&[column("m", &[Some(9.)], "<ml:placeholder/>")], true),
        query("m", "<ml:placeholder/>"),
    ]);
    let rows = mathfolio_compat::evaluate_worksheet(&xml).unwrap();
    assert_eq!(rows.len(), 1);
    let Some(Value::Scalar { quantity }) = rows[0].value else {
        panic!("{rows:?}")
    };
    assert_eq!(
        quantity.dimension,
        mathfolio_core::unit("m").unwrap().dimension
    );
}

#[test]
fn changed_table_updates_all_checkpoint_bindings_and_stays_one_external_region() {
    let source = |value| {
        sheet(&[
            table(
                &[
                    column("a", &[Some(value)], "<ml:placeholder/>"),
                    column("b", &[Some(3.)], "<ml:placeholder/>"),
                ],
                false,
            ),
            query("a", "<ml:placeholder/>"),
            query("b", "<ml:placeholder/>"),
        ])
    };
    let mut calculator = WorksheetCalculator::default();
    calculator.update(&source(2.), None).unwrap();
    let first = calculator.calculate(CalculationRequest::Worksheet).unwrap();
    assert_eq!(first.executed_region_ids, ["1", "2", "3"]);
    calculator.update(&source(42.), None).unwrap();
    let table_only = calculator
        .calculate(CalculationRequest::ThroughRegion("1".into()))
        .unwrap();
    assert_eq!(table_only.executed_region_ids, ["1"]);
    let rest = calculator
        .calculate(CalculationRequest::ThroughRegion("3".into()))
        .unwrap();
    assert_eq!(rest.executed_region_ids, ["2", "3"]);
    let Some(Value::Scalar { quantity }) = rest.rows[1].value else {
        panic!("{rest:?}")
    };
    assert_eq!(quantity.value.re, 42.);
    let Some(Value::Scalar { quantity }) = rest.rows[2].value else {
        panic!("{rest:?}")
    };
    assert_eq!(quantity.value.re, 3.);
}

#[test]
fn invalid_column_redefinition_hides_prior_value_and_survives_checkpoint_replay() {
    let xml = sheet(&[
        "<ml:define><ml:id>a</ml:id><ml:real>99</ml:real></ml:define>".into(),
        table(
            &[column(
                "a",
                &[Some(1.), None, Some(3.)],
                "<ml:placeholder/>",
            )],
            false,
        ),
        query("a", "<ml:placeholder/>"),
    ]);
    let mut calculator = WorksheetCalculator::default();
    calculator.update(&xml, None).unwrap();
    let first = calculator
        .calculate(CalculationRequest::ThroughRegion("2".into()))
        .unwrap();
    assert!(first.rows[1].error.is_some());
    let rest = calculator
        .calculate(CalculationRequest::ThroughRegion("3".into()))
        .unwrap();
    assert_eq!(rest.executed_region_ids, ["3"]);
    assert!(rest.rows[2].error.is_some());
    assert!(rest.rows[2].value.is_none());
}
