//! Actual app pointer proposal; unexecuted. Full source/cache authority remains
//! in existing Fixture/Inert. No programmatic gesture call substitutes for hits.
use super::*;
use mathcraft_document::canvas::{MatrixResultViewport, MatrixViewOffset, MatrixViewSize};

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
    let marker = (0_usize..4)
        .find_map(|i| f.ctx.read_response(id.with(("marker", i))))
        .unwrap();
    let at = global_center(&f.ctx, &marker);
    f.frame(vec![egui::Event::PointerMoved(at)]);
    f.frame(button(at, true));
    f.frame(button(at, false));
    f.frame(vec![]);
    f.frame(vec![]);
    assert!(f.ctx.read_response(id.with("window")).is_some());
    assert!(f.app.matrix_view_drag.is_none());
}

fn begin(f: &mut Fixture, ordinal: u8) -> egui::Pos2 {
    let r = f
        .ctx
        .read_response(navigator(f).with(("handle", ordinal)))
        .unwrap();
    let at = global_center(&f.ctx, &r);
    f.frame(vec![egui::Event::PointerMoved(at)]);
    f.frame(button(at, true));
    assert!(f.app.matrix_view_drag.is_some());
    at
}
fn full(f: &Fixture) -> Rect {
    let r = f.ctx.read_response(navigator(f).with("full")).unwrap();
    f.ctx
        .layer_transform_to_global(r.layer_id)
        .unwrap_or_default()
        * r.rect
}
#[test]
fn actual_app_inner_down_right_resize_is_transient_then_one_undo_and_keeps_all_400_values() {
    let mut f = Fixture::new(1.);
    let inert = Inert::new(&f.app);
    let target = MatrixResultTarget::math(f.owner);
    open(&mut f);
    let at = begin(&mut f, 7); // Original Direction.DownRight.
    let light = full(&f);
    let moved = at + vec2(light.width() * 0.125, light.height() * 0.125);
    f.frame(vec![egui::Event::PointerMoved(moved)]);
    inert.check(&f.app);
    assert!(f.app.document.matrix_result_views.get(&target).is_none());
    assert!(f.app.undo.is_empty() && !f.app.dirty);
    let preview = f.app.requested_matrix_view(f.owner).unwrap();
    assert_eq!(preview.size, Some(MatrixViewSize { rows: 15, cols: 15 }));
    f.frame(vec![]);
    let p = f.app.display_cache[&f.owner].matrix.value().unwrap();
    assert_eq!((p.full_rows, p.full_cols, p.rows, p.cols), (20, 20, 15, 15));
    assert_eq!(p.cells.len(), 225);
    assert!(p.cells[224].plain().starts_with("295"));
    f.frame(button(moved, false));
    inert.check(&f.app);
    assert!(f.app.matrix_view_drag.is_none());
    assert_eq!(
        f.app.document.matrix_result_views.get(&target).copied(),
        Some(preview)
    );
    assert_eq!(f.app.undo.len(), 1);
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
}
#[test]
fn actual_app_inner_up_left_changes_offset_and_size_together_then_undo_restores_request() {
    let mut f = Fixture::new(1.25);
    let target = MatrixResultTarget::math(f.owner);
    let before = MatrixResultViewport {
        size: Some(MatrixViewSize { rows: 12, cols: 12 }),
        offset: Some(MatrixViewOffset { row: 4, col: 4 }),
        show_indices: None,
    };
    assert!(f.app.set_matrix_result_view(f.owner, Some(before)).unwrap());
    f.app.undo.clear();
    f.app.redo.clear();
    f.app.dirty = false;
    f.frame(vec![]);
    f.frame(vec![]);
    let inert = Inert::new(&f.app);
    open(&mut f);
    let at = begin(&mut f, 4);
    let light = full(&f);
    let moved = at - vec2(light.width() * 0.125, light.height() * 0.125);
    f.frame(vec![egui::Event::PointerMoved(moved)]);
    f.frame(vec![]);
    inert.check(&f.app);
    assert_eq!(
        f.app.document.matrix_result_views.get(&target).copied(),
        Some(before)
    );
    let preview = f.app.requested_matrix_view(f.owner).unwrap();
    assert_eq!(preview.size, Some(MatrixViewSize { rows: 15, cols: 15 }));
    assert_eq!(preview.offset, Some(MatrixViewOffset { row: 1, col: 1 }));
    f.frame(button(moved, false));
    inert.check(&f.app);
    assert_eq!(f.app.undo.len(), 1);
    f.frame(command(egui::Key::Z));
    inert.check(&f.app);
    assert_eq!(
        f.app.document.matrix_result_views.get(&target).copied(),
        Some(before)
    );
}
#[test]
fn actual_app_resize_escape_pointer_loss_and_owner_disappearance_cancel_without_history() {
    for mode in 0..3 {
        let mut f = Fixture::new(1.);
        let target = MatrixResultTarget::math(f.owner);
        let inert = Inert::new(&f.app);
        open(&mut f);
        let at = begin(&mut f, 7);
        let moved = at + vec2(20., 20.);
        f.frame(vec![egui::Event::PointerMoved(moved)]);
        f.frame(vec![]);
        assert!(f.app.matrix_view_drag.is_some());
        match mode {
            0 => f.frame(vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: Some(egui::Key::Escape),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }]),
            1 => f.frame(vec![egui::Event::PointerGone]),
            _ => {
                f.app.selected = None;
                f.frame(vec![]);
            }
        }
        inert.check(&f.app);
        assert!(f.app.matrix_view_drag.is_none());
        assert!(f.app.document.matrix_result_views.get(&target).is_none());
        f.frame(button(moved, false));
        f.frame(vec![]);
        inert.check(&f.app);
        assert!(f.app.undo.is_empty() && f.app.redo.is_empty() && !f.app.dirty);
        assert!(f.app.document.matrix_result_views.get(&target).is_none());
    }
}
