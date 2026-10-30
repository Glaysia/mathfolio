//! Headless STRING input/native/geometry integration; no original runtime.
use super::*;

fn literal(text: &str) -> PortableTree {
    PortableTree {
        kind: NodeKind::StringLiteral(text.into()),
        children: vec![],
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

fn context() -> egui::Context {
    let ctx = egui::Context::default();
    crate::configure(&ctx);
    ctx
}

fn frame(ctx: &egui::Context, editor: &mut Editor, events: Vec<egui::Event>) -> (bool, bool) {
    let mut observed = None;
    ctx.run_ui(
        egui::RawInput {
            events,
            ..Default::default()
        },
        |ui| {
            let drawn = formula_ui_with_actions(ui, editor, None, true);
            observed = Some((drawn.response.changed(), drawn.evaluation_requested));
        },
    )
    .drop_without_applying_deltas();
    observed.unwrap()
}

#[test]
fn one_text_event_creates_quotes_then_keeps_operators_and_ascii_quote_as_contents() {
    let mut editor = Editor::new();
    let original = editor.caret();
    let outcome = input_text_with_actions(&mut editor, "\" 12+(한글)^x=\"");
    assert!(outcome.document_changed);
    assert!(!outcome.evaluation_requested);
    assert_eq!(editor.to_portable().unwrap(), literal(" 12+(한글)^x=\""));
    assert!(editor.is_inside_string_literal());
    assert!(editor.undo());
    assert_eq!(editor.to_portable().unwrap().kind, NodeKind::Placeholder);
    assert_eq!(editor.caret(), original);
    assert!(!editor.undo());
    assert!(editor.redo());
    assert_eq!(editor.to_portable().unwrap(), literal(" 12+(한글)^x=\""));
}

#[test]
fn actual_widget_suppresses_math_shortcuts_but_accepts_literal_space_and_equals() {
    let ctx = context();
    let mut editor = Editor::new();
    frame(&ctx, &mut editor, vec![]);
    assert_eq!(
        frame(&ctx, &mut editor, vec![egui::Event::Text("\"x".into())]),
        (true, false)
    );
    let source = editor.to_portable().unwrap();
    let caret = editor.caret();
    let revision = editor.arena().revision();
    let shifted = egui::Modifiers {
        shift: true,
        ..egui::Modifiers::COMMAND
    };
    for (code, modifiers) in [
        (Key::Minus, egui::Modifiers::COMMAND),
        (Key::Q, egui::Modifiers::COMMAND),
        (Key::J, egui::Modifiers::COMMAND),
        (Key::U, shifted),
        (Key::P, shifted),
        (Key::Z, shifted),
        (Key::Space, egui::Modifiers::NONE),
    ] {
        assert_eq!(
            frame(&ctx, &mut editor, vec![key(code, modifiers)]),
            (false, false),
            "{code:?}"
        );
        assert_eq!(editor.to_portable().unwrap(), source);
        assert_eq!(editor.caret(), caret);
        assert_eq!(editor.arena().revision(), revision);
    }
    assert_eq!(
        frame(&ctx, &mut editor, vec![egui::Event::Text(" =+/()".into())]),
        (true, false)
    );
    assert_eq!(editor.to_portable().unwrap(), literal("x =+/()"));
    assert_eq!(editor.caret().insertion, InsertionMode::Normal);
}

#[test]
fn actual_widget_bmp_selection_deletion_and_undo_keep_empty_string_type() {
    let ctx = context();
    let source = literal("한글 α");
    let mut editor = Editor::from_portable(&source).unwrap();
    frame(&ctx, &mut editor, vec![]);
    editor.place_text_selection(editor.root(), 4, 0).unwrap();
    let selected = editor.caret();
    assert_eq!(
        frame(
            &ctx,
            &mut editor,
            vec![key(Key::Backspace, egui::Modifiers::NONE)]
        ),
        (true, false)
    );
    assert_eq!(editor.to_portable().unwrap(), literal(""));
    assert_eq!(
        frame(
            &ctx,
            &mut editor,
            vec![key(Key::Delete, egui::Modifiers::NONE)]
        ),
        (false, false)
    );
    assert_eq!(
        frame(
            &ctx,
            &mut editor,
            vec![key(Key::Z, egui::Modifiers::COMMAND)]
        ),
        (true, false)
    );
    assert_eq!(editor.to_portable().unwrap(), source);
    assert_eq!(editor.caret(), selected);
    assert_eq!(
        frame(&ctx, &mut editor, vec![egui::Event::Text("나".into())]),
        (true, false)
    );
    assert_eq!(editor.to_portable().unwrap(), literal("나"));
    assert_eq!(
        frame(
            &ctx,
            &mut editor,
            vec![key(Key::Backspace, egui::Modifiers::NONE)]
        ),
        (true, false)
    );
    assert_eq!(editor.to_portable().unwrap(), literal(""));
}

#[test]
fn literal_equals_under_existing_evaluation_never_requests_calculation() {
    let tree = PortableTree {
        kind: NodeKind::Binary(BinaryOp::Evaluate),
        children: vec![
            literal("x"),
            PortableTree {
                kind: NodeKind::Placeholder,
                children: vec![],
            },
        ],
    };
    let mut editor = Editor::from_portable(&tree).unwrap();
    let literal_id = editor.arena().get(editor.root()).unwrap().children()[0];
    editor.place_text_caret(literal_id, 1).unwrap();
    let outcome = input_text_with_actions(&mut editor, "===");
    assert!(outcome.document_changed && !outcome.evaluation_requested);
    let next = editor.to_portable().unwrap();
    assert_eq!(next.kind, tree.kind);
    assert_eq!(next.children[0], literal("x==="));
    assert_eq!(next.children[1], tree.children[1]);
}

#[test]
fn widget_arrows_visit_inner_end_and_outer_edges_before_math_gestures_resume() {
    let ctx = context();
    let mut editor = Editor::from_portable(&literal("a")).unwrap();
    frame(&ctx, &mut editor, vec![]);
    editor.place_caret(editor.root(), Direction::Left).unwrap();
    let revision = editor.arena().revision();
    assert_eq!(
        frame(
            &ctx,
            &mut editor,
            vec![key(Key::ArrowRight, egui::Modifiers::NONE)]
        ),
        (false, false)
    );
    assert_eq!(editor.caret().value_range.unwrap().focus, 0);
    assert!(editor.is_inside_string_literal());
    frame(
        &ctx,
        &mut editor,
        vec![key(Key::ArrowRight, egui::Modifiers::NONE)],
    );
    assert_eq!(editor.caret().value_range.unwrap().focus, 1);
    frame(
        &ctx,
        &mut editor,
        vec![key(Key::ArrowRight, egui::Modifiers::NONE)],
    );
    assert_eq!(editor.caret().caret_side, Direction::Right);
    assert!(!editor.is_inside_string_literal());
    assert_eq!(editor.arena().revision(), revision);
    assert!(!editor.undo());
    assert_eq!(
        frame(&ctx, &mut editor, vec![egui::Event::Text("+".into())]),
        (true, false)
    );
    let tree = editor.to_portable().unwrap();
    assert_eq!(tree.kind, NodeKind::Binary(BinaryOp::Add));
    assert_eq!(tree.children[0], literal("a"));
}

#[test]
fn curved_quotes_are_display_only_and_body_hit_offsets_count_utf16() {
    let ctx = context();
    let mut editor = Editor::from_portable(&literal("A한😀B")).unwrap();
    let mut observed = None;
    ctx.run_ui(Default::default(), |ui| {
        let layout = measure(ui.painter(), editor.arena(), editor.root(), FONT_SIZE);
        let mut hits = vec![];
        collect_hits(&layout, pos2(20., 30.), &mut hits);
        let hit = hits.iter().find(|hit| hit.node == editor.root()).unwrap();
        let body = hit.atom.as_ref().unwrap();
        let glyphs: Vec<String> = layout
            .primitives
            .iter()
            .filter_map(|primitive| match primitive {
                Primitive::Text(_, galley) => Some(galley.text().to_owned()),
                _ => None,
            })
            .collect();
        let before_surrogate = text_caret_rect(hit, 2);
        let after_surrogate = text_caret_rect(hit, 4);
        let after_point =
            body.pos_from_cursor(egui::text::CCursor::new(3)).min + hit.text_origin.to_vec2();
        let offset = hit_text_offset(hit, after_point);
        editor.place_text_caret(editor.root(), 0).unwrap();
        let inner_left = caret_rect(hit, &editor);
        editor.place_caret(editor.root(), Direction::Left).unwrap();
        let outer_left = caret_rect(hit, &editor);
        editor.place_caret(editor.root(), Direction::Right).unwrap();
        let outer_right = caret_rect(hit, &editor);
        observed = Some((
            glyphs,
            hit.rect,
            hit.text_origin,
            body.text().to_owned(),
            offset,
            before_surrogate,
            after_surrogate,
            inner_left,
            outer_left,
            outer_right,
        ));
    })
    .drop_without_applying_deltas();
    let (glyphs, bounds, origin, text, offset, before, after, inner, left, right) =
        observed.unwrap();
    assert_eq!(glyphs, ["“", "A한😀B", "”"]);
    assert_eq!(text, "A한😀B");
    assert!(origin.x > bounds.left());
    assert_eq!(offset, 4);
    assert!(after.left() > before.left());
    assert!((inner.left() - origin.x).abs() < 0.01);
    assert!((left.left() - bounds.left()).abs() < 0.01);
    assert!((right.left() - bounds.right()).abs() < 0.01);
    assert_eq!(editor.to_portable().unwrap(), literal("A한😀B"));
}

#[test]
fn native_string_roundtrip_preserves_raw_xml_controls_spaces_and_supplementary_pair() {
    let source = literal(" \t\r\n😀 &<>\"\\ ");
    let xml = crate::worksheet_math::tree_to_mathml(&source).unwrap();
    assert!(xml.contains("xml:space=\"preserve\""));
    assert!(xml.contains("&#13;"), "{xml}");
    assert_eq!(
        crate::worksheet_math::tree_from_mathml(&xml).unwrap(),
        source
    );
    let mut editor = Editor::from_portable(&source).unwrap();
    editor.place_text_selection(editor.root(), 6, 4).unwrap();
    // The pair occupies UTF16 offsets4..6. Removing it is a whole-scalar
    // Mathfolio policy and cannot leave an unpaired surrogate in Rust String.
    assert!(delete_character(&mut editor, true));
    assert_eq!(editor.to_portable().unwrap(), literal(" \t\r\n &<>\"\\ "));
    assert!(editor.undo());
    assert_eq!(editor.to_portable().unwrap(), source);
    assert!(
        serde_json::from_str::<PortableTree>(
            r#"{"kind":{"StringLiteral":"\ud800"},"children":[]}"#
        )
        .is_err()
    );
}
