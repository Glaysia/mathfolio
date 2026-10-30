//! Covers the SECOND origin2000 golden in the original failing lifecycle:
//! an accepted result expands again after Pending removed index gutters.
use super::*;

fn dump(stage: &str, sample: &Sample, ordinal: usize) {
    eprintln!("PENDING_CYCLE_STAGE stage={stage} ordinal={ordinal}");
    sample.dump(ordinal);
}

#[test]
fn pending_then_same_origin_restore_first_frame_retains_all_clipped_golden_labels() {
    let mut f = known(true);
    labels(&paint(&mut f), 1000);
    let source = serde_json::to_vec(&f.app.document).unwrap();
    let before_values = values(&f.app);
    let generation = f.app.background.submitted_generation();
    deliver(&mut f, Some(2000), false);
    let first_callback = sample(&mut f);
    f.app.pending_regions.insert(f.owner);
    f.app.calculation_pending = true;
    let before_pending_callback = sample(&mut f);
    deliver(&mut f, None, true);
    let pending_callback = sample(&mut f);
    deliver(&mut f, Some(2000), false);
    f.app.calculation_pending = false;
    // No warm-up waiver: store all diagnostics before asserting restored[0].
    let restored = [sample(&mut f), sample(&mut f), sample(&mut f)];
    dump("initial_accepted", &first_callback, 0);
    dump(
        "app_pending_old_origin_retained",
        &before_pending_callback,
        1,
    );
    dump("compat_pending_origin_none", &pending_callback, 2);
    for (i, s) in restored.iter().enumerate() {
        dump("restored_accepted", s, 3 + i);
    }
    labels(&first_callback.paint(None), 2000);
    labels(&first_callback.paint(Some(first_callback.observed)), 2000);
    labels(&first_callback.paint(Some(first_callback.queried)), 2000);
    no_labels(&before_pending_callback.paint(None), &[0, 1000, 2000]);
    no_labels(&pending_callback.paint(None), &[0, 1000, 2000]);
    let first = &restored[0];
    // Preserve all original numeric goldens, including both 2013 copies and
    // the row/column after endpoints. Extra frames serve diagnosis only.
    labels(&first.paint(None), 2000);
    labels(&first.paint(Some(first.observed)), 2000);
    // Pinned egui0.36.2 regression witness: post-frame Context still returns
    // the previous smaller Pending rectangle, not this frame's returned one.
    // Keep this fact separate from actual paint correctness; never use that
    // queried rectangle to discard current right/bottom labels.
    assert_eq!(first.queried, pending_callback.observed);
    assert_ne!(first.queried, first.observed);
    assert_eq!(first.queried.min, first.observed.min);
    assert!(first.queried.width() < first.observed.width());
    assert!(first.queried.height() < first.observed.height());
    assert_eq!(first.observed, first_callback.observed);
    // Context converges one frame later, with no warm-up waiver for rendering.
    assert_eq!(restored[1].queried, restored[1].observed);
    assert!(first.markers.iter().all(|(_, r)| r.is_some()));
    // Queried marker coordinates can likewise be previous-pass. Presence is
    // useful telemetry, but is not a current geometry assertion.
    assert_eq!(values(&f.app), before_values);
    assert_eq!(serde_json::to_vec(&f.app.document).unwrap(), source);
    assert_eq!(f.app.background.submitted_generation(), generation);
    assert!(f.app.last_executed_regions.is_empty() && !f.app.background.running);
    assert!(f.app.undo.is_empty() && f.app.redo.is_empty());
}
