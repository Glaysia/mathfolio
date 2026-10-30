//! Normal operator entry uses the native text range, independently of Scope.
use super::*;

fn atom(value: &str) -> PortableTree {
    PortableTree {
        kind: NodeKind::Number(value.into()),
        children: vec![],
    }
}
fn node(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
    PortableTree { kind, children }
}
fn split(value: &str, start: usize, end: usize, input: &str) -> Editor {
    let mut editor = Editor::from_portable(&atom(value)).unwrap();
    editor
        .place_text_selection(editor.root(), start, end)
        .unwrap();
    let outcome = input_text_with_actions(&mut editor, input);
    assert!(
        outcome.document_changed && !outcome.evaluation_requested,
        "{input}"
    );
    editor
}

#[test]
fn ordinary_operator_text_splits_the_current_number_and_removes_only_selected_text() {
    for (text, kind) in [
        ("+", NodeKind::Binary(BinaryOp::Add)),
        ("-", NodeKind::Binary(BinaryOp::Subtract)),
        ("*", NodeKind::Binary(BinaryOp::Multiply)),
        ("/", NodeKind::Fraction),
        ("^", NodeKind::Power),
        ("<", NodeKind::Binary(BinaryOp::LessThan)),
        ("≥", NodeKind::Binary(BinaryOp::GreaterOrEqual)),
        ("∧", NodeKind::Binary(BinaryOp::And)),
    ] {
        for (start, end, left, right) in [(2, 2, "12", "34"), (1, 3, "1", "4"), (3, 1, "1", "4")] {
            let mut editor = split("1234", start, end, text);
            let expected = node(kind.clone(), vec![atom(left), atom(right)]);
            assert_eq!(
                editor.to_portable().unwrap(),
                expected,
                "{text} {start}..{end}"
            );
            let suffix = editor.arena().get(editor.root()).unwrap().children()[1];
            assert_eq!(editor.caret().node, suffix);
            assert_eq!(editor.caret().value_range.unwrap().focus, 0);
            let xml = crate::worksheet_math::tree_to_mathml(&expected).unwrap();
            assert_eq!(
                crate::worksheet_math::tree_from_mathml(&xml).unwrap(),
                expected
            );
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), atom("1234"));
            assert!(editor.redo());
            assert_eq!(editor.to_portable().unwrap(), expected);
        }
    }
}

#[test]
fn complete_normal_text_selection_inserts_an_empty_template_and_prefix_minus() {
    for (text, kind, arity) in [
        ("+", NodeKind::Binary(BinaryOp::Add), 2),
        ("/", NodeKind::Fraction, 2),
        ("^", NodeKind::Power, 2),
        ("-", NodeKind::UnaryNeg, 1),
        ("¬", NodeKind::LogicalNot, 1),
    ] {
        let editor = split("1234", 0, 4, text);
        assert_eq!(
            editor.to_portable().unwrap(),
            node(kind, vec![node(NodeKind::Placeholder, vec![]); arity]),
            "{text}"
        );
        let first = editor.arena().get(editor.root()).unwrap().children()[0];
        assert_eq!(editor.caret().node, first);
    }
    let editor = split("1234", 0, 0, "-");
    assert_eq!(
        editor.to_portable().unwrap(),
        node(NodeKind::UnaryNeg, vec![atom("1234")])
    );
}

#[test]
fn normal_split_reparses_neighboring_power_without_crossing_fraction_containers() {
    let mut power =
        Editor::from_portable(&node(NodeKind::Power, vec![atom("1234"), atom("5")])).unwrap();
    let base = power.arena().get(power.root()).unwrap().children()[0];
    power.place_text_caret(base, 2).unwrap();
    assert!(input_text(&mut power, "+"));
    assert_eq!(
        power.to_portable().unwrap(),
        node(
            NodeKind::Binary(BinaryOp::Add),
            vec![
                atom("12"),
                node(NodeKind::Power, vec![atom("34"), atom("5")])
            ]
        )
    );
    for slot in [0, 1] {
        let mut fraction =
            Editor::from_portable(&node(NodeKind::Fraction, vec![atom("1234"), atom("1234")]))
                .unwrap();
        let target = fraction.arena().get(fraction.root()).unwrap().children()[slot];
        fraction.place_text_caret(target, 2).unwrap();
        assert!(input_text(&mut fraction, "+"));
        let mut children = vec![atom("1234"), atom("1234")];
        children[slot] = node(
            NodeKind::Binary(BinaryOp::Add),
            vec![atom("12"), atom("34")],
        );
        assert_eq!(
            fraction.to_portable().unwrap(),
            node(NodeKind::Fraction, children)
        );
    }
    let not = split("1234", 2, 2, "¬").to_portable().unwrap();
    assert_eq!(
        not,
        node(
            NodeKind::Binary(BinaryOp::Scale),
            vec![atom("12"), node(NodeKind::LogicalNot, vec![atom("34")])]
        )
    );
}

#[test]
fn full_text_replacement_reparses_and_enters_the_missing_left_neighbor() {
    let mut editor = Editor::from_portable(&node(
        NodeKind::Binary(BinaryOp::Multiply),
        vec![atom("2"), atom("1234")],
    ))
    .unwrap();
    let rhs = editor.arena().get(editor.root()).unwrap().children()[1];
    editor.place_text_selection(rhs, 0, 4).unwrap();
    assert!(input_text(&mut editor, "+"));
    assert!(input_text(&mut editor, "3"));
    let empty = node(NodeKind::Placeholder, vec![]);
    assert_eq!(
        editor.to_portable().unwrap(),
        node(
            NodeKind::Binary(BinaryOp::Add),
            vec![
                node(
                    NodeKind::Binary(BinaryOp::Multiply),
                    vec![atom("2"), atom("3")]
                ),
                empty
            ]
        )
    );
}

#[test]
fn split_operator_sources_calculate_instead_of_retaining_the_unsplit_number() {
    for (source, start, end, text, expected) in [
        ("1234", 2, 2, "+", 46.),
        ("1234", 1, 3, "+", 5.),
        ("23", 1, 1, "^", 8.),
        ("24", 1, 1, "/", 0.5),
        ("23", 1, 1, "*", 6.),
        ("23", 1, 1, "-", -1.),
        ("23", 1, 1, "<", 1.),
    ] {
        let value = split(source, start, end, text).to_portable().unwrap();
        let eval = node(
            NodeKind::Binary(BinaryOp::Evaluate),
            vec![value, node(NodeKind::Placeholder, vec![])],
        );
        let fragment = crate::worksheet_math::tree_to_mathml(&eval).unwrap();
        let xml = crate::worksheet_math::worksheet_xml(&[(1, 0., 0., fragment)]).unwrap();
        let row = mathfolio_compat::evaluate_worksheet(&xml)
            .unwrap()
            .remove(0);
        assert!(row.error.is_none(), "{source} {text}: {row:?}");
        let Some(mathfolio_compat::Value::Scalar { quantity }) = row.value else {
            panic!("{row:?}")
        };
        assert_eq!(quantity.value.re, expected);
        assert_eq!(quantity.value.im, 0.);
    }
}

#[test]
fn actual_comparison_and_logic_shortcuts_use_the_same_normal_text_split() {
    for (key, shift, kind) in [
        (Key::Equals, false, NodeKind::Binary(BinaryOp::Equal)),
        (Key::Num9, false, NodeKind::Binary(BinaryOp::LessOrEqual)),
        (Key::Num0, false, NodeKind::Binary(BinaryOp::GreaterOrEqual)),
        (Key::Num2, true, NodeKind::Binary(BinaryOp::Or)),
        (Key::Num5, true, NodeKind::Binary(BinaryOp::Xor)),
        (Key::Num7, true, NodeKind::Binary(BinaryOp::And)),
    ] {
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut editor = Editor::from_portable(&atom("1234")).unwrap();
        ctx.run_ui(Default::default(), |ui| {
            formula_ui_with_actions(ui, &mut editor, None, true);
        })
        .drop_without_applying_deltas();
        editor.place_text_caret(editor.root(), 2).unwrap();
        let modifiers = egui::Modifiers {
            ctrl: true,
            command: true,
            shift,
            ..Default::default()
        };
        ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed: true,
                    repeat: false,
                    modifiers,
                }],
                ..Default::default()
            },
            |ui| {
                let response = formula_ui_with_actions(ui, &mut editor, None, true);
                assert!(response.response.changed() && !response.evaluation_requested);
            },
        )
        .drop_without_applying_deltas();
        assert_eq!(
            editor.to_portable().unwrap(),
            node(kind, vec![atom("12"), atom("34")])
        );
    }
}
