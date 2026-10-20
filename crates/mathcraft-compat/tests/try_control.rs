//! Native Try structure and independently bounded numerical/error semantics.
use mathcraft_compat::{
    Value,
    editor_bridge::{tree_from_mathml, tree_to_mathml, worksheet_xml},
    evaluate_worksheet,
};
use mathcraft_editor::{BinaryOp, Editor, NodeKind, PortableTree};
const ML: &str = "http://schemas.mathsoft.com/math50";
fn node(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
    PortableTree { kind, children }
}
fn number(value: i32) -> PortableTree {
    node(NodeKind::Number(value.to_string()), vec![])
}
fn name(value: &str) -> PortableTree {
    node(NodeKind::Identifier(value.into()), vec![])
}
fn binary(op: BinaryOp, left: PortableTree, right: PortableTree) -> PortableTree {
    node(NodeKind::Binary(op), vec![left, right])
}
fn program(lines: Vec<PortableTree>) -> PortableTree {
    node(
        NodeKind::Program {
            statement_count: lines.len(),
        },
        lines,
    )
}
fn attempt(first: Vec<PortableTree>, second: Vec<PortableTree>) -> PortableTree {
    node(NodeKind::Try, vec![program(first), program(second)])
}
fn divide_zero() -> PortableTree {
    binary(BinaryOp::InlineDivide, number(1), number(0))
}
fn assign(n: &str, value: PortableTree) -> PortableTree {
    node(NodeKind::LocalDefine, vec![name(n), value])
}
fn evaluation(value: PortableTree) -> PortableTree {
    binary(
        BinaryOp::Evaluate,
        value,
        node(NodeKind::Placeholder, vec![]),
    )
}
fn row(value: PortableTree) -> mathcraft_compat::RegionEvaluation {
    let tree = evaluation(value);
    let xml = tree_to_mathml(&tree).unwrap();
    assert_eq!(tree_from_mathml(&xml).unwrap(), tree);
    evaluate_worksheet(&worksheet_xml(&[(1, 0., 0., xml)]).unwrap())
        .unwrap()
        .remove(0)
}
fn scalar(value: PortableTree, expected: f64) {
    let result = row(value);
    assert!(result.error.is_none(), "{result:?}");
    let Some(Value::Scalar { quantity }) = result.value else {
        panic!("{result:?}")
    };
    assert_eq!(quantity.value.re, expected);
    assert_eq!(quantity.value.im, 0.);
}

#[test]
fn ctrl_j_try_and_canonical_native_order_roundtrip_two_independent_bodies() {
    let mut editor = Editor::from_portable(&name("try")).unwrap();
    assert!(editor.convert_program_keyword());
    let mut tree = editor.to_portable().unwrap();
    assert_eq!(tree.kind, NodeKind::Try);
    tree.children[0] = program(vec![assign("m", number(19)), name("m")]);
    tree.children[1] = program(vec![assign("m", number(43)), name("m")]);
    let xml = tree_to_mathml(&tree).unwrap();
    assert_eq!(tree_from_mathml(&xml).unwrap(), tree);
    assert!(xml.find("19").unwrap() < xml.find("43").unwrap());
    scalar(tree, 19.);
}

#[test]
fn scalar_native_bodies_normalize_to_body_programs_without_reordering_or_flattening() {
    let xml = format!(
        "<ml:tryCatch xmlns:ml='{ML}'><ml:real>7</ml:real><ml:program><ml:program><ml:real>11</ml:real></ml:program></ml:program></ml:tryCatch>"
    );
    let tree = tree_from_mathml(&xml).unwrap();
    assert_eq!(tree.children[0], program(vec![number(7)]));
    assert_eq!(tree.children[1], program(vec![program(vec![number(11)])]));
    let canonical = tree_to_mathml(&tree).unwrap();
    assert_eq!(tree_from_mathml(&canonical).unwrap(), tree);
    assert!(canonical.contains("<ml:program><ml:program>"));
}

#[test]
fn successful_try_is_lazy_and_documented_divide_by_zero_runs_on_error() {
    scalar(attempt(vec![number(17)], vec![divide_zero()]), 17.);
    scalar(attempt(vec![divide_zero()], vec![number(43)]), 43.);
    scalar(
        program(vec![attempt(vec![number(19)], vec![divide_zero()])]),
        19.,
    );
    scalar(
        program(vec![attempt(vec![divide_zero()], vec![number(47)])]),
        47.,
    );
}

#[test]
fn handler_failure_is_not_recaught_by_itself_and_an_outer_try_can_handle_it() {
    let nested = attempt(vec![divide_zero()], vec![divide_zero()]);
    let failed = row(nested.clone()).error.unwrap();
    assert_eq!(failed.code, "divide_by_zero");
    assert!(!failed.unsupported);
    scalar(attempt(vec![nested], vec![number(53)]), 53.);
}

#[test]
fn handler_loop_control_and_return_propagate_without_becoming_calculation_errors() {
    for (control, expected) in [(NodeKind::Continue, 12.), (NodeKind::Break, 3.)] {
        let reciprocal = binary(
            BinaryOp::InlineDivide,
            number(1),
            binary(BinaryOp::Subtract, name("i"), number(3)),
        );
        let body = program(vec![
            attempt(vec![reciprocal], vec![node(control, vec![])]),
            assign("total", binary(BinaryOp::Add, name("total"), name("i"))),
        ]);
        scalar(
            program(vec![
                assign("total", number(0)),
                node(
                    NodeKind::For,
                    vec![
                        name("i"),
                        node(
                            NodeKind::Range { has_step: false },
                            vec![number(1), number(5)],
                        ),
                        body,
                    ],
                ),
                name("total"),
            ]),
            expected,
        );
    }
    scalar(
        program(vec![
            attempt(
                vec![node(NodeKind::Return, vec![number(61)])],
                vec![divide_zero()],
            ),
            number(999),
        ]),
        61.,
    );
    let returned = binary(BinaryOp::Multiply, number(73), name("V"));
    let result = row(program(vec![
        attempt(
            vec![divide_zero()],
            vec![node(NodeKind::Return, vec![returned])],
        ),
        number(999),
    ]));
    assert!(result.error.is_none(), "{result:?}");
    let Some(Value::Scalar { quantity }) = result.value else {
        panic!("{result:?}")
    };
    assert_eq!(quantity.value.re, 73.);
    assert_eq!(
        quantity.dimension,
        mathcraft_core::unit("V").unwrap().dimension
    );
}

#[test]
fn bounded_interpreter_preserves_sequential_locals_across_failure_and_keeps_caller_values() {
    // Partial local state is an explicit interpreter policy; original rollback
    // behavior is not established by the shipped division-by-zero example.
    let value = program(vec![
        assign("m", number(1)),
        attempt(
            vec![assign("m", number(12)), divide_zero()],
            vec![assign("m", binary(BinaryOp::Add, name("m"), number(30)))],
        ),
        name("m"),
    ]);
    scalar(value.clone(), 42.);
    let declaration = binary(BinaryOp::Define, name("m"), number(5));
    let regions = vec![
        (1, 0., 0., tree_to_mathml(&declaration).unwrap()),
        (2, 0., 100., tree_to_mathml(&evaluation(value)).unwrap()),
        (3, 0., 200., tree_to_mathml(&evaluation(name("m"))).unwrap()),
    ];
    let rows = evaluate_worksheet(&worksheet_xml(&regions).unwrap()).unwrap();
    assert!(rows.iter().all(|row| row.error.is_none()), "{rows:?}");
    assert_eq!(
        rows[1].value.as_ref().unwrap().numeric_json()["real"].as_f64(),
        Some(42.)
    );
    assert_eq!(
        rows[2].value.as_ref().unwrap().numeric_json()["real"].as_f64(),
        Some(5.)
    );
}

#[test]
fn unsupported_runtime_features_and_prior_failed_regions_are_not_hidden_by_fallback() {
    let unsupported = attempt(
        vec![assign(
            "r",
            node(
                NodeKind::Range { has_step: false },
                vec![number(1), number(3)],
            ),
        )],
        vec![number(99)],
    );
    let result = row(unsupported);
    assert!(result.error.unwrap().unsupported);
    let failed = binary(BinaryOp::Define, name("failed"), divide_zero());
    let value = evaluation(attempt(vec![name("failed")], vec![number(99)]));
    let rows = evaluate_worksheet(
        &worksheet_xml(&[
            (1, 0., 0., tree_to_mathml(&failed).unwrap()),
            (2, 0., 100., tree_to_mathml(&value).unwrap()),
        ])
        .unwrap(),
    )
    .unwrap();
    assert!(rows[0].error.is_some());
    assert!(rows[1].error.as_ref().unwrap().previous_region.is_some());
    assert!(rows[1].value.is_none());
}

#[test]
fn a_deferred_function_value_fault_is_not_replaced_with_a_successful_handler() {
    let function = node(
        NodeKind::Function { parameter_count: 1 },
        vec![name("f"), name("x")],
    );
    let declaration = node(
        NodeKind::LocalDefine,
        vec![function, binary(BinaryOp::Add, name("x"), number(1))],
    );
    let result = row(attempt(vec![declaration], vec![number(99)]));
    let fault = result.error.unwrap();
    assert!(fault.unsupported);
    assert!(fault.detail.contains("function value"), "{fault:?}");
}

#[test]
fn invalid_try_namespace_attributes_arity_and_payload_are_rejected() {
    for body in [
        "",
        "<ml:real>1</ml:real>",
        "<ml:real>1</ml:real><ml:real>2</ml:real><ml:real>3</ml:real>",
        "<ml:real>1</ml:real>unexpected<ml:real>2</ml:real>",
        "<alien xmlns='wrong'/><ml:real>2</ml:real>",
    ] {
        let xml = format!("<ml:tryCatch xmlns:ml='{ML}'>{body}</ml:tryCatch>");
        assert!(tree_from_mathml(&xml).is_err(), "{xml}");
    }
    let xml = format!(
        "<ml:tryCatch xmlns:ml='{ML}' unknown='1'><ml:real>1</ml:real><ml:real>2</ml:real></ml:tryCatch>"
    );
    assert!(tree_from_mathml(&xml).is_err());
}
