//! Scoped program insertion through the native bridge and worksheet evaluator.
use mathfolio_compat::{
    Value,
    editor_bridge::{tree_from_mathml, tree_to_mathml, worksheet_xml},
    evaluate_worksheet,
};
use mathfolio_editor::{BinaryOp, Direction, Editor, NodeKind, PortableTree};

fn node(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
    PortableTree { kind, children }
}
fn number(value: i32) -> PortableTree {
    node(NodeKind::Number(value.to_string()), vec![])
}
fn name(value: &str) -> PortableTree {
    node(NodeKind::Identifier(value.into()), vec![])
}
fn program(children: Vec<PortableTree>) -> PortableTree {
    node(
        NodeKind::Program {
            statement_count: children.len(),
        },
        children,
    )
}
fn binary(kind: BinaryOp, left: PortableTree, right: PortableTree) -> PortableTree {
    node(NodeKind::Binary(kind), vec![left, right])
}
fn local(name_: &str, value: PortableTree) -> PortableTree {
    node(NodeKind::LocalDefine, vec![name(name_), value])
}
fn increment(name_: &str, value: PortableTree) -> PortableTree {
    local(name_, binary(BinaryOp::Add, name(name_), value))
}
fn run(tree: PortableTree) -> mathfolio_core::Quantity {
    let query = binary(
        BinaryOp::Evaluate,
        tree,
        node(NodeKind::Placeholder, vec![]),
    );
    let xml = tree_to_mathml(&query).unwrap();
    assert_eq!(tree_from_mathml(&xml).unwrap(), query);
    let source = worksheet_xml(&[(1, 0., 0., xml)]).unwrap();
    let row = evaluate_worksheet(&source).unwrap().remove(0);
    assert!(row.error.is_none(), "{row:?}");
    let Some(Value::Scalar { quantity }) = row.value else {
        panic!("{row:?}")
    };
    quantity
}
fn insert(
    kind: NodeKind,
    original: PortableTree,
    statement: usize,
    replacements: Vec<(usize, PortableTree)>,
    side: Direction,
) -> PortableTree {
    let mut editor = Editor::from_portable(&original).unwrap();
    let target = editor.arena().get(editor.root()).unwrap().children()[statement];
    let old = editor.export_subtree(target).unwrap();
    editor.select_subtree_from(target, side).unwrap();
    match kind {
        NodeKind::For => {
            editor.insert_for_loop();
        }
        NodeKind::While => {
            editor.insert_while_loop();
        }
        NodeKind::Return => {
            editor.insert_return();
        }
        NodeKind::Try => assert!(editor.insert_try()),
        NodeKind::If { .. } => assert!(editor.insert_if()),
        _ => unreachable!(),
    }
    assert_eq!(editor.export_subtree(target).unwrap(), old);
    let operator = editor
        .arena()
        .nodes()
        .find(|(_, entry)| entry.kind() == &kind)
        .unwrap()
        .0;
    let inserted = editor.to_portable().unwrap();
    assert!(editor.undo());
    assert_eq!(editor.to_portable().unwrap(), original);
    assert!(editor.redo());
    assert_eq!(editor.to_portable().unwrap(), inserted);
    for (child, value) in replacements {
        let target = editor.arena().get(operator).unwrap().children()[child];
        editor.select_subtree(target).unwrap();
        editor.replace_selection(&value).unwrap();
    }
    editor.to_portable().unwrap()
}

#[test]
fn scoped_loop_bodies_preserve_existing_assignments_and_execute_after_native_roundtrip() {
    for side in [Direction::Left, Direction::Right] {
        let tree = insert(
            NodeKind::While,
            program(vec![
                local("n", number(0)),
                increment("n", number(1)),
                name("n"),
            ]),
            1,
            vec![(0, binary(BinaryOp::LessThan, name("n"), number(3)))],
            side,
        );
        let result = run(tree);
        assert_eq!(result.value.re, 3.);
        assert_eq!(result.dimension, [0.; 7]);

        let tree = insert(
            NodeKind::For,
            program(vec![
                local("total", number(0)),
                increment("total", name("i")),
                name("total"),
            ]),
            1,
            vec![
                (0, name("i")),
                (
                    1,
                    node(
                        NodeKind::Range { has_step: false },
                        vec![number(1), number(3)],
                    ),
                ),
            ],
            side,
        );
        let result = run(tree);
        assert_eq!(result.value.re, 6.);
        assert_eq!(result.dimension, [0.; 7]);
    }
}

#[test]
fn scoped_if_and_try_preserve_the_selected_attempt_instead_of_moving_it_into_a_condition() {
    for side in [Direction::Left, Direction::Right] {
        let tree = insert(
            NodeKind::If {
                has_else: false,
                branch_count: 1,
                alternate: false,
            },
            program(vec![number(17)]),
            0,
            vec![(0, number(1))],
            side,
        );
        assert_eq!(run(tree).value.re, 17.);
        let tree = insert(
            NodeKind::Try,
            program(vec![binary(BinaryOp::InlineDivide, number(1), number(0))]),
            0,
            vec![(1, program(vec![number(43)]))],
            side,
        );
        assert_eq!(run(tree).value.re, 43.);
    }
}

#[test]
fn original_left_scope_return_preserves_units_and_returns_before_the_following_statement() {
    let tree = insert(
        NodeKind::Return,
        program(vec![
            binary(BinaryOp::Multiply, number(73), name("V")),
            number(999),
        ]),
        0,
        vec![],
        Direction::Left,
    );
    let value = run(tree);
    assert_eq!(value.value.re, 73.);
    assert_eq!(
        value.dimension,
        mathfolio_core::unit("V").unwrap().dimension
    );
}
