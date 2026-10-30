//! Existing admission/projection/cell counters only, no new production hooks.
use super::*;
use crate::result_display::take_formatted_matrix_cells;

fn window(rows: usize, cols: usize) -> MatrixWindow {
    MatrixWindow {
        rows,
        cols,
        row_offset: 0,
        col_offset: 0,
    }
}
fn live_fixture() -> (WorksheetApp, u64) {
    let (mut app, id) = fixture();
    let row = RegionEvaluation {
        region_id: id.to_string(),
        result_ref: None,
        left: 38.4,
        top: 38.4,
        value: Some(Value::Matrix {
            rows: 50,
            cols: 50,
            elements: (0..2500).map(|n| Quantity::scalar(n as f64)).collect(),
        }),
        display_value: None,
        error: None,
        redefinition_warning: false,
        matrix_origin: Some(0),
    };
    app.accept_calculation_update(CalculationUpdate {
        rows: vec![row],
        pending_region_ids: vec![],
        executed_region_ids: vec![],
        cancelled: false,
    });
    let _ = cached_formula_display(
        &mut app.display_cache,
        id,
        app.document.regions[0].format,
        app.results.get(&id),
        app.saved_results.get(&id),
        Some(window(12, 12)),
    );
    assert_eq!(app.display_cache[&id].matrix.work_counts(), (1, 1));
    (app, id)
}

#[test]
fn same_preview_commit_and_show_indices_toggle_do_not_format_cells_twice() {
    let (mut app, id) = live_fixture();
    let signature = app.document.calculation_signature();
    let generation = app.background.submitted_generation();
    let numeric = serde_json::to_string(&app.results[&id]).unwrap();
    let full = app.results[&id].value.as_ref().unwrap().numeric_json();
    take_formatted_matrix_cells();
    let counts = app.display_cache[&id].matrix.work_counts();
    assert!(app.set_matrix_indices(id, true).unwrap());
    assert!(app.set_matrix_indices(id, false).unwrap());
    assert_eq!(app.display_cache[&id].matrix.work_counts(), counts);
    assert_eq!(take_formatted_matrix_cells(), 0);
    begin(&mut app, id);
    app.matrix_view_gesture(id, gesture([13, 14], false));
    assert_eq!(take_formatted_matrix_cells(), 13 * 14);
    let preview_counts = app.display_cache[&id].matrix.work_counts();
    app.matrix_view_gesture(id, gesture([13, 14], true));
    assert_eq!(take_formatted_matrix_cells(), 0);
    assert_eq!(app.display_cache[&id].matrix.work_counts(), preview_counts);
    assert_eq!(app.document.calculation_signature(), signature);
    assert_eq!(app.background.submitted_generation(), generation);
    assert_eq!(serde_json::to_string(&app.results[&id]).unwrap(), numeric);
    assert_eq!(
        app.results[&id].value.as_ref().unwrap().numeric_json(),
        full
    );
    assert!(
        app.last_executed_regions.is_empty()
            && !app.background.running
            && app.pending_regions.is_empty()
    );
}

#[test]
fn changed_window_or_numeric_style_still_reformats_and_keeps_complete_authority() {
    let (mut app, id) = fixture();
    let mut format = app.document.regions[0].format;
    let matrix = &mut app.display_cache.get_mut(&id).unwrap().matrix;
    take_formatted_matrix_cells();
    matrix.preflight(window(12, 12), format).unwrap();
    assert_eq!(take_formatted_matrix_cells(), 0);
    matrix.preflight(window(13, 12), format).unwrap();
    assert_eq!(take_formatted_matrix_cells(), 156);
    format.precision = if format.precision == 3 { 4 } else { 3 };
    matrix.preflight(window(13, 12), format).unwrap();
    assert_eq!(take_formatted_matrix_cells(), 156);
    assert_eq!(matrix.work_counts(), (1, 3));
    assert_eq!(matrix.shape(), Some((50, 50)));
}

#[test]
fn failed_preflight_and_failed_published_projection_cannot_hit_a_success_cache() {
    let (mut app, id) = fixture();
    let format = app.document.regions[0].format;
    let matrix = &mut app.display_cache.get_mut(&id).unwrap().matrix;
    let old = matrix.value().unwrap().clone();
    let invalid = window(0, 12);
    take_formatted_matrix_cells();
    assert!(matrix.preflight(invalid, format).is_err());
    assert!(matrix.preflight(invalid, format).is_err());
    assert_eq!(matrix.value(), Some(&old));
    assert_eq!(take_formatted_matrix_cells(), 0);
    // A failed normal projection publishes an error and removes its value;
    // same-key preflight must retry validation and return Err, never Ok.
    assert!(matrix.project(invalid, format, None, None).is_none());
    assert!(matrix.error().is_some());
    assert!(matrix.preflight(invalid, format).is_err());
    assert!(matrix.value().is_none() && matrix.error().is_some());
    matrix.preflight(window(12, 12), format).unwrap();
    assert_eq!(take_formatted_matrix_cells(), 144);
    assert_eq!(matrix.value(), Some(&old));
    assert!(matrix.error().is_none());
}

#[test]
fn accepted_same_shape_source_replacement_rebuilds_authority_before_same_key_reuse() {
    let (mut app, id) = live_fixture();
    let format = app.document.regions[0].format;
    let mut changed = app.results[&id].clone();
    let Some(Value::Matrix { elements, .. }) = changed.value.as_mut() else {
        panic!()
    };
    elements[0] = Quantity::scalar(987654.);
    app.accept_calculation_update(CalculationUpdate {
        rows: vec![changed],
        pending_region_ids: vec![],
        executed_region_ids: vec![],
        cancelled: false,
    });
    assert!(!app.display_cache.contains_key(&id));
    take_formatted_matrix_cells();
    let _ = cached_formula_display(
        &mut app.display_cache,
        id,
        format,
        app.results.get(&id),
        app.saved_results.get(&id),
        Some(window(12, 12)),
    );
    let cache = &mut app.display_cache.get_mut(&id).unwrap().matrix;
    assert_eq!(cache.work_counts(), (1, 1));
    assert_eq!(take_formatted_matrix_cells(), 144);
    assert_ne!(cache.value().unwrap().cells[0].plain(), "0");
    let current = cache.value().unwrap().clone();
    cache.preflight(window(12, 12), format).unwrap();
    assert_eq!(take_formatted_matrix_cells(), 0);
    assert_eq!(cache.value(), Some(&current));
    assert_eq!(cache.shape(), Some((50, 50)));
}
