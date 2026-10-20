use super::*;
use mathcraft_editor::SourceMatrixDisplay;

fn matrix(display: &str) -> String {
    format!(
        "<ml:matrix xmlns:ml=\"{ML}\" rows=\"2\" cols=\"3\">{display}<ml:real>1</ml:real><ml:real>4</ml:real><ml:real>2</ml:real><ml:real>5</ml:real><ml:real>3</ml:real><ml:real>6</ml:real></ml:matrix>"
    )
}
fn prove_both(source: &PortableTree) -> String {
    source.validate().unwrap();
    let streamed = serialization_writer::serialize(source, true, 0, None).unwrap();
    assert_eq!(tree_to_mathml(source).unwrap(), streamed);
    let reference = serialize(source, true, 0).unwrap();
    assert_eq!(streamed, reference);
    assert_eq!(tree_from_mathml(&streamed).unwrap(), *source);
    streamed
}

#[test]
fn first_display_is_typed_but_every_column_major_cell_is_preserved() {
    let source = matrix("<ml:display offset=\"1,1\" size=\"1,2\"/>");
    let t = tree_from_mathml(&source).unwrap();
    assert_eq!(
        t.kind,
        NodeKind::Matrix {
            rows: 2,
            cols: 3,
            display: Some(SourceMatrixDisplay::from_parts(Some([1, 1]), Some([1, 2])).unwrap())
        }
    );
    assert_eq!(
        t.children
            .iter()
            .map(|c| match &c.kind {
                NodeKind::Number(text) => text.as_str(),
                _ => panic!("expected a complete numeric source cell"),
            })
            .collect::<Vec<_>>(),
        ["1", "2", "3", "4", "5", "6"]
    );
    assert_eq!(prove_both(&t), source);
    let json = serde_json::to_string(&t).unwrap();
    assert!(json.contains(r#""display":{"offset":[1,1],"size":[1,2]}"#));
    assert_eq!(serde_json::from_str::<PortableTree>(&json).unwrap(), t);
}

#[test]
fn absence_empty_display_offset_only_size_only_and_explicit_defaults_are_distinct() {
    for (display, effective) in [
        ("", ([0, 0], [2, 3])),
        ("<ml:display/>", ([0, 0], [2, 3])),
        ("<ml:display offset=\"1,1\"/>", ([1, 1], [1, 2])),
        ("<ml:display size=\"1,2\"/>", ([0, 0], [1, 2])),
        (
            "<ml:display offset=\"0,0\" size=\"2,3\"/>",
            ([0, 0], [2, 3]),
        ),
    ] {
        let raw = matrix(display);
        let t = tree_from_mathml(&raw).unwrap();
        let NodeKind::Matrix {
            display: metadata, ..
        } = t.kind
        else {
            unreachable!()
        };
        assert_eq!(metadata.is_some(), !display.is_empty());
        assert_eq!(
            metadata.unwrap_or_default().window([2, 3]).unwrap(),
            effective
        );
        assert_eq!(prove_both(&t), raw);
    }
}

#[test]
fn nested_and_sibling_metadata_stays_per_node_without_turning_it_into_result_format() {
    let a = tree_from_mathml(&matrix("<ml:display offset=\"1,1\"/>")).unwrap();
    let b = tree_from_mathml(&matrix("<ml:display size=\"2,1\"/>")).unwrap();
    let root = tree(
        NodeKind::Matrix {
            rows: 1,
            cols: 2,
            display: Some(SourceMatrixDisplay::default()),
        },
        vec![a.clone(), b.clone()],
    );
    let raw = prove_both(&root);
    assert_eq!(raw.matches("<ml:display").count(), 3);
    let restored = tree_from_mathml(&raw).unwrap();
    assert_eq!(restored.children, [a, b]);
    assert!(!raw.contains("origin="));
    assert!(!raw.contains("show-indices"));
    assert!(!raw.contains("resultRef="));
}

#[test]
fn closed_namespace_tuple_order_count_and_structure_reject_without_lossy_fallback() {
    for d in [
        "<ml:display size=\"0,1\"/>",
        "<ml:display offset=\"2,0\"/>",
        "<ml:display offset=\"1,1\" size=\"2,3\"/>",
        "<ml:display size=\"+1,1\"/>",
        "<ml:display size=\"1, 1\"/>",
        "<ml:display size=\"4294967296,1\"/>",
        "<ml:display size=\"１,1\"/>",
        "<ml:display show-indices=\"true\"/>",
        "<ml:display origin=\"0\"/>",
        "<ml:display xmlns:f=\"urn:f\" f:size=\"1,1\"/>",
        "<display xmlns=\"urn:f\" size=\"1,1\"/>",
        "<ml:display><!--x--></ml:display>",
        "<ml:display><?x y?></ml:display>",
        "<ml:display>bad</ml:display>",
        "<ml:display><ml:real>1</ml:real></ml:display>",
        "<ml:display/><ml:display/>",
    ] {
        assert!(tree_from_mathml(&matrix(d)).is_err(), "{d}");
    }
    for raw in [
        matrix("").replace("<ml:real>4</ml:real>", "<ml:display/>"),
        matrix("<ml:display/>").replace("<ml:real>6</ml:real>", ""),
        matrix("<ml:display/>").replace("<ml:real>6</ml:real>", "<!--x--><ml:real>6</ml:real>"),
        matrix("<ml:display/>").replace("<ml:real>6</ml:real>", "<ml:unsupported/>"),
    ] {
        assert!(tree_from_mathml(&raw).is_err(), "{raw}");
    }
}

#[test]
fn typed_to_raw_calculation_uses_full_value_and_hidden_expression_errors() {
    let source = tree_from_mathml(&matrix("<ml:display size=\"1,1\"/>")).unwrap();
    let xml = prove_both(&source);
    let evaluate = |fragment: String| {
        crate::CalculationContext { scope: crate::Scope::default(), labels: HashMap::new() }
            .evaluate_mathml(&format!("<ml:eval xmlns:ml=\"{ML}\">{fragment}<ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>"))
    };
    let value = evaluate(xml).unwrap();
    let crate::Value::Matrix {
        rows,
        cols,
        elements,
    } = value
    else {
        panic!("not a matrix")
    };
    assert_eq!((rows, cols, elements.len()), (2, 3, 6));
    assert_eq!(
        elements.iter().map(|q| q.value.re).collect::<Vec<_>>(),
        [1., 2., 3., 4., 5., 6.]
    );
    for replacement in [
        "<ml:id labels=\"VARIABLE\">missing_hidden</ml:id>",
        "<ml:apply><ml:plus/><ml:real>6</ml:real><ml:id labels=\"UNIT\">m</ml:id></ml:apply>",
    ] {
        let broken =
            matrix("<ml:display size=\"1,1\"/>").replace("<ml:real>6</ml:real>", replacement);
        let parsed = tree_from_mathml(&broken).unwrap();
        assert!(evaluate(prove_both(&parsed)).is_err());
    }
}

#[test]
fn unsupported_hidden_tree_is_never_skipped_and_explicit_labels_survive_worksheet_projection() {
    let raw = matrix("<ml:display size=\"1,1\"/>");
    assert!(
        tree_from_mathml(&raw.replace(
            "<ml:real>6</ml:real>",
            "<ml:apply><ml:plus/><ml:real>1</ml:real></ml:apply>"
        ))
        .is_err()
    );
    let mut t = tree_from_mathml(&raw).unwrap();
    t.children[5].kind = NodeKind::NativeIdentifier {
        name: "x".into(),
        labels: "VARIABLE".into(),
        contextual: Some(false),
        subscript: Some("n".into()),
    };
    let output = prove_both(&t);
    let evaluation = format!(
        "<ml:eval xmlns:ml=\"{ML}\">{output}<ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>"
    );
    let expected = tree_from_mathml(&evaluation).unwrap();
    let labelled = label_fragment(&evaluation, &mut HashMap::new()).unwrap();
    assert_eq!(tree_from_mathml(&labelled).unwrap(), expected);
    let worksheet = worksheet_xml(&[(9, 0., 0., evaluation)]).unwrap();
    assert!(worksheet.contains("<ml:display size=\"1,1\"/>"));
}
