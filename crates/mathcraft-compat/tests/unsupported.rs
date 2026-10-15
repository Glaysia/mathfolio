//! Unsupported input must not quietly restore an old valid binding.
use mathcraft_compat::evaluate_worksheet;

const ID_A: &str = r#"<id labels="VARIABLE">a</id>"#;
fn sheet(expressions: &[String]) -> String {
    let regions: String = expressions
        .iter()
        .enumerate()
        .map(|(i, e)| {
            format!(
                r#"<region region-id="{i}" left="20" top="{}"><math>{e}</math></region>"#,
                i * 65
            )
        })
        .collect();
    format!(
        r#"<worksheet xmlns="http://schemas.mathsoft.com/worksheet50"><regions xmlns:ml="http://schemas.mathsoft.com/math50">{}</regions></worksheet>"#,
        // Each expression declares its own default MathML namespace. Region
        // wrappers remain worksheet50 regardless of the expressions' prefixes.
        regions
    )
}
fn math(source: String) -> String {
    let offset = source.find('>').unwrap();
    format!(
        "{} xmlns=\"http://schemas.mathsoft.com/math50\"{}",
        &source[..offset],
        &source[offset..]
    )
}
#[test]
fn unsupported_redefinition_poisons_dependent_values() {
    let xml = sheet(&[
        math(format!("<define>{ID_A}<real>1</real></define>")),
        math(format!("<define>{ID_A}<unsupported-operator/></define>")),
        math(format!(
            "<eval>{ID_A}<unitOverride><placeholder/></unitOverride></eval>"
        )),
    ]);
    let rows = evaluate_worksheet(&xml).unwrap();
    assert!(rows[0].error.is_none());
    assert!(rows[1].error.as_ref().unwrap().unsupported);
    assert!(rows[2].value.is_none());
    assert_eq!(
        rows[2].error.as_ref().unwrap().previous_region.as_deref(),
        Some("1")
    );
}
#[test]
fn duplicate_globals_poison_both_definitions_and_reads() {
    let xml = sheet(&[
        math(format!("<globalDefine>{ID_A}<real>1</real></globalDefine>")),
        math(format!("<globalDefine>{ID_A}<real>2</real></globalDefine>")),
        math(format!(
            "<eval>{ID_A}<unitOverride><placeholder/></unitOverride></eval>"
        )),
    ]);
    let rows = evaluate_worksheet(&xml).unwrap();
    assert!(
        rows.iter()
            .all(|r| r.error.as_ref().unwrap().unsupported && r.value.is_none())
    );
}
#[test]
fn fractional_imaginary_or_dimensional_system_origin_never_truncates() {
    for origin in [
        "<real>0.5</real>",
        "<imag>1</imag>",
        r#"<apply><scale/><real>1</real><id labels="UNIT">m</id></apply>"#,
    ] {
        let xml = sheet(&[
            math(format!(r#"<define><id labels="SYSTEM">ORIGIN</id>{origin}</define>"#)),
            math(r#"<define><id labels="VARIABLE">v</id><matrix rows="2" cols="1"><real>10</real><real>20</real></matrix></define>"#.into()),
            math(r#"<eval><apply><indexer/><id labels="VARIABLE">v</id><real>1</real></apply><unitOverride><placeholder/></unitOverride></eval>"#.into()),
        ]);
        let rows = evaluate_worksheet(&xml).unwrap();
        assert!(rows[2].value.is_none(), "{origin}");
        assert!(rows[2].error.as_ref().unwrap().unsupported);
    }
}
#[test]
fn wrong_namespace_is_never_a_supported_expression() {
    let xml = sheet(&[math(r#"<eval><real xmlns="urn:not-mathml">5</real><unitOverride><placeholder/></unitOverride></eval>"#.into())]);
    let rows = evaluate_worksheet(&xml).unwrap();
    assert!(rows[0].error.as_ref().unwrap().unsupported);
}

#[test]
fn display_override_does_not_change_physical_value_or_later_reads() {
    let xml = sheet(&[
        math(format!(
            r#"<define>{ID_A}<apply><scale/><real>2</real><id labels="UNIT">m</id></apply></define>"#
        )),
        math(format!(
            r#"<eval>{ID_A}<unitOverride><id labels="UNIT">cm</id></unitOverride></eval>"#
        )),
        math(format!(
            r#"<eval>{ID_A}<unitOverride><id labels="UNIT">m</id></unitOverride></eval>"#
        )),
    ]);
    let rows = evaluate_worksheet(&xml).unwrap();
    for row in &rows[1..] {
        let mathcraft_compat::Value::Scalar { quantity } = row.value.as_ref().unwrap() else {
            panic!()
        };
        assert_eq!(quantity.value.re, 2.);
        assert_eq!(quantity.dimension, [1., 0., 0., 0., 0., 0., 0.]);
    }
    assert_eq!(
        rows[1].display_value.as_ref().unwrap().numeric_json()["real"],
        200.
    );
    assert_eq!(
        rows[2].display_value.as_ref().unwrap().numeric_json()["real"],
        2.
    );
}
