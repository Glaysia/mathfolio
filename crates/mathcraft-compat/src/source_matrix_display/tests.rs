//! Unexecuted next-cohort regressions. No original runtime/GUI is used.
use super::*;
use crate::{CalculationContext, Scope, Value, editor_bridge};
use std::collections::HashMap;

fn matrix(display: &str) -> String {
    // Distinct2x3 data is COLUMN-major in source; expected row-major1..6.
    format!(
        "<ml:matrix rows=\"2\" cols=\"3\">{display}<ml:real>1</ml:real><ml:real>4</ml:real><ml:real>2</ml:real><ml:real>5</ml:real><ml:real>3</ml:real><ml:real>6</ml:real></ml:matrix>"
    )
}
fn eval(source: &str) -> String {
    format!(
        "<ml:eval xmlns:ml=\"{ML}\">{source}<ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>"
    )
}
fn value(source: &str) -> crate::Result<Value> {
    CalculationContext {
        scope: Scope::default(),
        labels: HashMap::new(),
    }
    .evaluate_mathml(&eval(source))
}
fn scalars(value: &Value) -> (usize, usize, Vec<f64>) {
    let Value::Matrix {
        rows,
        cols,
        elements,
    } = value
    else {
        panic!("not a full matrix: {value:?}")
    };
    (*rows, *cols, elements.iter().map(|v| v.value.re).collect())
}
fn display(fragment: &str, shape: [usize; 2]) -> Result<SourceMatrixDisplay, String> {
    let wrapped = format!("<context xmlns:ml=\"{ML}\">{fragment}</context>");
    let doc = roxmltree::Document::parse(&wrapped).map_err(|e| e.to_string())?;
    read_display(doc.root_element().first_element_child().unwrap(), shape)
}

#[test]
fn first_display_validates_but_full_six_cells_and_column_major_order_are_unchanged() {
    for d in [
        "<ml:display size=\"1,1\"/>",
        "<ml:display offset=\"1,1\" size=\"1,2\"/>",
    ] {
        let raw = matrix(d);
        let before = raw.clone();
        let full = value(&raw).unwrap();
        assert_eq!(scalars(&full), (2, 3, vec![1., 2., 3., 4., 5., 6.]));
        assert_eq!(scalars(&full), scalars(&value(&matrix("")).unwrap()));
        assert_eq!(raw, before); // Borrowed source is never rewritten.
    }
}

#[test]
fn raw_worksheet_label_pipeline_and_typed_editor_keep_identical_complete_source() {
    let fragment = eval(&matrix("<ml:display offset=\"1,1\" size=\"1,2\"/>"));
    let before = fragment.clone();
    let typed = editor_bridge::tree_from_mathml(&fragment).unwrap();
    assert_eq!(editor_bridge::tree_to_mathml(&typed).unwrap(), fragment);
    let ws = editor_bridge::worksheet_xml(&[(41, 0., 0., fragment.clone())]).unwrap();
    assert!(ws.contains("<ml:display offset=\"1,1\" size=\"1,2\"/>"));
    let rows = crate::evaluate_worksheet(&ws).unwrap();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].error.is_none(), "{:?}", rows[0].error);
    assert_eq!(
        scalars(rows[0].value.as_ref().unwrap()),
        (2, 3, vec![1., 2., 3., 4., 5., 6.])
    );
    assert_eq!(fragment, before);
    // Both App source projections now admit the strict typed metadata while
    // the raw statement route continues to evaluate every source cell.
    let generated = editor_bridge::WorksheetXmlCache::default()
        .build_prepared_with_trees(&[(41, 0., 0., fragment)])
        .unwrap();
    let (source, _, trees) = generated.into_parts_with_trees();
    assert_eq!(trees[0].as_ref().unwrap().as_ref().unwrap(), &typed);
    assert!(source.contains("<ml:display"));
    assert!(
        crate::evaluate_worksheet(&source).unwrap()[0]
            .error
            .is_none()
    );
}

#[test]
fn offset_only_size_only_empty_and_explicit_defaults_follow_closed_source_semantics() {
    for (d, expected) in [
        ("<ml:display/>", ([0, 0], [2, 3])),
        ("<ml:display offset=\"1,1\"/>", ([1, 1], [1, 2])),
        ("<ml:display size=\"1,2\"/>", ([0, 0], [1, 2])),
        (
            "<ml:display offset=\"0,0\" size=\"2,3\"/>",
            ([0, 0], [2, 3]),
        ),
        (
            "<ml:display offset=\"01,001\" size=\"01,02\"/>",
            ([1, 1], [1, 2]),
        ),
    ] {
        assert_eq!(
            display(d, [2, 3]).unwrap().window([2, 3]).unwrap(),
            expected
        );
        assert_eq!(
            scalars(&value(&matrix(d)).unwrap()).2,
            vec![1., 2., 3., 4., 5., 6.]
        );
    }
}

#[test]
fn unsupported_pairs_zero_overhang_and_overflow_never_become_default_whole_matrix() {
    for attrs in [
        "size=\"0,0\"",
        "size=\"0,1\"",
        "size=\"1,0\"",
        "size=\"3,1\"",
        "offset=\"2,0\"",
        "offset=\"0,3\"",
        "offset=\"1,1\" size=\"2,3\"",
        "offset=\"-1,0\"",
        "size=\"+1,1\"",
        "size=\"1, 1\"",
        "size=\"１,1\"",
        "size=\"1\"",
        "size=\"1,1,1\"",
        "size=\",1\"",
        "size=\"1,\"",
        "size=\"4294967296,1\"",
        "offset=\"18446744073709551616,0\"",
    ] {
        let d = format!("<ml:display {attrs}/>");
        assert!(display(&d, [2, 3]).is_err(), "{attrs}");
        assert!(value(&matrix(&d)).is_err(), "{attrs}");
    }
}

#[test]
fn namespaces_attributes_and_structural_content_are_closed() {
    for d in [
        "<ml:display show-indices=\"true\"/>",
        "<ml:display origin=\"1\"/>",
        "<ml:display resultRef=\"7\"/>",
        "<ml:display unit=\"m\"/>",
        "<ml:display xmlns:f=\"urn:fake\" f:size=\"1,1\"/>",
        "<display xmlns=\"urn:fake\" size=\"1,1\"/>",
        "<ml:display><ml:placeholder/></ml:display>",
        "<ml:display>garbage</ml:display>",
        "<ml:display><!--x--></ml:display>",
        "<ml:display><?x y?></ml:display>",
    ] {
        assert!(value(&matrix(d)).is_err(), "{d}");
    }
    // Duplicate XML attrs are rejected by XML admission before this parser.
    assert!(value(&matrix("<ml:display size=\"1,1\" size=\"1,2\"/>")).is_err());
}

#[test]
fn display_must_be_first_unique_and_not_count_as_a_cell() {
    let d = "<ml:display size=\"1,1\"/>";
    for source in [
        matrix(&format!("{d}{d}")),
        matrix("").replace("<ml:real>4</ml:real>", &format!("{d}<ml:real>4</ml:real>")),
        matrix(d).replace("<ml:real>6</ml:real>", ""),
        matrix(d).replace("</ml:matrix>", "<ml:real>7</ml:real></ml:matrix>"),
        matrix(d).replace(d, &format!("{d}arbitrary text")),
        matrix(d).replace(d, &format!("{d}<!--not a cell-->")),
    ] {
        assert!(value(&source).is_err(), "{source}");
    }
}

#[test]
fn offscreen_invalid_expression_and_unbound_name_still_fail_full_calculation() {
    let source = matrix("<ml:display size=\"1,1\"/>");
    for last in [
        "<ml:unsupported/>",
        "<ml:real>NaN</ml:real>",
        "<ml:id labels=\"VARIABLE\">missing</ml:id>",
    ] {
        let broken = source.replace("<ml:real>6</ml:real>", last);
        assert!(
            value(&broken).is_err(),
            "hidden source cell was skipped: {last}"
        );
    }
}

#[test]
fn sibling_and_nested_source_matrices_keep_independent_metadata_without_losing_data() {
    let a = matrix("<ml:display offset=\"1,1\" size=\"1,1\"/>");
    let b = matrix("<ml:display size=\"2,1\"/>");
    let outer =
        format!("<ml:matrix rows=\"1\" cols=\"2\"><ml:display size=\"1,1\"/>{a}{b}</ml:matrix>");
    let wrapped = format!("<context xmlns:ml=\"{ML}\">{outer}</context>");
    let doc = roxmltree::Document::parse(&wrapped).unwrap();
    let parsed =
        crate::parse_expression(doc.root_element().first_element_child().unwrap(), 0).unwrap();
    let crate::Expression::Matrix {
        rows,
        cols,
        elements,
    } = parsed
    else {
        panic!("missing outer source")
    };
    assert_eq!((rows, cols, elements.len()), (1, 2, 2));
    for element in elements {
        let crate::Expression::Matrix {
            rows,
            cols,
            elements,
        } = element
        else {
            panic!("missing nested source")
        };
        assert_eq!((rows, cols, elements.len()), (2, 3, 6));
    }
    // Existing runtime matrices hold scalar quantities only. This patch must
    // not pretend nested numeric Value support was added by metadata parsing.
    assert!(value(&outer).is_err());
}

#[test]
fn legacy_editor_json_and_public_writer_remain_exact_without_display() {
    let tree = editor_bridge::tree_from_mathml(&matrix("")).unwrap();
    let json = serde_json::to_string(&tree).unwrap();
    assert!(json.starts_with("{\"kind\":{\"Matrix\":{\"rows\":2,\"cols\":3}},\"children\":"));
    assert!(!json.contains("display"));
    let raw = editor_bridge::tree_to_mathml(&tree).unwrap();
    assert_eq!(
        raw,
        format!(
            "<ml:matrix xmlns:ml=\"{ML}\" rows=\"2\" cols=\"3\"><ml:real>1</ml:real><ml:real>4</ml:real><ml:real>2</ml:real><ml:real>5</ml:real><ml:real>3</ml:real><ml:real>6</ml:real></ml:matrix>"
        )
    );
    let eval_tree = editor_bridge::tree_from_mathml(&eval(&matrix(""))).unwrap();
    assert!(
        editor_bridge::TypedMathml::new(&eval_tree)
            .unwrap()
            .source()
            .contains("<ml:matrix rows=\"2\" cols=\"3\">")
    );
}

#[test]
fn shape_budget_and_zero_dimensions_are_not_relaxed_by_display() {
    for (rows, cols) in [(0usize, 1usize), (1, 0), (usize::MAX, 2), (1, 16_385)] {
        assert!(display("<ml:display size=\"1,1\"/>", [rows, cols]).is_err());
    }
    assert!(
        value(&matrix("<ml:display size=\"1,1\"/>").replace("rows=\"2\"", "rows=\"0\"")).is_err()
    );
}
