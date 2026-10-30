//! Direct native notation input and structural, headless rendering checks.
use super::*;

fn n(value: &str) -> PortableTree {
    PortableTree {
        kind: NodeKind::Number(value.into()),
        children: vec![],
    }
}
fn unary(kind: NodeKind, child: PortableTree) -> PortableTree {
    PortableTree {
        kind,
        children: vec![child],
    }
}
fn binary(op: BinaryOp, left: PortableTree, right: PortableTree) -> PortableTree {
    PortableTree {
        kind: NodeKind::Binary(op),
        children: vec![left, right],
    }
}
fn key(modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key: Key::P,
        physical_key: Some(Key::P),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

#[test]
fn percent_and_degree_are_postfix_commands_with_one_undo_and_native_xml() {
    for (input, kind, native) in [
        ("%", NodeKind::Percent, "percent"),
        ("°", NodeKind::Degree, "degree_op"),
    ] {
        let source = n("50");
        let mut editor = Editor::from_portable(&source).unwrap();
        editor.place_caret(editor.root(), Direction::Right).unwrap();
        let outcome = input_text_with_actions(&mut editor, input);
        assert!(outcome.document_changed && !outcome.evaluation_requested);
        let tree = editor.to_portable().unwrap();
        assert_eq!(tree, unary(kind, source.clone()));
        let xml = crate::worksheet_math::tree_to_mathml(&tree).unwrap();
        assert!(xml.contains(&format!("<ml:{native}/>")));
        assert_eq!(crate::worksheet_math::tree_from_mathml(&xml).unwrap(), tree);
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), source);
        assert!(!editor.undo());
        assert!(editor.redo());
    }
}

#[test]
fn literal_name_suffix_keeps_percent_and_degree_as_name_text() {
    let mut editor = Editor::from_portable(&PortableTree {
        kind: NodeKind::Identifier("x".into()),
        children: vec![],
    })
    .unwrap();
    editor.place_caret(editor.root(), Direction::Right).unwrap();
    editor.toggle_literal_subscript();
    let outcome = input_text_with_actions(&mut editor, "p%°");
    assert!(outcome.document_changed && !outcome.evaluation_requested);
    assert!(
        matches!(editor.to_portable().unwrap().kind, NodeKind::NativeIdentifier {subscript: Some(s), ..} if s == "p%°")
    );
}

#[test]
fn polar_shortcut_has_exact_modifiers_and_does_not_request_calculation() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    for (modifiers, insert) in [
        (egui::Modifiers::COMMAND, false),
        (egui::Modifiers::SHIFT, false),
        (
            egui::Modifiers {
                shift: true,
                ..egui::Modifiers::COMMAND
            },
            true,
        ),
        (
            egui::Modifiers {
                shift: true,
                alt: true,
                ..egui::Modifiers::COMMAND
            },
            false,
        ),
    ] {
        let source = n("2");
        let mut editor = Editor::from_portable(&source).unwrap();
        editor.place_caret(editor.root(), Direction::Right).unwrap();
        ctx.run_ui(
            egui::RawInput {
                events: vec![key(modifiers)],
                ..Default::default()
            },
            |ui| {
                let response = formula_ui_with_actions(ui, &mut editor, None, true);
                assert_eq!(response.response.changed(), insert);
                assert!(!response.evaluation_requested);
            },
        )
        .drop_without_applying_deltas();
        if insert {
            let tree = editor.to_portable().unwrap();
            assert_eq!(tree.kind, NodeKind::Binary(BinaryOp::Polar));
            assert_eq!(tree.children[0], source);
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), source);
        } else {
            assert_eq!(editor.to_portable().unwrap(), source);
        }
    }
}

#[test]
fn notation_input_does_not_replace_matrix_range_typeover() {
    let source = PortableTree {
        kind: NodeKind::Matrix {
            rows: 2,
            cols: 1,
            display: None,
        },
        children: vec![n("1"), n("2")],
    };
    let mut editor = Editor::from_portable(&source).unwrap();
    editor.select_matrix_cells(editor.root(), 0, 1);
    let outcome = input_text_with_actions(&mut editor, "%°∠");
    assert_eq!(outcome, InputOutcome::default());
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    ctx.run_ui(
        egui::RawInput {
            events: vec![key(egui::Modifiers {
                shift: true,
                ..egui::Modifiers::COMMAND
            })],
            ..Default::default()
        },
        |ui| {
            let response = formula_ui_with_actions(ui, &mut editor, None, true);
            assert!(!response.response.changed() && !response.evaluation_requested);
        },
    )
    .drop_without_applying_deltas();
    assert_eq!(editor.to_portable().unwrap(), source);
    assert!(!editor.undo());
}

#[test]
fn read_only_percent_degree_and_polar_use_vector_glyphs_and_keep_source_hits() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    for (source, glyph) in [
        (unary(NodeKind::Percent, n("20")), "%"),
        (unary(NodeKind::Degree, n("90")), "°"),
        (
            binary(BinaryOp::Polar, n("2"), unary(NodeKind::Degree, n("90"))),
            "∠",
        ),
    ] {
        let editor = Editor::from_portable(&source).unwrap();
        ctx.run_ui(egui::RawInput::default(), |ui| {
            let layout = readonly_layout(ui, &editor, None, None, 12.);
            assert!(
                layout
                    .primitives
                    .iter()
                    .any(|p| matches!(p,Primitive::Text(_,g) if g.job.text == glyph))
            );
            let mut hits = vec![];
            collect_hits(&layout, Pos2::ZERO, &mut hits);
            assert!(
                hits.iter()
                    .all(|hit| editor.arena().get(hit.node).is_some())
            );
            let measured = measured_size_readonly_styled(ui, &editor, None, None, 12.);
            let painted = formula_readonly_ui_styled(ui, &editor, None, None, 12.);
            let delta = measured - painted.rect.size();
            let tolerance = ui.ctx().pixels_per_point().recip();
            assert!(delta.x.abs() <= tolerance && delta.y.abs() <= tolerance);
            assert!(!painted.changed() && !painted.has_focus());
        })
        .drop_without_applying_deltas();
        assert_eq!(editor.to_portable().unwrap(), source);
    }
}

#[test]
fn equal_precedence_polar_and_cross_keep_their_different_associativity_visible() {
    let polar = NodeKind::Binary(BinaryOp::Polar);
    let cross = NodeKind::Binary(BinaryOp::CrossProduct);
    assert!(cross_operand_requires_group(
        BinaryOp::CrossProduct,
        &polar,
        true
    ));
    assert!(cross_operand_requires_group(
        BinaryOp::CrossProduct,
        &polar,
        false
    ));
    assert!(cross_operand_requires_group(BinaryOp::Polar, &polar, false));
    assert!(!cross_operand_requires_group(BinaryOp::Polar, &polar, true));
    assert!(!cross_operand_requires_group(BinaryOp::Polar, &cross, true));
    assert!(!cross_operand_requires_group(
        BinaryOp::Polar,
        &cross,
        false
    ));
    // The raw imported AST is preserved; only its presentation adds fences.
    let source = binary(
        BinaryOp::Polar,
        n("2"),
        binary(BinaryOp::Polar, n("3"), n("4")),
    );
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let editor = Editor::from_portable(&source).unwrap();
    ctx.run_ui(egui::RawInput::default(), |ui| {
        let layout = readonly_layout(ui, &editor, None, None, 12.);
        assert!(
            layout.children[1]
                .1
                .primitives
                .iter()
                .any(|p| matches!(p, Primitive::Line(_, _)))
        );
    })
    .drop_without_applying_deltas();
    assert_eq!(editor.to_portable().unwrap(), source);
}

#[test]
fn percent_power_group_rule_is_not_applied_to_degree_power() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    for kind in [NodeKind::Percent, NodeKind::Degree] {
        let source = unary(
            kind.clone(),
            PortableTree {
                kind: NodeKind::Power,
                children: vec![n("2"), n("3")],
            },
        );
        let editor = Editor::from_portable(&source).unwrap();
        ctx.run_ui(egui::RawInput::default(), |ui| {
            let layout = readonly_layout(ui, &editor, None, None, 12.);
            let has_fence = layout.children[0]
                .1
                .primitives
                .iter()
                .any(|p| matches!(p, Primitive::Line(_, _)));
            assert_eq!(has_fence, kind == NodeKind::Percent);
        })
        .drop_without_applying_deltas();
        assert_eq!(editor.to_portable().unwrap(), source);
    }
}

#[test]
fn ordinary_text_entry_builds_and_evaluates_original_scalar_notation() {
    for (text, re, im, physical) in [
        ("50%=", 0.5, 0., false),
        ("90°=", std::f64::consts::FRAC_PI_2, 0., false),
        ("2∠90°=", 0., 2., false),
        ("200*10%=", 20., 0., false),
        ("sin(90°)=", 1., 0., false),
        ("(2∠90°)*V=", 0., 2., true),
    ] {
        let mut editor = Editor::new();
        let outcome = input_text_with_actions(&mut editor, text);
        assert!(outcome.document_changed && outcome.evaluation_requested);
        let xml = crate::worksheet_math::tree_to_mathml(&editor.to_portable().unwrap()).unwrap();
        let worksheet = crate::worksheet_math::worksheet_xml(&[(1, 0., 0., xml)]).unwrap();
        let rows = mathfolio_compat::evaluate_worksheet(&worksheet).unwrap();
        assert!(rows[0].error.is_none(), "{text}: {:?}", rows[0]);
        let Some(mathfolio_compat::Value::Scalar { quantity }) = &rows[0].value else {
            panic!("{text}: {:?}", rows[0])
        };
        assert!(
            (quantity.value.re - re).abs() < 1e-12 && (quantity.value.im - im).abs() < 1e-12,
            "{text}: {quantity:?}"
        );
        assert_eq!(
            quantity.dimension,
            if physical {
                mathfolio_core::unit("V").unwrap().dimension
            } else {
                [0.; 7]
            }
        );
    }
}
