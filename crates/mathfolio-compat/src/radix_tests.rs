//! Calculation integration; original parser limits remain unresolved.
use super::*;
fn value(xml: &str) -> Result<Value> {
    let wrapped = format!(
        "<ml:eval xmlns:ml=\"{ML}\">{xml}<ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>"
    );
    let context = CalculationContext {
        scope: Scope::default(),
        labels: HashMap::new(),
    };
    context.evaluate_mathml(&wrapped)
}
#[test]
fn explicit_integral_real_bases_are_exact_and_unary_sign_is_separate() {
    for (digits, base, expected) in [
        ("001101", 2, 13.),
        ("00032", 8, 26.),
        ("0a32", 16, 2610.),
        ("2B3c", 16, 11068.),
    ] {
        let xml = format!("<ml:real base=\"{base}\">{digits}</ml:real>");
        let q = value(&xml).unwrap().as_quantity().unwrap();
        assert_eq!(q.value.re, expected);
        assert_eq!(q.value.im, 0.);
        assert!(q.is_scalar());
    }
    let neg = "<ml:apply><ml:neg/><ml:real base=\"2\">1101</ml:real></ml:apply>";
    assert_eq!(value(neg).unwrap().as_quantity().unwrap().value.re, -13.);
}
#[test]
fn decimal_absent_or_explicit_ten_preserves_existing_contract() {
    for text in ["-0.000e+0", "+2.5", "1e3"] {
        let a = value(&format!("<ml:real>{text}</ml:real>")).unwrap();
        let b = value(&format!("<ml:real base=\"10\">{text}</ml:real>")).unwrap();
        assert_eq!(
            a.as_quantity().unwrap().value.re.to_bits(),
            b.as_quantity().unwrap().value.re.to_bits()
        );
    }
    assert_eq!(
        value("<ml:imag symbol=\"j\">2</ml:imag>")
            .unwrap()
            .as_quantity()
            .unwrap()
            .value
            .im,
        2.
    );
}
#[test]
fn unsupported_lexical_and_metadata_cases_never_fall_back_to_decimal() {
    for xml in [
        "<ml:real base=\"2\">2</ml:real>",
        "<ml:real base=\"8\">8</ml:real>",
        "<ml:real base=\"16\">g</ml:real>",
        "<ml:real base=\"2\">1.1</ml:real>",
        "<ml:real base=\"2\">-1</ml:real>",
        "<ml:real base=\"16\">+A</ml:real>",
        "<ml:real base=\"2\">1_0</ml:real>",
        "<ml:real base=\"2\">１</ml:real>",
        "<ml:real base=\"2\">1b</ml:real>",
        "<ml:real base=\"3\">10</ml:real>",
        "<ml:real base=\"02\">10</ml:real>",
        "<ml:real xmlns:f=\"urn:fake\" f:base=\"2\">10</ml:real>",
        "<ml:imag base=\"2\">10</ml:imag>",
        "<ml:imag base=\"10\">10</ml:imag>",
        "<ml:real base=\"2\" extra=\"x\">10</ml:real>",
        "<ml:real base=\"2\">1<!--hidden-->0</ml:real>",
        "<ml:real base=\"2\">1<?hidden?>0</ml:real>",
        "<ml:real base=\"16\">20000000000001</ml:real>",
    ] {
        assert!(value(xml).unwrap_err().unsupported, "{xml}");
    }
    assert_eq!(
        value("<ml:real base=\"16\">20000000000000</ml:real>")
            .unwrap()
            .as_quantity()
            .unwrap()
            .value
            .re,
        (1_u64 << 53) as f64
    );
}
#[test]
fn definition_function_and_program_keep_calculation_separate_from_native_lexeme() {
    let definition = "<ml:define><ml:id labels=\"VARIABLE\">a</ml:id><ml:real base=\"16\">000aB</ml:real></ml:define>";
    let function = "<ml:define><ml:function><ml:id labels=\"FUNCTION\">echo</ml:id><ml:boundVars><ml:id labels=\"VARIABLE\">v</ml:id></ml:boundVars></ml:function><ml:program><ml:return><ml:id labels=\"VARIABLE\">v</ml:id></ml:return></ml:program></ml:define>";
    let eval = "<ml:eval><ml:apply><ml:id labels=\"FUNCTION\">echo</ml:id><ml:id labels=\"VARIABLE\">a</ml:id></ml:apply><ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>";
    let source = editor_bridge::worksheet_xml(&[
        (1, 0., 0., definition.into()),
        (2, 0., 30., function.into()),
        (3, 0., 60., eval.into()),
    ])
    .unwrap();
    let mut calculator = WorksheetCalculator::default();
    calculator.update(&source, None).unwrap();
    let result = calculator.calculate(CalculationRequest::Worksheet).unwrap();
    assert!(
        result.rows.iter().all(|r| r.error.is_none()),
        "{:?}",
        result.rows
    );
    assert_eq!(
        result.rows[2]
            .value
            .as_ref()
            .unwrap()
            .as_quantity()
            .unwrap()
            .value
            .re,
        171.
    );
    assert_eq!(calculator.source.as_deref(), Some(source.as_str()));
    assert!(
        calculator
            .source
            .as_ref()
            .unwrap()
            .contains("base=\"16\">000aB")
    );
}
