//! STRING is source-valid in an expression-only editor, not a numeric curve.
use super::*;
use crate::canvas::{PlotAxis, PlotSpec};

fn string(text: &str) -> PortableTree {
    PortableTree {
        kind: NodeKind::StringLiteral(text.into()),
        children: vec![],
    }
}

#[test]
fn string_first_math_json_and_native_roundtrip_preserve_empty_and_raw_contents() {
    for text in ["", "123", " a + (b)=\" ", "\r\n\t😀&<>\\"] {
        let source = string(text);
        let expression = PlotExpression::from_tree(source.clone()).unwrap();
        let mut spec = PlotSpec::typed_blank_xy();
        spec.set_axis_expression(PlotAxis::Y, 0, expression)
            .unwrap();
        let json = serde_json::to_string(&spec).unwrap();
        let restored: PlotSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, spec);
        assert_eq!(restored.primary_y_expression().as_tree(), Some(&source));
        let xml = crate::native_plot::payload(&spec).unwrap();
        assert!(
            xml.contains("<ml:str") && xml.contains("xml:space=\"preserve\""),
            "{xml}"
        );
        if text.contains('\r') {
            assert!(xml.contains("&#13;"));
        }
        // Owning requested dimensions are separate source metadata, not part
        // of payload(). Preserve them rather than inventing a graph-size.
        let geometry = spec.native_geometry.as_ref().unwrap();
        let owner = format!(
            "<region xmlns=\"http://schemas.mathsoft.com/worksheet50\" width=\"{}\" height=\"{}\">{xml}</region>",
            geometry.requested_width.unwrap(),
            geometry.requested_height.unwrap()
        );
        let parsed = crate::native_plot::parse(&owner).unwrap();
        assert_eq!(parsed.primary_y_expression().as_tree(), Some(&source));
        assert!(crate::native_plot::same_source(&spec, &parsed).unwrap());
        assert_eq!(crate::native_plot::payload(&parsed).unwrap(), xml);
    }
}

#[test]
fn new_string_admission_keeps_constructor_xml_and_aggregate_receipt_budgets() {
    assert!(PlotExpression::from_tree(string("\0")).is_err());
    assert!(
        PlotExpression::from_tree(string(&"x".repeat(mathcraft_editor::MAX_ATOM_BYTES + 1)))
            .is_err()
    );
    // XML escaping expands this valid raw atom beyond the per-expression bound.
    assert_eq!(
        PlotExpression::from_tree(string(&"&".repeat(16_000))).unwrap_err(),
        "Plot expression serialized budget exceeded"
    );
    let value = PlotExpression::from_tree(string(&"x".repeat(16_000))).unwrap();
    assert!(expression_budget(std::iter::repeat_n(&value, 16)).is_ok());
    assert!(expression_budget(std::iter::repeat_n(&value, 17)).is_err());
    let value = PlotExpression::from_tree(string("")).unwrap();
    let wire = serde_json::to_string(&value).unwrap();
    assert!(
        serde_json::from_str::<PlotExpression>(&wire.replacen('{', "{\"tree\":null,", 1)).is_err()
    );
    assert_eq!(value.as_legacy_text(), None);
    assert!(value.is_complete());
    assert!(!value.is_exact_placeholder());
}
