//! Common path admission regressions, not timing/counter threshold tests.
use super::*;
use crate::{CalculationContext, Scope, Value};
use std::collections::HashMap;

fn matrix(prefix: &str, last: &str) -> String {
    format!(
        "<ml:matrix rows=\"2\" cols=\"3\">{prefix}<ml:real>1</ml:real><ml:real>4</ml:real><ml:real>2</ml:real><ml:real>5</ml:real><ml:real>3</ml:real>{last}</ml:matrix>"
    )
}
fn evaluate(source: &str) -> crate::Result<Value> {
    let xml = format!(
        "<ml:eval xmlns:ml=\"{ML}\">{source}<ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>"
    );
    CalculationContext {
        scope: Scope::default(),
        labels: HashMap::new(),
    }
    .evaluate_mathml(&xml)
}
fn row_major(source: &str) -> Vec<f64> {
    let Value::Matrix {
        rows,
        cols,
        elements,
    } = evaluate(source).unwrap()
    else {
        panic!("not full matrix")
    };
    assert_eq!((rows, cols), (2, 3));
    elements.iter().map(|q| q.value.re).collect()
}

fn assert_fault(source: &str, detail: &str) {
    let fault = evaluate(source).unwrap_err();
    assert_eq!(fault.code, "unsupported");
    assert_eq!(fault.detail, detail);
    assert!(fault.unsupported);
    assert_eq!(fault.previous_region, None);
}

#[test]
fn exact_count_nonfirst_display_is_rejected_by_existing_full_cell_parser() {
    // Replaces one actual cell: count is exactly six, so a count guard alone
    // cannot make these tests pass. The full parser must visit the final cell.
    for (last, detail) in [
        ("<ml:display/>", "MathML element display"),
        ("<ml:display size=\"1,1\"/>", "Unknown display metadata"),
        (
            "<ml:display xmlns:ml=\"urn:foreign\"/>",
            "Foreign expression namespace",
        ),
    ] {
        assert_fault(&matrix("", last), detail);
    }
    assert_fault(&matrix("", ""), "Invalid matrix size");
    // Likewise duplicate metadata cannot replace an offscreen cell after an
    // admitted first display. The strict helper retains its original contract.
    assert!(evaluate(&matrix("<ml:display size=\"1,1\"/>", "<ml:display/>")).is_err());
    assert_eq!(
        row_major(&matrix("", "<ml:real>6</ml:real>")),
        vec![1., 2., 3., 4., 5., 6.]
    );
}

#[test]
fn existing_root_namespace_attribute_and_dimension_lexical_contracts_are_not_widened() {
    let plain = matrix("", "<ml:real>6</ml:real>");
    let indexed = matrix("<ml:display size=\"1,1\"/>", "<ml:real>6</ml:real>");
    for source in [&plain, &indexed] {
        for extra in [
            "unknown=\"1\"",
            "xmlns:f=\"urn:f\" f:rows=\"2\"",
            "show-indices=\"true\"",
            "origin=\"0\"",
        ] {
            let changed = source.replace("rows=\"2\"", &format!("rows=\"2\" {extra}"));
            assert_fault(&changed, "Unknown matrix metadata");
        }
        let foreign = source.replace("<ml:matrix ", "<ml:matrix xmlns:ml=\"urn:foreign\" ");
        assert_fault(&foreign, "Foreign expression namespace");
        // Existing raw usize parser accepts these root lexemes. The new display
        // tuple parser is stricter, but this patch must not claim new root-XSD
        // lexical validation or silently change the established raw contract.
        for rows in ["02", "+2"] {
            assert_eq!(
                row_major(&source.replace("rows=\"2\"", &format!("rows=\"{rows}\""))),
                vec![1., 2., 3., 4., 5., 6.]
            );
        }
    }
}

#[test]
fn no_display_structural_tolerance_stays_legacy_but_new_display_profile_is_strict() {
    // children() historically filters only elements. This is a documented
    // existing raw-path limitation, not a new permissive metadata decision.
    for material in ["legacy text", "<!--legacy comment-->", "<?legacy data?>"] {
        let old = matrix(material, "<ml:real>6</ml:real>");
        assert_eq!(row_major(&old), vec![1., 2., 3., 4., 5., 6.]);
        let new = matrix(
            &format!("<ml:display size=\"1,1\"/>{material}"),
            "<ml:real>6</ml:real>",
        );
        assert!(
            evaluate(&new).is_err(),
            "display profile structural guard disappeared"
        );
    }
    // A hidden invalid expression is still parsed on both paths.
    for prefix in ["", "<ml:display size=\"1,1\"/>"] {
        assert!(evaluate(&matrix(prefix, "<ml:unsupported/>")).is_err());
    }
}

#[test]
fn ordinary_2500_cell_source_preserves_full_value_and_column_major_decoding() {
    let mut source = String::from("<ml:matrix rows=\"50\" cols=\"50\">");
    for col in 0..50 {
        for row in 0..50 {
            source.push_str(&format!("<ml:real>{}</ml:real>", row * 50 + col + 1));
        }
    }
    source.push_str("</ml:matrix>");
    assert!(!source.contains("display"));
    let Value::Matrix {
        rows,
        cols,
        elements,
    } = evaluate(&source).unwrap()
    else {
        panic!("missing full matrix")
    };
    assert_eq!((rows, cols, elements.len()), (50, 50, 2500));
    for (i, q) in elements.iter().enumerate() {
        assert_eq!(q.value.re, (i + 1) as f64);
    }
}
