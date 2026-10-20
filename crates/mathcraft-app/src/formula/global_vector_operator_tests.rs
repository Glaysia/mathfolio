//! Native operator input and geometry, entirely in headless egui frames.
use super::*;

fn number(text: &str) -> PortableTree {
    PortableTree {
        kind: NodeKind::Number(text.into()),
        children: vec![],
    }
}
fn identifier(text: &str) -> PortableTree {
    PortableTree {
        kind: NodeKind::Identifier(text.into()),
        children: vec![],
    }
}
fn column(values: [&str; 3]) -> PortableTree {
    PortableTree {
        kind: NodeKind::Matrix {
            rows: 3,
            cols: 1,
            display: None,
        },
        children: values.into_iter().map(number).collect(),
    }
}
fn binary(op: BinaryOp, left: PortableTree, right: PortableTree) -> PortableTree {
    PortableTree {
        kind: NodeKind::Binary(op),
        children: vec![left, right],
    }
}
fn key(key: Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}
fn shifted() -> egui::Modifiers {
    egui::Modifiers {
        shift: true,
        ..egui::Modifiers::COMMAND
    }
}
fn send(ctx: &egui::Context, editor: &mut Editor, event: egui::Event) -> egui::FullOutput {
    ctx.run_ui(
        egui::RawInput {
            events: vec![event],
            ..Default::default()
        },
        |ui| {
            assert!(!formula_ui_with_actions(ui, editor, None, true).evaluation_requested);
        },
    )
}
fn evaluated(tree: PortableTree) -> mathcraft_compat::Value {
    let tree = PortableTree {
        kind: NodeKind::Binary(BinaryOp::Evaluate),
        children: vec![
            tree,
            PortableTree {
                kind: NodeKind::Placeholder,
                children: vec![],
            },
        ],
    };
    let xml = crate::worksheet_math::tree_to_mathml(&tree).unwrap();
    let worksheet = crate::worksheet_math::worksheet_xml(&[(1, 0., 0., xml)]).unwrap();
    let rows = mathcraft_compat::evaluate_worksheet(&worksheet).unwrap();
    assert!(rows[0].error.is_none(), "{:?}", rows[0].error);
    rows[0].value.clone().unwrap()
}

#[test]
fn native_shortcuts_insert_global_vector_and_cross_nodes_with_one_undo_no_evaluation() {
    for (shortcut, modifiers, source, expected) in [
        (
            Key::Backtick,
            shifted(),
            identifier("x"),
            NodeKind::Binary(BinaryOp::GlobalDefine),
        ),
        (Key::Num6, shifted(), number("7"), NodeKind::Vectorize),
        (
            Key::Num8,
            egui::Modifiers::COMMAND,
            column(["1", "0", "0"]),
            NodeKind::Binary(BinaryOp::CrossProduct),
        ),
    ] {
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let mut editor = Editor::from_portable(&source).unwrap();
        editor.select_subtree(editor.root()).unwrap();
        send(&ctx, &mut editor, key(shortcut, modifiers)).drop_without_applying_deltas();
        let changed = editor.to_portable().unwrap();
        assert_eq!(changed.kind, expected);
        assert_eq!(changed.children[0], source);
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), source);
        assert!(!editor.undo());
    }
}

#[test]
fn global_definition_equals_evaluates_rhs_once_and_empty_rhs_is_not_evaluated() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut editor = Editor::from_portable(&identifier("g")).unwrap();
    send(&ctx, &mut editor, key(Key::Backtick, shifted())).drop_without_applying_deltas();
    assert_eq!(
        input_text_with_actions(&mut editor, "="),
        InputOutcome::default()
    );
    assert!(!input_text_with_actions(&mut editor, "7").evaluation_requested);
    let first = input_text_with_actions(&mut editor, "=");
    assert!(first.document_changed && first.evaluation_requested);
    let before = editor.to_portable().unwrap();
    assert_eq!(before.kind, NodeKind::Binary(BinaryOp::GlobalDefine));
    assert_eq!(
        before.children[1].kind,
        NodeKind::Binary(BinaryOp::Evaluate)
    );
    let again = input_text_with_actions(&mut editor, "=");
    assert!(!again.document_changed && again.evaluation_requested);
    assert_eq!(editor.to_portable().unwrap(), before);
    let fragment = crate::worksheet_math::tree_to_mathml(&before).unwrap();
    assert!(fragment.contains("globalDefine"));
    let restored = crate::worksheet_math::tree_from_mathml(&fragment).unwrap();
    assert_eq!(restored, before);
}

#[test]
fn global_and_cross_glyphs_and_vector_arrow_have_native_shape_and_real_source_hits() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    for (tree, glyph) in [
        (
            binary(BinaryOp::GlobalDefine, identifier("g"), number("7")),
            "≡",
        ),
        (
            binary(
                BinaryOp::CrossProduct,
                column(["1", "0", "0"]),
                column(["0", "1", "0"]),
            ),
            "⨯",
        ),
    ] {
        let editor = Editor::from_portable(&tree).unwrap();
        ctx.run_ui(egui::RawInput::default(), |ui| {
            let layout = measure(ui.painter(), editor.arena(), editor.root(), FONT_SIZE);
            assert!(
                layout
                    .primitives
                    .iter()
                    .any(|primitive| matches!(primitive,Primitive::Text(_,g) if g.text()==glyph))
            );
            assert!(
                !layout
                    .primitives
                    .iter()
                    .any(|primitive| matches!(primitive,Primitive::Text(_,g) if g.text()=="×"))
            );
            let mut hits = vec![];
            collect_hits(&layout, Pos2::ZERO, &mut hits);
            assert!(
                hits.iter()
                    .all(|hit| editor.arena().get(hit.node).is_some())
            );
        })
        .drop_without_applying_deltas();
    }
    let source = PortableTree {
        kind: NodeKind::Vectorize,
        children: vec![binary(BinaryOp::Add, number("2"), number("3"))],
    };
    let editor = Editor::from_portable(&source).unwrap();
    ctx.run_ui(
        egui::RawInput {
            events: vec![egui::Event::Text("9=".into())],
            ..Default::default()
        },
        |ui| {
            let layout = readonly_layout(ui, &editor, None, None, 12.);
            assert_eq!(layout.primitives.len(), 3);
            let (offset, child) = &layout.children[0];
            assert_eq!(offset.y, 4.);
            let Primitive::Line(a, b) = layout.primitives[0] else {
                panic!()
            };
            assert_eq!(a.y, b.y);
            assert!(b.x - a.x >= child.size.x);
            let Primitive::Line(upper, tip) = layout.primitives[1] else {
                panic!()
            };
            let Primitive::Line(lower, other_tip) = layout.primitives[2] else {
                panic!()
            };
            assert_eq!(tip, other_tip);
            assert!(upper.y < tip.y && lower.y > tip.y);
            let measured = measured_size_readonly_styled(ui, &editor, None, None, 12.);
            let painted = formula_readonly_ui_styled(ui, &editor, None, None, 12.);
            let delta = measured - painted.rect.size();
            let pixel = ui.ctx().pixels_per_point().recip();
            assert!(delta.x.abs() <= pixel && delta.y.abs() <= pixel);
            assert!(!painted.changed() && !painted.has_focus());
            assert!(ui.input(|i| {
                i.events
                    .iter()
                    .any(|event| matches!(event, egui::Event::Text(_)))
            }));
            let narrow = measure_display_math(
                ui.painter(),
                editor.root(),
                &crate::result_display::DisplayMath::Text("i".into(), MathStyle::Operator),
                FONT_SIZE,
            );
            let narrow = visual_overarrow(ui.painter(), editor.root(), narrow, FONT_SIZE);
            assert!(narrow.size.x >= text_galley(ui.painter(), "→", FONT_SIZE).size().x);
            let (offset, child) = &narrow.children[0];
            assert_eq!(offset.y, 4.);
            assert!((offset.x - (narrow.size.x - child.size.x) * 0.5).abs() < 0.001);
        },
    )
    .drop_without_applying_deltas();
    assert_eq!(editor.to_portable().unwrap(), source);
}

#[test]
fn native_cross_and_vectorize_roundtrip_then_compute_operator_semantics() {
    let cross = binary(
        BinaryOp::CrossProduct,
        column(["1", "0", "0"]),
        column(["0", "1", "0"]),
    );
    let matrix = PortableTree {
        kind: NodeKind::Matrix {
            rows: 2,
            cols: 2,
            display: None,
        },
        children: ["1", "2", "3", "4"].into_iter().map(number).collect(),
    };
    let vectorized = PortableTree {
        kind: NodeKind::Vectorize,
        children: vec![PortableTree {
            kind: NodeKind::Power,
            children: vec![matrix, number("2")],
        }],
    };
    for (source, expected) in [
        (cross, vec![0., 0., 1.]),
        (vectorized, vec![1., 4., 9., 16.]),
    ] {
        let xml = crate::worksheet_math::tree_to_mathml(&source).unwrap();
        let restored = crate::worksheet_math::tree_from_mathml(&xml).unwrap();
        assert_eq!(restored, source);
        let mathcraft_compat::Value::Matrix { elements, .. } = evaluated(restored) else {
            panic!()
        };
        assert_eq!(
            elements
                .iter()
                .map(|quantity| quantity.value.re)
                .collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn new_operator_shortcuts_are_noops_for_matrix_cell_ranges_and_unmodified_keys_are_not_aliases() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let source = column(["1", "2", "3"]);
    let mut editor = Editor::from_portable(&source).unwrap();
    let root = editor.root();
    editor.select_matrix_cells(root, 0, 2);
    for event in [
        key(Key::Backtick, shifted()),
        key(Key::Num6, shifted()),
        key(Key::Num8, egui::Modifiers::COMMAND),
    ] {
        ctx.run_ui(
            egui::RawInput {
                events: vec![event],
                ..Default::default()
            },
            |ui| {
                let response = formula_ui_with_actions(ui, &mut editor, None, true);
                assert!(!response.response.changed() && !response.evaluation_requested);
            },
        )
        .drop_without_applying_deltas();
        assert_eq!(editor.to_portable().unwrap(), source);
    }
    assert!(!editor.undo());
    let mut ordinary = Editor::new();
    input_text(&mut ordinary, "a*b");
    assert_eq!(
        ordinary.to_portable().unwrap().kind,
        NodeKind::Binary(BinaryOp::Multiply)
    );
    let before = ordinary.to_portable().unwrap();
    for shortcut in [Key::Num8, Key::Num6, Key::Backtick] {
        send(&ctx, &mut ordinary, key(shortcut, egui::Modifiers::NONE))
            .drop_without_applying_deltas();
        assert_eq!(ordinary.to_portable().unwrap(), before);
    }
}

#[test]
fn cross_product_keeps_native_right_association_and_groups_raw_left_cross_operands() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut editor = Editor::new();
    input_text(&mut editor, "a");
    send(&ctx, &mut editor, key(Key::Num8, egui::Modifiers::COMMAND))
        .drop_without_applying_deltas();
    input_text(&mut editor, "b");
    send(&ctx, &mut editor, key(Key::Num8, egui::Modifiers::COMMAND))
        .drop_without_applying_deltas();
    input_text(&mut editor, "c");
    let typed = editor.to_portable().unwrap();
    assert_eq!(typed.kind, NodeKind::Binary(BinaryOp::CrossProduct));
    assert_eq!(typed.children[0], identifier("a"));
    assert_eq!(
        typed.children[1].kind,
        NodeKind::Binary(BinaryOp::CrossProduct)
    );
    let cross = binary(BinaryOp::CrossProduct, identifier("a"), identifier("b"));
    for (source, index, grouped) in [
        (
            binary(BinaryOp::CrossProduct, cross.clone(), identifier("c")),
            0,
            true,
        ),
        (
            binary(BinaryOp::CrossProduct, identifier("c"), cross.clone()),
            1,
            false,
        ),
        (binary(BinaryOp::Multiply, cross, identifier("c")), 0, true),
        (
            binary(
                BinaryOp::CrossProduct,
                identifier("a"),
                binary(BinaryOp::Add, identifier("b"), identifier("c")),
            ),
            1,
            true,
        ),
    ] {
        let editor = Editor::from_portable(&source).unwrap();
        ctx.run_ui(egui::RawInput::default(), |ui| {
            let layout = measure(ui.painter(), editor.arena(), editor.root(), FONT_SIZE);
            assert_eq!(
                layout.children[index]
                    .1
                    .primitives
                    .iter()
                    .any(|p| matches!(p, Primitive::Line(..))),
                grouped,
                "{:?}",
                source
            );
        })
        .drop_without_applying_deltas();
        assert_eq!(editor.to_portable().unwrap(), source);
    }
}

#[test]
fn vectorize_wraps_selected_scope_and_named_calls_are_not_rewritten_as_operators() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let source = binary(BinaryOp::Add, number("2"), number("3"));
    for entire in [false, true] {
        let mut editor = Editor::from_portable(&source).unwrap();
        let selected = if entire {
            editor.root()
        } else {
            editor.arena().get(editor.root()).unwrap().children()[0]
        };
        editor.select_subtree(selected).unwrap();
        send(&ctx, &mut editor, key(Key::Num6, shifted())).drop_without_applying_deltas();
        let changed = editor.to_portable().unwrap();
        if entire {
            assert_eq!(changed.kind, NodeKind::Vectorize);
            assert_eq!(changed.children[0], source);
        } else {
            assert_eq!(changed.kind, source.kind);
            assert_eq!(changed.children[0].kind, NodeKind::Vectorize);
            assert_eq!(changed.children[1], source.children[1]);
        }
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), source);
        assert!(!editor.undo());
    }
    for name in ["vectorize", "crossProduct"] {
        let mut editor = Editor::new();
        input_text(&mut editor, &format!("{name}("));
        assert!(matches!(
            editor.to_portable().unwrap().kind,
            NodeKind::Call { .. }
        ));
    }
}

#[test]
fn global_rhs_saved_result_is_drawn_at_the_source_unit_slot_without_fake_editable_nodes() {
    let source = binary(
        BinaryOp::GlobalDefine,
        identifier("g"),
        binary(
            BinaryOp::Evaluate,
            number("7"),
            PortableTree {
                kind: NodeKind::Placeholder,
                children: vec![],
            },
        ),
    );
    let editor = Editor::from_portable(&source).unwrap();
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    ctx.run_ui(egui::RawInput::default(), |ui| {
        let layout = measure_with_result(
            ui.painter(),
            editor.arena(),
            editor.root(),
            FONT_SIZE,
            Some("cached-seven"),
        );
        fn contains(layout: &FormulaBox, text: &str) -> bool {
            layout
                .primitives
                .iter()
                .any(|p| matches!(p,Primitive::Text(_,g) if g.text()==text))
                || layout
                    .children
                    .iter()
                    .any(|(_, child)| contains(child, text))
        }
        assert!(contains(&layout, "cached-seven"));
        let mut hits = vec![];
        collect_hits(&layout, Pos2::ZERO, &mut hits);
        let rhs = editor.arena().get(editor.root()).unwrap().children()[1];
        let unit = editor.arena().get(rhs).unwrap().children()[1];
        assert!(hits.iter().any(|hit| hit.node == unit));
        assert!(
            hits.iter()
                .all(|hit| editor.arena().get(hit.node).is_some())
        );
        let display = crate::result_display::DisplayMath::OverArrow(Box::new(
            crate::result_display::DisplayMath::Text("x".into(), MathStyle::Operator),
        ));
        let result = measure_display_math(ui.painter(), editor.root(), &display, FONT_SIZE);
        assert_eq!(
            result
                .primitives
                .iter()
                .filter(|p| matches!(p, Primitive::Line(..)))
                .count(),
            3
        );
        assert!(
            result.children.is_empty(),
            "cached result glyphs must not become editable source cells"
        );
    })
    .drop_without_applying_deltas();
    assert_eq!(editor.to_portable().unwrap(), source);
}

#[test]
fn native_table_expression_context_rejects_both_definition_inputs_without_dirty_or_undo() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    for event in [egui::Event::Text(":".into()), key(Key::Backtick, shifted())] {
        let source = identifier("table_value");
        let mut editor = Editor::from_portable(&source).unwrap();
        editor.set_expressions_only(true);
        let revision = editor.arena().revision();
        let outcome = input_text_with_actions(&mut editor, ":");
        assert!(!outcome.document_changed && !outcome.evaluation_requested);
        ctx.run_ui(
            egui::RawInput {
                events: vec![event],
                ..Default::default()
            },
            |ui| {
                let response = formula_ui_with_actions(ui, &mut editor, None, true);
                assert!(!response.response.changed());
                assert!(!response.evaluation_requested);
            },
        )
        .drop_without_applying_deltas();
        assert_eq!(editor.to_portable().unwrap(), source);
        assert_eq!(editor.arena().revision(), revision);
        assert!(!editor.undo());
    }
}
