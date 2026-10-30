//! Controller policy regression proposal, not a real original/runtime fixture.
//! Reuses the existing saved-value controller fixture, with full20x20 authority.
use super::*;
use formula::MatrixViewGesture as G;

fn inherited() -> MatrixResultViewport {
    MatrixResultViewport {
        size: Some(MatrixViewSize { rows: 30, cols: 30 }),
        offset: Some(MatrixViewOffset { row: 7, col: 8 }),
        show_indices: Some(true),
    }
}
fn fixture() -> (WorksheetApp, u64) {
    let (mut app, id) = super::fixture();
    let value = mathfolio_compat::Value::Matrix {
        rows: 20,
        cols: 20,
        elements: (0..400).map(|i| Quantity::scalar(i as f64)).collect(),
    };
    app.saved_results.get_mut(&id).unwrap().numeric_source = numeric_cache::source(&value);
    app.display_cache.clear();
    app.matrix_view_sources = SourceFormats {
        defaults: Some(inherited()),
        owners: HashMap::new(),
    };
    refresh(&mut app, id);
    assert_eq!(shown(&app, id), (20, 20, 0, 0));
    (app, id)
}
fn refresh(app: &mut WorksheetApp, id: u64) {
    let request = app.requested_matrix_view(id);
    let window = app.matrix_view_sources.window(id, request);
    let shown = cached_formula_display(
        &mut app.display_cache,
        id,
        app.document.regions[0].format,
        None,
        app.saved_results.get(&id),
        window,
    );
    assert!(matches!(
        shown,
        Some(formula::FormulaResult::MatrixViewport(_))
    ));
}
fn shown(app: &WorksheetApp, id: u64) -> (usize, usize, usize, usize) {
    let p = app.display_cache[&id].matrix.value().unwrap();
    (p.rows, p.cols, p.row_offset, p.col_offset)
}
fn begin(app: &mut WorksheetApp, id: u64) {
    app.matrix_view_gesture(
        id,
        G::BeginInnerResize {
            size: [20, 20],
            offset: [0, 0],
        },
    );
    assert!(app.matrix_view_drag.is_some());
}
struct Inert {
    regions: Vec<CanvasRegion>,
    row: SavedCalculationRow,
    signature: String,
    generation: u64,
}
impl Inert {
    fn new(app: &WorksheetApp, id: u64) -> Self {
        Self {
            regions: app.document.regions.clone(),
            row: app.saved_results[&id].clone(),
            signature: app.document.calculation_signature(),
            generation: app.background.submitted_generation(),
        }
    }
    fn check(&self, app: &WorksheetApp, id: u64) {
        assert_eq!(app.document.regions, self.regions);
        assert_eq!(app.saved_results[&id], self.row);
        assert_eq!(app.document.calculation_signature(), self.signature);
        assert_eq!(app.background.submitted_generation(), self.generation);
        assert!(app.pending_regions.is_empty() && app.last_executed_regions.is_empty());
    }
}

#[test]
fn first_inner_resize_clones_requested_flags_then_explicit_pair_overwrites_clamped_raw_offset() {
    let (mut app, id) = fixture();
    let inert = Inert::new(&app, id);
    let target = MatrixResultTarget::math(id);
    begin(&mut app, id);
    assert_eq!(
        app.matrix_view_drag.as_ref().unwrap().inner_resize_base,
        Some(inherited())
    );
    assert!(app.document.matrix_result_views.get(&target).is_none());
    app.matrix_view_gesture(
        id,
        G::PreviewInnerResize {
            size: [16, 16],
            offset: [0, 0],
        },
    );
    let expected = MatrixResultViewport {
        size: Some(MatrixViewSize { rows: 16, cols: 16 }),
        offset: Some(MatrixViewOffset { row: 0, col: 0 }),
        show_indices: Some(true),
    };
    assert_eq!(app.requested_matrix_view(id), Some(expected));
    refresh(&mut app, id);
    assert_eq!(shown(&app, id), (16, 16, 0, 0));
    assert!(app.document.matrix_result_views.get(&target).is_none());
    assert!(app.undo.is_empty() && !app.dirty);
    inert.check(&app, id);
    app.matrix_view_gesture(
        id,
        G::CommitInnerResize {
            size: [16, 16],
            offset: [0, 0],
        },
    );
    assert_eq!(
        app.document.matrix_result_views.get(&target).copied(),
        Some(expected)
    );
    assert_eq!(app.undo.len(), 1);
    inert.check(&app, id);
    // A later parent default change does not change the now-local request.
    app.matrix_view_sources.defaults = Some(MatrixResultViewport {
        size: Some(MatrixViewSize { rows: 2, cols: 3 }),
        offset: Some(MatrixViewOffset { row: 1, col: 1 }),
        show_indices: Some(false),
    });
    assert_eq!(
        app.matrix_view_sources.effective(id, Some(expected)),
        Some(expected)
    );
    app.undo();
    assert!(app.document.matrix_result_views.get(&target).is_none());
    assert_eq!(
        app.matrix_view_sources.effective(id, None),
        app.matrix_view_sources.defaults
    );
    app.redo();
    assert_eq!(
        app.document.matrix_result_views.get(&target).copied(),
        Some(expected)
    );
    inert.check(&app, id);
    let wire = serde_json::to_vec(&app.document).unwrap();
    let cold: CanvasDocument = serde_json::from_slice(&wire).unwrap();
    assert_eq!(
        cold.matrix_result_views.get(&target).copied(),
        Some(expected)
    );
}
#[test]
fn no_motion_and_moved_back_inner_commit_do_not_publish_captured_clone_or_history() {
    for moved in [false, true] {
        let (mut app, id) = fixture();
        let inert = Inert::new(&app, id);
        begin(&mut app, id);
        if moved {
            app.matrix_view_gesture(
                id,
                G::PreviewInnerResize {
                    size: [16, 16],
                    offset: [0, 0],
                },
            );
            app.matrix_view_gesture(
                id,
                G::PreviewInnerResize {
                    size: [20, 20],
                    offset: [0, 0],
                },
            );
            refresh(&mut app, id);
            assert_eq!(shown(&app, id), (20, 20, 0, 0));
        }
        app.matrix_view_gesture(
            id,
            G::CommitInnerResize {
                size: [20, 20],
                offset: [0, 0],
            },
        );
        assert!(app.matrix_view_drag.is_none() && app.document.matrix_result_views.is_empty());
        assert!(app.undo.is_empty() && app.redo.is_empty() && !app.dirty);
        assert_eq!(
            app.matrix_view_sources.effective(id, None),
            Some(inherited())
        );
        inert.check(&app, id);
    }
}
#[test]
fn captured_inner_base_is_stable_across_previews_and_parent_changes_and_cancel_is_inert() {
    let (mut app, id) = fixture();
    let inert = Inert::new(&app, id);
    begin(&mut app, id);
    app.matrix_view_sources.defaults = Some(MatrixResultViewport {
        size: Some(MatrixViewSize { rows: 3, cols: 3 }),
        offset: Some(MatrixViewOffset { row: 1, col: 1 }),
        show_indices: Some(false),
    });
    for size in [[16, 16], [15, 17], [18, 14]] {
        app.matrix_view_gesture(
            id,
            G::PreviewInnerResize {
                size,
                offset: [0, 0],
            },
        );
        let request = app.requested_matrix_view(id).unwrap();
        assert_eq!(request.show_indices, Some(true));
        assert_eq!(
            request.size,
            Some(MatrixViewSize {
                rows: size[0] as u32,
                cols: size[1] as u32
            })
        );
        assert_eq!(request.offset, Some(MatrixViewOffset { row: 0, col: 0 }));
    }
    app.matrix_view_gesture(id, G::Cancel);
    assert!(app.matrix_view_drag.is_none() && app.document.matrix_result_views.is_empty());
    assert!(app.undo.is_empty() && !app.dirty);
    inert.check(&app, id);
}
#[test]
fn inner_resize_preserves_existing_local_partial_flags_and_undo_restores_that_partial() {
    let (mut app, id) = fixture();
    let target = MatrixResultTarget::math(id);
    let partial = MatrixResultViewport {
        show_indices: Some(false),
        ..Default::default()
    };
    app.document
        .matrix_result_views
        .set(target.clone(), partial)
        .unwrap();
    begin(&mut app, id);
    app.matrix_view_gesture(
        id,
        G::CommitInnerResize {
            size: [20, 16],
            offset: [0, 0],
        },
    );
    let request = app
        .document
        .matrix_result_views
        .get(&target)
        .copied()
        .unwrap();
    assert_eq!(request.size, Some(MatrixViewSize { rows: 20, cols: 16 }));
    assert_eq!(request.offset, Some(MatrixViewOffset { row: 0, col: 0 }));
    assert_eq!(request.show_indices, Some(false));
    app.undo();
    assert_eq!(
        app.document.matrix_result_views.get(&target).copied(),
        Some(partial)
    );
}
#[test]
fn unknown_inherited_inner_base_is_refused_while_old_sparse_bracket_profile_is_unchanged() {
    let (mut app, id) = fixture();
    app.matrix_view_sources.defaults = Some(MatrixResultViewport {
        show_indices: None,
        ..inherited()
    });
    app.matrix_view_gesture(
        id,
        G::BeginInnerResize {
            size: [20, 20],
            offset: [0, 0],
        },
    );
    assert!(app.matrix_view_drag.is_none() && app.document.matrix_result_views.is_empty());
    assert!(app.undo.is_empty() && !app.dirty);
    // This remains the historical bounded outer/pan API, not a new clone claim.
    app.matrix_view_gesture(
        id,
        G::Begin {
            size: [20, 20],
            offset: [0, 0],
        },
    );
    app.matrix_view_gesture(
        id,
        G::Commit {
            size: [16, 16],
            offset: [0, 0],
        },
    );
    let request = app
        .document
        .matrix_result_views
        .get(&MatrixResultTarget::math(id))
        .unwrap();
    assert_eq!(request.size, Some(MatrixViewSize { rows: 16, cols: 16 }));
    assert!(request.offset.is_none() && request.show_indices.is_none());
}
#[test]
fn inner_and_sparse_gesture_profiles_cannot_cross_commit_or_publish_each_others_preview() {
    for inner_begin in [false, true] {
        let (mut app, id) = fixture();
        app.matrix_view_gesture(
            id,
            if inner_begin {
                G::BeginInnerResize {
                    size: [20, 20],
                    offset: [0, 0],
                }
            } else {
                G::Begin {
                    size: [20, 20],
                    offset: [0, 0],
                }
            },
        );
        app.matrix_view_gesture(
            id,
            if inner_begin {
                G::Commit {
                    size: [16, 16],
                    offset: [0, 0],
                }
            } else {
                G::CommitInnerResize {
                    size: [16, 16],
                    offset: [0, 0],
                }
            },
        );
        assert!(app.matrix_view_drag.is_none() && app.document.matrix_result_views.is_empty());
        assert!(app.undo.is_empty() && !app.dirty);
    }
}
