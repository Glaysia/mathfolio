use super::*;

fn literal(text: &str) -> PortableTree {
    PortableTree {
        kind: NodeKind::StringLiteral(text.into()),
        children: vec![],
    }
}

#[test]
fn descriptor_and_wire_keep_source_contents_and_old_variants() {
    let tree = literal(" 12 ");
    assert_eq!(tree.kind.descriptor_name(), "STRING");
    assert_eq!(tree.kind.precedence(), i32::MAX);
    assert_eq!(tree.kind.arity(), 0);
    assert_eq!(
        serde_json::to_string(&tree).unwrap(),
        r#"{"kind":{"StringLiteral":" 12 "},"children":[]}"#
    );
    assert_eq!(
        serde_json::to_string(&NodeKind::Number("12".into())).unwrap(),
        r#"{"Number":"12"}"#
    );
    let loaded: PortableTree =
        serde_json::from_str(&serde_json::to_string(&tree).unwrap()).unwrap();
    assert_eq!(loaded, tree);
}

#[test]
fn quote_creates_empty_value_not_placeholder_and_one_undo_restores_caret() {
    let mut e = Editor::new();
    let id = e.root();
    let before = e.caret();
    assert!(e.insert_string_literal());
    assert_eq!(e.root(), id);
    assert_eq!(e.to_portable().unwrap(), literal(""));
    assert!(e.is_inside_string_literal());
    assert_eq!(
        e.caret().value_range,
        Some(ValueRange {
            anchor: 0,
            focus: 0
        })
    );
    assert!(e.undo());
    assert_eq!(e.caret(), before);
    assert_eq!(e.to_portable().unwrap().kind, NodeKind::Placeholder);
    assert!(!e.undo());
    assert!(e.redo());
    assert_eq!(e.to_portable().unwrap(), literal(""));
}

#[test]
fn inner_operator_characters_spaces_and_ascii_quote_are_real_contents() {
    let mut e = Editor::new();
    e.insert_string_literal();
    assert_eq!(e.insert_literal_text(" 12 + (한글)=x "), Some(true));
    assert!(e.insert_string_literal());
    assert_eq!(e.to_portable().unwrap(), literal(" 12 + (한글)=x \""));
    assert!(e.is_inside_string_literal());
    assert_eq!(e.insert_literal_text("\\&<>"), Some(true));
    assert!(matches!(
        e.to_portable().unwrap().kind,
        NodeKind::StringLiteral(_)
    ));
    assert!(!e.insert_typed_operator(NodeKind::Binary(BinaryOp::Add)));
    assert!(e.undo());
    assert_eq!(e.to_portable().unwrap(), literal(" 12 + (한글)=x \""));
}

#[test]
fn utf16_reverse_selection_replacement_and_invalid_boundary_are_atomic() {
    let mut e = Editor::from_portable(&literal("a😀한b")).unwrap();
    let id = e.root();
    e.place_text_selection(id, 4, 1).unwrap();
    let selected = e.caret();
    assert_eq!(e.insert_literal_text("가"), Some(true));
    assert_eq!(e.to_portable().unwrap(), literal("a가b"));
    assert_eq!(e.caret().value_range.unwrap().focus, 2);
    assert!(e.undo());
    assert_eq!(e.caret(), selected);
    assert_eq!(e.to_portable().unwrap(), literal("a😀한b"));
    let revision = e.arena().revision();
    assert_eq!(
        e.place_text_caret(id, 2),
        Err(EditError::InvalidTextBoundary)
    );
    assert_eq!(e.caret(), selected);
    assert_eq!(e.arena().revision(), revision);
    assert!(e.redo());
    assert_eq!(e.to_portable().unwrap(), literal("a가b"));
}

#[test]
fn delete_last_character_and_all_selected_content_keep_empty_string() {
    for backwards in [false, true] {
        let mut e = Editor::from_portable(&literal("z")).unwrap();
        e.place_text_caret(e.root(), usize::from(backwards))
            .unwrap();
        let before = e.caret();
        assert_eq!(e.delete_literal_text(backwards), Some(true));
        assert_eq!(e.to_portable().unwrap(), literal(""));
        assert_eq!(e.delete_literal_text(backwards), Some(false));
        assert!(e.undo());
        assert_eq!(e.to_portable().unwrap(), literal("z"));
        assert_eq!(e.caret(), before);
        assert!(!e.undo());
        assert!(e.redo());
        assert_eq!(e.to_portable().unwrap(), literal(""));
    }
    let mut e = Editor::from_portable(&literal("a😀b")).unwrap();
    e.place_text_selection(e.root(), 4, 0).unwrap();
    assert!(e.delete_structure(true));
    assert_eq!(e.to_portable().unwrap(), literal(""));
    assert!(e.undo());
    assert_eq!(e.to_portable().unwrap(), literal("a😀b"));
    assert_eq!(
        e.caret().value_range,
        Some(ValueRange {
            anchor: 4,
            focus: 0
        })
    );
}

#[test]
fn supplementary_scalar_deletion_does_not_create_an_unpaired_surrogate() {
    let mut e = Editor::from_portable(&literal("x😀y")).unwrap();
    e.place_text_caret(e.root(), 3).unwrap();
    assert_eq!(e.delete_literal_text(true), Some(true));
    assert_eq!(e.to_portable().unwrap(), literal("xy"));
    assert_eq!(e.caret().value_range.unwrap().focus, 1);
}

#[test]
fn stored_xml_text_and_new_authoring_have_separate_validation_boundaries() {
    assert!(literal("\t\r\n😀").validate().is_ok());
    for text in ["\0", "\u{1}", "\u{fffe}", "\u{ffff}"] {
        assert!(literal(text).validate().is_err());
    }
    let mut e = Editor::new();
    e.insert_string_literal();
    for text in ["\n", "\t", "\u{200d}", "\u{e000}", "😀"] {
        let revision = e.arena().revision();
        assert_eq!(e.insert_literal_text(text), Some(false));
        assert_eq!(e.arena().revision(), revision);
    }
    assert_eq!(e.insert_literal_text("한글 α 12"), Some(true));
}

#[test]
fn rejected_or_equal_replacement_keeps_redo_and_history() {
    let mut e = Editor::new();
    e.insert_string_literal();
    e.insert_literal_text("abc");
    assert!(e.undo());
    let revision = e.arena().revision();
    assert_eq!(e.insert_literal_text(""), Some(false));
    assert_eq!(e.insert_literal_text("\n"), Some(false));
    assert_eq!(e.arena().revision(), revision);
    assert!(e.redo());
    e.place_text_selection(e.root(), 0, 3).unwrap();
    let before = e.caret();
    let revision = e.arena().revision();
    assert_eq!(e.insert_literal_text("abc"), Some(false));
    assert_eq!(e.caret(), before);
    assert_eq!(e.arena().revision(), revision);
}

#[test]
fn independent_text_budget_rejection_is_atomic_without_native_length_claim() {
    let mut e = Editor::new();
    e.insert_string_literal();
    assert_eq!(
        e.insert_literal_text(&"a".repeat(MAX_ATOM_BYTES)),
        Some(true)
    );
    let revision = e.arena().revision();
    let caret = e.caret();
    assert_eq!(e.insert_literal_text("b"), Some(false));
    assert_eq!(e.arena().revision(), revision);
    assert_eq!(e.caret(), caret);
    assert!(e.undo());
    assert_eq!(e.to_portable().unwrap(), literal(""));
}

#[test]
fn plain_conversion_retains_position_but_not_identifier_metadata_undo_restores_it() {
    let source = PortableTree {
        kind: NodeKind::NativeIdentifier {
            name: "abcd".into(),
            labels: "UNIT".into(),
            contextual: None,
            subscript: None,
        },
        children: vec![],
    };
    let mut e = Editor::from_portable(&source).unwrap();
    e.place_text_caret(e.root(), 2).unwrap();
    let before = e.caret();
    assert!(e.insert_string_literal());
    assert_eq!(e.to_portable().unwrap(), literal("abcd"));
    assert_eq!(e.caret().value_range.unwrap().focus, 2);
    assert!(e.undo());
    assert_eq!(e.to_portable().unwrap(), source);
    assert_eq!(e.caret(), before);
}

#[test]
fn structured_name_scope_and_matrix_selection_do_not_guess_conversion() {
    let source = PortableTree {
        kind: NodeKind::NativeIdentifier {
            name: "x".into(),
            labels: "VARIABLE".into(),
            contextual: Some(true),
            subscript: Some("dc".into()),
        },
        children: vec![],
    };
    let mut e = Editor::from_portable(&source).unwrap();
    e.place_text_caret(e.root(), 0).unwrap();
    assert!(!e.insert_string_literal());
    assert_eq!(e.to_portable().unwrap(), source);
    assert!(!e.undo());
    e.select_subtree(e.root()).unwrap();
    assert!(!e.insert_string_literal());
    let matrix = PortableTree {
        kind: NodeKind::Matrix {
            rows: 1,
            cols: 1,
            display: None,
        },
        children: vec![literal("a")],
    };
    let mut e = Editor::from_portable(&matrix).unwrap();
    e.select_matrix_cells(e.root(), 0, 0);
    assert!(!e.insert_string_literal());
    assert_eq!(e.insert_literal_text("a"), None);
    assert_eq!(e.to_portable().unwrap(), matrix);
}

#[test]
fn navigation_enters_and_leaves_distinct_literal_edges_without_edit_or_undo() {
    let mut e = Editor::from_portable(&literal("a😀")).unwrap();
    let id = e.root();
    e.place_caret(id, Direction::Left).unwrap();
    let revision = e.arena().revision();
    assert!(!e.is_inside_string_literal());
    assert_eq!(e.move_literal_caret(Direction::Right, false), Some(true));
    assert_eq!(e.caret().value_range.unwrap().focus, 0);
    assert_eq!(e.move_literal_caret(Direction::Right, true), Some(true));
    assert_eq!(e.move_literal_caret(Direction::Right, true), Some(true));
    assert_eq!(
        e.caret().value_range,
        Some(ValueRange {
            anchor: 0,
            focus: 3
        })
    );
    e.move_literal_caret(Direction::Right, false);
    assert_eq!(e.caret().value_range.unwrap().focus, 3);
    e.move_literal_caret(Direction::Right, false);
    assert_eq!(e.caret().caret_side, Direction::Right);
    assert!(!e.is_inside_string_literal());
    assert_eq!(e.insert_literal_text("+"), None);
    assert_eq!(e.arena().revision(), revision);
    assert!(!e.undo());
    e.move_literal_caret(Direction::Left, false);
    assert!(e.is_inside_string_literal());
    assert_eq!(e.caret().value_range.unwrap().focus, 3);
}

#[test]
fn outer_edge_restores_typed_operator_policy_and_preserves_string_operand() {
    let mut e = Editor::from_portable(&literal("12")).unwrap();
    e.place_caret(e.root(), Direction::Right).unwrap();
    assert!(e.insert_typed_operator(NodeKind::Binary(BinaryOp::Add)));
    let tree = e.to_portable().unwrap();
    assert_eq!(tree.kind, NodeKind::Binary(BinaryOp::Add));
    assert_eq!(tree.children[0], literal("12"));
    assert_eq!(tree.children[1].kind, NodeKind::Placeholder);
    assert!(e.undo());
    assert_eq!(e.to_portable().unwrap(), literal("12"));
}
