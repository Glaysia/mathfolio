//! Additive child of the already adopted matrix_result_view_app_tests fixture.
//! Unexecuted: exact release + source input ordering, not a direct controller call.
use super::*;

#[test]
fn actual_source_edit_on_bracket_release_cancels_old_view_without_matrix_history() {
    let mut f = Fixture::new(1.);
    let owner = f.owner;
    let target = MatrixResultTarget::math(owner);
    let start = f.begin();
    let moved = start + vec2(120., 80.);
    f.frame(vec![egui::Event::PointerMoved(moved)]);
    f.frame(vec![]);
    assert!(f.app.matrix_view_drag.is_some());
    assert!(f.app.requested_matrix_view(owner).unwrap().size.is_some());
    assert!(f.app.document.matrix_result_views.get(&target).is_none());
    assert!(f.app.undo.is_empty() && !f.app.dirty);

    // A real source widget may retain keyboard focus during a captured pointer
    // drag. Establish that positive state rather than bypassing Text dispatch.
    let formula_id = FORMULA_IDS
        .with_borrow(|slot| slot.as_ref().and_then(|ids| ids.get(&owner)).copied())
        .unwrap();
    let editor = f.app.editors.get_mut(&owner).unwrap();
    let lhs = editor.arena().get(editor.root()).unwrap().children()[0];
    editor.place_text_caret(lhs, 1).unwrap();
    f.ctx.memory_mut(|memory| memory.request_focus(formula_id));
    f.frame(vec![]);
    assert!(f.ctx.read_response(formula_id).unwrap().has_focus());
    assert!(f.app.matrix_view_drag.is_some());
    let before = f
        .app
        .document
        .regions
        .iter()
        .find(|r| r.id == owner)
        .unwrap()
        .clone();
    let before_signature = f.app.document.active_calculation_signature();

    // The ordinary Math FIFO admits this same delivery unchanged. Matrix gesture
    // was measured before keyboard handling, so the app must discard that old
    // Commit after observing Response.changed(), while retaining the real edit.
    f.frame(vec![
        egui::Event::PointerMoved(moved),
        egui::Event::Text("x".into()),
        egui::Event::PointerButton {
            pos: moved,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        },
    ]);
    let after = f
        .app
        .document
        .regions
        .iter()
        .find(|r| r.id == owner)
        .unwrap();
    assert_ne!(
        after.content, before.content,
        "the Text event must actually edit source"
    );
    let CanvasContent::Math(source) = &after.content else {
        panic!()
    };
    assert_eq!(source.children[0].kind, NodeKind::Identifier("Mx".into()));
    assert_ne!(
        f.app.document.active_calculation_signature(),
        before_signature
    );
    assert!(f.app.matrix_view_drag.is_none());
    assert!(f.app.document.matrix_result_views.get(&target).is_none());
    assert!(
        f.app
            .undo
            .iter()
            .all(|entry| !matches!(&entry.document, DocumentHistory::MatrixResultView { .. }))
    );
    assert!(
        !f.app.undo.is_empty() && f.app.dirty,
        "ordinary source history is expected"
    );
    assert!(f.app.last_executed_regions.is_empty());
    f.frame(vec![]);
    assert!(f.app.matrix_view_drag.is_none());
    assert!(f.app.document.matrix_result_views.get(&target).is_none());
    assert!(
        f.app
            .undo
            .iter()
            .all(|entry| !matches!(&entry.document, DocumentHistory::MatrixResultView { .. }))
    );
}
