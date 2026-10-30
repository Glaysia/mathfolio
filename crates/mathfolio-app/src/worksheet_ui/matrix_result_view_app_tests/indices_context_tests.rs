//! Actual App fixture/results; no injected result value/ORIGIN metadata.
use super::*;
#[test]
fn matrix_definition_without_drawn_evaluation_result_cannot_target_result_indices() {
    let mut f = Fixture::new(1.);
    let definition = f
        .app
        .document
        .regions
        .iter()
        .find(|r| r.id != f.owner)
        .unwrap()
        .id;
    // Definitions establish bindings but expose no displayed result row. The
    // actual M= query proves the matrix binding without forging a result here.
    assert!(f.app.results[&definition].error.is_none());
    assert!(f.app.results[&definition].value.is_none());
    assert!(f.app.results[&definition].matrix_origin.is_none());
    assert!(matches!(
        f.app.results[&f.owner].value.as_ref(),
        Some(mathfolio_compat::Value::Matrix { .. })
    ));
    f.app.selected = Some(definition);
    f.app.editing_region = None;
    f.app.focus_new = None;
    f.frame(vec![]);
    let inert = Inert::new(&f.app);
    let views = f.app.document.matrix_result_views.clone();
    let undo = f.app.undo.len();
    assert!(
        f.app.matrix_indices_context().is_none(),
        "A definition's calculated matrix is not a displayed Eval result"
    );
    assert!(f.app.set_matrix_indices(definition, true).is_err());
    assert_eq!(f.app.document.matrix_result_views, views);
    assert_eq!(f.app.undo.len(), undo);
    inert.check(&f.app);
}
#[test]
fn active_source_matrix_caret_in_eval_cannot_route_to_result_format() {
    let mut f = Fixture::new(1.);
    let tree = node(
        NodeKind::Binary(BinaryOp::Evaluate),
        vec![
            node(
                NodeKind::Matrix {
                    rows: 20,
                    cols: 20,
                    display: None,
                },
                (1..=400).map(number).collect(),
            ),
            node(NodeKind::Placeholder, vec![]),
        ],
    );
    f.app
        .document
        .regions
        .iter_mut()
        .find(|r| r.id == f.owner)
        .unwrap()
        .content = CanvasContent::Math(tree.clone());
    f.app
        .editors
        .insert(f.owner, Editor::from_portable(&tree).unwrap());
    f.app.recalculate();
    f.app.last_executed_regions.clear();
    f.app.undo.clear();
    f.app.redo.clear();
    f.app.selected = Some(f.owner);
    f.app.editing_region = None;
    f.app.focus_new = None;
    for _ in 0..3 {
        f.frame(vec![]);
    }
    assert!(f.app.results[&f.owner].matrix_origin.is_some());
    assert_eq!(f.app.display_cache[&f.owner].matrix.shape(), Some((20, 20)));
    let formula_id = FORMULA_IDS
        .with_borrow(|slot| slot.as_ref().and_then(|ids| ids.get(&f.owner)).copied())
        .unwrap();
    let editor = f.app.editors.get_mut(&f.owner).unwrap();
    let source_matrix = editor.arena().get(editor.root()).unwrap().children()[0];
    let first_cell = editor.arena().get(source_matrix).unwrap().children()[0];
    editor.place_text_caret(first_cell, 1).unwrap();
    f.ctx.memory_mut(|memory| memory.request_focus(formula_id));
    f.frame(vec![]);
    assert!(f.ctx.read_response(formula_id).unwrap().has_focus());
    assert_eq!(f.app.editing_region, Some(f.owner));
    assert_eq!(f.app.editors[&f.owner].caret().node, first_cell);
    let inert = Inert::new(&f.app);
    let views = f.app.document.matrix_result_views.clone();
    assert!(
        f.app.matrix_indices_context().is_none(),
        "Source MATRIX has its own format; RESULT action cannot reinterpret it"
    );
    assert!(f.app.set_matrix_indices(f.owner, true).is_err());
    assert_eq!(f.app.document.matrix_result_views, views);
    inert.check(&f.app);
}
#[test]
fn ordinary_identifier_eval_with_known_result_origin_keeps_result_toggle_available() {
    let mut f = Fixture::new(1.);
    let inert = Inert::new(&f.app);
    assert_eq!(f.app.matrix_indices_context().unwrap().owner, f.owner);
    f.app.ribbon_command(
        &f.ctx,
        RibbonAction::SetMatrixIndices {
            owner: f.owner,
            show: true,
        },
    );
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
    inert.check(&f.app);
}
