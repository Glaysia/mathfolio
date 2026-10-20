use super::*;
fn n(value: &str) -> PortableTree {
    PortableTree {
        kind: NodeKind::Number(value.into()),
        children: vec![],
    }
}
fn op(kind: NodeKind, a: PortableTree, b: PortableTree) -> PortableTree {
    PortableTree {
        kind,
        children: vec![a, b],
    }
}
fn add(a: PortableTree, b: PortableTree) -> PortableTree {
    op(NodeKind::Binary(BinaryOp::Add), a, b)
}

#[test]
fn actual_operator_center_text_replaces_the_operator_and_retains_both_operands() {
    for (input, kind) in [
        ("*", NodeKind::Binary(BinaryOp::Multiply)),
        ("-", NodeKind::Binary(BinaryOp::Subtract)),
        ("/", NodeKind::Fraction),
        ("^", NodeKind::Power),
        ("<", NodeKind::Binary(BinaryOp::LessThan)),
    ] {
        let ctx = egui::Context::default();
        crate::configure(&ctx);
        let source = add(n("12"), n("34"));
        let mut editor = Editor::from_portable(&source).unwrap();
        ctx.run_ui(Default::default(), |ui| {
            formula_ui_with_actions(ui, &mut editor, None, true);
        })
        .drop_without_applying_deltas();
        editor
            .place_operator_caret(editor.root(), Direction::Indeterminate)
            .unwrap();
        let caret = editor.caret();
        ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Text(input.into())],
                ..Default::default()
            },
            |ui| {
                let response = formula_ui_with_actions(ui, &mut editor, None, true);
                assert!(response.response.changed() && !response.evaluation_requested);
            },
        )
        .drop_without_applying_deltas();
        assert_eq!(editor.to_portable().unwrap(), op(kind, n("12"), n("34")));
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), source);
        assert_eq!(editor.caret(), caret);
        assert!(editor.redo());
        assert!(!editor.redo());
    }
}

#[test]
fn actual_center_replacement_in_an_additive_chain_keeps_the_nearest_operands() {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    let mut editor = Editor::from_portable(&add(add(n("1"), n("2")), n("3"))).unwrap();
    ctx.run_ui(Default::default(), |ui| {
        formula_ui_with_actions(ui, &mut editor, None, true);
    })
    .drop_without_applying_deltas();
    editor
        .place_operator_caret(editor.root(), Direction::Indeterminate)
        .unwrap();
    ctx.run_ui(
        egui::RawInput {
            events: vec![egui::Event::Text("*".into())],
            ..Default::default()
        },
        |ui| {
            assert!(
                formula_ui_with_actions(ui, &mut editor, None, true)
                    .response
                    .changed()
            );
        },
    )
    .drop_without_applying_deltas();
    assert_eq!(
        editor.to_portable().unwrap(),
        add(
            n("1"),
            op(NodeKind::Binary(BinaryOp::Multiply), n("2"), n("3"))
        )
    );
}
