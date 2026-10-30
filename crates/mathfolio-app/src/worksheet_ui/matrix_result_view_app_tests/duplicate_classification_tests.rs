//! Synthetic cached-state regressions; this source marker is not native import proof.
use super::*;
fn scalar_error(f: &mut Fixture) {
    f.app
        .document
        .regions
        .iter_mut()
        .find(|r| r.id == f.owner)
        .unwrap()
        .source_region = Some("42".into());
    f.app.results.remove(&f.owner);
    // The live fixture stores calculated rows in the document cache, rather
    // than the cold-import saved_results map. Install that actual row first.
    let saved = f
        .app
        .document
        .calculation_cache
        .as_ref()
        .unwrap()
        .rows
        .iter()
        .find(|row| row.region_id == f.owner)
        .unwrap()
        .clone();
    f.app.saved_results.insert(f.owner, saved);
    let row = f.app.saved_results.get_mut(&f.owner).unwrap();
    row.numeric_source = None;
    row.display = None;
    row.error = Some("ordinary scalar query error".into());
    f.app.display_cache.remove(&f.owner);
}
#[test]
fn ordinary_native_cached_error_without_matrix_request_does_not_block_duplicate() {
    let mut f = Fixture::new(1.);
    scalar_error(&mut f);
    assert_eq!(f.app.matrix_view_for_duplicate(f.owner).unwrap(), None);
    let count = f.app.document.regions.len();
    f.app.selected = Some(f.owner);
    f.app.duplicate_selected();
    assert_eq!(f.app.document.regions.len(), count + 1);
    assert_ne!(f.app.selected, Some(f.owner));
    assert!(f.app.last_executed_regions.is_empty());
}
#[test]
fn requested_view_without_result_and_recognized_rejected_matrix_still_refuse_duplicate() {
    let mut f = Fixture::new(1.);
    scalar_error(&mut f);
    let view = mathfolio_document::canvas::MatrixResultViewport {
        size: Some(mathfolio_document::canvas::MatrixViewSize { rows: 1, cols: 1 }),
        ..Default::default()
    };
    f.app
        .document
        .matrix_result_views
        .set(MatrixResultTarget::math(f.owner), view)
        .unwrap();
    assert!(f.app.matrix_view_for_duplicate(f.owner).is_err());
    f.app
        .document
        .matrix_result_views
        .remove(&MatrixResultTarget::math(f.owner));
    let row = f.app.saved_results.get_mut(&f.owner).unwrap();
    row.error = None;
    row.numeric_source = Some(SavedNumericSource::NativeMathml("<matrix xmlns='http://schemas.mathsoft.com/math50' rows='2' cols='2'><real>1</real></matrix>".into()));
    f.app.display_cache.remove(&f.owner);
    assert!(f.app.matrix_view_for_duplicate(f.owner).is_err());
    assert!(f.app.display_cache[&f.owner].matrix.is_matrix());
}
