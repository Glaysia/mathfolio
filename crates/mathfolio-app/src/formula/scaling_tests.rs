//! Typed implicit SCALE, its original shortcut and focus-only operator glyph.
use super::*;

fn number(value: &str) -> PortableTree {
    PortableTree {
        kind: NodeKind::Number(value.into()),
        children: vec![],
    }
}
fn name(value: &str) -> PortableTree {
    PortableTree {
        kind: NodeKind::Identifier(value.into()),
        children: vec![],
    }
}
fn binary(op: BinaryOp, left: PortableTree, right: PortableTree) -> PortableTree {
    PortableTree {
        kind: NodeKind::Binary(op),
        children: vec![left, right],
    }
}
fn count(tree: &PortableTree, op: BinaryOp) -> usize {
    usize::from(tree.kind == NodeKind::Binary(op))
        + tree
            .children
            .iter()
            .map(|child| count(child, op))
            .sum::<usize>()
}
fn key(modifiers: egui::Modifiers, pressed: bool) -> egui::Event {
    egui::Event::Key {
        key: Key::U,
        physical_key: Some(Key::U),
        pressed,
        repeat: false,
        modifiers,
    }
}

#[test]
fn adjacent_numeric_and_complete_expression_operands_create_scale_but_star_stays_mult() {
    for (input, scales, mults) in [
        ("5y", 1, 0),
        ("(2+3)m", 1, 0),
        ("(2+3)4", 1, 0),
        ("2m+3cm", 2, 0),
        ("5*y", 0, 1),
        ("x*y", 0, 1),
        ("xy", 0, 0),
        ("2e-3", 0, 0),
        ("2i", 0, 0),
    ] {
        let mut editor = Editor::new();
        let outcome = input_text_with_actions(&mut editor, input);
        assert!(outcome.document_changed && !outcome.evaluation_requested);
        let tree = editor.to_portable().unwrap();
        assert_eq!(
            (
                count(&tree, BinaryOp::Scale),
                count(&tree, BinaryOp::Multiply)
            ),
            (scales, mults),
            "{input}: {tree:?}"
        );
        let xml = crate::worksheet_math::tree_to_mathml(&tree).unwrap();
        assert_eq!(crate::worksheet_math::tree_from_mathml(&xml).unwrap(), tree);
    }
    let mut editor = Editor::new();
    input_text(&mut editor, "2m^2");
    assert_eq!(
        editor.to_portable().unwrap(),
        binary(
            BinaryOp::Scale,
            number("2"),
            PortableTree {
                kind: NodeKind::Power,
                children: vec![name("m"), number("2")]
            }
        )
    );
}

#[test]
fn scale_shortcut_requires_exact_ctrl_shift_u_and_creates_one_undoable_edit() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    for (modifiers, pressed, changed) in [
        (
            egui::Modifiers {
                shift: true,
                ..egui::Modifiers::COMMAND
            },
            true,
            true,
        ),
        (egui::Modifiers::COMMAND, true, false),
        (
            egui::Modifiers {
                shift: true,
                alt: true,
                ..egui::Modifiers::COMMAND
            },
            true,
            false,
        ),
        (
            egui::Modifiers {
                shift: true,
                ..egui::Modifiers::COMMAND
            },
            false,
            false,
        ),
    ] {
        let source = number("5");
        let mut editor = Editor::from_portable(&source).unwrap();
        ctx.run_ui(Default::default(), |ui| {
            formula_ui_with_focus(ui, &mut editor, None, true);
        })
        .drop_without_applying_deltas();
        ctx.run_ui(
            egui::RawInput {
                events: vec![key(modifiers, pressed)],
                ..Default::default()
            },
            |ui| {
                let response = formula_ui_with_actions(ui, &mut editor, None, true);
                assert!(!response.evaluation_requested);
            },
        )
        .drop_without_applying_deltas();
        if changed {
            assert_eq!(
                editor.to_portable().unwrap(),
                binary(
                    BinaryOp::Scale,
                    source.clone(),
                    PortableTree {
                        kind: NodeKind::Placeholder,
                        children: vec![]
                    }
                )
            );
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), source);
            assert!(!editor.undo());
        } else {
            assert_eq!(editor.to_portable().unwrap(), source);
            assert!(!editor.undo());
        }
    }
}

#[test]
fn scale_shortcut_on_a_matrix_cell_range_keeps_source_selection_and_history_unchanged() {
    let source = PortableTree {
        kind: NodeKind::Matrix {
            rows: 2,
            cols: 2,
            display: None,
        },
        children: vec![number("1"), number("2"), number("3"), number("4")],
    };
    let mut editor = Editor::from_portable(&source).unwrap();
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    ctx.run_ui(Default::default(), |ui| {
        formula_ui_with_focus(ui, &mut editor, None, true);
    })
    .drop_without_applying_deltas();
    assert!(editor.select_matrix_cells(editor.root(), 0, 3));
    let selected = editor.matrix_selection();
    let revision = editor.arena().revision();
    ctx.run_ui(
        egui::RawInput {
            events: vec![key(
                egui::Modifiers {
                    shift: true,
                    ..egui::Modifiers::COMMAND
                },
                true,
            )],
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
    assert_eq!(editor.matrix_selection(), selected);
    assert!(!editor.undo());
    assert!(!editor.redo());
}

#[test]
fn focus_glyph_is_the_original_ring_and_hidden_paint_keeps_the_same_intrinsic_layout() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let tree = binary(BinaryOp::Scale, number("5"), name("y"));
    let editor = Editor::from_portable(&tree).unwrap();
    let mut sizes = vec![];
    for focused in [false, true, false] {
        let output = ctx.run_ui(Default::default(), |ui| {
            let layout = measure(ui.painter(), editor.arena(), editor.root(), FONT_SIZE);
            sizes.push(layout.size);
            assert!(
                layout
                    .primitives
                    .iter()
                    .any(|p| matches!(p,Primitive::FocusedText(_,g) if g.text()=="∘"))
            );
            paint_box_with_focus(ui.painter(), &layout, pos2(20., 20.), focused);
        });
        let rings = output
            .shapes
            .iter()
            .filter(|shape| matches!(&shape.shape,egui::Shape::Text(t) if t.galley.text()=="∘"))
            .count();
        assert_eq!(rings, usize::from(focused));
        output.drop_without_applying_deltas();
    }
    assert!(sizes.windows(2).all(|pair| pair[0] == pair[1]));
}

#[test]
fn readonly_and_result_paint_do_not_publish_the_editing_scale_marker() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let editor = Editor::from_portable(&binary(BinaryOp::Scale, number("5"), name("m"))).unwrap();
    let output = ctx.run_ui(Default::default(), |ui| {
        formula_readonly_ui_styled(ui, &editor, None, None, 12.);
    });
    assert!(
        !output
            .shapes
            .iter()
            .any(|shape| matches!(&shape.shape,egui::Shape::Text(t) if t.galley.text()=="∘"))
    );
    output.drop_without_applying_deltas();
}

#[test]
fn typed_implicit_units_keep_dimension_arithmetic_and_explicit_evaluation_boundary() {
    for input in ["2m+3cm=", "2*m+3*cm="] {
        let mut editor = Editor::new();
        let typed = input_text_with_actions(&mut editor, input.trim_end_matches('='));
        assert!(!typed.evaluation_requested);
        let evaluation = input_text_with_actions(&mut editor, "=");
        assert!(evaluation.evaluation_requested);
        let fragment =
            crate::worksheet_math::tree_to_mathml(&editor.to_portable().unwrap()).unwrap();
        let xml = crate::worksheet_math::worksheet_xml(&[(1, 0., 0., fragment)]).unwrap();
        let row = mathfolio_compat::evaluate_worksheet(&xml)
            .unwrap()
            .remove(0);
        assert!(row.error.is_none(), "{input}: {row:?}");
        let Some(mathfolio_compat::Value::Scalar { quantity }) = row.value else {
            panic!("{row:?}")
        };
        assert!((quantity.value.re - 2.03).abs() < 1e-12);
        assert_eq!(
            quantity.dimension,
            mathfolio_core::unit("m").unwrap().dimension
        );
    }
}
