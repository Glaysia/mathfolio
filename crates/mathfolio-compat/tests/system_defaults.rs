use mathfolio_compat::{Value, editor_bridge::worksheet_xml, evaluate_worksheet};

fn evaluation(label: &str, name: &str) -> String {
    format!(
        "<ml:eval><ml:id labels=\"{label}\">{name}</ml:id><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>"
    )
}

#[test]
fn system_defaults_are_visible_as_values_with_separate_variable_identity() {
    for (name, expected) in [
        ("ORIGIN", 0.),
        ("TOL", 0.001),
        ("CTOL", 0.001),
        ("PRNPRECISION", 4.),
        ("PRNCOLWIDTH", 8.),
    ] {
        let xml = worksheet_xml(&[(0, 0., 0., evaluation("SYSTEM", name))]).unwrap();
        let result = evaluate_worksheet(&xml).unwrap().remove(0);
        assert!(result.error.is_none(), "{name}: {:?}", result.error);
        let Value::Scalar { quantity } = result.value.unwrap() else {
            panic!("scalar expected")
        };
        assert_eq!(quantity.value.re, expected);
        assert!(quantity.is_scalar());
        if name != "ORIGIN" {
            let xml = worksheet_xml(&[(0, 0., 0., evaluation("VARIABLE", name))]).unwrap();
            assert!(evaluate_worksheet(&xml).unwrap()[0].error.is_some());
        }
    }
}

#[test]
fn explicit_and_failed_system_definitions_take_precedence_over_defaults() {
    let define =
        |rhs: &str| format!("<ml:define><ml:id labels=\"SYSTEM\">TOL</ml:id>{rhs}</ml:define>");
    let xml = worksheet_xml(&[
        (0, 0., 0., define("<ml:real>0.02</ml:real>")),
        (1, 0., 30., evaluation("SYSTEM", "TOL")),
        (
            2,
            0.,
            60.,
            define("<ml:id labels=\"VARIABLE\">missing</ml:id>"),
        ),
        (3, 0., 90., evaluation("SYSTEM", "TOL")),
    ])
    .unwrap();
    let rows = evaluate_worksheet(&xml).unwrap();
    let Value::Scalar { quantity } = rows[1].value.as_ref().unwrap() else {
        panic!("scalar expected")
    };
    assert_eq!(quantity.value.re, 0.02);
    assert!(rows[2].error.is_some());
    assert!(rows[3].value.is_none());
    assert!(rows[3].error.as_ref().unwrap().previous_region.is_some());
}

#[test]
fn seed_and_working_directory_are_not_invented_numeric_system_constants() {
    for name in ["Seed", "RandSeed", "SEED", "CWD"] {
        let xml = worksheet_xml(&[(0, 0., 0., evaluation("SYSTEM", name))]).unwrap();
        assert!(
            evaluate_worksheet(&xml).unwrap()[0].error.is_some(),
            "{name}"
        );
    }
}
