use super::*;
use crate::result_display::MatrixWindow;
use mathcraft_core::Quantity;

fn fixture() -> (WorksheetApp, u64) {
    let mut app = WorksheetApp {
        automatic_calculation: false,
        ..Default::default()
    };
    let id = app.document.add_math(38.4, 38.4);
    app.document.regions[0].content = CanvasContent::Math(PortableTree {
        kind: NodeKind::Binary(BinaryOp::Evaluate),
        children: vec![
            PortableTree {
                kind: NodeKind::Number("1".into()),
                children: vec![],
            },
            PortableTree {
                kind: NodeKind::Placeholder,
                children: vec![],
            },
        ],
    });
    let value = mathcraft_compat::Value::Matrix {
        rows: 50,
        cols: 50,
        elements: (0..2500).map(|i| Quantity::scalar(i as f64)).collect(),
    };
    let row = SavedCalculationRow {
        region_id: id,
        status: SavedCalculationStatus::Synchronized,
        display: Some(
            crate::result_display::format(&value, &app.document.regions[0].format).to_saved(),
        ),
        error: None,
        warnings: vec![],
        symbolic_mathml: None,
        numeric_source: numeric_cache::source(&value),
    };
    app.saved_results.insert(id, row);
    app.insertion = None;
    app.selected = Some(id);
    let window = MatrixWindow {
        rows: 12,
        cols: 12,
        row_offset: 0,
        col_offset: 0,
    };
    let shown = cached_formula_display(
        &mut app.display_cache,
        id,
        app.document.regions[0].format,
        None,
        app.saved_results.get(&id),
        Some(window),
    );
    assert!(matches!(
        shown,
        Some(formula::FormulaResult::MatrixViewport(_))
    ));
    (app, id)
}
fn gesture(size: [usize; 2], commit: bool) -> formula::MatrixViewGesture {
    if commit {
        formula::MatrixViewGesture::Commit {
            size,
            offset: [0, 0],
        }
    } else {
        formula::MatrixViewGesture::Preview {
            size,
            offset: [0, 0],
        }
    }
}
fn begin(app: &mut WorksheetApp, id: u64) {
    app.matrix_view_gesture(
        id,
        formula::MatrixViewGesture::Begin {
            size: [12, 12],
            offset: [0, 0],
        },
    );
}
#[test]
fn preview_stays_transient_and_one_history_transaction_preserves_all_numeric_state() {
    let (mut app, id) = fixture();
    let before = app.document.calculation_signature();
    let active = app.document.active_calculation_signature();
    let row = app.saved_results[&id].clone();
    let generation = app.background.submitted_generation();
    begin(&mut app, id);
    app.matrix_view_gesture(id, gesture([13, 14], false));
    assert!(app.document.matrix_result_views.is_empty());
    assert!(!app.dirty && app.undo.is_empty());
    assert_eq!(
        app.requested_matrix_view(id).unwrap().size,
        Some(MatrixViewSize { rows: 13, cols: 14 })
    );
    app.matrix_view_gesture(id, gesture([13, 14], true));
    assert_eq!(app.undo.len(), 1);
    assert!(app.dirty);
    let saved = app.document.matrix_result_views.clone();
    assert_eq!(
        saved.get(&MatrixResultTarget::math(id)).unwrap().offset,
        None
    );
    app.undo();
    assert!(app.document.matrix_result_views.is_empty());
    app.redo();
    assert_eq!(app.document.matrix_result_views, saved);
    assert_eq!(app.saved_results[&id], row);
    assert_eq!(app.document.calculation_signature(), before);
    assert_eq!(app.document.active_calculation_signature(), active);
    assert_eq!(app.background.submitted_generation(), generation);
    assert!(
        app.pending_regions.is_empty()
            && app.last_executed_regions.is_empty()
            && app.results.is_empty()
    );
}
#[test]
fn rejected_large_projection_cancel_and_no_motion_publish_no_history_or_dirty_state() {
    let (mut app, id) = fixture();
    let old = app.display_cache[&id].matrix.value().unwrap().clone();
    begin(&mut app, id);
    app.matrix_view_gesture(id, gesture([50, 50], false));
    // 2500 simple cells fit4096 nodes; a request can still be vetoed by the
    // independent geometry DTO bound before any publication.
    app.matrix_view_gesture(id, gesture([65536, 2], true));
    assert!(app.document.matrix_result_views.is_empty());
    assert!(app.undo.is_empty() && !app.dirty && app.matrix_view_drag.is_none());
    begin(&mut app, id);
    app.matrix_view_gesture(id, gesture([12, 12], true));
    assert!(app.document.matrix_result_views.is_empty() && app.undo.is_empty() && !app.dirty);
    begin(&mut app, id);
    app.matrix_view_gesture(id, gesture([13, 14], false));
    app.matrix_view_gesture(id, formula::MatrixViewGesture::Cancel);
    let view = cached_formula_display(
        &mut app.display_cache,
        id,
        app.document.regions[0].format,
        None,
        app.saved_results.get(&id),
        Some(MatrixWindow {
            rows: 12,
            cols: 12,
            row_offset: 0,
            col_offset: 0,
        }),
    );
    let Some(formula::FormulaResult::MatrixViewport(view)) = view else {
        panic!()
    };
    assert_eq!(view, &old);
    assert!(app.document.matrix_result_views.is_empty() && app.undo.is_empty() && !app.dirty);
}
#[test]
fn idle_cached_projection_does_no_full_admission_or_cell_projection_work() {
    let (mut app, id) = fixture();
    let before = app.display_cache[&id].matrix.work_counts();
    for _ in 0..64 {
        let view = cached_formula_display(
            &mut app.display_cache,
            id,
            app.document.regions[0].format,
            None,
            app.saved_results.get(&id),
            Some(MatrixWindow {
                rows: 12,
                cols: 12,
                row_offset: 0,
                col_offset: 0,
            }),
        );
        assert!(matches!(
            view,
            Some(formula::FormulaResult::MatrixViewport(_))
        ));
    }
    assert_eq!(app.display_cache[&id].matrix.work_counts(), before);
    assert_eq!(before, (1, 1));
    assert!(!app.display_cache[&id].legacy_initialized);
}

#[test]
fn unsupported_matrix_never_falls_back_to_plausible_legacy_cells_under_a_requested_window() {
    let (mut app, id) = fixture();
    let raw = "<matrix xmlns='http://schemas.mathsoft.com/math50' rows='2' cols='2' future='retained'><real>1</real><real>3</real><real>2</real><real>4</real></matrix>";
    let row = app.saved_results.get_mut(&id).unwrap();
    row.display = Some(
        crate::result_display::from_native_result(raw, &app.document.regions[0].format)
            .unwrap()
            .to_saved(),
    );
    row.numeric_source = Some(SavedNumericSource::NativeMathml(raw.into()));
    let before = row.clone();
    app.display_cache.clear();
    let display = cached_formula_display(
        &mut app.display_cache,
        id,
        app.document.regions[0].format,
        None,
        app.saved_results.get(&id),
        Some(MatrixWindow {
            rows: 1,
            cols: 1,
            row_offset: 1,
            col_offset: 1,
        }),
    );
    assert!(
        matches!(display, Some(formula::FormulaResult::Text(_))),
        "unsupported native metadata must not silently ignore the requested window"
    );
    assert_eq!(app.saved_results[&id], before);
    assert!(app.last_executed_regions.is_empty());
}

#[path = "inner_resize_clone_tests.rs"]
mod inner_resize_clone_tests;

mod preflight_reuse_tests;
