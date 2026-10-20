//! Derived scalar regression cases using the installed help's actual source
//! shape. Expected numerical answers are mathematical assertions, not captured
//! output from an original calculation runtime.
use mathcraft_compat::{RegionEvaluation, Value, editor_bridge::worksheet_xml};
use mathcraft_core::Quantity;

fn name(text: &str, label: &str) -> String {
    format!("<ml:id labels='{label}'>{text}</ml:id>")
}
fn real(value: f64) -> String {
    format!("<ml:real>{value}</ml:real>")
}
fn apply(operator: &str, left: String, right: String) -> String {
    format!("<ml:apply><ml:{operator}/>{left}{right}</ml:apply>")
}
fn temperature(value: f64, unit: &str) -> String {
    apply("scale", real(value), name(unit, "UNIT"))
}
fn query(expression: String, unit: Option<&str>) -> String {
    let unit = unit.map_or_else(|| "<ml:placeholder/>".into(), |u| name(u, "UNIT"));
    format!("<ml:eval>{expression}<ml:unitOverride>{unit}</ml:unitOverride></ml:eval>")
}
fn define(left: String, right: String) -> String {
    format!("<ml:define>{left}{right}</ml:define>")
}
fn run(lines: &[String]) -> Vec<RegionEvaluation> {
    let source = worksheet_xml(
        &lines
            .iter()
            .enumerate()
            .map(|(i, xml)| (i as u64 + 1, 0., i as f64 * 40., xml.clone()))
            .collect::<Vec<_>>(),
    )
    .unwrap();
    mathcraft_compat::evaluate_worksheet(&source).unwrap()
}
fn quantity(value: &Option<Value>) -> Quantity {
    let Some(Value::Scalar { quantity }) = value else {
        panic!("Expected scalar: {value:?}")
    };
    *quantity
}
fn check(row: &RegionEvaluation, canonical: f64, display: Option<f64>) {
    assert!(row.error.is_none(), "{row:?}");
    let q = quantity(&row.value);
    assert!((q.value.re - canonical).abs() < 1e-10, "{q:?}");
    assert_eq!(q.value.im, 0.);
    assert_eq!(q.dimension, mathcraft_core::unit("K").unwrap().dimension);
    if let Some(display) = display {
        let q = quantity(&row.display_value);
        assert!(
            (q.value.re - display).abs() < 1e-10,
            "{q:?} versus {display}"
        );
        assert_eq!(q.value.im, 0.);
        assert_eq!(q.dimension, [0.; 7]);
    } else {
        assert!(row.display_value.is_none());
    }
}

#[test]
fn actual_scale_and_unit_override_source_uses_forward_and_inverse_pairs() {
    let rows = run(&[
        query(temperature(20., "°C"), Some("°F")),
        query(temperature(32., "°F"), Some("K")),
        query(temperature(212., "°F"), Some("°C")),
        query(temperature(0., "°C"), None),
    ]);
    check(&rows[0], 293.15, Some(68.));
    check(&rows[1], 273.15, Some(273.15));
    check(&rows[2], 373.15, Some(100.));
    check(&rows[3], 273.15, None);
}

#[test]
fn linear_delta_units_and_existing_canonical_arithmetic_remain_distinct() {
    let rows = run(&[
        query(temperature(1., "Δ°C"), Some("K")),
        query(temperature(1., "Δ°F"), Some("K")),
        query(
            apply("plus", temperature(20., "°C"), temperature(10., "°C")),
            Some("°C"),
        ),
        query(
            apply("plus", temperature(20., "°C"), temperature(10., "Δ°C")),
            Some("°C"),
        ),
        query(
            apply("minus", temperature(20., "°C"), temperature(10., "°C")),
            Some("Δ°C"),
        ),
    ]);
    check(&rows[0], 1., Some(1.));
    check(&rows[1], 5. / 9., Some(5. / 9.));
    // Original help explains that this arithmetic is physically unintended;
    // it does not introduce a special absolute+absolute type error.
    check(&rows[2], 576.3, Some(303.15));
    check(&rows[3], 303.15, Some(30.));
    check(&rows[4], 10., Some(10.));
    assert!(mathcraft_core::units::lookup("°C").is_none());
    assert!(mathcraft_core::units::lookup("°F").is_none());
}

#[test]
fn wrong_dimensions_and_explicit_multiplication_do_not_apply_an_offset() {
    let rows = run(&[
        query(
            apply("scale", temperature(1., "m"), name("°C", "UNIT")),
            None,
        ),
        query(temperature(1., "m"), Some("°F")),
        query(apply("mult", real(20.), name("°C", "UNIT")), None),
        query(name("°C", "UNIT"), None),
    ]);
    assert_eq!(rows[0].error.as_ref().unwrap().code, "bad%_dimensions");
    assert_eq!(rows[1].error.as_ref().unwrap().code, "bad%_dimensions");
    assert!(rows[2].error.is_some(), "{:?}", rows[2]);
    assert!(rows[3].error.is_some(), "{:?}", rows[3]);
}

#[test]
fn labelled_variable_and_explicit_unit_bindings_keep_their_existing_precedence() {
    let rows = run(&[
        define(name("°C", "VARIABLE"), real(3.)),
        query(apply("scale", real(20.), name("°C", "VARIABLE")), None),
        query(temperature(20., "°C"), Some("°F")),
        define(name("°C", "UNIT"), temperature(2., "K")),
        query(temperature(20., "°C"), Some("°C")),
    ]);
    assert!(rows[1].error.is_none(), "{:?}", rows[1]);
    assert_eq!(quantity(&rows[1].value), Quantity::scalar(60.));
    check(&rows[2], 293.15, Some(68.));
    // An explicitly stored UNIT quantity is a worksheet linear unit, not
    // the built-in Celsius function or its registered inverse.
    check(&rows[4], 40., Some(20.));
}

#[test]
fn failed_and_user_function_bindings_are_not_bypassed_by_the_builtin() {
    let rows = run(&[
        define(name("°C", "UNIT"), name("missing", "VARIABLE")),
        query(temperature(20., "°C"), None),
        query(temperature(20., "K"), Some("°C")),
    ]);
    assert!(rows[0].error.is_some());
    for row in &rows[1..] {
        assert_eq!(
            row.error.as_ref().unwrap().previous_region.as_deref(),
            Some("1")
        );
    }
    let declaration = format!(
        "<ml:function>{}<ml:boundVars>{}</ml:boundVars></ml:function>",
        name("°C", "UNIT"),
        name("x", "VARIABLE"),
    );
    let rows = run(&[
        define(declaration, apply("plus", name("x", "VARIABLE"), real(1.))),
        query(temperature(20., "°C"), None),
    ]);
    assert!(rows[0].error.is_none(), "{:?}", rows[0]);
    assert!(rows[1].error.is_none(), "{:?}", rows[1]);
    assert_eq!(quantity(&rows[1].value), Quantity::scalar(21.));
}

#[test]
fn explicit_apply_uses_the_same_label_sensitive_scalar_dispatch() {
    // A dispatch consistency test. Installed help documents explicit °C(x);
    // the shipped captured source fixture proves SCALE, not this exact XML.
    let explicit = format!("<ml:apply>{}{}</ml:apply>", name("°C", "UNIT"), real(20.));
    let rows = run(&[
        query(explicit, Some("°F")),
        query(apply("scale", real(20.), name("°C", "FUNCTION")), None),
        query(apply("scale", real(20.), name("degC", "UNIT")), None),
    ]);
    check(&rows[0], 293.15, Some(68.));
    assert!(rows[1].error.is_some());
    assert!(rows[2].error.is_some());
}

#[test]
fn source_tree_round_trip_and_existing_value_wire_do_not_gain_affine_tags() {
    use mathcraft_compat::editor_bridge::{tree_from_mathml, tree_to_mathml};
    let xml = format!(
        "<ws:math xmlns:ws='http://schemas.mathsoft.com/worksheet50' xmlns:ml='http://schemas.mathsoft.com/math50'>{}</ws:math>",
        query(temperature(20., "°C"), Some("°F")),
    );
    let tree = tree_from_mathml(&xml).unwrap();
    let saved = tree_to_mathml(&tree).unwrap();
    assert_eq!(tree_from_mathml(&saved).unwrap(), tree);
    assert!(saved.contains("<ml:scale/>"), "{saved}");
    assert!(saved.contains("°C") && saved.contains("°F"), "{saved}");

    let row = run(&[query(temperature(20., "°C"), Some("°F"))]).remove(0);
    let wire = serde_json::to_value(row.value.as_ref().unwrap()).unwrap();
    assert_eq!(wire["kind"], "scalar");
    assert_eq!(wire.as_object().unwrap().len(), 2);
    assert_eq!(wire["quantity"].as_object().unwrap().len(), 2);
    assert!(wire["quantity"].get("dimension").is_some());
    assert!(wire["quantity"].get("value").is_some());
    let restored: Value = serde_json::from_value(wire).unwrap();
    assert_eq!(quantity(&Some(restored)), quantity(&row.value));
    assert_eq!(quantity(&row.display_value).dimension, [0.; 7]);
}
