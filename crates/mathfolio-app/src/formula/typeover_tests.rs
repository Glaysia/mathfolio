use super::*;
fn number(value: &str) -> PortableTree {
    PortableTree {
        kind: NodeKind::Number(value.into()),
        children: vec![],
    }
}
fn source(kind: NodeKind) -> PortableTree {
    PortableTree {
        kind,
        children: vec![number("12"), number("2")],
    }
}
fn key(key: Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}
fn start(ctx: &egui::Context, editor: &mut Editor) {
    crate::configure(ctx);
    ctx.run_ui(Default::default(), |ui| {
        formula_ui_with_actions(ui, editor, None, true);
    })
    .drop_without_applying_deltas();
    editor.place_caret(editor.root(), Direction::Right).unwrap();
}
fn event(ctx: &egui::Context, editor: &mut Editor, event: egui::Event) -> (bool, egui::FullOutput) {
    let mut changed = false;
    let output = ctx.run_ui(
        egui::RawInput {
            events: vec![event],
            ..Default::default()
        },
        |ui| {
            let response = formula_ui_with_actions(ui, editor, None, true);
            changed = response.response.changed();
            assert!(!response.evaluation_requested);
        },
    );
    (changed, output)
}

#[test]
fn actual_first_delete_selects_red_without_mutating_source_second_delete_is_one_undo() {
    for kind in [NodeKind::Power, NodeKind::Fraction] {
        let power = kind == NodeKind::Power;
        let ctx = egui::Context::default();
        let tree = source(kind);
        let mut editor = Editor::from_portable(&tree).unwrap();
        start(&ctx, &mut editor);
        let origin = editor.caret();
        let revision = editor.arena().revision();
        let (changed, output) = event(&ctx, &mut editor, key(Key::Backspace));
        assert!(!changed);
        assert_eq!(editor.typeover_target(), Some(editor.root()));
        assert_eq!(editor.to_portable().unwrap(), tree);
        assert_eq!(editor.caret(), origin);
        assert_eq!(editor.arena().revision(), revision);
        assert!(output.shapes.iter().any(|shape|matches!(&shape.shape,egui::Shape::Rect(rect) if rect.fill==Color32::from_rgba_unmultiplied(255,0,0,64))));
        assert!(!output.shapes.iter().any(|shape|matches!(&shape.shape,egui::Shape::LineSegment{stroke,..} if stroke.color==CARET)));
        output.drop_without_applying_deltas();
        assert!(!editor.undo());
        let (changed, output) = event(&ctx, &mut editor, key(Key::Delete));
        output.drop_without_applying_deltas();
        assert!(changed);
        if power {
            assert_eq!(editor.to_portable().unwrap(), number("12"));
            assert_eq!(editor.caret().value_range.unwrap().focus, 2);
        } else {
            assert_eq!(editor.to_portable().unwrap().kind, NodeKind::Placeholder);
            assert_eq!(editor.caret().caret_side, Direction::Left);
        }
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), tree);
        assert_eq!(editor.typeover_target(), Some(editor.root()));
        assert_eq!(editor.caret(), origin);
        assert!(editor.redo());
        assert_eq!(editor.typeover_target(), None);
    }
}

#[test]
fn actual_typeover_text_replaces_whole_term_and_arrows_only_cancel_first() {
    let ctx = egui::Context::default();
    let tree = source(NodeKind::Fraction);
    let mut editor = Editor::from_portable(&tree).unwrap();
    start(&ctx, &mut editor);
    let (_, output) = event(&ctx, &mut editor, key(Key::Backspace));
    output.drop_without_applying_deltas();
    let (changed, output) = event(&ctx, &mut editor, egui::Event::Text("24".into()));
    output.drop_without_applying_deltas();
    assert!(changed);
    assert_eq!(editor.to_portable().unwrap(), number("24"));
    assert!(editor.undo());
    assert_eq!(editor.typeover_target(), Some(editor.root()));
    let origin = editor.caret();
    let (changed, output) = event(&ctx, &mut editor, key(Key::ArrowLeft));
    output.drop_without_applying_deltas();
    assert!(!changed);
    assert_eq!(editor.typeover_target(), None);
    assert_eq!(editor.caret(), origin);
    assert_eq!(editor.to_portable().unwrap(), tree);
    let (changed, output) = event(&ctx, &mut editor, key(Key::ArrowLeft));
    output.drop_without_applying_deltas();
    assert!(!changed);
    assert_ne!(editor.caret(), origin);
    assert_eq!(editor.to_portable().unwrap(), tree);
    assert!(editor.redo());
    assert_eq!(editor.to_portable().unwrap(), number("24"));
}

#[test]
fn actual_typeover_operators_use_empty_templates_and_close_paren_keeps_the_pending_term() {
    for (input, kind) in [
        ("+", NodeKind::Binary(BinaryOp::Add)),
        ("/", NodeKind::Fraction),
        ("^", NodeKind::Power),
        ("-", NodeKind::UnaryNeg),
        ("%", NodeKind::Percent),
        ("!", NodeKind::Factorial),
        ("(", NodeKind::Group),
        ("|", NodeKind::Absolute),
    ] {
        let ctx = egui::Context::default();
        let old = source(NodeKind::Fraction);
        let mut editor = Editor::from_portable(&old).unwrap();
        start(&ctx, &mut editor);
        let (_, output) = event(&ctx, &mut editor, key(Key::Backspace));
        output.drop_without_applying_deltas();
        let (changed, output) = event(&ctx, &mut editor, egui::Event::Text(input.into()));
        output.drop_without_applying_deltas();
        assert!(changed, "{input}");
        let tree = editor.to_portable().unwrap();
        assert_eq!(tree.kind, kind);
        assert!(
            tree.children
                .iter()
                .all(|child| child.kind == NodeKind::Placeholder)
        );
        let first = editor.arena().get(editor.root()).unwrap().children()[0];
        assert_eq!(editor.caret().node, first);
        assert_eq!(editor.caret().caret_side, Direction::Indeterminate);
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), old);
        assert_eq!(editor.typeover_target(), Some(editor.root()));
        let (changed, output) = event(&ctx, &mut editor, egui::Event::Text(")".into()));
        output.drop_without_applying_deltas();
        assert!(!changed);
        assert_eq!(editor.to_portable().unwrap(), old);
        assert_eq!(editor.typeover_target(), Some(editor.root()));
    }
}

#[test]
fn typeover_equals_inserts_blank_evaluation_but_existing_evaluation_only_requests_calculation() {
    for wrapped in [false, true] {
        let fraction = source(NodeKind::Fraction);
        let old = if wrapped {
            PortableTree {
                kind: NodeKind::Binary(BinaryOp::Evaluate),
                children: vec![
                    fraction,
                    PortableTree {
                        kind: NodeKind::Placeholder,
                        children: vec![],
                    },
                ],
            }
        } else {
            fraction
        };
        let mut editor = Editor::from_portable(&old).unwrap();
        let target = if wrapped {
            editor.arena().get(editor.root()).unwrap().children()[0]
        } else {
            editor.root()
        };
        editor.place_caret(target, Direction::Right).unwrap();
        assert!(!editor.delete_structure(true));
        assert_eq!(editor.typeover_target(), Some(target));
        let outcome = input_text_with_actions(&mut editor, "=");
        assert!(outcome.evaluation_requested);
        assert_eq!(outcome.document_changed, !wrapped);
        if wrapped {
            assert_eq!(editor.to_portable().unwrap(), old);
            assert_eq!(editor.typeover_target(), Some(target));
            assert!(!editor.undo());
        } else {
            let tree = editor.to_portable().unwrap();
            assert_eq!(tree.kind, NodeKind::Binary(BinaryOp::Evaluate));
            assert!(
                tree.children
                    .iter()
                    .all(|child| child.kind == NodeKind::Placeholder)
            );
            assert_eq!(editor.caret().caret_side, Direction::Left);
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), old);
            assert_eq!(editor.typeover_target(), Some(target));
        }
    }
}

#[test]
fn typeover_colon_replaces_a_bare_fraction_as_one_definition_command() {
    let old = source(NodeKind::Fraction);
    let mut editor = Editor::from_portable(&old).unwrap();
    editor.place_caret(editor.root(), Direction::Right).unwrap();
    assert!(!editor.delete_structure(true));
    let outcome = input_text_with_actions(&mut editor, ":");
    assert!(outcome.document_changed && !outcome.evaluation_requested);
    let tree = editor.to_portable().unwrap();
    assert_eq!(tree.kind, NodeKind::Binary(BinaryOp::Define));
    assert!(
        tree.children
            .iter()
            .all(|child| child.kind == NodeKind::Placeholder)
    );
    assert!(editor.undo());
    assert_eq!(editor.to_portable().unwrap(), old);
    assert_eq!(editor.typeover_target(), Some(editor.root()));
}
