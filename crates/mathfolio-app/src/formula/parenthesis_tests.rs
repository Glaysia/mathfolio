//! Original text '(' creates GROUPING, then syntactic SCALE/APPLY repair.
use super::*;

fn node(kind: NodeKind, children: Vec<PortableTree>) -> PortableTree {
    PortableTree { kind, children }
}
fn number(value: &str) -> PortableTree {
    node(NodeKind::Number(value.into()), vec![])
}
fn name(value: &str) -> PortableTree {
    node(NodeKind::Identifier(value.into()), vec![])
}
fn binary(op: BinaryOp, left: PortableTree, right: PortableTree) -> PortableTree {
    node(NodeKind::Binary(op), vec![left, right])
}
fn group(value: PortableTree) -> PortableTree {
    node(NodeKind::Group, vec![value])
}
fn call(value: &str, argument: PortableTree) -> PortableTree {
    node(
        NodeKind::Call { argument_count: 1 },
        vec![name(value), argument],
    )
}
fn typed(input: &str) -> PortableTree {
    let mut editor = Editor::new();
    for ch in input.chars() {
        let outcome = input_text_with_actions(&mut editor, &ch.to_string());
        assert!(!outcome.evaluation_requested, "{input}");
    }
    editor.to_portable().unwrap()
}

#[test]
fn normal_parentheses_preserve_numeric_and_completed_group_operands_by_scaling() {
    for (input, expected) in [
        (
            "2(3)",
            binary(BinaryOp::Scale, number("2"), group(number("3"))),
        ),
        (
            "(2+3)(4)",
            binary(
                BinaryOp::Scale,
                group(binary(BinaryOp::Add, number("2"), number("3"))),
                group(number("4")),
            ),
        ),
        (
            "2(3+4)",
            binary(
                BinaryOp::Scale,
                number("2"),
                group(binary(BinaryOp::Add, number("3"), number("4"))),
            ),
        ),
        (
            "3(2m)",
            binary(
                BinaryOp::Scale,
                number("3"),
                group(binary(BinaryOp::Scale, number("2"), name("m"))),
            ),
        ),
    ] {
        let tree = typed(input);
        assert_eq!(tree, expected, "{input}");
        let xml = crate::worksheet_math::tree_to_mathml(&tree).unwrap();
        assert_eq!(crate::worksheet_math::tree_from_mathml(&xml).unwrap(), tree);
    }
}

#[test]
fn ordinary_identifier_calls_do_not_turn_sqrt_or_abs_words_into_templates() {
    for (input, expected) in [
        ("f(3)", call("f", number("3"))),
        ("m(3)", call("m", number("3"))),
        ("sqrt(4)", call("sqrt", number("4"))),
        (
            "abs(-3)",
            call("abs", node(NodeKind::UnaryNeg, vec![number("3")])),
        ),
    ] {
        assert_eq!(typed(input), expected, "{input}");
    }
}

#[test]
fn adjacency_respects_infix_precedence_and_the_exponent_and_denominator_containers() {
    for (input, expected) in [
        (
            "2+3(4)",
            binary(
                BinaryOp::Add,
                number("2"),
                binary(BinaryOp::Scale, number("3"), group(number("4"))),
            ),
        ),
        (
            "2*3(4)",
            binary(
                BinaryOp::Multiply,
                number("2"),
                binary(BinaryOp::Scale, number("3"), group(number("4"))),
            ),
        ),
        (
            "2^3(4)",
            node(
                NodeKind::Power,
                vec![
                    number("2"),
                    binary(BinaryOp::Scale, number("3"), group(number("4"))),
                ],
            ),
        ),
        (
            "2/3(4)",
            node(
                NodeKind::Fraction,
                vec![
                    number("2"),
                    binary(BinaryOp::Scale, number("3"), group(number("4"))),
                ],
            ),
        ),
    ] {
        assert_eq!(typed(input), expected, "{input}");
    }
}

#[test]
fn ordinary_infix_input_stays_in_the_current_exponent_or_denominator() {
    for (input, expected) in [
        (
            "2^3+4",
            node(
                NodeKind::Power,
                vec![number("2"), binary(BinaryOp::Add, number("3"), number("4"))],
            ),
        ),
        (
            "2/3+4",
            node(
                NodeKind::Fraction,
                vec![number("2"), binary(BinaryOp::Add, number("3"), number("4"))],
            ),
        ),
        (
            "2^3m",
            node(
                NodeKind::Power,
                vec![number("2"), binary(BinaryOp::Scale, number("3"), name("m"))],
            ),
        ),
        (
            "(2^3)+4",
            group(node(
                NodeKind::Power,
                vec![
                    number("2"),
                    binary(BinaryOp::Add, group(number("3")), number("4")),
                ],
            )),
        ),
    ] {
        assert_eq!(typed(input), expected, "{input}");
    }
    let value = eval_xml(typed("2^3m"));
    let row = mathfolio_compat::evaluate_worksheet(&value)
        .unwrap()
        .remove(0);
    assert!(
        row.error.is_some(),
        "A dimensional exponent must not become an outside unit: {row:?}"
    );
}

fn eval_xml(value: PortableTree) -> String {
    let source = binary(
        BinaryOp::Evaluate,
        value,
        node(NodeKind::Placeholder, vec![]),
    );
    let xml = crate::worksheet_math::tree_to_mathml(&source).unwrap();
    crate::worksheet_math::worksheet_xml(&[(1, 0., 0., xml)]).unwrap()
}

#[test]
fn typed_numeric_adjacency_and_builtin_calls_calculate_with_existing_units() {
    for (input, expected, physical) in [
        ("2(3)", 6., false),
        ("(2+3)(4)", 20., false),
        ("sqrt(4)", 2., false),
        ("abs(-3)", 3., false),
        ("3(2m)", 6., true),
        ("2+3(4)", 14., false),
        ("2*3(4)", 24., false),
        ("2^3(2)", 64., false),
        ("2^3+4", 128., false),
        ("2/3+4", 2. / 7., false),
        ("(2^3)+4", 128., false),
    ] {
        let source = binary(
            BinaryOp::Evaluate,
            typed(input),
            node(NodeKind::Placeholder, vec![]),
        );
        let xml = crate::worksheet_math::tree_to_mathml(&source).unwrap();
        let worksheet = crate::worksheet_math::worksheet_xml(&[(1, 0., 0., xml)]).unwrap();
        let row = mathfolio_compat::evaluate_worksheet(&worksheet)
            .unwrap()
            .remove(0);
        assert!(row.error.is_none(), "{input}: {row:?}");
        let Some(mathfolio_compat::Value::Scalar { quantity }) = row.value else {
            panic!("{row:?}")
        };
        assert_eq!(quantity.value.re, expected, "{input}");
        assert_eq!(quantity.value.im, 0.);
        if physical {
            assert_eq!(
                quantity.dimension,
                mathfolio_core::unit("m").unwrap().dimension
            );
        } else {
            assert!(quantity.is_scalar());
        }
    }
}

#[test]
fn actual_right_arrow_leaves_power_and_fraction_before_the_next_operator_or_unit() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    for (prefix, suffix, expected, physical) in [
        ("2^3", "+4", 12., false),
        ("8/4", "*2", 4., false),
        ("2^3", "m", 8., true),
        ("(2^3", ")+4", 12., false),
    ] {
        let mut editor = Editor::new();
        input_text(&mut editor, prefix);
        ctx.run_ui(Default::default(), |ui| {
            formula_ui_with_actions(ui, &mut editor, None, true);
        })
        .drop_without_applying_deltas();
        ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: Key::ArrowRight,
                    physical_key: Some(Key::ArrowRight),
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ui| {
                let response = formula_ui_with_actions(ui, &mut editor, None, true);
                assert!(!response.response.changed() && !response.evaluation_requested);
            },
        )
        .drop_without_applying_deltas();
        input_text(&mut editor, suffix);
        let xml = eval_xml(editor.to_portable().unwrap());
        let result = mathfolio_compat::evaluate_worksheet(&xml)
            .unwrap()
            .remove(0);
        assert!(
            result.error.is_none(),
            "{prefix} ArrowRight {suffix}: {result:?}"
        );
        let Some(mathfolio_compat::Value::Scalar { quantity }) = result.value else {
            panic!("{result:?}")
        };
        assert_eq!(quantity.value.re, expected, "{prefix} ArrowRight {suffix}");
        if physical {
            assert_eq!(
                quantity.dimension,
                mathfolio_core::unit("m").unwrap().dimension
            );
        } else {
            assert!(quantity.is_scalar());
        }
    }
}
