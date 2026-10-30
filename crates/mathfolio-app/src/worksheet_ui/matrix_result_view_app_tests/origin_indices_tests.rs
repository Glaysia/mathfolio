//! Actual App pending-before-callback gate; original runtime is not invoked.
use super::*;

#[test]
fn pending_before_callback_cannot_reuse_old_live_origin_for_indices_command() {
    let mut f = Fixture::new(1.);
    // The existing fixture really executed the Rust matrix in its default
    // scope. This is a live known zero, never a cold zero inference.
    assert_eq!(f.app.results[&f.owner].matrix_origin, Some(0));
    assert!(f.app.matrix_indices_context().is_some());
    let values = rows(&f.app);
    let document = serde_json::to_vec(&f.app.document).unwrap();
    let generation = f.app.background.submitted_generation();
    let history = (f.app.undo.len(), f.app.redo.len());
    let projection = f.app.display_cache[&f.owner]
        .matrix
        .value()
        .unwrap()
        .clone();
    // Model the actual publication gap: App has invalidated this owner, while
    // the previous accepted RegionEvaluation has not been replaced yet.
    f.app.pending_regions.insert(f.owner);
    f.app.calculation_pending = true;
    assert_eq!(f.app.results[&f.owner].matrix_origin, Some(0));
    assert!(
        f.app.matrix_indices_context().is_none(),
        "pending App state must hide stale result-associated ORIGIN before callback"
    );
    assert!(f.app.set_matrix_indices(f.owner, true).is_err());
    assert_eq!(rows(&f.app), values);
    assert_eq!(serde_json::to_vec(&f.app.document).unwrap(), document);
    assert_eq!(f.app.background.submitted_generation(), generation);
    assert_eq!((f.app.undo.len(), f.app.redo.len()), history);
    assert_eq!(
        f.app.display_cache[&f.owner].matrix.value(),
        Some(&projection)
    );
    assert!(f.app.last_executed_regions.is_empty());
    assert!(!f.app.background.running);
}

mod lifecycle_tests;
