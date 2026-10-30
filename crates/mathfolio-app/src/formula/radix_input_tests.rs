use super::*;

fn leaf(kind: NodeKind) -> PortableTree {
    PortableTree {
        kind,
        children: vec![],
    }
}
fn number(text: &str) -> PortableTree {
    leaf(NodeKind::Number(text.into()))
}
fn name(text: &str) -> PortableTree {
    leaf(NodeKind::Identifier(text.into()))
}
fn pair(op: BinaryOp, left: PortableTree, right: PortableTree) -> PortableTree {
    PortableTree {
        kind: NodeKind::Binary(op),
        children: vec![left, right],
    }
}
fn based(digits: &str, base: NumericBase) -> PortableTree {
    leaf(NodeKind::based_number(digits, base).unwrap())
}

#[test]
fn lowercase_suffix_is_mode_gated_and_undo_restores_the_original_caret() {
    for (digits, ch, base) in [
        ("1101", 'b', NumericBase::Binary),
        ("32", 'o', NumericBase::Octal),
        ("2", 'h', NumericBase::Hexadecimal),
    ] {
        let source = number(digits);
        let mut editor = Editor::from_portable(&source).unwrap();
        editor
            .place_text_caret(editor.root(), digits.len())
            .unwrap();
        let caret = editor.caret();
        let revision = editor.arena().revision();
        assert_eq!(input_suffix(&mut editor, ch), None);
        assert_eq!(editor.to_portable().unwrap(), source);
        assert_eq!(editor.caret(), caret);
        assert_eq!(editor.arena().revision(), revision);
        editor.set_bin_oct_hex_mode(true);
        assert_eq!(input_suffix(&mut editor, ch), Some(true));
        assert_eq!(editor.to_portable().unwrap(), based(digits, base));
        assert_eq!(editor.caret().value_range.unwrap().focus, digits.len() + 1);
        assert!(editor.undo());
        assert_eq!(editor.to_portable().unwrap(), source);
        assert_eq!(editor.caret(), caret);
        assert!(editor.bin_oct_hex_mode());
        assert!(editor.redo());
        assert_eq!(editor.to_portable().unwrap(), based(digits, base));
    }
}

#[test]
fn hex_direct_scale_fusion_is_one_command_and_preserves_surrounding_ids_and_metadata() {
    let sibling = leaf(NodeKind::NativeIdentifier {
        name: "k".into(),
        labels: "UNIT".into(),
        contextual: Some(true),
        subscript: Some("dc".into()),
    });
    let source = pair(
        BinaryOp::Add,
        sibling.clone(),
        pair(BinaryOp::Scale, number("0"), name("a32")),
    );
    let mut editor = Editor::from_portable(&source).unwrap();
    editor.set_bin_oct_hex_mode(true);
    let root = editor.root();
    let left_id = editor.arena().get(root).unwrap().children()[0];
    let scale_id = editor.arena().get(root).unwrap().children()[1];
    let tail_id = editor.arena().get(scale_id).unwrap().children()[1];
    editor.place_text_caret(tail_id, 3).unwrap();
    let caret = editor.caret();
    assert_eq!(input_suffix(&mut editor, 'h'), Some(true));
    assert_eq!(
        editor.to_portable().unwrap(),
        pair(
            BinaryOp::Add,
            sibling,
            based("0a32", NumericBase::Hexadecimal)
        )
    );
    assert_eq!(editor.root(), root);
    assert_eq!(editor.arena().get(root).unwrap().children()[0], left_id);
    assert_eq!(editor.caret().value_range.unwrap().focus, 5);
    assert!(editor.undo());
    assert_eq!(editor.to_portable().unwrap(), source);
    assert_eq!(editor.caret(), caret);
    assert_eq!(editor.arena().get(root).unwrap().children()[1], scale_id);
    assert!(!editor.undo());
    assert!(editor.redo());
    assert_eq!(
        editor.to_portable().unwrap().children[1],
        based("0a32", NumericBase::Hexadecimal)
    );
}

#[test]
fn labels_subscripts_explicit_multiply_and_nonhex_names_are_not_erased() {
    for tail in [
        NodeKind::AnnotatedIdentifier {
            name: "a32".into(),
            labels: "UNIT".into(),
        },
        NodeKind::NativeIdentifier {
            name: "a32".into(),
            labels: "UNIT".into(),
            contextual: Some(true),
            subscript: None,
        },
        NodeKind::NativeIdentifier {
            name: "a32".into(),
            labels: "VARIABLE".into(),
            contextual: None,
            subscript: Some("dc".into()),
        },
        NodeKind::Identifier("foo".into()),
        NodeKind::StringLiteral("a32".into()),
    ] {
        let source = pair(BinaryOp::Scale, number("0"), leaf(tail));
        let mut editor = Editor::from_portable(&source).unwrap();
        editor.set_bin_oct_hex_mode(true);
        let tail = editor.arena().get(editor.root()).unwrap().children()[1];
        editor.place_caret(tail, Direction::Right).unwrap();
        let caret = editor.caret();
        let revision = editor.arena().revision();
        assert_eq!(input_suffix(&mut editor, 'h'), None);
        assert_eq!(editor.to_portable().unwrap(), source);
        assert_eq!(editor.caret(), caret);
        assert_eq!(editor.arena().revision(), revision);
    }
    let source = pair(BinaryOp::Multiply, number("0"), name("a32"));
    let mut editor = Editor::from_portable(&source).unwrap();
    editor.set_bin_oct_hex_mode(true);
    let tail = editor.arena().get(editor.root()).unwrap().children()[1];
    editor.place_caret(tail, Direction::Right).unwrap();
    assert_eq!(input_suffix(&mut editor, 'h'), None);
    assert_eq!(editor.to_portable().unwrap(), source);
}

#[test]
fn partial_scope_subscript_and_callee_boundaries_leave_the_source_and_redo_untouched() {
    let mut editor = Editor::new();
    editor
        .set_atom(editor.root(), NodeKind::Number("13".into()))
        .unwrap();
    editor
        .set_atom(editor.root(), NodeKind::Number("14".into()))
        .unwrap();
    assert!(editor.undo());
    editor.set_bin_oct_hex_mode(true);
    editor.place_text_selection(editor.root(), 2, 1).unwrap();
    assert_eq!(input_suffix(&mut editor, 'b'), None);
    assert_eq!(edited_kind(&editor, editor.root(), "1b"), None);
    assert!(editor.redo());
    assert_eq!(editor.to_portable().unwrap(), number("14"));
    editor.place_text_caret(editor.root(), 0).unwrap();
    assert_eq!(input_suffix(&mut editor, 'b'), None);
    editor.select_subtree(editor.root()).unwrap();
    assert_eq!(input_suffix(&mut editor, 'b'), None);
    editor.place_caret(editor.root(), Direction::Right).unwrap();
    editor.toggle_subscript();
    assert_eq!(input_suffix(&mut editor, 'b'), None);
    let call = PortableTree {
        kind: NodeKind::Call { argument_count: 1 },
        children: vec![
            PortableTree {
                kind: NodeKind::Index { indices: 1 },
                children: vec![number("10"), number("1")],
            },
            leaf(NodeKind::Placeholder),
        ],
    };
    let mut editor = Editor::from_portable(&call).unwrap();
    editor.set_bin_oct_hex_mode(true);
    let callee = editor.arena().get(editor.root()).unwrap().children()[0];
    let base = editor.arena().get(callee).unwrap().children()[0];
    editor.place_text_caret(base, 2).unwrap();
    assert_eq!(input_suffix(&mut editor, 'b'), None);
    assert_eq!(edited_kind(&editor, base, "10b"), None);
    assert_eq!(editor.to_portable().unwrap(), call);
}

#[test]
fn existing_based_source_reclassifies_suffix_edits_without_promoting_ordinary_mode_off_text() {
    let mut editor = Editor::from_portable(&based("1101", NumericBase::Binary)).unwrap();
    let id = editor.root();
    assert!(!editor.bin_oct_hex_mode());
    editor.place_text_selection(id, 5, 4).unwrap();
    assert_eq!(
        edited_kind(&editor, id, "1101"),
        Some(NodeKind::Number("1101".into()))
    );
    assert_eq!(
        edited_kind(&editor, id, "1101h"),
        Some(NodeKind::based_number("1101", NumericBase::Hexadecimal).unwrap())
    );
    assert_eq!(edited_kind(&editor, id, ""), Some(NodeKind::Placeholder));
    assert_eq!(edited_kind(&editor, id, "1H"), None);
    assert_eq!(edited_kind(&editor, id, "0a32"), None);
    let mut decimal = Editor::from_portable(&number("1101")).unwrap();
    let id = decimal.root();
    assert_eq!(edited_kind(&decimal, id, "1101b"), None);
    decimal.set_bin_oct_hex_mode(true);
    assert_eq!(
        edited_kind(&decimal, id, "1101b"),
        Some(NodeKind::based_number("1101", NumericBase::Binary).unwrap())
    );
}

#[test]
fn recognized_over_budget_input_is_an_atomic_noop_and_uppercase_delegates() {
    let source = number(&"0".repeat(mathfolio_editor::MAX_ATOM_BYTES));
    let mut editor = Editor::from_portable(&source).unwrap();
    editor.set_bin_oct_hex_mode(true);
    let caret = editor.caret();
    let revision = editor.arena().revision();
    assert_eq!(input_suffix(&mut editor, 'b'), Some(false));
    assert_eq!(input_suffix(&mut editor, 'B'), None);
    assert_eq!(editor.to_portable().unwrap(), source);
    assert_eq!(editor.caret(), caret);
    assert_eq!(editor.arena().revision(), revision);
    assert!(!editor.undo());
}

#[test]
fn decimal_based_continuation_retains_both_values_and_one_command_undo_in_either_mode() {
    for mode in [false, true] {
        for (ch, tail) in [('3', "b3"), ('c', "bc")] {
            let source = pair(
                BinaryOp::Evaluate,
                based("2", NumericBase::Binary),
                leaf(NodeKind::Placeholder),
            );
            let mut editor = Editor::from_portable(&source).unwrap();
            editor.set_bin_oct_hex_mode(mode);
            let root = editor.root();
            let original = editor.arena().get(root).unwrap().children()[0];
            let result = editor.arena().get(root).unwrap().children()[1];
            editor.place_text_caret(original, 2).unwrap();
            let caret = editor.caret();
            assert_eq!(input_suffix(&mut editor, ch), Some(true));
            assert_eq!(
                editor.to_portable().unwrap(),
                pair(
                    BinaryOp::Evaluate,
                    pair(BinaryOp::Scale, number("2"), name(tail)),
                    leaf(NodeKind::Placeholder)
                )
            );
            assert_eq!(editor.root(), root);
            assert_eq!(editor.arena().get(root).unwrap().children()[1], result);
            assert_eq!(
                editor.arena().get(original).unwrap().kind(),
                &NodeKind::Number("2".into())
            );
            assert_eq!(editor.caret().value_range.unwrap().focus, 2);
            assert!(editor.undo());
            assert_eq!(editor.to_portable().unwrap(), source);
            assert_eq!(editor.caret(), caret);
            assert!(!editor.undo());
            assert!(editor.redo());
            assert_eq!(
                editor.to_portable().unwrap().children[0],
                pair(BinaryOp::Scale, number("2"), name(tail))
            );
        }
    }
}

#[test]
fn based_continuation_over_the_complete_evaluate_budget_preserves_source_caret_and_redo() {
    let mut cell = number("0");
    for _ in 0..3 {
        cell = PortableTree {
            kind: NodeKind::Group,
            children: vec![cell],
        };
    }
    let source = pair(
        BinaryOp::Evaluate,
        based("2", NumericBase::Binary),
        PortableTree {
            kind: NodeKind::Matrix {
                rows: 1,
                cols: 4095,
                display: None,
            },
            children: vec![cell; 4095],
        },
    );
    source.validate().unwrap(); // 16,383 nodes before the two-node insertion.
    let mut editor = Editor::from_portable(&source).unwrap();
    editor.set_bin_oct_hex_mode(true);
    let left = editor.arena().get(editor.root()).unwrap().children()[0];
    editor
        .set_atom(
            left,
            NodeKind::based_number("3", NumericBase::Binary).unwrap(),
        )
        .unwrap();
    assert!(editor.undo());
    editor.place_text_caret(left, 2).unwrap();
    let caret = editor.caret();
    let revision = editor.arena().revision();
    assert_eq!(input_suffix(&mut editor, '3'), Some(false));
    assert_eq!(editor.to_portable().unwrap(), source);
    assert_eq!(editor.caret(), caret);
    assert_eq!(editor.arena().revision(), revision);
    assert!(editor.redo());
    assert_eq!(
        editor.arena().get(left).unwrap().kind(),
        &NodeKind::based_number("3", NumericBase::Binary).unwrap()
    );
}

#[test]
fn actual_character_pipeline_builds_lowercase_based_values_but_keeps_mode_off_and_uppercase_scale()
{
    use super::super::input_text_with_actions;
    for (text, expected) in [
        ("1101b", based("1101", NumericBase::Binary)),
        ("32o", based("32", NumericBase::Octal)),
        ("0a32h", based("0a32", NumericBase::Hexadecimal)),
        ("2b3ch", based("2b3c", NumericBase::Hexadecimal)),
    ] {
        let mut editor = Editor::new();
        editor.set_bin_oct_hex_mode(true);
        let outcome = input_text_with_actions(&mut editor, text);
        assert!(outcome.document_changed && !outcome.evaluation_requested);
        assert_eq!(editor.to_portable().unwrap(), expected, "{text}");
    }
    for (mode, text, letter) in [(false, "2b", "b"), (true, "2B", "B")] {
        let mut editor = Editor::new();
        editor.set_bin_oct_hex_mode(mode);
        input_text_with_actions(&mut editor, text);
        assert_eq!(
            editor.to_portable().unwrap(),
            pair(BinaryOp::Scale, number("2"), name(letter))
        );
    }
}
