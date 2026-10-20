//! Actual editor control tokens, native bridge and engine flow, without a GUI.
use mathcraft_compat::{
    Value,
    editor_bridge::{tree_from_mathml, tree_to_mathml, worksheet_xml},
    evaluate_worksheet,
};
use mathcraft_editor::{BinaryOp, Editor, NodeKind, PortableTree};

fn node(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
    PortableTree { kind, children }
}
fn number(n: i32) -> PortableTree {
    node(NodeKind::Number(n.to_string()), vec![])
}
fn name(n: &str) -> PortableTree {
    node(NodeKind::Identifier(n.into()), vec![])
}
fn binary(op: BinaryOp, l: PortableTree, r: PortableTree) -> PortableTree {
    node(NodeKind::Binary(op), vec![l, r])
}
fn program(lines: Vec<PortableTree>) -> PortableTree {
    node(
        NodeKind::Program {
            statement_count: lines.len(),
        },
        lines,
    )
}
fn local(n: &str, value: PortableTree) -> PortableTree {
    node(NodeKind::LocalDefine, vec![name(n), value])
}
fn increment(n: &str, value: PortableTree) -> PortableTree {
    local(n, binary(BinaryOp::Add, name(n), value))
}
fn keyword(word: &str) -> PortableTree {
    let mut editor = Editor::from_portable(&name(word)).unwrap();
    assert!(editor.convert_program_keyword());
    editor.to_portable().unwrap()
}
fn conditional(test: PortableTree, statement: PortableTree) -> PortableTree {
    node(
        NodeKind::If {
            has_else: false,
            branch_count: 1,
            alternate: false,
        },
        vec![test, program(vec![statement])],
    )
}
fn loop_for(n: &str, end: i32, body: Vec<PortableTree>) -> PortableTree {
    node(
        NodeKind::For,
        vec![
            name(n),
            node(
                NodeKind::Range { has_step: false },
                vec![number(1), number(end)],
            ),
            program(body),
        ],
    )
}
fn eval(value: PortableTree) -> PortableTree {
    binary(
        BinaryOp::Evaluate,
        value,
        node(NodeKind::Placeholder, vec![]),
    )
}
fn run(value: PortableTree) -> mathcraft_compat::RegionEvaluation {
    let tree = eval(value);
    let fragment = tree_to_mathml(&tree).unwrap();
    assert_eq!(tree_from_mathml(&fragment).unwrap(), tree);
    let worksheet = worksheet_xml(&[(1, 0., 0., fragment)]).unwrap();
    evaluate_worksheet(&worksheet).unwrap().remove(0)
}
fn scalar(value: PortableTree, expected: f64) {
    let row = run(value);
    assert!(row.error.is_none(), "{row:?}");
    let Some(Value::Scalar { quantity }) = row.value else {
        panic!("{row:?}")
    };
    assert_eq!(quantity.value.re, expected);
    assert_eq!(quantity.value.im, 0.);
    assert_eq!(quantity.dimension, [0.; 7]);
}

#[test]
fn converted_break_and_continue_roundtrip_as_empty_original_elements() {
    for (word, kind) in [("break", NodeKind::Break), ("continue", NodeKind::Continue)] {
        let tree = keyword(word);
        assert_eq!(tree.kind, kind);
        assert_eq!(
            tree_to_mathml(&tree).unwrap(),
            format!("<ml:{word} xmlns:ml=\"http://schemas.mathsoft.com/math50\"/>")
        );
        assert_eq!(
            tree_from_mathml(&tree_to_mathml(&tree).unwrap()).unwrap(),
            tree
        );
        for invalid in [
            format!("<ml:{word} fake='1'/>"),
            format!("<ml:{word}><ml:real>1</ml:real></ml:{word}>"),
            format!("<ml:{word}>text</ml:{word}>"),
        ] {
            let source = format!(
                "<context xmlns:ml=\"http://schemas.mathsoft.com/math50\">{invalid}</context>"
            );
            assert!(tree_from_mathml(&source).is_err(), "{source}");
        }
    }
}

#[test]
fn converted_control_skips_iteration_or_exits_the_nearest_for_loop() {
    for (word, expected) in [("break", 3.), ("continue", 12.)] {
        scalar(
            program(vec![
                local("total", number(0)),
                loop_for(
                    "i",
                    5,
                    vec![
                        conditional(binary(BinaryOp::Equal, name("i"), number(3)), keyword(word)),
                        increment("total", name("i")),
                    ],
                ),
                name("total"),
            ]),
            expected,
        );
    }
    scalar(
        program(vec![
            local("total", number(0)),
            loop_for(
                "i",
                3,
                vec![loop_for(
                    "j",
                    3,
                    vec![
                        conditional(
                            binary(BinaryOp::Equal, name("j"), number(2)),
                            keyword("break"),
                        ),
                        increment("total", number(1)),
                    ],
                )],
            ),
            name("total"),
        ]),
        3.,
    );
}

#[test]
fn converted_while_controls_keep_condition_updates_and_return_preempts_later_lines() {
    scalar(
        program(vec![
            local("i", number(0)),
            local("total", number(0)),
            node(
                NodeKind::While,
                vec![
                    binary(BinaryOp::LessThan, name("i"), number(6)),
                    program(vec![
                        increment("i", number(1)),
                        conditional(
                            binary(BinaryOp::Equal, name("i"), number(2)),
                            keyword("continue"),
                        ),
                        conditional(
                            binary(BinaryOp::Equal, name("i"), number(5)),
                            keyword("break"),
                        ),
                        increment("total", name("i")),
                    ]),
                ],
            ),
            name("total"),
        ]),
        8.,
    );
    let mut ret = keyword("return");
    ret.children[0] = number(7);
    scalar(
        program(vec![
            node(NodeKind::While, vec![number(1), program(vec![ret])]),
            number(999),
        ]),
        7.,
    );
}

#[test]
fn zero_arity_authoring_is_preserved_even_when_execution_context_is_invalid() {
    for word in ["break", "continue"] {
        // Descriptor insertion has no invented program/loop authoring gate.
        let row = run(keyword(word));
        assert!(row.error.is_some());
        assert!(row.value.is_none());
        let row = run(program(vec![keyword(word)]));
        assert!(row.error.is_some());
        assert!(row.value.is_none());
    }
}
