//! Prime's `{` local assignment gesture in the structured program editor.
use super::*;

#[test]
fn local_assignment_text_event_reaches_the_focused_program_editor() {
    let mut editor = Editor::new();
    editor.insert_program();
    input_text(&mut editor, "f(x)");
    let context = egui::Context::default();
    crate::configure(&context);
    context
        .run_ui(
            egui::RawInput {
                events: vec![egui::Event::Text("{".into())],
                ..Default::default()
            },
            |ui| {
                let response = formula_ui_with_actions(ui, &mut editor, None, true);
                assert!(response.response.changed());
                assert!(!response.evaluation_requested);
            },
        )
        .drop_without_applying_deltas();
    let tree = editor.to_portable().unwrap();
    assert_eq!(tree.children[0].kind, NodeKind::LocalDefine);
    assert_eq!(
        tree.children[0].children[0].kind,
        NodeKind::Function { parameter_count: 1 }
    );
}

#[test]
fn typing_a_local_function_uses_function_lhs_and_evaluates_after_native_roundtrip() {
    let mut editor = Editor::new();
    editor.insert_program();
    for ch in "f(x)".chars() {
        input_text(&mut editor, &ch.to_string());
    }
    let before = editor.to_portable().unwrap();
    let outcome = input_text_with_actions(&mut editor, "{");
    assert!(outcome.document_changed && !outcome.evaluation_requested);
    let declaration = editor.to_portable().unwrap();
    assert_eq!(declaration.kind, NodeKind::Program { statement_count: 1 });
    assert_eq!(declaration.children[0].kind, NodeKind::LocalDefine);
    assert_eq!(
        declaration.children[0].children[0].kind,
        NodeKind::Function { parameter_count: 1 }
    );
    assert_eq!(
        declaration.children[0].children[1].kind,
        NodeKind::Placeholder
    );
    editor.undo();
    assert_eq!(editor.to_portable().unwrap(), before);
    editor.redo();
    for ch in "x^2".chars() {
        input_text(&mut editor, &ch.to_string());
    }
    assert!(editor.insert_program_line());
    for ch in "f(3)".chars() {
        input_text(&mut editor, &ch.to_string());
    }
    let native =
        mathfolio_compat::editor_bridge::tree_to_mathml(&editor.to_portable().unwrap()).unwrap();
    let reparsed = mathfolio_compat::editor_bridge::tree_from_mathml(&native).unwrap();
    assert_eq!(reparsed, editor.to_portable().unwrap());
    let evaluation = format!(
        "<ml:eval xmlns:ml='http://schemas.mathsoft.com/math50'>{native}<ml:unitOverride><ml:placeholder/></ml:unitOverride></ml:eval>"
    );
    let worksheet = crate::worksheet_math::worksheet_xml(&[(1, 0., 0., evaluation)]).unwrap();
    let result = mathfolio_compat::evaluate_worksheet(&worksheet).unwrap();
    assert_eq!(result.len(), 1);
    assert!(result[0].error.is_none(), "{:?}", result[0]);
    assert!(
        matches!(&result[0].value,Some(mathfolio_compat::Value::Scalar {quantity}) if quantity.value.re==9.)
    );
}

#[test]
fn local_assignment_gesture_does_not_escape_expression_only_or_literal_subscript_context() {
    let mut outside = Editor::new();
    input_text(&mut outside, "f(x)");
    let before = outside.to_portable().unwrap();
    assert!(!input_text(&mut outside, "{"));
    assert_eq!(outside.to_portable().unwrap(), before);
    let mut restricted = Editor::new();
    restricted.set_expressions_only(true);
    let before = restricted.to_portable().unwrap();
    assert!(!input_text(&mut restricted, "{"));
    assert_eq!(restricted.to_portable().unwrap(), before);
    let mut suffix = Editor::new();
    input_text(&mut suffix, "I");
    suffix.toggle_literal_subscript();
    input_text(&mut suffix, "{");
    let value = suffix.to_portable().unwrap();
    assert!(
        matches!(value.kind,NodeKind::NativeIdentifier {subscript:Some(ref text),..} if text=="{")
    );
}
