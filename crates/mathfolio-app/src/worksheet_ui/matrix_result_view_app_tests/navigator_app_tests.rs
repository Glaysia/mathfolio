//! Additive actual full-App navigator pan regressions; author did not execute.
use super::*;
use mathfolio_document::canvas::MatrixViewOffset;

fn navigator(f: &Fixture) -> egui::Id {
    FORMULA_IDS
        .with_borrow(|slot| slot.as_ref().and_then(|ids| ids.get(&f.owner)).copied())
        .unwrap()
        .with("matrix-result-navigator")
}
fn global_center(ctx: &egui::Context, response: &egui::Response) -> egui::Pos2 {
    ctx.layer_transform_to_global(response.layer_id)
        .unwrap_or_default()
        * response.rect.center()
}
fn open(f: &mut Fixture) {
    let id = navigator(f);
    let marker = [0_usize, 1, 2, 3]
        .into_iter()
        .find_map(|ordinal| f.ctx.read_response(id.with(("marker", ordinal))))
        .expect("actual source viewport has no registered continuation hit");
    let at = global_center(&f.ctx, &marker);
    f.frame(vec![egui::Event::PointerMoved(at)]);
    f.frame(button(at, true));
    f.frame(button(at, false));
    f.frame(vec![]);
    f.frame(vec![]);
    assert!(
        f.ctx.read_response(id.with("window")).is_some(),
        "marker click did not open actual popup"
    );
    assert!(f.app.matrix_view_drag.is_none());
}
fn begin_pan(f: &mut Fixture) -> egui::Pos2 {
    let response = f.ctx.read_response(navigator(f).with("window")).unwrap();
    let at = global_center(&f.ctx, &response);
    f.frame(vec![egui::Event::PointerMoved(at)]);
    f.frame(button(at, true));
    assert!(
        f.app.matrix_view_drag.is_some(),
        "actual popup window press did not Begin"
    );
    at
}
fn bottom_right(f: &Fixture, start: egui::Pos2) -> egui::Pos2 {
    let full = f.ctx.read_response(navigator(f).with("full")).unwrap();
    let global = f
        .ctx
        .layer_transform_to_global(full.layer_id)
        .unwrap_or_default()
        * full.rect;
    // Shape20x20, view12x12 -> maximum offset8/8. Move .4 of full bounds
    // so the pointer remains INSIDE the popup on release (outside would cancel).
    start + vec2(global.width() * 0.4, global.height() * 0.4)
}

#[test]
fn actual_app_navigator_pan_reaches_unseen_cells_and_commits_one_view_undo_without_execution() {
    let mut f = Fixture::new(1.);
    let target = MatrixResultTarget::math(f.owner);
    let inert = Inert::new(&f.app);
    open(&mut f);
    inert.check(&f.app);
    assert!(f.app.undo.is_empty() && !f.app.dirty);
    let start = begin_pan(&mut f);
    inert.check(&f.app);
    let moved = bottom_right(&f, start);
    f.frame(vec![egui::Event::PointerMoved(moved)]);
    inert.check(&f.app);
    assert!(f.app.document.matrix_result_views.get(&target).is_none());
    assert!(f.app.undo.is_empty() && !f.app.dirty);
    let preview = f
        .app
        .requested_matrix_view(f.owner)
        .expect("actual pan Preview missing");
    assert_eq!(preview.offset, Some(MatrixViewOffset { row: 8, col: 8 }));
    f.frame(vec![]); // Draw accepted transient projection before button release.
    inert.check(&f.app);
    let p = f.app.display_cache[&f.owner].matrix.value().unwrap();
    assert_eq!((p.rows, p.cols, p.row_offset, p.col_offset), (12, 12, 8, 8));
    assert_eq!(p.cells.len(), 144);
    assert!(
        p.cells[0].plain().starts_with("169"),
        "wrong first newly visible cell"
    );
    assert!(
        p.cells[143].plain().starts_with("400"),
        "complete bottom-right value not reached"
    );
    f.frame(button(moved, false));
    inert.check(&f.app);
    assert!(f.app.matrix_view_drag.is_none());
    assert_eq!(
        f.app.document.matrix_result_views.get(&target).copied(),
        Some(preview)
    );
    assert_eq!(f.app.undo.len(), 1);
    assert!(f.app.redo.is_empty() && f.app.dirty);
    f.frame(command(egui::Key::Z));
    inert.check(&f.app);
    assert!(f.app.document.matrix_result_views.get(&target).is_none());
    assert!(f.app.undo.is_empty());
    assert_eq!(f.app.redo.len(), 1);
    f.frame(command(egui::Key::Y));
    inert.check(&f.app);
    assert_eq!(
        f.app.document.matrix_result_views.get(&target).copied(),
        Some(preview)
    );
    assert_eq!(f.app.undo.len(), 1);
    assert!(f.app.redo.is_empty());
}

#[test]
fn actual_app_navigator_escape_cancels_preview_and_late_release_cannot_commit() {
    let mut f = Fixture::new(1.);
    let target = MatrixResultTarget::math(f.owner);
    let inert = Inert::new(&f.app);
    open(&mut f);
    let start = begin_pan(&mut f);
    let moved = bottom_right(&f, start);
    f.frame(vec![egui::Event::PointerMoved(moved)]);
    f.frame(vec![]);
    assert!(f.app.matrix_view_drag.is_some());
    assert_eq!(
        f.app.requested_matrix_view(f.owner).unwrap().offset,
        Some(MatrixViewOffset { row: 8, col: 8 })
    );
    f.frame(vec![egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: Some(egui::Key::Escape),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    inert.check(&f.app);
    assert!(f.app.matrix_view_drag.is_none());
    assert!(f.app.document.matrix_result_views.get(&target).is_none());
    f.frame(button(moved, false));
    f.frame(vec![]);
    inert.check(&f.app);
    assert!(f.app.matrix_view_drag.is_none());
    assert!(f.app.document.matrix_result_views.get(&target).is_none());
    assert!(f.app.undo.is_empty() && f.app.redo.is_empty() && !f.app.dirty);
}
