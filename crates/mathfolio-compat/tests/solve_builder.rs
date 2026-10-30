use mathfolio_compat::{
    Value, evaluate_worksheet,
    solve_builder::{SolveBlockBuilder, SolveMethod},
};
use mathfolio_editor::{BinaryOp, NodeKind, PortableTree};

fn tree(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
    PortableTree { kind, children }
}

fn id(name: &str) -> PortableTree {
    tree(
        NodeKind::AnnotatedIdentifier {
            name: name.into(),
            labels: "VARIABLE".into(),
        },
        vec![],
    )
}

fn number(value: &str) -> PortableTree {
    tree(NodeKind::Number(value.into()), vec![])
}

fn binary(operation: BinaryOp, left: PortableTree, right: PortableTree) -> PortableTree {
    tree(NodeKind::Binary(operation), vec![left, right])
}

fn values(value: &Value) -> Vec<f64> {
    match value {
        Value::Scalar { quantity } => vec![quantity.value.re],
        Value::Matrix {
            cols: 1, elements, ..
        } => elements.iter().map(|value| value.value.re).collect(),
        other => panic!("Expected scalar or column result, got {other:?}"),
    }
}

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 0.001, "{actual} != {expected}");
}

#[test]
fn builder_emits_native_find_structure_that_executes_through_the_worksheet_path() {
    let mut builder = SolveBlockBuilder::new(SolveMethod::Find);
    builder.guess("x", number("1")).unwrap();
    builder.guess("y", number("1")).unwrap();
    builder
        .equality(binary(BinaryOp::Add, id("x"), id("y")), number("3"))
        .unwrap();
    builder
        .equality(binary(BinaryOp::Subtract, id("x"), id("y")), number("1"))
        .unwrap();
    builder.solve_for(["x", "y"]).unwrap();
    builder.export_as("answer").unwrap();
    let model = builder.build().unwrap();

    assert_eq!(model.method(), SolveMethod::Find);
    assert_eq!(model.variables(), &["x", "y"]);
    assert_eq!(model.output(), Some("answer"));
    assert_eq!(model.result_ref(), "solve-result");
    let block = model.to_solveblock_xml().unwrap();
    let document = roxmltree::Document::parse(&block).unwrap();
    let root = document.root_element();
    assert_eq!(root.tag_name().name(), "solveblock");
    let categories = root
        .descendants()
        .filter_map(|node| node.attribute("solve-block-category"))
        .collect::<Vec<_>>();
    assert_eq!(
        categories,
        [
            "guess-value",
            "guess-value",
            "constraint",
            "constraint",
            "solver"
        ]
    );
    assert!(document.descendants().any(|node| {
        node.tag_name().name() == "id"
            && node.attribute("labels") == Some("KEYWORD")
            && node.text() == Some("Find")
    }));
    assert!(block.contains("resultRef=\"solve-result\""));

    let rows = evaluate_worksheet(&model.to_worksheet_xml(7, 10., 20.).unwrap()).unwrap();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].error.is_none(), "{:?}", rows[0].error);
    let solution = values(rows[0].value.as_ref().unwrap());
    close(solution[0], 2.);
    close(solution[1], 1.);
    assert_eq!(rows[0].result_ref.as_deref(), Some("solve-result"));
}

#[test]
fn builder_emits_minerr_and_preserves_solver_variable_order() {
    let mut builder = SolveBlockBuilder::new(SolveMethod::Minerr);
    builder.guess("x", number("0")).unwrap();
    builder.equality(id("x"), number("1")).unwrap();
    builder.equality(id("x"), number("3")).unwrap();
    builder.solve_for(["x"]).unwrap();
    let model = builder.build().unwrap();
    let xml = model.to_worksheet_xml(1, 0., 0.).unwrap();
    let document = roxmltree::Document::parse(&xml).unwrap();
    assert!(document.descendants().any(|node| {
        node.tag_name().name() == "id"
            && node.attribute("labels") == Some("KEYWORD")
            && node.text() == Some("Minerr")
    }));
    let rows = evaluate_worksheet(&xml).unwrap();
    assert!(rows[0].error.is_none(), "{:?}", rows[0].error);
    close(values(rows[0].value.as_ref().unwrap())[0], 2.);
}

#[test]
fn builder_rejects_incomplete_or_ambiguous_models_and_escapes_identifiers() {
    let mut placeholder = SolveBlockBuilder::new(SolveMethod::Find);
    assert!(
        placeholder
            .guess("x", tree(NodeKind::Placeholder, vec![]))
            .is_err()
    );

    let mut missing = SolveBlockBuilder::new(SolveMethod::Find);
    missing.guess("x", number("0")).unwrap();
    missing.solve_for(["x", "y"]).unwrap();
    missing.equality(id("x"), number("1")).unwrap();
    missing.equality(id("y"), number("2")).unwrap();
    assert!(missing.build().is_err());

    let mut nonsquare = SolveBlockBuilder::new(SolveMethod::Find);
    nonsquare.guess("x", number("0")).unwrap();
    nonsquare.equality(id("x"), number("1")).unwrap();
    nonsquare.equality(id("x"), number("2")).unwrap();
    nonsquare.solve_for(["x"]).unwrap();
    assert!(nonsquare.build().is_err());

    let unusual = "x\"><injected attr='1'";
    let mut escaped = SolveBlockBuilder::new(SolveMethod::Find);
    escaped.guess(unusual, number("0")).unwrap();
    escaped.equality(id(unusual), number("2")).unwrap();
    escaped.solve_for([unusual]).unwrap();
    let model = escaped.build().unwrap();
    let xml = model.to_solveblock_xml().unwrap();
    assert!(!xml.contains("<injected"));
    assert!(xml.contains("&lt;injected"));
    let document = roxmltree::Document::parse(&xml).unwrap();
    assert!(
        document
            .descendants()
            .any(|node| node.text() == Some(unusual))
    );
    assert!(model.to_worksheet_xml(1, f64::NAN, 0.).is_err());
}
