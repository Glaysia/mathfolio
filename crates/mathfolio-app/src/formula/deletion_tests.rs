//! Native deletion joins integer operands and preserves the resulting text caret.
use super::*;

fn number(value: &str) -> PortableTree {
    PortableTree {
        kind: NodeKind::Number(value.into()),
        children: vec![],
    }
}
fn binary(left: &str, right: &str) -> PortableTree {
    PortableTree {
        kind: NodeKind::Binary(BinaryOp::Add),
        children: vec![number(left), number(right)],
    }
}
fn pressed(key: Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}
fn prepare(ctx: &egui::Context, editor: &mut Editor) {
    crate::configure(ctx);
    ctx.run_ui(Default::default(), |ui| {
        formula_ui_with_actions(ui, editor, None, true);
    })
    .drop_without_applying_deltas();
}
fn command(ctx: &egui::Context, editor: &mut Editor, key: Key) -> bool {
    let mut changed = false;
    ctx.run_ui(
        egui::RawInput {
            events: vec![pressed(key)],
            ..Default::default()
        },
        |ui| {
            let result = formula_ui_with_actions(ui, editor, None, true);
            changed = result.response.changed();
            assert!(!result.evaluation_requested);
        },
    )
    .drop_without_applying_deltas();
    changed
}

#[test]
fn actual_outward_text_delete_removes_the_operator_and_joins_the_two_integer_operands() {
    for key in [Key::Backspace, Key::Delete] {
        let source = binary("12", "34");
        let mut editor = Editor::from_portable(&source).unwrap();
        let ctx = egui::Context::default();
        prepare(&ctx, &mut editor);
        let slot = usize::from(key == Key::Backspace);
        let operand = editor.arena().get(editor.root()).unwrap().children()[slot];
        let offset = if key == Key::Backspace { 0 } else { 2 };
        editor.place_text_caret(operand, offset).unwrap();
        let before = editor.caret();
        assert!(command(&ctx, &mut editor, key));
        assert_eq!(editor.to_portable().unwrap(), number("1234"));
        let range = editor.caret().value_range.unwrap();
        assert_eq!((range.anchor, range.focus), (2, 2));
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), source);
        assert_eq!(editor.caret(), before);
        assert!(editor.redo());
        assert_eq!(editor.to_portable().unwrap(), number("1234"));
        assert!(input_text(&mut editor, "0"));
        assert_eq!(editor.to_portable().unwrap(), number("12034"));
    }
}

#[test]
fn actual_operator_center_delete_joins_integers_without_dropping_either_operand() {
    for key in [Key::Backspace, Key::Delete] {
        let mut editor = Editor::from_portable(&binary("12", "34")).unwrap();
        let ctx = egui::Context::default();
        prepare(&ctx, &mut editor);
        editor
            .place_caret(editor.root(), Direction::Indeterminate)
            .unwrap();
        assert!(command(&ctx, &mut editor, key));
        assert_eq!(editor.to_portable().unwrap(), number("1234"));
        assert_eq!(editor.caret().value_range.unwrap().focus, 2);
    }
}

#[test]
fn actual_group_boundary_delete_removes_both_fences_and_keeps_the_same_side() {
    for side in [Direction::Left, Direction::Right] {
        for key in [Key::Backspace, Key::Delete] {
            let tree = PortableTree {
                kind: NodeKind::Group,
                children: vec![number("12")],
            };
            let mut editor = Editor::from_portable(&tree).unwrap();
            let ctx = egui::Context::default();
            prepare(&ctx, &mut editor);
            editor.place_caret(editor.root(), side).unwrap();
            assert!(command(&ctx, &mut editor, key));
            assert_eq!(editor.to_portable().unwrap(), number("12"));
            let xml =
                crate::worksheet_math::tree_to_mathml(&editor.to_portable().unwrap()).unwrap();
            assert_eq!(
                crate::worksheet_math::tree_from_mathml(&xml).unwrap(),
                number("12")
            );
            assert!(input_text(&mut editor, "0"));
            assert_eq!(
                editor.to_portable().unwrap(),
                number(if side == Direction::Left {
                    "012"
                } else {
                    "120"
                })
            );
        }
    }
}

#[test]
fn actual_pointer_selects_the_binary_glyph_center_before_deleting_the_operator() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let source = binary("12", "34");
    let mut editor = Editor::from_portable(&source).unwrap();
    let mut point = Pos2::ZERO;
    ctx.run_ui(Default::default(), |ui| {
        let result = formula_ui_with_actions(ui, &mut editor, None, true);
        let layout = measure(ui.painter(), editor.arena(), editor.root(), FONT_SIZE);
        let bounds = layout
            .operator_hits
            .iter()
            .find_map(|(target, rect)| {
                matches!(target, OperatorTarget::Side(Direction::Indeterminate)).then_some(rect)
            })
            .unwrap();
        point = bounds.center() + result.response.rect.min.to_vec2() + vec2(PADDING, PADDING);
    })
    .drop_without_applying_deltas();
    for pressed in [true, false] {
        ctx.run_ui(
            egui::RawInput {
                events: vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..Default::default()
            },
            |ui| {
                let response = formula_ui_with_actions(ui, &mut editor, None, true);
                assert!(!response.response.changed() && !response.evaluation_requested);
            },
        )
        .drop_without_applying_deltas();
    }
    assert_eq!(editor.to_portable().unwrap(), source);
    assert_eq!(editor.caret().node, editor.root());
    assert_eq!(editor.caret().caret_side, Direction::Indeterminate);
    assert_eq!(editor.caret().node_side, Direction::Indeterminate);
    assert!(command(&ctx, &mut editor, Key::Backspace));
    assert_eq!(editor.to_portable().unwrap(), number("1234"));
}

#[test]
fn actual_operator_delete_uses_valid_lexical_fusion_and_retains_invalid_decimal_operands() {
    for key in [Key::Backspace, Key::Delete] {
        for (source, expected, join) in [
            (binary("1.2", "3"), number("1.23"), 3),
            (
                PortableTree {
                    kind: NodeKind::Binary(BinaryOp::Add),
                    children: vec![
                        PortableTree {
                            kind: NodeKind::Identifier("x".into()),
                            children: vec![],
                        },
                        PortableTree {
                            kind: NodeKind::Identifier("y".into()),
                            children: vec![],
                        },
                    ],
                },
                PortableTree {
                    kind: NodeKind::Identifier("xy".into()),
                    children: vec![],
                },
                1,
            ),
        ] {
            let mut editor = Editor::from_portable(&source).unwrap();
            let ctx = egui::Context::default();
            prepare(&ctx, &mut editor);
            editor
                .place_operator_caret(editor.root(), Direction::Indeterminate)
                .unwrap();
            assert!(command(&ctx, &mut editor, key));
            assert_eq!(editor.to_portable().unwrap(), expected);
            assert_eq!(editor.caret().value_range.unwrap().focus, join);
        }
        let mut editor = Editor::from_portable(&binary("1.2", "3.4")).unwrap();
        let ctx = egui::Context::default();
        prepare(&ctx, &mut editor);
        editor
            .place_operator_caret(editor.root(), Direction::Indeterminate)
            .unwrap();
        assert!(command(&ctx, &mut editor, key));
        assert_eq!(
            editor.to_portable().unwrap(),
            PortableTree {
                kind: NodeKind::Binary(BinaryOp::Scale),
                children: vec![number("1.2"), number("3.4")],
            }
        );
        let slot = usize::from(key == Key::Backspace);
        assert_eq!(
            editor.caret().node,
            editor.arena().get(editor.root()).unwrap().children()[slot]
        );
        assert_eq!(
            editor.caret().value_range.unwrap().focus,
            if slot == 1 { 0 } else { 3 }
        );
    }
}

#[test]
fn selected_binary_uses_blue_operator_geometry_and_gray_operand_bounds_without_rebuilding_galleys()
{
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut editor = Editor::from_portable(&binary("12", "34")).unwrap();
    prepare(&ctx, &mut editor);
    editor
        .place_operator_caret(editor.root(), Direction::Indeterminate)
        .unwrap();
    let before = editor.to_portable().unwrap();
    let mut original = None;
    let output = ctx.run_ui(Default::default(), |ui| {
        let layout = retained_layout(ui, &editor, None, FONT_SIZE, None);
        original = layout
            .primitives
            .iter()
            .find_map(|primitive| match primitive {
                Primitive::Text(_, galley) => Some(galley.clone()),
                _ => None,
            });
        let response = formula_ui_with_actions(ui, &mut editor, None, true);
        assert!(!response.response.changed());
    });
    let blue = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.override_text_color == Some(CARET) => Some(text),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(blue.len(), 1);
    assert!(Arc::ptr_eq(&original.unwrap(), &blue[0].galley));
    let gray = Color32::from_rgba_unmultiplied(169, 169, 169, 64);
    assert_eq!(
        output
            .shapes
            .iter()
            .filter(|shape| matches!(&shape.shape, egui::Shape::Rect(rect) if rect.fill == gray))
            .count(),
        2
    );
    assert_eq!(editor.to_portable().unwrap(), before);
    output.drop_without_applying_deltas();
}
