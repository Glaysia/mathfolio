use super::*;
fn atom(kind: NodeKind) -> PortableTree {
    PortableTree {
        kind,
        children: vec![],
    }
}
fn native(name: &str, suffix: Option<&str>) -> NodeKind {
    NodeKind::NativeIdentifier {
        name: name.into(),
        labels: "VARIABLE".into(),
        contextual: None,
        subscript: suffix.map(str::to_owned),
    }
}
fn setup(ctx: &egui::Context, editor: &mut Editor) {
    crate::configure(ctx);
    ctx.run_ui(Default::default(), |ui| {
        formula_ui_with_actions(ui, editor, None, true);
    })
    .drop_without_applying_deltas();
}
fn event(ctx: &egui::Context, editor: &mut Editor, event: egui::Event) -> bool {
    let mut changed = false;
    ctx.run_ui(
        egui::RawInput {
            events: vec![event],
            ..Default::default()
        },
        |ui| {
            let response = formula_ui_with_actions(ui, editor, None, true);
            changed = response.response.changed();
            assert!(!response.evaluation_requested);
        },
    )
    .drop_without_applying_deltas();
    changed
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

#[test]
fn actual_last_labelled_baseline_character_becomes_placeholder_and_undo_restores_metadata() {
    for kind in [
        native("V", None),
        NodeKind::AnnotatedIdentifier {
            name: "V".into(),
            labels: "UNIT".into(),
        },
    ] {
        let ctx = egui::Context::default();
        let old = atom(kind);
        let mut editor = Editor::from_portable(&old).unwrap();
        setup(&ctx, &mut editor);
        editor.place_text_caret(editor.root(), 1).unwrap();
        assert!(event(&ctx, &mut editor, key(Key::Backspace)));
        assert_eq!(editor.to_portable().unwrap(), atom(NodeKind::Placeholder));
        assert_eq!(editor.caret().caret_side, Direction::Left);
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), old);
        assert_eq!(editor.caret().value_range.unwrap().focus, 1);
    }
}

#[test]
fn actual_suffix_start_and_base_end_delete_across_the_formatted_boundary() {
    let ctx = egui::Context::default();
    let mut editor = Editor::from_portable(&atom(native("Voltage", Some("dc")))).unwrap();
    setup(&ctx, &mut editor);
    editor
        .place_subscript_selection(editor.root(), 0, 0)
        .unwrap();
    assert!(event(&ctx, &mut editor, key(Key::Backspace)));
    assert_eq!(
        editor.to_portable().unwrap(),
        atom(native("Voltag", Some("dc")))
    );
    assert_eq!(editor.caret().insertion, InsertionMode::Normal);
    assert_eq!(editor.caret().value_range.unwrap().focus, 6);
    editor.place_text_caret(editor.root(), 6).unwrap();
    assert!(event(&ctx, &mut editor, key(Key::Delete)));
    assert_eq!(
        editor.to_portable().unwrap(),
        atom(native("Voltag", Some("c")))
    );
    assert_eq!(editor.caret().value_range.unwrap().focus, 6);
}

#[test]
fn actual_last_suffix_delete_keeps_pending_subscript_input_and_undo() {
    let ctx = egui::Context::default();
    let old = atom(native("V", Some("d")));
    let mut editor = Editor::from_portable(&old).unwrap();
    setup(&ctx, &mut editor);
    editor
        .place_subscript_selection(editor.root(), 1, 1)
        .unwrap();
    assert!(event(&ctx, &mut editor, key(Key::Backspace)));
    assert_eq!(editor.to_portable().unwrap(), atom(native("V", None)));
    assert_eq!(editor.caret().insertion, InsertionMode::Subscript);
    assert_eq!(editor.editing_text(editor.root()), Some(""));
    assert!(event(&ctx, &mut editor, egui::Event::Text("c".into())));
    assert_eq!(editor.to_portable().unwrap(), atom(native("V", Some("c"))));
    assert!(editor.undo());
    assert_eq!(editor.caret().insertion, InsertionMode::Subscript);
    assert_eq!(editor.editing_text(editor.root()), Some(""));
    assert!(editor.undo());
    assert_eq!(editor.to_portable().unwrap(), old);
}

#[test]
fn actual_imaginary_symbol_deletion_reclassifies_the_complete_literal() {
    for key_action in [Key::Backspace, Key::Delete] {
        let ctx = egui::Context::default();
        let mut editor = Editor::from_portable(&atom(NodeKind::ImaginarySymbol {
            value: "12".into(),
            symbol: "j".into(),
        }))
        .unwrap();
        setup(&ctx, &mut editor);
        if key_action == Key::Backspace {
            editor.place_caret(editor.root(), Direction::Right).unwrap();
        } else {
            editor.place_text_caret(editor.root(), 2).unwrap();
        }
        assert!(event(&ctx, &mut editor, key(key_action)));
        assert_eq!(
            editor.to_portable().unwrap(),
            atom(NodeKind::Number("12".into()))
        );
    }
}

#[test]
fn actual_formatted_fusion_preserves_explicit_left_label_and_right_suffix() {
    let ctx = egui::Context::default();
    let right = NodeKind::NativeIdentifier {
        name: "B".into(),
        labels: "UNIT".into(),
        contextual: Some(true),
        subscript: Some("dc".into()),
    };
    let source = PortableTree {
        kind: NodeKind::Binary(BinaryOp::Add),
        children: vec![
            atom(NodeKind::AnnotatedIdentifier {
                name: "A".into(),
                labels: "VARIABLE".into(),
            }),
            atom(right),
        ],
    };
    let mut editor = Editor::from_portable(&source).unwrap();
    setup(&ctx, &mut editor);
    editor
        .place_operator_caret(editor.root(), Direction::Indeterminate)
        .unwrap();
    assert!(event(&ctx, &mut editor, key(Key::Backspace)));
    assert_eq!(
        editor.to_portable().unwrap(),
        atom(native("AB", Some("dc")))
    );
    assert_eq!(editor.caret().insertion, InsertionMode::Normal);
    assert_eq!(editor.caret().value_range.unwrap().focus, 1);
    assert!(editor.undo());
    assert_eq!(editor.to_portable().unwrap(), source);
}
