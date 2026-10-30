use super::*;
#[test]
fn indices_action_history_and_own_wire_preserve_full_source_results_and_generation() {
    let mut f = Fixture::new(1.);
    let inert = Inert::new(&f.app);
    let c = f.app.matrix_indices_context().unwrap();
    assert!(!c.show_indices);
    let action = RibbonAction::SetMatrixIndices {
        owner: f.owner,
        show: true,
    };
    assert!(!input_intent::ribbon_needs_baseline(action));
    f.app.ribbon_command(&f.ctx, action);
    inert.check(&f.app);
    assert_eq!(
        f.app
            .document
            .matrix_result_views
            .get(&MatrixResultTarget::math(f.owner))
            .unwrap()
            .show_indices,
        Some(true)
    );
    assert_eq!(f.app.undo.len(), 1);
    let wire = serde_json::to_vec(&f.app.document).unwrap();
    let cold: CanvasDocument = serde_json::from_slice(&wire).unwrap();
    assert_eq!(cold.matrix_result_views, f.app.document.matrix_result_views);
    f.app.undo();
    inert.check(&f.app);
    assert!(f.app.document.matrix_result_views.is_empty());
    f.app.redo();
    inert.check(&f.app);
    assert!(f.app.matrix_indices_context().unwrap().show_indices);
    // Repeated explicit value is a no-op in this narrow app adapter.
    let undo = f.app.undo.len();
    f.app.ribbon_command(&f.ctx, action);
    assert_eq!(f.app.undo.len(), undo);
}
#[test]
fn stale_owner_unknown_native_format_and_drag_refuse_indices_without_history_or_source_edits() {
    let mut f = Fixture::new(1.);
    let before = serde_json::to_vec(&f.app.document).unwrap();
    assert!(f.app.set_matrix_indices(f.owner + 1000, true).is_err());
    assert!(serde_json::to_vec(&f.app.document).unwrap() == before);
    assert!(f.app.undo.is_empty());
    let start = f.begin();
    assert!(f.app.matrix_indices_context().is_none());
    assert!(f.app.set_matrix_indices(f.owner, true).is_err());
    f.frame(button(start, false));
    f.app.cancel_matrix_view_drag();
    // Unknown native presentation is not defaulted to 12/false; no cold ORIGIN
    // is created by this command. The fixture is explicitly synthetic here.
    let mut doc = f.app.document.clone();
    doc.native_archive = Some(vec![1, 2, 3]);
    f.app.matrix_view_sources = matrix_result_view::SourceFormats::from_document(&doc, None);
    assert!(f.app.matrix_indices_context().is_none());
    assert!(f.app.set_matrix_indices(f.owner, true).is_err());
    assert!(serde_json::to_vec(&f.app.document).unwrap() == before);
}

#[test]
fn actual_action_preserves_active_uncommitted_editor_source_and_separate_editor_undo() {
    let mut f = Fixture::new(1.);
    f.app.editing_region = Some(f.owner);
    let editor = f.app.editors.get_mut(&f.owner).unwrap();
    let original = editor.to_portable().unwrap();
    let left = editor.arena().get(editor.root()).unwrap().children()[0];
    editor
        .place_caret(left, mathfolio_editor::Direction::Right)
        .unwrap();
    editor.insert_infix(BinaryOp::Add);
    let draft = editor.to_portable().unwrap();
    assert_ne!(draft, original);
    let inert = Inert::new(&f.app);
    f.app.ribbon_command(
        &f.ctx,
        RibbonAction::SetMatrixIndices {
            owner: f.owner,
            show: true,
        },
    );
    inert.check(&f.app);
    assert_eq!(f.app.editing_region, Some(f.owner));
    assert_eq!(f.app.editors[&f.owner].to_portable().unwrap(), draft);
    assert_eq!(
        f.app
            .document
            .matrix_result_views
            .get(&MatrixResultTarget::math(f.owner))
            .unwrap()
            .show_indices,
        Some(true)
    );
    assert!(f.app.editors.get_mut(&f.owner).unwrap().undo());
    assert_eq!(f.app.editors[&f.owner].to_portable().unwrap(), original);
    inert.check(&f.app);
}
