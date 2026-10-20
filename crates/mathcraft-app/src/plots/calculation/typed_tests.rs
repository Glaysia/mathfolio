//! Typed source meaning reaches the evaluator without a text projection.
use super::*;
use mathcraft_document::canvas::{PlotAxis, PlotExpression, PlotScaling};
fn number(n: &str) -> PortableTree {
    tree(NodeKind::Number(n.into()), vec![])
}
fn id(name: &str, label: &str, suffix: Option<&str>) -> PortableTree {
    tree(
        NodeKind::NativeIdentifier {
            name: name.into(),
            labels: label.into(),
            contextual: Some(false),
            subscript: suffix.map(str::to_owned),
        },
        vec![],
    )
}
fn vector(values: Vec<PortableTree>) -> PortableTree {
    tree(
        NodeKind::Matrix {
            rows: values.len(),
            cols: 1,
            display: None,
        },
        values,
    )
}
fn ints(values: &[i32]) -> PortableTree {
    vector(values.iter().map(|n| number(&n.to_string())).collect())
}
fn pow(value: PortableTree) -> PortableTree {
    tree(NodeKind::Power, vec![value, number("2")])
}
fn spec(x: PortableTree, y: PortableTree) -> PlotSpec {
    let mut p = PlotSpec::typed_blank_xy();
    p.set_axis_expression(PlotAxis::X, 0, PlotExpression::from_tree(x).unwrap())
        .unwrap();
    p.set_axis_expression(PlotAxis::Y, 0, PlotExpression::from_tree(y).unwrap())
        .unwrap();
    p
}
fn define(name: PortableTree, value: PortableTree) -> String {
    editor_bridge::tree_to_mathml(&tree(NodeKind::Binary(BinaryOp::Define), vec![name, value]))
        .unwrap()
}
fn run(p: &PlotSpec, definitions: Vec<String>) -> PlotData {
    let before = serde_json::to_string(p).unwrap();
    let prior = definitions
        .into_iter()
        .enumerate()
        .map(|(i, s)| (i as u64 + 1, 0., i as f64, s))
        .collect::<Vec<_>>();
    let result = evaluate(p, &prior, 999, 0., 100.).unwrap();
    assert_eq!(serde_json::to_string(p).unwrap(), before);
    result
}
fn curve(data: &PlotData) -> &PlotTraceData {
    match &data.traces[0] {
        TraceOutcome::Ready(curve) => curve,
        other => panic!("{other:?}"),
    }
}
#[test]
fn concrete_typed_matrices_ranges_and_scalar_broadcast_use_direct_queries() {
    let p = spec(
        tree(
            NodeKind::Range { has_step: false },
            vec![number("1"), number("3")],
        ),
        ints(&[2, 4, 6]),
    );
    assert_eq!(
        curve(&run(&p, vec![])).points,
        vec![[1., 2.], [2., 4.], [3., 6.]]
    );
    let p = spec(number("9"), ints(&[2, 4, 6]));
    assert_eq!(
        curve(&run(&p, vec![])).points,
        vec![[9., 2.], [9., 4.], [9., 6.]]
    );
}
#[test]
fn physical_pointwise_query_preserves_literal_suffix_and_explicit_scales() {
    let x = id("x", "VARIABLE", Some("dc𝛼"));
    let physical = |n| {
        tree(
            NodeKind::Binary(BinaryOp::Multiply),
            vec![number(n), id("m", "UNIT", None)],
        )
    };
    let mut p = spec(x.clone(), pow(x.clone()));
    p.set_axis_scaling(
        PlotAxis::X,
        0,
        PlotScaling::from_tree(id("in", "UNIT", None)).unwrap(),
    )
    .unwrap();
    p.set_axis_scaling(
        PlotAxis::Y,
        0,
        PlotScaling::from_tree(pow(id("m", "UNIT", None))).unwrap(),
    )
    .unwrap();
    let data = run(
        &p,
        vec![
            define(x, vector(vec![physical("1"), physical("2"), physical("3")])),
            define(id("m", "VARIABLE", None), number("1000")),
        ],
    );
    let c = curve(&data);
    for (i, point) in c.points.iter().enumerate() {
        let n = (i + 1) as f64;
        assert!((point[0] - n / 0.0254).abs() < 1e-10);
        assert_eq!(point[1], n * n);
    }
    let presentation = c.presentation.as_ref().unwrap();
    assert!(presentation.x.as_ref().unwrap().residual_units().is_none());
    assert!(presentation.y.as_ref().unwrap().residual_units().is_none());
}
#[test]
fn forced_unit_and_variable_homonyms_do_not_share_sample_identity() {
    let variable = id("m", "VARIABLE", None);
    let p = spec(variable.clone(), id("m", "UNIT", None));
    let data = run(&p, vec![define(variable, ints(&[2, 3, 4]))]);
    assert_eq!(curve(&data).points, vec![[2., 1.], [3., 1.], [4., 1.]]);
    let unit_root = spec(id("m", "UNIT", None), id("m", "UNIT", None));
    assert_eq!(curve(&run(&unit_root, vec![])).points, vec![[1., 1.]]);
}
#[test]
fn formatted_identifiers_with_same_base_retain_separate_value_bindings() {
    let dc = id("x", "VARIABLE", Some("dc"));
    let ac = id("x", "VARIABLE", Some("ac"));
    let p = spec(dc.clone(), ac.clone());
    assert_eq!(
        curve(&run(
            &p,
            vec![define(dc, ints(&[1, 2])), define(ac, ints(&[10, 20]))]
        ))
        .points,
        vec![[1., 10.], [2., 20.]]
    );
}
#[test]
fn call_callee_and_captured_function_environment_are_never_rebound() {
    let x = id("x", "VARIABLE", None);
    let make_call = |name| {
        tree(
            NodeKind::Call { argument_count: 1 },
            vec![id(name, "FUNCTION", None), x.clone()],
        )
    };
    let function = "<ml:define><ml:function><ml:id labels='FUNCTION' label-is-contextual='false'>x</ml:id><ml:boundVars><ml:id labels='VARIABLE' label-is-contextual='false'>t</ml:id></ml:boundVars></ml:function><ml:apply><ml:plus/><ml:id labels='VARIABLE' label-is-contextual='false'>t</ml:id><ml:real>10</ml:real></ml:apply></ml:define>";
    let p = spec(x.clone(), make_call("x"));
    assert_eq!(
        curve(&run(
            &p,
            vec![define(x.clone(), ints(&[1, 2])), function.into()]
        ))
        .points,
        vec![[1., 11.], [2., 12.]]
    );
    let captured = "<ml:define><ml:function><ml:id labels='FUNCTION' label-is-contextual='false'>f</ml:id><ml:boundVars><ml:id labels='VARIABLE' label-is-contextual='false'>t</ml:id></ml:boundVars></ml:function><ml:apply><ml:plus/><ml:id labels='VARIABLE' label-is-contextual='false'>t</ml:id><ml:apply><ml:indexer/><ml:id labels='VARIABLE' label-is-contextual='false'>x</ml:id><ml:real>0</ml:real></ml:apply></ml:apply></ml:define>";
    let p = spec(x.clone(), make_call("f"));
    assert_eq!(
        curve(&run(&p, vec![define(x, ints(&[1, 2])), captured.into()])).points,
        vec![[1., 2.], [2., 3.]]
    );
}
#[test]
fn typed_implicit_policy_requires_resolved_variable_and_preserves_old_domain() {
    let x = id("plot_var_unbound", "VARIABLE", None);
    let p = spec(x.clone(), pow(x));
    let data = run(&p, vec![]);
    let c = curve(&data);
    assert!(c.implicit_domain);
    assert_eq!(c.points.len(), 500);
    assert_eq!(c.points[0], [-10., 100.]);
    assert_eq!(*c.points.last().unwrap(), [10., 100.]);
    let unit = spec(id("plot_var_unbound", "UNIT", None), number("2"));
    assert!(run(&unit, vec![]).has_failures());
    let mut physical = p;
    physical
        .set_axis_scaling(
            PlotAxis::X,
            0,
            PlotScaling::from_tree(id("m", "UNIT", None)).unwrap(),
        )
        .unwrap();
    assert!(run(&physical, vec![]).has_failures());
}
#[test]
fn mixed_legacy_typed_sources_and_nested_placeholders_keep_distinct_paths() {
    let p = spec(id("m", "VARIABLE", None), id("m", "UNIT", None));
    let mut mixed = p.clone();
    mixed.set_axis_expression_text(PlotAxis::X, 0, "m").unwrap();
    let defs = vec![define(id("m", "VARIABLE", None), ints(&[2, 3]))];
    assert_eq!(run(&p, defs.clone()), run(&mixed, defs));
    let p = spec(
        ints(&[1, 2]),
        tree(NodeKind::Group, vec![tree(NodeKind::Placeholder, vec![])]),
    );
    assert!(matches!(
        run(&p, vec![]).traces[0],
        TraceOutcome::Incomplete
    ));
}

#[test]
fn string_first_math_is_preserved_source_but_never_a_numeric_curve() {
    for value in ["", "12", "x", "1 + x", "한글"] {
        let literal = tree(NodeKind::StringLiteral(value.into()), vec![]);
        for (x, y, axis) in [
            (literal.clone(), ints(&[1, 2]), "x축"),
            (ints(&[1, 2]), literal, "y축"),
        ] {
            let p = spec(x, y);
            let data = run(&p, vec![]);
            let TraceOutcome::Failed(message) = &data.traces[0] else {
                panic!("string source must not become a curve: {:?}", data.traces);
            };
            assert!(message.contains(axis), "{message}");
            assert!(message.contains("수치 값"), "{message}");
            assert!(data.first_curve().is_none());
            assert!(!data.has_implicit_domain());
        }
    }
    // A prior definition is also a String value; neither its spelling nor its
    // numeric-looking contents permits a numeric conversion or implicit domain.
    let name = id("string_axis", "VARIABLE", None);
    let p = spec(name.clone(), ints(&[1, 2]));
    let data = run(
        &p,
        vec![define(
            name,
            tree(NodeKind::StringLiteral("42".into()), vec![]),
        )],
    );
    assert!(
        matches!(&data.traces[0], TraceOutcome::Failed(message) if message.contains("수치 값"))
    );
    assert!(data.first_curve().is_none());
}
